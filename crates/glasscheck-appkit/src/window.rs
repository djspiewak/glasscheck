#[cfg(target_os = "macos")]
mod imp {
    use glasscheck_core::{
        resolve_node_recipes, HitPointSearch, HitPointStrategy, Image, InputSynthesisError,
        InstrumentedNode, NodeProvenanceKind, Point, PropertyValue, Rect, RegionResolveError,
        RegionSpec, Role, Scene, Selector, SemanticNode, SemanticProvider, SemanticSnapshot, Size,
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
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use crate::capture::{capture_view_image, crop_image_in_view_coordinates};
    use crate::input::AppKitInputDriver;
    use crate::menu::{AppKitContextMenu, AppKitContextMenuError};
    use crate::screen::{
        configure_background_test_window, offscreen_window_content_rect,
        offscreen_window_frame_rect,
    };
    use crate::text::AppKitTextHarness;

    enum RegisteredViewClickRoute {
        Target(Retained<NSView>),
        Descendant(Retained<NSView>),
        Blocked,
    }

    /// Host-aware contextual semantic provider for AppKit scenes.
    pub trait AppKitSceneSource {
        /// Produces the semantic snapshot for the active AppKit host.
        fn snapshot(&self, context: &AppKitSnapshotContext<'_>) -> SemanticSnapshot;
    }

    /// Compatibility descriptor for registered AppKit views.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct InstrumentedView {
        /// Stable semantic identifier.
        pub id: Option<String>,
        /// Semantic role.
        pub role: Option<Role>,
        /// Human-readable label.
        pub label: Option<String>,
        /// Stable test-facing selectors or aliases.
        pub selectors: Vec<String>,
    }

    /// Host-aware geometry helpers for AppKit semantic providers.
    pub struct AppKitSnapshotContext<'a> {
        host: &'a AppKitWindowHost,
    }

    impl<'a> AppKitSnapshotContext<'a> {
        fn new(host: &'a AppKitWindowHost) -> Self {
            Self { host }
        }

        /// Returns the active root bounds in root coordinates.
        #[must_use]
        pub fn root_bounds(&self) -> Rect {
            self.host.root_bounds()
        }

        /// Converts `rect` from `view` coordinates into root coordinates.
        #[must_use]
        pub fn convert_rect_from_view(&self, view: &NSView, rect: NSRect) -> Option<Rect> {
            let root = self.host.root_view()?;
            let rect = root.convertRect_fromView(rect, Some(view));
            Some(Rect::new(
                Point::new(rect.origin.x, rect.origin.y),
                Size::new(rect.size.width, rect.size.height),
            ))
        }

        /// Returns the bounds of `view` in root coordinates.
        #[must_use]
        pub fn view_rect(&self, view: &NSView) -> Option<Rect> {
            self.host
                .root_view()
                .map(|root| rect_in_root(view, Some(&root)))
        }

        /// Returns the visible/clipped rect of `view` in root coordinates.
        #[must_use]
        pub fn visible_rect(&self, view: &NSView) -> Option<Rect> {
            convert_visible_rect_to_root(view, self.host.root_view().as_deref())
        }

        /// Returns the root-space rect for a text range.
        #[must_use]
        pub fn text_range_rect(&self, view: &NSTextView, range: TextRange) -> Option<Rect> {
            self.host.text_range_rect(view, range)
        }

        /// Returns the insertion caret rect for `location` in root coordinates.
        #[must_use]
        pub fn insertion_caret_rect(&self, view: &NSTextView, location: usize) -> Option<Rect> {
            self.host.insertion_caret_rect(view, location)
        }

        /// Returns the selected scalar range for `view`.
        #[must_use]
        pub fn selected_text_range(&self, view: &NSTextView) -> TextRange {
            self.host.selected_text_range(view)
        }
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

    type ProviderSnapshot = SemanticSnapshot;

    /// AppKit window host used to build, capture, query, and drive a hidden test scene.
    ///
    /// This is the main AppKit integration surface. Use it to attach semantic
    /// metadata to native views, capture pixels, query the current scene, and
    /// drive AppKit-mediated input. Standard `NSControl` clicks may be
    /// activated through `performClick:` instead of raw pointer-event
    /// injection. AppKit construction and attachment APIs remain explicitly
    /// main-thread-bound, but once a host has been created it carries that
    /// capability internally so common post-mount operations such as `input()`
    /// and `text_renderer()` stay marker-free.
    pub struct AppKitWindowHost {
        mtm: MainThreadMarker,
        window: Retained<NSWindow>,
        root_view: RefCell<Option<Retained<NSView>>>,
        registry: RefCell<Vec<RegisteredView>>,
        provider: RefCell<Option<Box<dyn AppKitSceneSource>>>,
        detached_root_view: bool,
        tracks_window_content: bool,
        attached_child_window_at_attach: bool,
    }

    impl AppKitWindowHost {
        /// Creates a window host with a new `NSWindow`.
        ///
        /// Most downstream tests should call `AppKitHarness::create_window`
        /// instead of constructing hosts directly.
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
            window.setFrame_display(
                offscreen_window_frame_rect(mtm, style, width, height),
                false,
            );
            configure_background_test_window(&window);
            Self {
                mtm,
                window,
                root_view: RefCell::new(None),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                detached_root_view: false,
                tracks_window_content: true,
                attached_child_window_at_attach: false,
            }
        }

        /// Attaches a host to an existing `NSWindow`.
        ///
        /// Prefer `AppKitHarness::attach_window` when a harness is already in
        /// scope so the harness remains the public carrier for the main-thread
        /// capability.
        #[must_use]
        pub fn from_window(window: &NSWindow, mtm: MainThreadMarker) -> Self {
            let attached_child_window_at_attach = window.parentWindow().is_some();
            let retained = unsafe {
                Retained::retain(window as *const NSWindow as *mut NSWindow)
                    .expect("window attachment should retain successfully")
            };
            let root_view = retained.contentView().map(|view| unsafe {
                Retained::retain(&*view as *const NSView as *mut NSView)
                    .expect("content view attachment should retain successfully")
            });
            Self {
                mtm,
                window: retained,
                root_view: RefCell::new(root_view),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                detached_root_view: false,
                tracks_window_content: true,
                attached_child_window_at_attach,
            }
        }

        /// Attaches a host to an existing root view and optional parent window.
        ///
        /// When no window is supplied, the host installs the view into a managed offscreen
        /// window so capture and low-level input APIs remain usable. Semantic click APIs still
        /// report `DetachedRootView` because the supplied root is not treated as window-rooted.
        /// Prefer `AppKitHarness::attach_root_view` when a harness is already in scope.
        #[must_use]
        pub fn from_root(view: &NSView, window: Option<&NSWindow>, mtm: MainThreadMarker) -> Self {
            let attached_child_window_at_attach =
                window.is_some_and(|window| window.parentWindow().is_some());
            let root = unsafe {
                Retained::retain(view as *const NSView as *mut NSView)
                    .expect("root view attachment should retain successfully")
            };
            let attached_window = window
                .map(|window| unsafe {
                    Retained::retain(window as *const NSWindow as *mut NSWindow)
                        .expect("window attachment should retain successfully")
                })
                .unwrap_or_else(|| managed_window_for_root_view(view, mtm));
            Self {
                mtm,
                window: attached_window,
                root_view: RefCell::new(Some(root)),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                detached_root_view: window.is_none(),
                tracks_window_content: false,
                attached_child_window_at_attach,
            }
        }

        /// Compatibility wrapper for the legacy name.
        #[must_use]
        pub fn from_root_view(
            view: &NSView,
            window: Option<&NSWindow>,
            mtm: MainThreadMarker,
        ) -> Self {
            Self::from_root(view, window, mtm)
        }

        /// Returns the underlying `NSWindow`.
        #[must_use]
        pub fn window(&self) -> &NSWindow {
            &self.window
        }

        /// Sets the window content view.
        pub fn set_root(&self, view: &NSView) {
            self.window.setContentView(Some(view));
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
        ///
        /// Use a provider when the test needs semantic nodes that do not map
        /// one-to-one onto concrete AppKit views.
        pub fn set_semantic_provider(&self, provider: Box<dyn SemanticProvider>) {
            self.set_contextual_scene_source(Box::new(LegacyAppKitSceneSource { provider }));
        }

        /// Preferred name for registering a pull-based scene source.
        pub fn set_scene_source(&self, provider: Box<dyn SemanticProvider>) {
            self.set_semantic_provider(provider);
        }

        /// Registers a host-aware AppKit scene source.
        pub fn set_contextual_scene_source(&self, provider: Box<dyn AppKitSceneSource>) {
            *self.provider.borrow_mut() = Some(provider);
        }

        /// Captures the current root view as an image.
        #[must_use]
        pub fn capture(&self) -> Option<glasscheck_core::Image> {
            let root = self.root_view()?;
            capture_view_image(&root, self.mtm)
        }

        /// Captures a specific view as an image.
        #[must_use]
        pub fn capture_subtree(&self, view: &NSView) -> Option<glasscheck_core::Image> {
            capture_view_image(view, self.mtm)
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
            let image = self
                .capture()
                .ok_or(RegionResolveError::CaptureUnavailable)?;
            let view_image = image.flip_vertical();
            let provider_snapshot = self.provider_snapshot();
            let scene =
                self.snapshot_scene_with_provider_snapshot(Some(&view_image), provider_snapshot);
            let rect =
                scene.resolve_region_with_image(self.root_bounds(), Some(&view_image), region)?;
            Ok(crop_image_in_view_coordinates(&image, rect))
        }

        /// Returns an input driver scoped to this window.
        ///
        /// The returned driver still satisfies the shared `InputDriver` trait,
        /// but the main-thread capability is taken from the host rather than a
        /// call-site argument.
        #[must_use]
        pub fn input(&self) -> AppKitInputDriver<'_> {
            AppKitInputDriver::new(
                &self.window,
                self.mtm,
                self.uses_attached_child_click_path(),
            )
        }

        /// Clicks the semantic hit point of the unique node matching `predicate`.
        ///
        /// Prefer this over raw coordinate clicks when the test is about user
        /// intent rather than a specific pixel location.
        pub fn click_node(&self, predicate: &Selector) -> Result<(), RegionResolveError> {
            self.click_node_with_search(predicate, &HitPointSearch::default())
        }

        /// Opens the native AppKit context menu for the unique node matching `predicate`.
        ///
        /// The returned value is a retained `NSMenu` semantic handle, not a popup
        /// window surface. AppKit-specific right-click versus Control-click
        /// handling is encapsulated by the backend.
        pub fn context_click_node(
            &self,
            predicate: &Selector,
        ) -> Result<AppKitContextMenu, AppKitContextMenuError> {
            self.context_click_node_with_search(predicate, &HitPointSearch::default())
        }

        /// Resolves a semantic hit point for the unique node matching `predicate`.
        pub fn resolve_hit_point(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<Point, RegionResolveError> {
            if self.detached_root_view {
                return Err(RegionResolveError::DetachedRootView);
            }
            let scene = self.snapshot_scene();
            let handle = scene.find(predicate).map_err(map_query_error)?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            let root_view = self.root_view();
            let registered_view = self.registered_view_for_handle(handle, root_view.as_deref());
            let (point, _) = self
                .click_target(
                    root_view.as_deref(),
                    registered_view.as_deref(),
                    node,
                    search,
                )
                .ok_or(RegionResolveError::InputUnavailable)?;
            let point = self.root_point_to_window_point(point);
            Ok(Point::new(point.x, point.y))
        }

        /// Resolves a semantic hit point in root coordinates.
        pub fn resolve_root_hit_point(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<Point, RegionResolveError> {
            if self.detached_root_view {
                return Err(RegionResolveError::DetachedRootView);
            }
            let scene = self.snapshot_scene();
            let handle = scene.find(predicate).map_err(map_query_error)?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            let root_view = self.root_view();
            let registered_view = self.registered_view_for_handle(handle, root_view.as_deref());
            let (point, _) = self
                .click_target(
                    root_view.as_deref(),
                    registered_view.as_deref(),
                    node,
                    search,
                )
                .ok_or(RegionResolveError::InputUnavailable)?;
            Ok(Point::new(point.x, point.y))
        }

        /// Clicks the unique node matching `predicate` using semantic hit-point search.
        ///
        /// Standard `NSControl` targets are activated via `performClick:` when
        /// available, including unregistered controls found by hit-testing;
        /// other views use point-based input dispatch.
        pub fn click_node_with_search(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<(), RegionResolveError> {
            if self.detached_root_view {
                return Err(RegionResolveError::DetachedRootView);
            }
            let scene = self.snapshot_scene();
            let handle = scene.find(predicate).map_err(map_query_error)?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            let root_view = self.root_view();
            let registered_view = self.registered_view_for_handle(handle, root_view.as_deref());
            if let Some(view) = registered_view
                .as_deref()
                .filter(|view| is_control_view(view))
            {
                unsafe {
                    let () = msg_send![&**view, performClick: std::ptr::null::<AnyObject>()];
                }
                return Ok(());
            }
            let (root_point, click_view) = match self.click_target(
                root_view.as_deref(),
                registered_view.as_deref(),
                node,
                search,
            ) {
                Some((point, click_view)) => (point, click_view),
                None => return Err(RegionResolveError::InputUnavailable),
            };
            let point = self.root_point_to_window_point(root_point);
            let control_fallback = (click_view.is_none() && registered_view.is_none())
                .then(|| hit_test_control_view(root_view.as_deref(), root_point))
                .flatten();
            if let Some(view) = click_view.or(registered_view).or(control_fallback) {
                if is_control_view(&view) {
                    unsafe {
                        let () = msg_send![&*view, performClick: std::ptr::null::<AnyObject>()];
                    }
                } else {
                    self.input()
                        .click_target(&view, point)
                        .map_err(map_input_error)?;
                }
                return Ok(());
            }
            if self.window.contentView().is_none() {
                return Err(RegionResolveError::InputUnavailable);
            }
            if self.tracks_window_content {
                self.input()
                    .click_window_point_with_local_mouse_up_monitor(Point::new(point.x, point.y))
                    .map_err(map_input_error)?;
            } else {
                self.input()
                    .click(Point::new(point.x, point.y))
                    .map_err(map_input_error)?;
            }
            Ok(())
        }

        /// Opens the native AppKit context menu using semantic hit-point search.
        pub fn context_click_node_with_search(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<AppKitContextMenu, AppKitContextMenuError> {
            if self.detached_root_view {
                return Err(RegionResolveError::DetachedRootView.into());
            }
            let scene = self.snapshot_scene();
            let handle = scene.find(predicate).map_err(map_query_error)?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            let root_view = self.root_view();
            let registered_view = self.registered_view_for_handle(handle, root_view.as_deref());
            let (root_point, click_view) = self
                .click_target(
                    root_view.as_deref(),
                    registered_view.as_deref(),
                    node,
                    search,
                )
                .ok_or(RegionResolveError::InputUnavailable)?;
            let point = self.root_point_to_window_point(root_point);
            let input = self.input();
            let menu = match (click_view, registered_view) {
                (Some(click_view), Some(registered_view)) => {
                    let menu = input
                        .context_click_target(&click_view, point)
                        .map_err(|error| AppKitContextMenuError::Resolve(map_input_error(error)))?;
                    if menu.is_some() || std::ptr::eq(&*click_view, &*registered_view) {
                        menu
                    } else {
                        input
                            .context_click_target(&registered_view, point)
                            .map_err(|error| {
                                AppKitContextMenuError::Resolve(map_input_error(error))
                            })?
                    }
                }
                (Some(view), None) | (None, Some(view)) => input
                    .context_click_target(&view, point)
                    .map_err(|error| AppKitContextMenuError::Resolve(map_input_error(error)))?,
                (None, None) => input
                    .context_click(Point::new(point.x, point.y))
                    .map_err(|error| AppKitContextMenuError::Resolve(map_input_error(error)))?,
            }
            .ok_or(AppKitContextMenuError::NoContextMenu)?;
            Ok(AppKitContextMenu::new(menu))
        }

        /// Opens the native AppKit context menu at a root-coordinate point.
        pub fn context_click_root_point(
            &self,
            point: Point,
        ) -> Result<AppKitContextMenu, AppKitContextMenuError> {
            if self.detached_root_view {
                return Err(RegionResolveError::DetachedRootView.into());
            }
            let point = self.root_point_to_window_point(NSPoint::new(point.x, point.y));
            let menu = self
                .input()
                .context_click(Point::new(point.x, point.y))
                .map_err(|error| AppKitContextMenuError::Resolve(map_input_error(error)))?
                .ok_or(AppKitContextMenuError::NoContextMenu)?;
            Ok(AppKitContextMenu::new(menu))
        }

        /// Moves the pointer to the semantic hit point of the unique node.
        pub fn hover_node(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<(), RegionResolveError> {
            let point = self.resolve_hit_point(predicate, search)?;
            self.input().move_mouse(point).map_err(map_input_error)
        }

        /// Returns a text-rendering harness that uses this host for live capture.
        ///
        /// This method is intentionally marker-free so a shared post-mount test
        /// body can call `host.text_renderer()` unchanged across backends.
        #[must_use]
        pub fn text_renderer(&self) -> AppKitTextHarness<'_> {
            AppKitTextHarness::new(self)
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

        /// Returns the selected scalar range in a live `NSTextView`.
        #[must_use]
        pub fn selected_text_range(&self, view: &NSTextView) -> TextRange {
            let range = NSTextInputClient::selectedRange(view);
            let content = view.string().to_string();
            let start = utf16_offset_to_scalar_index(&content, range.location);
            let end = utf16_offset_to_scalar_index(&content, range.location + range.length);
            TextRange::new(start, end.saturating_sub(start))
        }

        /// Clicks the insertion point for `location` in `view`.
        pub fn click_text_position(
            &self,
            view: &NSTextView,
            location: usize,
        ) -> Result<(), InputSynthesisError> {
            let desired = TextRange::new(location, 0);
            let Some(rect) = self.insertion_caret_rect(view, location) else {
                self.input().set_selection(view, desired);
                return Ok(());
            };
            let root_point = NSPoint::new(
                rect.origin.x + (rect.size.width / 2.0).max(0.5),
                rect.origin.y + (rect.size.height / 2.0).max(0.5),
            );
            let point = self.root_point_to_window_point(root_point);
            self.input()
                .click_text_view(view, Point::new(point.x, point.y))?;
            if self.selected_text_range(view) != desired {
                self.input().set_selection(view, desired);
            }
            Ok(())
        }

        /// Registers semantic metadata for a view so it can be queried later.
        pub fn register_node(&self, view: &NSView, descriptor: InstrumentedView) {
            self.register_instrumented_node(
                view,
                InstrumentedNode {
                    id: descriptor.id,
                    role: descriptor.role,
                    label: descriptor.label,
                    selectors: descriptor.selectors,
                    ..InstrumentedNode::default()
                },
            );
        }

        /// Registers a fully populated backend-neutral native node descriptor.
        pub fn register_instrumented_node(&self, view: &NSView, descriptor: InstrumentedNode) {
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
        pub fn register_view(&self, view: &NSView, descriptor: InstrumentedView) {
            self.register_node(view, descriptor);
        }

        /// Builds the current merged scene from registered native views and virtual nodes.
        #[must_use]
        pub fn snapshot_scene(&self) -> Scene {
            let (provider_snapshot, image) = self.provider_snapshot_with_optional_capture(false);
            self.snapshot_scene_with_provider_snapshot(image.as_ref(), provider_snapshot)
        }

        fn snapshot_scene_with_provider_snapshot(
            &self,
            image: Option<&Image>,
            provider_snapshot: Option<ProviderSnapshot>,
        ) -> Scene {
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
                    )
                    .with_provenance(NodeProvenanceKind::Native);
                    if let Some(source_id) = registered_ids
                        .get(&(&*entry.view as *const NSView))
                        .and_then(|registered| registered.source_id.clone())
                    {
                        node.properties.insert(
                            "glasscheck:source_id".into(),
                            glasscheck_core::PropertyValue::String(source_id),
                        );
                        node.property_provenance
                            .insert("glasscheck:source_id".into(), NodeProvenanceKind::Native);
                    }
                    node.properties.insert(
                        "glasscheck:paint_order_path".into(),
                        glasscheck_core::PropertyValue::String(native_paint_order_path(
                            &entry.view,
                            root_view.as_deref(),
                        )),
                    );
                    node.property_provenance.insert(
                        "glasscheck:paint_order_path".into(),
                        NodeProvenanceKind::Native,
                    );
                    let (visible, visible_rect, hit_testable) =
                        native_visibility(&entry.view, root_view.as_deref());
                    node.label = entry.descriptor.label.clone();
                    node.selectors
                        .extend(entry.descriptor.selectors.iter().cloned());
                    node.state.extend(entry.descriptor.state.clone());
                    node.properties.extend(entry.descriptor.properties.clone());
                    node.state_provenance.extend(
                        entry
                            .descriptor
                            .state
                            .keys()
                            .cloned()
                            .map(|key| (key, NodeProvenanceKind::Native)),
                    );
                    node.property_provenance.extend(
                        entry
                            .descriptor
                            .properties
                            .keys()
                            .cloned()
                            .map(|key| (key, NodeProvenanceKind::Native)),
                    );
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

            if let Some(snapshot) = provider_snapshot {
                let native_ids = nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<BTreeSet<_>>();
                let native_parent_id_counts = registered_ids.values().fold(
                    BTreeMap::<String, usize>::new(),
                    |mut counts, registered| {
                        let raw_id = registered
                            .source_id
                            .clone()
                            .unwrap_or_else(|| registered.snapshot_id.clone());
                        *counts.entry(raw_id).or_default() += 1;
                        counts
                    },
                );
                let unique_native_parent_ids = native_parent_id_counts
                    .iter()
                    .filter_map(|(id, count)| (*count == 1).then_some(id.clone()))
                    .collect::<BTreeSet<_>>();
                nodes.extend(normalize_provider_nodes(
                    snapshot.nodes,
                    &native_ids,
                    &unique_native_parent_ids,
                ));
                if snapshot.recipes.is_empty() {
                    return Scene::new(nodes);
                }
                let resolved_recipes =
                    resolve_node_recipes(nodes, self.root_bounds(), image, &snapshot.recipes);
                let recipe_errors = resolved_recipes.errors;
                return Scene::with_recipe_errors(resolved_recipes.nodes, recipe_errors);
            }

            Scene::new(nodes)
        }

        /// Resolves a semantic region against the current scene.
        pub fn resolve_region(&self, region: &RegionSpec) -> Result<Rect, RegionResolveError> {
            let (provider_snapshot, image) =
                self.provider_snapshot_with_optional_capture(region.requires_image());
            let root_bounds = self.root_bounds();
            self.snapshot_scene_with_provider_snapshot(image.as_ref(), provider_snapshot)
                .resolve_region_with_image(root_bounds, image.as_ref(), region)
        }

        /// Sets the host window title.
        pub fn set_title(&self, title: &str) {
            let title = NSString::from_str(title);
            self.window.setTitle(&title);
        }

        #[must_use]
        pub(crate) fn main_thread_marker(&self) -> MainThreadMarker {
            self.mtm
        }

        fn root_view(&self) -> Option<Retained<NSView>> {
            if self.tracks_window_content {
                let current = self.window.contentView();
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
            self.root_view.borrow().as_ref().map(|view| unsafe {
                Retained::retain(&**view as *const NSView as *mut NSView)
                    .expect("root view should retain successfully")
            })
        }

        fn uses_attached_child_click_path(&self) -> bool {
            self.attached_child_window_at_attach || self.window.parentWindow().is_some()
        }

        fn provider_snapshot(&self) -> Option<ProviderSnapshot> {
            self.provider
                .borrow()
                .as_ref()
                .map(|provider| provider.snapshot(&AppKitSnapshotContext::new(self)))
        }

        fn provider_snapshot_with_optional_capture(
            &self,
            force_image: bool,
        ) -> (Option<ProviderSnapshot>, Option<Image>) {
            let mut provider_snapshot = self.provider_snapshot();
            let needs_image = force_image
                || provider_snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot
                        .recipes
                        .iter()
                        .any(|recipe| recipe.requires_image())
                });
            let image = needs_image.then(|| self.capture()).flatten().map(|image| {
                provider_snapshot = self.provider_snapshot();
                image.flip_vertical()
            });
            (provider_snapshot, image)
        }

        fn root_bounds(&self) -> Rect {
            let content = self
                .root_view()
                .map(|view| view.bounds())
                .unwrap_or_else(|| window_root_local_bounds(&self.window));
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
            search: &HitPointSearch,
        ) -> Option<(NSPoint, Option<Retained<NSView>>)> {
            let target_rect = explicit_hit_rect(node)
                .or(node.visible_rect)
                .unwrap_or(node.rect);
            if rect_is_empty_rect(target_rect) {
                if let Some(point) = explicit_hit_point(node) {
                    return Some((NSPoint::new(point.x, point.y), None));
                }
                return None;
            }
            for point in hit_point_candidates(target_rect, search) {
                if let Some(target_view) = target_view {
                    match self.resolve_registered_click_route(
                        root_view,
                        target_view,
                        target_rect,
                        point,
                    ) {
                        RegisteredViewClickRoute::Target(view) => return Some((point, Some(view))),
                        RegisteredViewClickRoute::Descendant(view) => {
                            return Some((point, Some(view)))
                        }
                        RegisteredViewClickRoute::Blocked => continue,
                    }
                }
                return Some((point, None));
            }
            None
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
        hit_point_candidates(visible_rect, &HitPointSearch::default())
            .into_iter()
            .any(|point| {
                let hit = match root_view {
                    Some(root_view) => root_view.hitTest(point),
                    None => view.hitTest(NSPoint::new(
                        point.x - visible_rect.origin.x,
                        point.y - visible_rect.origin.y,
                    )),
                };

                hit.as_deref()
                    .is_some_and(|hit| std::ptr::eq(hit, view) || is_descendant_of_view(hit, view))
            })
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

    fn hit_test_control_view(
        root_view: Option<&NSView>,
        point: NSPoint,
    ) -> Option<Retained<NSView>> {
        let hit = root_view?.hitTest(point)?;
        is_control_view(&hit).then_some(hit)
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

    fn managed_window_for_root_view(view: &NSView, mtm: MainThreadMarker) -> Retained<NSWindow> {
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
            window.setFrame_display(
                offscreen_window_frame_rect(mtm, style, bounds.size.width, bounds.size.height),
                false,
            );
            window.setContentView(Some(view));
        }
        configure_background_test_window(&window);
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
                let source_id = (stable_counts.get(&stable_id).copied().unwrap_or(0) > 1)
                    .then(|| stable_id.clone());

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

    fn native_paint_order_path(view: &NSView, root_view: Option<&NSView>) -> String {
        let mut segments = Vec::new();
        let mut current = unsafe {
            Retained::retain(view as *const NSView as *mut NSView)
                .expect("paint-order view retention should succeed")
        };

        loop {
            if let Some(parent) = unsafe { current.superview() } {
                segments.push(sibling_index(&current, &parent));
                if root_view.is_some_and(|root| std::ptr::eq(&*parent, root)) {
                    segments.push(0);
                    break;
                }
                current = parent;
            } else {
                segments.push(0);
                break;
            }
        }

        segments.reverse();
        segments
            .into_iter()
            .map(|segment| format!("{segment:010}"))
            .collect::<Vec<_>>()
            .join("/")
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

    fn normalize_provider_nodes(
        mut nodes: Vec<SemanticNode>,
        native_ids: &BTreeSet<String>,
        native_parent_ids: &BTreeSet<String>,
    ) -> Vec<SemanticNode> {
        let provider_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
        let original_counts = provider_ids.iter().fold(BTreeMap::new(), |mut counts, id| {
            *counts.entry(id.clone()).or_default() += 1;
            counts
        });
        let needs_namespace =
            provider_ids.iter().any(|id| native_ids.contains(id)) || has_duplicates(&provider_ids);
        if !needs_namespace {
            for node in &mut nodes {
                node.properties
                    .entry("glasscheck:source_id".into())
                    .or_insert_with(|| glasscheck_core::PropertyValue::String(node.id.clone()));
                node.property_provenance
                    .entry("glasscheck:source_id".into())
                    .or_insert(node.provenance);
            }
            let identity_ids = provider_ids
                .iter()
                .filter(|id| original_counts.get(*id).copied() == Some(1))
                .map(|id| (id.clone(), id.clone()))
                .collect::<BTreeMap<_, _>>();
            repair_provider_parent_ids(
                &mut nodes,
                &original_counts,
                &identity_ids,
                native_parent_ids,
                true,
            );
            return nodes;
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
            node.property_provenance
                .insert("glasscheck:source_id".into(), node.provenance);
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

        repair_provider_parent_ids(
            &mut nodes,
            &original_counts,
            &original_to_unique,
            native_parent_ids,
            false,
        );

        nodes
    }

    fn repair_provider_parent_ids(
        nodes: &mut [SemanticNode],
        original_counts: &BTreeMap<String, usize>,
        original_to_unique: &BTreeMap<String, String>,
        native_parent_ids: &BTreeSet<String>,
        allow_unique_native_parents: bool,
    ) {
        for node in nodes {
            if let Some(parent_id) = node.parent_id.as_ref() {
                if original_counts.get(parent_id).copied().unwrap_or(0) == 1 {
                    node.parent_id = original_to_unique.get(parent_id).cloned();
                } else if allow_unique_native_parents && native_parent_ids.contains(parent_id) {
                    node.parent_id = Some(parent_id.clone());
                } else {
                    node.properties.insert(
                        "glasscheck:ambiguous_parent_id".into(),
                        glasscheck_core::PropertyValue::String(parent_id.clone()),
                    );
                    node.property_provenance
                        .insert("glasscheck:ambiguous_parent_id".into(), node.provenance);
                    node.parent_id = None;
                }
            }
        }
    }

    fn has_duplicates(ids: &[String]) -> bool {
        let mut seen = BTreeSet::new();
        ids.iter().any(|id| !seen.insert(id.clone()))
    }

    fn ns_range_for_scalar_range(view: &NSTextView, range: TextRange) -> NSRange {
        let content = view.string().to_string();
        let start = scalar_index_to_utf16_offset(&content, range.start);
        let end = scalar_index_to_utf16_offset(&content, range.start + range.len);
        NSRange::new(start, end.saturating_sub(start))
    }

    fn utf16_offset_to_scalar_index(text: &str, utf16_offset: usize) -> usize {
        let mut utf16_count = 0usize;
        for (index, ch) in text.chars().enumerate() {
            if utf16_count >= utf16_offset {
                return index;
            }
            utf16_count += ch.len_utf16();
        }
        text.chars().count()
    }

    fn hit_point_candidates(rect: Rect, search: &HitPointSearch) -> Vec<NSPoint> {
        match search.strategy {
            HitPointStrategy::VisibleCenterFirst => {
                let center = NSPoint::new(
                    rect.origin.x + rect.size.width / 2.0,
                    rect.origin.y + rect.size.height / 2.0,
                );
                let mut points = vec![center];
                for point in hit_point_candidates(
                    rect,
                    &HitPointSearch {
                        strategy: HitPointStrategy::Grid,
                        sample_count: search.sample_count.max(9),
                    },
                ) {
                    if !points
                        .iter()
                        .any(|candidate| candidate.x == point.x && candidate.y == point.y)
                    {
                        points.push(point);
                    }
                }
                points
            }
            HitPointStrategy::CornersAndCenter => vec![
                NSPoint::new(rect.origin.x, rect.origin.y),
                NSPoint::new(rect.origin.x + rect.size.width, rect.origin.y),
                NSPoint::new(rect.origin.x, rect.origin.y + rect.size.height),
                NSPoint::new(
                    rect.origin.x + rect.size.width,
                    rect.origin.y + rect.size.height,
                ),
                NSPoint::new(
                    rect.origin.x + rect.size.width / 2.0,
                    rect.origin.y + rect.size.height / 2.0,
                ),
            ],
            HitPointStrategy::Grid => {
                let dimension = (search.sample_count.max(4) as f64).sqrt().ceil() as usize;
                let mut points = Vec::new();
                for row in 0..dimension {
                    for column in 0..dimension {
                        points.push(NSPoint::new(
                            rect.origin.x
                                + rect.size.width * ((column as f64 + 0.5) / dimension as f64),
                            rect.origin.y
                                + rect.size.height * ((row as f64 + 0.5) / dimension as f64),
                        ));
                    }
                }
                points
            }
        }
    }

    fn map_query_error(error: glasscheck_core::QueryError) -> RegionResolveError {
        match error {
            glasscheck_core::QueryError::NotFound(predicate) => {
                RegionResolveError::NotFound(predicate)
            }
            glasscheck_core::QueryError::MultipleMatches { selector, count } => {
                RegionResolveError::MultipleMatches {
                    predicate: selector,
                    count,
                }
            }
        }
    }

    fn map_input_error(_error: InputSynthesisError) -> RegionResolveError {
        RegionResolveError::InputUnavailable
    }

    fn explicit_hit_point(node: &SemanticNode) -> Option<Point> {
        match (
            node.properties.get("glasscheck:hit_point_x"),
            node.properties.get("glasscheck:hit_point_y"),
        ) {
            (Some(PropertyValue::Integer(x)), Some(PropertyValue::Integer(y))) => {
                Some(Point::new(*x as f64, *y as f64))
            }
            _ => None,
        }
    }

    fn explicit_hit_rect(node: &SemanticNode) -> Option<Rect> {
        match (
            node.properties.get("glasscheck:hit_rect_x"),
            node.properties.get("glasscheck:hit_rect_y"),
            node.properties.get("glasscheck:hit_rect_width"),
            node.properties.get("glasscheck:hit_rect_height"),
        ) {
            (
                Some(PropertyValue::Integer(x)),
                Some(PropertyValue::Integer(y)),
                Some(PropertyValue::Integer(width)),
                Some(PropertyValue::Integer(height)),
            ) => Some(Rect::new(
                Point::new(*x as f64, *y as f64),
                Size::new(*width as f64, *height as f64),
            )),
            _ => None,
        }
    }

    struct LegacyAppKitSceneSource {
        provider: Box<dyn SemanticProvider>,
    }

    impl AppKitSceneSource for LegacyAppKitSceneSource {
        fn snapshot(&self, _context: &AppKitSnapshotContext<'_>) -> SemanticSnapshot {
            self.provider.snapshot()
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct InstrumentedView {
        pub id: Option<String>,
        pub role: Option<glasscheck_core::Role>,
        pub label: Option<String>,
        pub selectors: Vec<String>,
    }
    pub trait AppKitSceneSource {}
    pub struct AppKitSnapshotContext<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
    pub struct AppKitWindowHost;
}

#[allow(unused_imports)]
pub use imp::{AppKitSceneSource, AppKitSnapshotContext, AppKitWindowHost, InstrumentedView};
