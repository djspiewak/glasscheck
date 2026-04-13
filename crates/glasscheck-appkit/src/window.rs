#[cfg(target_os = "macos")]
mod imp {
    use glasscheck_core::{
        normalize_provider_nodes, InstrumentedNode, NodePredicate, Point, QueryRoot, Rect,
        RegionResolveError, RegionSpec, Role, SceneSnapshot, SemanticNode, SemanticProvider, Size,
        TextRange,
    };
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, rc::Retained, ClassType, MainThreadOnly};
    use objc2_app_kit::{
        NSBackingStoreType, NSControl, NSTextInputClient, NSTextView, NSView, NSWindow,
        NSWindowStyleMask,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSRect, NSString};
    use std::cell::RefCell;
    use std::collections::{BTreeSet, HashMap};

    use crate::capture::{capture_view_image, crop_image_in_view_coordinates};
    use crate::input::AppKitInputDriver;
    use crate::screen::offscreen_window_content_rect;
    use crate::text::AppKitTextHarness;

    enum RegisteredViewClickRoute {
        Target(Retained<NSView>),
        Descendant(Retained<NSView>),
        Blocked,
    }

    #[derive(Clone, Debug)]
    struct RegisteredView {
        view: Retained<NSView>,
        descriptor: InstrumentedNode,
    }

    #[derive(Clone, Debug)]
    struct RegisteredNodeId {
        snapshot_id: String,
        source_id: Option<String>,
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
        /// window so capture and low-level input APIs remain usable. Semantic click APIs still
        /// report `DetachedRootView` because the supplied root is not treated as window-rooted.
        #[must_use]
        pub fn from_root(view: &NSView, window: Option<&NSWindow>) -> Self {
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

        /// Compatibility wrapper for the legacy name.
        #[must_use]
        pub fn from_root_view(view: &NSView, window: Option<&NSWindow>) -> Self {
            Self::from_root(view, window)
        }

        /// Returns the underlying `NSWindow`.
        #[must_use]
        pub fn window(&self) -> &NSWindow {
            self.window
                .as_deref()
                .expect("window access requires an attached window")
        }

        /// Sets the window content view.
        pub fn set_root(&self, view: &NSView) {
            if let Some(window) = self.window.as_deref() {
                window.setContentView(Some(view));
            }
            let retained = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("content view should retain successfully")
            };
            *self.root_view.borrow_mut() = Some(retained);
        }

        /// Compatibility wrapper for the legacy name.
        pub fn set_content_view(&self, view: &NSView) {
            self.set_root(view);
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
        pub fn capture_subtree(&self, view: &NSView) -> Option<glasscheck_core::Image> {
            capture_view_image(view)
        }

        /// Compatibility wrapper for the legacy name.
        #[must_use]
        pub fn capture_view(&self, view: &NSView) -> Option<glasscheck_core::Image> {
            self.capture_subtree(view)
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
            let root_view = self.root_view();
            let registered_view = self.registered_view_for_handle(handle, root_view.as_deref());
            let (point, click_view) =
                match self.click_target(root_view.as_deref(), registered_view.as_deref(), node) {
                    Some((point, click_view)) => {
                        (self.root_point_to_window_point(point), click_view)
                    }
                    None => return Err(RegionResolveError::InputUnavailable),
                };
            if let Some(view) = click_view.or(registered_view) {
                if is_control_view(&view) {
                    unsafe {
                        let () = msg_send![&*view, performClick: std::ptr::null::<AnyObject>()];
                    }
                } else {
                    self.input().click_target(&view, point);
                }
                return Ok(());
            }
            if self
                .window
                .as_deref()
                .and_then(|window| window.contentView())
                .is_none()
            {
                return Err(RegionResolveError::InputUnavailable);
            }
            self.input().click(Point::new(point.x, point.y));
            Ok(())
        }

        /// Returns a text-rendering harness that uses this host for live capture.
        #[must_use]
        pub fn text_renderer(&self, mtm: MainThreadMarker) -> AppKitTextHarness<'_> {
            AppKitTextHarness::new(self, mtm)
        }

        /// Returns the bounding rect for a character range in a live `NSTextView`.
        #[must_use]
        pub fn text_range_rect(&self, view: &NSTextView, range: TextRange) -> Option<Rect> {
            let range = ns_range_for_scalar_range(view, range);
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
            let location = scalar_index_to_utf16_offset(&view.string().to_string(), location);
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
        pub fn register_node(&self, view: &NSView, descriptor: InstrumentedNode) {
            let retained = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("registered view should retain successfully")
            };
            self.registry.borrow_mut().push(RegisteredView {
                view: retained,
                descriptor,
            });
        }

        /// Compatibility wrapper for the legacy name.
        pub fn register_view(&self, view: &NSView, descriptor: InstrumentedNode) {
            self.register_node(view, descriptor);
        }

        /// Builds a merged scene snapshot from registered native views and virtual nodes.
        #[must_use]
        pub fn snapshot_scene(&self) -> SceneSnapshot {
            let root_view = self.root_view();
            let registry = self.active_registered_views(root_view.as_deref());
            let registered_ids = registered_node_ids(&registry);

            let mut nodes: Vec<SemanticNode> = registry
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let id = registered_ids
                        .get(&(&*entry.view as *const NSView))
                        .map(|registered| registered.snapshot_id.clone())
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
                    if let Some(source_id) = registered_ids
                        .get(&(&*entry.view as *const NSView))
                        .and_then(|registered| registered.source_id.clone())
                    {
                        node.properties.insert(
                            "glasscheck:source_id".into(),
                            glasscheck_core::PropertyValue::String(source_id),
                        );
                    }
                    let (visible, visible_rect, hit_testable) =
                        native_visibility(&entry.view, root_view.as_deref());
                    node.label = entry.descriptor.label.clone();
                    node.visible = visible;
                    node.visible_rect = visible_rect;
                    node.hit_testable = hit_testable;
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

        /// Builds a scene-backed compatibility `QueryRoot` from the current snapshot.
        #[must_use]
        pub fn query_root(&self) -> QueryRoot {
            QueryRoot::from_scene(self.snapshot_scene())
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
                .or_else(|| self.window.as_deref().map(window_root_local_bounds))
                .expect("host should have either a root view or a window");
            Rect::new(
                Point::new(content.origin.x, content.origin.y),
                Size::new(content.size.width, content.size.height),
            )
        }

        fn registered_view_for_handle(
            &self,
            handle: glasscheck_core::NodeHandle,
            root_view: Option<&NSView>,
        ) -> Option<Retained<NSView>> {
            self.active_registered_view_at_handle(handle, root_view)
                .map(|entry| unsafe {
                    Retained::retain(&*entry.view as *const NSView as *mut NSView)
                        .expect("registered view should retain successfully")
                })
        }

        fn active_registered_view_at_handle(
            &self,
            handle: glasscheck_core::NodeHandle,
            root_view: Option<&NSView>,
        ) -> Option<RegisteredView> {
            self.active_registered_views(root_view)
                .get(handle.index())
                .cloned()
        }

        fn root_point_to_window_point(&self, point: NSPoint) -> NSPoint {
            self.root_view()
                .as_deref()
                .map_or(point, |root| root.convertPoint_toView(point, None))
        }

        fn click_target(
            &self,
            root_view: Option<&NSView>,
            target_view: Option<&NSView>,
            node: &SemanticNode,
        ) -> Option<(NSPoint, Option<Retained<NSView>>)> {
            let target_rect = node.visible_rect.unwrap_or(node.rect);
            if rect_is_empty_rect(target_rect) {
                return None;
            }
            let point = NSPoint::new(
                target_rect.origin.x + target_rect.size.width / 2.0,
                target_rect.origin.y + target_rect.size.height / 2.0,
            );
            if let Some(target_view) = target_view {
                match self.resolve_registered_click_route(
                    root_view,
                    target_view,
                    target_rect,
                    point,
                ) {
                    RegisteredViewClickRoute::Target(view) => return Some((point, Some(view))),
                    RegisteredViewClickRoute::Descendant(view) => return Some((point, Some(view))),
                    RegisteredViewClickRoute::Blocked => return None,
                }
            }
            Some((point, None))
        }

        fn resolve_registered_click_route(
            &self,
            root_view: Option<&NSView>,
            target_view: &NSView,
            target_rect: Rect,
            point: NSPoint,
        ) -> RegisteredViewClickRoute {
            match registered_view_hit_test(root_view, target_view, target_rect, point) {
                Some(hit) if std::ptr::eq(&*hit, target_view) => {
                    RegisteredViewClickRoute::Target(hit)
                }
                Some(hit) => RegisteredViewClickRoute::Descendant(hit),
                None if (!self.tracks_window_content
                    && root_view.is_some_and(|root_view| std::ptr::eq(root_view, target_view)))
                    || (root_view.is_none() && unsafe { target_view.superview() }.is_none()) =>
                {
                    RegisteredViewClickRoute::Target(unsafe {
                        Retained::retain(target_view as *const NSView as *mut NSView)
                            .expect("registered view should retain successfully")
                    })
                }
                None => RegisteredViewClickRoute::Blocked,
            }
        }

        fn prune_stale_registered_views(&self, root_view: Option<&NSView>) {
            self.registry
                .borrow_mut()
                .retain(|entry| registered_view_is_active(&entry.view, root_view));
        }

        fn active_registered_views(&self, root_view: Option<&NSView>) -> Vec<RegisteredView> {
            self.prune_stale_registered_views(root_view);
            self.registry
                .borrow()
                .iter()
                .filter(|entry| registered_view_is_active(&entry.view, root_view))
                .map(|entry| RegisteredView {
                    view: unsafe {
                        Retained::retain(&*entry.view as *const NSView as *mut NSView)
                            .expect("registered view should retain successfully")
                    },
                    descriptor: entry.descriptor.clone(),
                })
                .collect()
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

    fn native_visibility(view: &NSView, root_view: Option<&NSView>) -> (bool, Option<Rect>, bool) {
        if view.isHiddenOrHasHiddenAncestor() {
            return (false, None, false);
        }

        let visible_rect = convert_visible_rect_to_root(view, root_view);
        let visible = visible_rect.is_some();
        let hit_testable = visible
            && visible_rect
                .as_ref()
                .is_some_and(|rect| view_participates_in_hit_testing(view, root_view, *rect));
        (visible, visible_rect, hit_testable)
    }

    fn convert_visible_rect_to_root(view: &NSView, root_view: Option<&NSView>) -> Option<Rect> {
        let root_view = root_view?;
        let mut visible_rect = rect_in_root(view, Some(root_view));
        if rect_is_empty_rect(visible_rect) {
            return None;
        }

        let mut current = unsafe { view.superview() };
        while let Some(parent) = current {
            let parent_rect = rect_in_root(&parent, Some(root_view));
            visible_rect = intersect_rects(visible_rect, parent_rect)?;
            if std::ptr::eq(&*parent, root_view) {
                break;
            }
            current = unsafe { parent.superview() };
        }

        Some(visible_rect)
    }

    fn view_participates_in_hit_testing(
        view: &NSView,
        root_view: Option<&NSView>,
        visible_rect: Rect,
    ) -> bool {
        let point = NSPoint::new(
            visible_rect.origin.x + visible_rect.size.width / 2.0,
            visible_rect.origin.y + visible_rect.size.height / 2.0,
        );

        let hit = match root_view {
            Some(root_view) => root_view.hitTest(point),
            None => view.hitTest(NSPoint::new(
                visible_rect.size.width / 2.0,
                visible_rect.size.height / 2.0,
            )),
        };

        hit.as_deref()
            .is_some_and(|hit| std::ptr::eq(hit, view) || is_descendant_of_view(hit, view))
    }

    fn registered_view_hit_test(
        root_view: Option<&NSView>,
        target_view: &NSView,
        target_rect: Rect,
        point: NSPoint,
    ) -> Option<Retained<NSView>> {
        let hit = match root_view {
            Some(root_view) => root_view.hitTest(point),
            None => target_view.hitTest(NSPoint::new(
                point.x - target_rect.origin.x,
                point.y - target_rect.origin.y,
            )),
        };

        match hit.as_deref() {
            Some(hit)
                if std::ptr::eq(hit, target_view) || is_descendant_of_view(hit, target_view) =>
            {
                Some(unsafe {
                    Retained::retain(hit as *const NSView as *mut NSView)
                        .expect("hit-tested view should retain successfully")
                })
            }
            Some(_) | None => None,
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

    fn rect_is_empty_rect(rect: Rect) -> bool {
        rect.size.width <= 0.0 || rect.size.height <= 0.0
    }

    fn intersect_rects(left: Rect, right: Rect) -> Option<Rect> {
        let x1 = left.origin.x.max(right.origin.x);
        let y1 = left.origin.y.max(right.origin.y);
        let x2 = (left.origin.x + left.size.width).min(right.origin.x + right.size.width);
        let y2 = (left.origin.y + left.size.height).min(right.origin.y + right.size.height);
        (x2 > x1 && y2 > y1).then_some(Rect::new(Point::new(x1, y1), Size::new(x2 - x1, y2 - y1)))
    }

    fn registered_view_is_active(view: &NSView, root_view: Option<&NSView>) -> bool {
        root_view.is_some_and(|root| std::ptr::eq(view, root) || is_descendant_of_view(view, root))
    }

    fn is_descendant_of_view(view: &NSView, root_view: &NSView) -> bool {
        let mut current = unsafe { view.superview() };
        while let Some(parent) = current {
            if std::ptr::eq(&*parent, root_view) {
                return true;
            }
            current = unsafe { parent.superview() };
        }
        false
    }

    fn window_root_local_bounds(window: &NSWindow) -> NSRect {
        let content = window.contentLayoutRect();
        NSRect::new(NSPoint::new(0.0, 0.0), content.size)
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

    fn registered_node_ids(
        registry: &[RegisteredView],
    ) -> HashMap<*const NSView, RegisteredNodeId> {
        let mut stable_counts = HashMap::<String, usize>::new();
        for (index, entry) in registry.iter().enumerate() {
            let stable_id = entry
                .descriptor
                .id
                .clone()
                .unwrap_or_else(|| format!("view-{index}"));
            *stable_counts.entry(stable_id).or_default() += 1;
        }

        let mut assigned_counts = HashMap::<String, usize>::new();
        registry
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let stable_id = entry
                    .descriptor
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("view-{index}"));
                let occurrence = assigned_counts.entry(stable_id.clone()).or_default();
                let snapshot_id = if stable_counts.get(&stable_id).copied().unwrap_or(0) == 1 {
                    stable_id.clone()
                } else if *occurrence == 0 {
                    format!("native::{stable_id}")
                } else {
                    format!("native::{stable_id}#{occurrence}")
                };
                *occurrence += 1;
                let source_id =
                    (stable_counts.get(&stable_id).copied().unwrap_or(0) > 1).then(|| stable_id);

                (
                    &*entry.view as *const NSView,
                    RegisteredNodeId {
                        snapshot_id,
                        source_id,
                    },
                )
            })
            .collect()
    }

    fn registered_ancestor_id(
        view: &NSView,
        registered_ids: &HashMap<*const NSView, RegisteredNodeId>,
    ) -> Option<String> {
        let mut current = unsafe { view.superview() };
        while let Some(parent) = current {
            let parent_ptr = &*parent as *const NSView;
            if let Some(id) = registered_ids.get(&parent_ptr) {
                return Some(id.snapshot_id.clone());
            }
            current = unsafe { parent.superview() };
        }
        None
    }

    fn is_control_view(view: &NSView) -> bool {
        unsafe { msg_send![view, isKindOfClass: NSControl::class()] }
    }

    fn scalar_index_to_utf16_offset(text: &str, scalar_index: usize) -> usize {
        text.chars()
            .take(scalar_index)
            .map(char::len_utf16)
            .sum::<usize>()
    }

    fn ns_range_for_scalar_range(view: &NSTextView, range: TextRange) -> NSRange {
        let content = view.string().to_string();
        let start = scalar_index_to_utf16_offset(&content, range.start);
        let end = scalar_index_to_utf16_offset(&content, range.start + range.len);
        NSRange::new(start, end.saturating_sub(start))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct AppKitWindowHost;
}

pub use imp::AppKitWindowHost;
