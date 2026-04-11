#[cfg(target_os = "macos")]
mod imp {
    use glasscheck_core::{
        NodePredicate, Point, QueryRoot, Rect, RegionResolveError, RegionSpec, Role, SceneSnapshot,
        SemanticNode, SemanticProvider, Size,
    };
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, rc::Retained, ClassType, MainThreadOnly};
    use objc2_app_kit::{
        NSBackingStoreType, NSControl, NSTextInputClient, NSTextView, NSView, NSWindow,
        NSWindowStyleMask,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSRect, NSString};
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use crate::capture::{capture_view_image, crop_image_in_view_coordinates};
    use crate::input::AppKitInputDriver;
    use crate::screen::offscreen_window_content_rect;
    use crate::text::AppKitTextHarness;

    /// Semantic metadata registered for a view exposed to querying APIs.
    #[derive(Clone, Debug)]
    pub struct InstrumentedView {
        /// Stable semantic identifier.
        pub id: Option<String>,
        /// Semantic role.
        pub role: Option<Role>,
        /// Human-readable label.
        pub label: Option<String>,
    }

    struct RegisteredView {
        view: Retained<NSView>,
        descriptor: InstrumentedView,
    }

    /// AppKit window host used to build, capture, query, and drive a test scene.
    pub struct AppKitWindowHost {
        window: Option<Retained<NSWindow>>,
        root_view: RefCell<Option<Retained<NSView>>>,
        registry: RefCell<Vec<RegisteredView>>,
        provider: RefCell<Option<Box<dyn SemanticProvider>>>,
        detached_root_view: bool,
        tracks_window_content: bool,
    }

    impl AppKitWindowHost {
        /// Creates a window host with a new `NSWindow`.
        #[must_use]
        pub fn new(mtm: MainThreadMarker, width: f64, height: f64) -> Self {
            let rect = offscreen_window_content_rect(mtm, width, height);
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable;
            let window = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    rect,
                    style,
                    NSBackingStoreType::Buffered,
                    false,
                )
            };
            unsafe { window.setReleasedWhenClosed(false) };
            Self {
                window: Some(window),
                root_view: RefCell::new(None),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                detached_root_view: false,
                tracks_window_content: true,
            }
        }

        /// Attaches a host to an existing `NSWindow`.
        #[must_use]
        pub fn from_window(window: &NSWindow) -> Self {
            let retained = unsafe {
                Retained::retain(window as *const NSWindow as *mut NSWindow)
                    .expect("window attachment should retain successfully")
            };
            let root_view = retained.contentView().map(|view| unsafe {
                Retained::retain(&*view as *const NSView as *mut NSView)
                    .expect("content view attachment should retain successfully")
            });
            Self {
                window: Some(retained),
                root_view: RefCell::new(root_view),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                detached_root_view: false,
                tracks_window_content: true,
            }
        }

        /// Attaches a host to an existing root view and optional parent window.
        ///
        /// When no window is supplied, the host installs the view into a managed offscreen
        /// window so capture and input APIs remain usable.
        #[must_use]
        pub fn from_root_view(view: &NSView, window: Option<&NSWindow>) -> Self {
            let root = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("root view attachment should retain successfully")
            };
            let attached_window = window
                .map(|window| unsafe {
                    Retained::retain(window as *const NSWindow as *mut NSWindow)
                        .expect("window attachment should retain successfully")
                })
                .or_else(|| Some(managed_window_for_root_view(view)));
            Self {
                window: attached_window,
                root_view: RefCell::new(Some(root)),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                detached_root_view: window.is_none(),
                tracks_window_content: false,
            }
        }

        /// Returns the underlying `NSWindow`.
        #[must_use]
        pub fn window(&self) -> &NSWindow {
            self.window
                .as_deref()
                .expect("window access requires an attached window")
        }

        /// Sets the window content view.
        pub fn set_content_view(&self, view: &NSView) {
            if let Some(window) = self.window.as_deref() {
                window.setContentView(Some(view));
            }
            let retained = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("content view should retain successfully")
            };
            *self.root_view.borrow_mut() = Some(retained);
        }

        /// Registers a pull-based semantic provider for virtual nodes.
        pub fn set_semantic_provider(&self, provider: Box<dyn SemanticProvider>) {
            *self.provider.borrow_mut() = Some(provider);
        }

        /// Captures the current root view as an image.
        #[must_use]
        pub fn capture(&self) -> Option<glasscheck_core::Image> {
            let root = self.root_view()?;
            capture_view_image(&root)
        }

        /// Captures a specific view as an image.
        #[must_use]
        pub fn capture_view(&self, view: &NSView) -> Option<glasscheck_core::Image> {
            capture_view_image(view)
        }

        /// Captures a semantically resolved region as an image.
        pub fn capture_region(
            &self,
            region: &RegionSpec,
        ) -> Result<glasscheck_core::Image, RegionResolveError> {
            let rect = self.resolve_region(region)?;
            let image = self
                .capture()
                .ok_or(RegionResolveError::CaptureUnavailable)?;
            Ok(crop_image_in_view_coordinates(&image, rect))
        }

        /// Returns an input driver scoped to this window.
        #[must_use]
        pub fn input(&self) -> AppKitInputDriver<'_> {
            let window = self
                .window
                .as_deref()
                .expect("input requires an attached window");
            AppKitInputDriver::new(window)
        }

        /// Clicks the visual center of the unique node matching `predicate`.
        pub fn click_node(&self, predicate: &NodePredicate) -> Result<(), RegionResolveError> {
            if self.detached_root_view {
                return Err(RegionResolveError::DetachedRootView);
            }
            let scene = self.snapshot_scene();
            let handle = scene.find(predicate).map_err(|error| match error {
                glasscheck_core::QueryError::NotFoundPredicate(predicate) => {
                    RegionResolveError::NotFound(predicate)
                }
                glasscheck_core::QueryError::MultiplePredicateMatches { predicate, count } => {
                    RegionResolveError::MultipleMatches { predicate, count }
                }
                glasscheck_core::QueryError::NotFound(selector) => RegionResolveError::NotFound(
                    NodePredicate::id_eq(selector.id.unwrap_or_default()),
                ),
                glasscheck_core::QueryError::MultipleMatches { selector, count } => {
                    RegionResolveError::MultipleMatches {
                        predicate: NodePredicate::id_eq(selector.id.unwrap_or_default()),
                        count,
                    }
                }
            })?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            let point = self.root_point_to_window_point(NSPoint::new(
                node.rect.origin.x + node.rect.size.width / 2.0,
                node.rect.origin.y + node.rect.size.height / 2.0,
            ));
            if let Some(view) = self.registered_view_for_handle(handle) {
                if is_control_view(&view) {
                    unsafe {
                        let () = msg_send![&*view, performClick: std::ptr::null::<AnyObject>()];
                    }
                } else {
                    self.input().click_target(&view, point);
                }
                return Ok(());
            }
            if self.root_view().is_none() {
                return Err(RegionResolveError::InputUnavailable);
            }
            self.input().click(point);
            Ok(())
        }

        /// Returns a text-rendering harness that uses this host for live capture.
        #[must_use]
        pub fn text_renderer(&self, mtm: MainThreadMarker) -> AppKitTextHarness<'_> {
            AppKitTextHarness::new(self, mtm)
        }

        /// Returns the bounding rect for a character range in a live `NSTextView`.
        #[must_use]
        pub fn text_range_rect(&self, view: &NSTextView, range: NSRange) -> Option<Rect> {
            let layout_manager = unsafe { view.layoutManager() }?;
            let text_container = unsafe { view.textContainer() }?;
            let glyph_range = unsafe {
                layout_manager
                    .glyphRangeForCharacterRange_actualCharacterRange(range, std::ptr::null_mut())
            };
            let rect = layout_manager
                .boundingRectForGlyphRange_inTextContainer(glyph_range, &text_container);
            let origin = view.textContainerOrigin();
            let rect = NSRect::new(
                NSPoint::new(rect.origin.x + origin.x, rect.origin.y + origin.y),
                rect.size,
            );
            let root = self.root_view()?;
            let rect = root.convertRect_fromView(rect, Some(view));
            Some(Rect::new(
                Point::new(rect.origin.x, rect.origin.y),
                Size::new(rect.size.width, rect.size.height),
            ))
        }

        /// Returns the insertion caret rect for `location` in a live `NSTextView`.
        #[must_use]
        pub fn insertion_caret_rect(&self, view: &NSTextView, location: usize) -> Option<Rect> {
            let window = view.window()?;
            let rect = unsafe {
                view.firstRectForCharacterRange_actualRange(
                    NSRange::new(location, 0),
                    std::ptr::null_mut(),
                )
            };
            let rect = window.convertRectFromScreen(rect);
            let root = self.root_view()?;
            let rect = root.convertRect_fromView(rect, None);
            (rect.size.width > 0.0 || rect.size.height > 0.0).then_some(Rect::new(
                Point::new(rect.origin.x, rect.origin.y),
                Size::new(rect.size.width, rect.size.height),
            ))
        }

        /// Registers semantic metadata for a view so it can be queried later.
        pub fn register_view(&self, view: &NSView, descriptor: InstrumentedView) {
            let retained = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("registered view should retain successfully")
            };
            self.registry.borrow_mut().push(RegisteredView {
                view: retained,
                descriptor,
            });
        }

        /// Builds a merged scene snapshot from registered native views and virtual nodes.
        #[must_use]
        pub fn snapshot_scene(&self) -> SceneSnapshot {
            let root_view = self.root_view();
            let registry = self.registry.borrow();
            let registered_ids = registry
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let id = entry
                        .descriptor
                        .id
                        .clone()
                        .unwrap_or_else(|| format!("view-{index}"));
                    let view_ptr = &*entry.view as *const NSView;
                    (view_ptr, id)
                })
                .collect::<HashMap<*const NSView, String>>();

            let mut nodes: Vec<SemanticNode> = registry
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let id = entry
                        .descriptor
                        .id
                        .clone()
                        .unwrap_or_else(|| format!("view-{index}"));
                    let mut node = SemanticNode::new(
                        id,
                        entry
                            .descriptor
                            .role
                            .clone()
                            .unwrap_or_else(|| infer_role(&entry.view)),
                        rect_in_root(&entry.view, root_view.as_deref()),
                    );
                    node.label = entry.descriptor.label.clone();
                    node.visible = !entry.view.isHidden();
                    node.visible_rect = Some(node.rect);
                    node.hit_testable = true;
                    if let Some(parent) = unsafe { entry.view.superview() } {
                        node.child_index = sibling_index(&entry.view, &parent);
                        node.z_index = node.child_index as i32;
                    } else {
                        node.child_index = index;
                        node.z_index = index as i32;
                    }
                    if let Some(parent_id) = registered_ancestor_id(&entry.view, &registered_ids) {
                        node.parent_id = Some(parent_id);
                    }
                    node
                })
                .collect();

            if let Some(provider) = self.provider.borrow().as_ref() {
                let native_ids = nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<BTreeSet<_>>();
                nodes.extend(normalize_provider_nodes(
                    provider.snapshot_nodes(),
                    &native_ids,
                ));
            }

            SceneSnapshot::new(nodes)
        }

        /// Builds a compatibility `QueryRoot` from the current scene snapshot.
        #[must_use]
        pub fn query_root(&self) -> QueryRoot {
            let root_view = self.root_view();
            let nodes = self
                .registry
                .borrow()
                .iter()
                .map(|entry| glasscheck_core::NodeMetadata {
                    id: entry.descriptor.id.clone(),
                    role: entry.descriptor.role.clone(),
                    label: entry.descriptor.label.clone(),
                    rect: rect_in_root(&entry.view, root_view.as_deref()),
                })
                .collect();
            QueryRoot::new(nodes)
        }

        /// Resolves a semantic region against the current scene snapshot.
        pub fn resolve_region(&self, region: &RegionSpec) -> Result<Rect, RegionResolveError> {
            let root_bounds = self.root_bounds();
            self.snapshot_scene().resolve_region(root_bounds, region)
        }

        /// Sets the window title when a window is attached.
        pub fn set_title(&self, title: &str) {
            if let Some(window) = self.window.as_deref() {
                let title = NSString::from_str(title);
                window.setTitle(&title);
            }
        }

        fn root_view(&self) -> Option<Retained<NSView>> {
            if self.tracks_window_content {
                if let Some(window) = self.window.as_deref() {
                    let current = window.contentView();
                    let mut cached = self.root_view.borrow_mut();
                    match current {
                        Some(content) => {
                            let is_stale = cached
                                .as_ref()
                                .is_none_or(|view| !std::ptr::eq(&**view, &*content));
                            if is_stale {
                                let retained = unsafe {
                                    Retained::retain(&*content as *const NSView as *mut NSView)
                                        .expect("content view should retain successfully")
                                };
                                *cached = Some(retained);
                            }
                        }
                        None => {
                            *cached = None;
                        }
                    }
                }
            }
            self.root_view.borrow().as_ref().map(|view| unsafe {
                Retained::retain(&**view as *const NSView as *mut NSView)
                    .expect("root view should retain successfully")
            })
        }

        fn root_bounds(&self) -> Rect {
            let content = self
                .root_view()
                .map(|view| view.bounds())
                .or_else(|| self.window.as_deref().map(NSWindow::frame))
                .expect("host should have either a root view or a window");
            Rect::new(
                Point::new(content.origin.x, content.origin.y),
                Size::new(content.size.width, content.size.height),
            )
        }

        fn registered_view_for_handle(
            &self,
            handle: glasscheck_core::NodeHandle,
        ) -> Option<Retained<NSView>> {
            self.registry
                .borrow()
                .get(handle.index())
                .map(|entry| unsafe {
                    Retained::retain(&*entry.view as *const NSView as *mut NSView)
                        .expect("registered view should retain successfully")
                })
        }

        fn root_point_to_window_point(&self, point: NSPoint) -> NSPoint {
            self.root_view()
                .as_deref()
                .map_or(point, |root| root.convertPoint_toView(point, None))
        }
    }

    fn infer_role(view: &NSView) -> Role {
        let class_name = format!("{:?}", view.class());
        if class_name.contains("NSTextView") {
            Role::TextInput
        } else if class_name.contains("NSTextField") {
            Role::Label
        } else {
            Role::Container
        }
    }

    fn rect_in_root(view: &NSView, root_view: Option<&NSView>) -> Rect {
        let converted = match root_view {
            Some(root_view) => view.convertRect_toView(view.bounds(), Some(root_view)),
            None => view.frame(),
        };
        Rect::new(
            Point::new(converted.origin.x, converted.origin.y),
            Size::new(converted.size.width, converted.size.height),
        )
    }

    fn sibling_index(view: &NSView, parent: &NSView) -> usize {
        parent
            .subviews()
            .to_vec()
            .iter()
            .position(|candidate| std::ptr::eq(&**candidate, view))
            .unwrap_or(0)
    }

    fn managed_window_for_root_view(view: &NSView) -> Retained<NSWindow> {
        let mtm = MainThreadMarker::new()
            .expect("root-view attachment without a window must run on the main thread");
        let bounds = view.bounds();
        let rect = offscreen_window_content_rect(mtm, bounds.size.width, bounds.size.height);
        let style =
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable;
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
            window.setContentView(Some(view));
        }
        window
    }

    fn registered_ancestor_id(
        view: &NSView,
        registered_ids: &HashMap<*const NSView, String>,
    ) -> Option<String> {
        let mut current = unsafe { view.superview() };
        while let Some(parent) = current {
            let parent_ptr = &*parent as *const NSView;
            if let Some(id) = registered_ids.get(&parent_ptr) {
                return Some(id.clone());
            }
            current = unsafe { parent.superview() };
        }
        None
    }

    fn is_control_view(view: &NSView) -> bool {
        unsafe { msg_send![view, isKindOfClass: NSControl::class()] }
    }

    fn normalize_provider_nodes(
        mut nodes: Vec<SemanticNode>,
        native_ids: &BTreeSet<String>,
    ) -> Vec<SemanticNode> {
        let provider_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
        let needs_namespace =
            provider_ids.iter().any(|id| native_ids.contains(id)) || has_duplicates(&provider_ids);
        if !needs_namespace {
            return nodes;
        }

        let mut original_counts = BTreeMap::<String, usize>::new();
        for id in &provider_ids {
            *original_counts.entry(id.clone()).or_default() += 1;
        }

        let mut original_to_unique = BTreeMap::<String, String>::new();
        let mut assigned_counts = BTreeMap::<String, usize>::new();

        for node in &mut nodes {
            let original_id = node.id.clone();
            let base_id = format!("provider::{original_id}");
            let occurrence = assigned_counts.entry(base_id.clone()).or_default();
            let unique_id = if *occurrence == 0 {
                base_id.clone()
            } else {
                format!("{base_id}#{occurrence}")
            };
            *occurrence += 1;
            node.id = unique_id.clone();
            node.properties.insert(
                "glasscheck:source_id".into(),
                glasscheck_core::PropertyValue::String(original_id),
            );
        }

        for node in &nodes {
            let Some(glasscheck_core::PropertyValue::String(original_id)) =
                node.properties.get("glasscheck:source_id")
            else {
                continue;
            };
            if original_counts.get(original_id).copied().unwrap_or(0) == 1 {
                original_to_unique.insert(original_id.clone(), node.id.clone());
            }
        }

        for node in &mut nodes {
            if let Some(parent_id) = node.parent_id.as_ref() {
                if original_counts.get(parent_id).copied().unwrap_or(0) == 1 {
                    node.parent_id = original_to_unique.get(parent_id).cloned();
                } else {
                    node.properties.insert(
                        "glasscheck:ambiguous_parent_id".into(),
                        glasscheck_core::PropertyValue::String(parent_id.clone()),
                    );
                    node.parent_id = None;
                }
            }
        }

        nodes
    }

    fn has_duplicates(ids: &[String]) -> bool {
        let mut seen = BTreeSet::new();
        ids.iter().any(|id| !seen.insert(id.clone()))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    #[derive(Clone, Debug)]
    pub struct InstrumentedView {
        pub id: Option<String>,
        pub role: Option<glasscheck_core::Role>,
        pub label: Option<String>,
    }

    pub struct AppKitWindowHost;
}

pub use imp::{AppKitWindowHost, InstrumentedView};
