#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::collections::{BTreeSet, HashMap};

    use glasscheck_core::{
        crop_image_bottom_left, normalize_provider_nodes, registered_node_id, resolve_node_recipes,
        HitPointSearch, HitPointStrategy, Image, InputSynthesisError, InstrumentedNode,
        NodeProvenanceKind, Point, PropertyValue, Rect, RegionResolveError, RegionSpec, Role,
        Scene, Selector, SemanticNode, SemanticProvider, SemanticSnapshot, Size, TextRange,
    };
    use gtk4::graphene;
    use gtk4::prelude::*;
    use gtk4::{TextView, TextWindowType, Widget, Window};

    use crate::input::GtkInputDriver;
    use crate::screen::present_window_offscreen;
    use crate::text::GtkTextHarness;

    struct RegisteredWidget {
        widget: Widget,
        descriptor: InstrumentedNode,
    }

    type ProviderSnapshot = SemanticSnapshot;

    /// Host-aware contextual semantic provider for GTK scenes.
    pub trait GtkSceneSource {
        /// Produces the semantic snapshot for the active GTK host.
        fn snapshot(&self, context: &GtkSnapshotContext<'_>) -> SemanticSnapshot;
    }

    /// Host-aware geometry helpers for GTK semantic providers.
    pub struct GtkSnapshotContext<'a> {
        host: &'a GtkWindowHost,
    }

    impl<'a> GtkSnapshotContext<'a> {
        fn new(host: &'a GtkWindowHost) -> Self {
            Self { host }
        }

        #[must_use]
        pub(crate) fn host(&self) -> &'a GtkWindowHost {
            self.host
        }

        #[must_use]
        pub fn root_bounds(&self) -> Rect {
            self.host.root_bounds()
        }

        #[must_use]
        pub fn widget_rect(&self, widget: &Widget) -> Option<Rect> {
            let root = self.host.root_widget()?;
            rect_of_widget(widget, Some(&root))
        }

        #[must_use]
        pub fn visible_rect(&self, widget: &Widget) -> Option<Rect> {
            visible_rect_of_widget(widget, self.host.root_widget().as_ref())
        }

        #[must_use]
        pub fn text_range_rect(&self, view: &TextView, range: TextRange) -> Option<Rect> {
            self.host.text_range_rect(view, range)
        }

        #[must_use]
        pub fn insertion_caret_rect(&self, view: &TextView, location: usize) -> Option<Rect> {
            self.host.insertion_caret_rect(view, location)
        }

        #[must_use]
        pub fn selected_text_range(&self, view: &TextView) -> TextRange {
            self.host.selected_text_range(view)
        }
    }

    struct ResolvedClickTarget {
        root_widget: Widget,
        registered_widget: Option<Widget>,
        point: Point,
    }

    /// GTK4 window host used to build, capture, query, and drive a test scene.
    ///
    /// Use it to attach semantic metadata to live widgets, capture rendered
    /// pixels, and query the current widget tree through a backend-neutral
    /// scene model.
    pub struct GtkWindowHost {
        window: Window,
        root_widget: RefCell<Option<Widget>>,
        registry: RefCell<Vec<RegisteredWidget>>,
        provider: RefCell<Option<Box<dyn GtkSceneSource>>>,
        owns_window: bool,
        detached_root_widget: bool,
        tracks_window_child: bool,
    }

    impl GtkWindowHost {
        /// Creates a window host with a new `gtk4::Window`.
        #[must_use]
        pub fn new(width: f64, height: f64) -> Self {
            let window = Window::builder()
                .default_width(width.round() as i32)
                .default_height(height.round() as i32)
                .build();
            Self {
                window,
                root_widget: RefCell::new(None),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                owns_window: true,
                detached_root_widget: false,
                tracks_window_child: true,
            }
        }

        /// Attaches a host to an existing `Window`.
        #[must_use]
        pub fn from_window(window: &Window) -> Self {
            Self {
                window: window.clone(),
                root_widget: RefCell::new(window.child()),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                owns_window: false,
                detached_root_widget: false,
                tracks_window_child: true,
            }
        }

        /// Attaches a host to an existing root widget and optional parent window.
        #[must_use]
        pub fn from_root(widget: &impl IsA<Widget>, window: Option<&Window>) -> Self {
            let root = widget.as_ref().clone();
            let owns_window = window.is_none();
            let attached_window = window
                .cloned()
                .unwrap_or_else(|| managed_window_for_root(widget));
            Self {
                window: attached_window,
                root_widget: RefCell::new(Some(root)),
                registry: RefCell::new(Vec::new()),
                provider: RefCell::new(None),
                owns_window,
                detached_root_widget: window.is_none(),
                tracks_window_child: false,
            }
        }

        /// Compatibility wrapper for the legacy name.
        #[must_use]
        pub fn from_root_view(widget: &impl IsA<Widget>, window: Option<&Window>) -> Self {
            Self::from_root(widget, window)
        }

        /// Returns the underlying `Window`.
        #[must_use]
        pub fn window(&self) -> &Window {
            &self.window
        }

        /// Sets the window child/root widget.
        pub fn set_root(&self, widget: &impl IsA<Widget>) {
            self.window.set_child(Some(widget));
            present_window_offscreen(&self.window);
            *self.root_widget.borrow_mut() = Some(widget.as_ref().clone());
        }

        /// Compatibility wrapper for the legacy name.
        pub fn set_content_view(&self, widget: &impl IsA<Widget>) {
            self.set_root(widget);
        }

        /// Registers a pull-based semantic provider for virtual nodes.
        ///
        /// This is useful for semantic overlays or logical nodes that are not
        /// represented by a single concrete widget.
        pub fn set_semantic_provider(&self, provider: Box<dyn SemanticProvider>) {
            self.set_contextual_scene_source(Box::new(LegacyGtkSceneSource { provider }));
        }

        /// Preferred name for registering a pull-based scene source.
        pub fn set_scene_source(&self, provider: Box<dyn SemanticProvider>) {
            self.set_semantic_provider(provider);
        }

        /// Registers a host-aware GTK scene source.
        pub fn set_contextual_scene_source(&self, provider: Box<dyn GtkSceneSource>) {
            *self.provider.borrow_mut() = Some(provider);
        }

        /// Captures the current root widget as an image.
        #[must_use]
        pub fn capture(&self) -> Option<glasscheck_core::Image> {
            let root = self.root_widget()?;
            capture_widget_image(&root)
        }

        /// Captures a specific widget subtree as an image.
        #[must_use]
        pub fn capture_subtree(&self, widget: &impl IsA<Widget>) -> Option<glasscheck_core::Image> {
            let cropped_from_root = || {
                let root = self.root_widget()?;
                let rect = rect_of_widget(widget.as_ref(), Some(&root))?;
                let image = self.capture()?;
                Some(crop_image_bottom_left(&image, rect))
            };

            cropped_from_root().or_else(|| capture_widget_image(widget.as_ref()))
        }

        /// Compatibility wrapper for the legacy name.
        #[must_use]
        pub fn capture_view(&self, widget: &impl IsA<Widget>) -> Option<glasscheck_core::Image> {
            self.capture_subtree(widget)
        }

        /// Captures a semantically resolved region as an image.
        pub fn capture_region(
            &self,
            region: &RegionSpec,
        ) -> Result<glasscheck_core::Image, RegionResolveError> {
            let provider_snapshot = self.provider_snapshot();
            let image = self
                .capture()
                .ok_or(RegionResolveError::CaptureUnavailable)?;
            let view_image = image.flip_vertical();
            let (scene, _) = self.snapshot_scene_with_registered_indices_and_provider_snapshot(
                Some(&view_image),
                provider_snapshot,
            );
            let rect =
                scene.resolve_region_with_image(self.root_bounds(), Some(&view_image), region)?;
            Ok(crop_image_bottom_left(&image, rect))
        }

        /// Returns an input driver scoped to this window.
        #[must_use]
        pub fn input(&self) -> GtkInputDriver<'_> {
            GtkInputDriver::new(&self.window)
        }

        /// Clicks the unique node matching `predicate`.
        ///
        /// GTK semantic clicks are best-effort. Registered widgets may be
        /// activated through GTK controller or widget APIs before falling back
        /// to native pointer synthesis. Prefer `input().click(...)` when strict
        /// X11 pointer fidelity matters more than semantic activation.
        pub fn click_node(&self, predicate: &Selector) -> Result<(), RegionResolveError> {
            self.click_node_with_search(predicate, &HitPointSearch::default())
        }

        /// Resolves a semantic hit point for the unique node matching `predicate`.
        pub fn resolve_hit_point(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<Point, RegionResolveError> {
            self.resolve_click_target(predicate, search)
                .map(|target| target.point)
        }

        /// Resolves a semantic hit point in root coordinates.
        pub fn resolve_root_hit_point(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<Point, RegionResolveError> {
            self.resolve_hit_point(predicate, search)
        }

        /// Clicks the unique node matching `predicate` using semantic hit-point search.
        ///
        /// This is a best-effort semantic interaction helper. It may use
        /// widget-local GTK activation and gesture dispatch for registered
        /// widgets before falling back to native pointer synthesis.
        pub fn click_node_with_search(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<(), RegionResolveError> {
            let target = self.resolve_click_target(predicate, search)?;
            if let Some(registered_widget) = target.registered_widget.as_ref() {
                if dispatch_semantic_click(&target.root_widget, registered_widget, target.point) {
                    return Ok(());
                }
            }
            self.input().click(target.point).map_err(map_input_error)?;
            Ok(())
        }

        /// Opens a GTK context menu at the unique node matching `predicate`.
        pub fn context_click_node(
            &self,
            predicate: &Selector,
        ) -> Result<crate::GtkContextMenu, crate::GtkContextMenuError> {
            self.context_click_node_with_search(predicate, &HitPointSearch::default())
        }

        /// Opens a GTK context menu at the unique node matching `predicate`.
        pub fn context_click_node_with_search(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<crate::GtkContextMenu, crate::GtkContextMenuError> {
            let target = self.resolve_click_target(predicate, search)?;
            let baseline = crate::menu::visible_context_menu_ids(&self.window);
            if let Some(registered_widget) = target.registered_widget.as_ref() {
                if dispatch_semantic_context_click(
                    &target.root_widget,
                    registered_widget,
                    target.point,
                ) {
                    crate::menu::flush_main_context(4);
                    if let Some(menu) =
                        crate::menu::discover_context_menu_excluding(&self.window, &baseline)
                    {
                        return Ok(menu);
                    }
                    return Err(crate::GtkContextMenuError::NoContextMenu);
                }
            }
            crate::menu::context_click_root_point_excluding(&self.window, target.point, &baseline)
        }

        /// Opens a GTK context menu at a root-coordinate point.
        pub fn context_click_root_point(
            &self,
            point: Point,
        ) -> Result<crate::GtkContextMenu, crate::GtkContextMenuError> {
            crate::menu::context_click_root_point(&self.window, point)
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
        #[must_use]
        pub fn text_renderer(&self) -> GtkTextHarness<'_> {
            GtkTextHarness::new(self)
        }

        /// Returns the bounding rect for a text range in a live `TextView`.
        #[must_use]
        pub fn text_range_rect(&self, view: &TextView, range: TextRange) -> Option<Rect> {
            let buffer = view.buffer();
            let start = buffer.iter_at_offset(range.start as i32);
            let end = buffer.iter_at_offset((range.start + range.len) as i32);
            let start_rect = view.iter_location(&start);
            let end_rect = view.iter_location(&end);
            let x = start_rect.x();
            let y = start_rect.y().min(end_rect.y());
            let width = if start_rect.y() == end_rect.y() {
                (end_rect.x() - start_rect.x()).abs().max(1)
            } else {
                start_rect.width().max(1)
            };
            let height = start_rect.height().max(end_rect.height()).max(1);
            let (window_x, window_y) = view.buffer_to_window_coords(TextWindowType::Widget, x, y);
            rect_in_root(
                view.upcast_ref(),
                self.root_widget().as_ref(),
                window_x as f64,
                window_y as f64,
                width as f64,
                height as f64,
            )
        }

        /// Returns the insertion caret rect for `location` in a live `TextView`.
        #[must_use]
        pub fn insertion_caret_rect(&self, view: &TextView, location: usize) -> Option<Rect> {
            let buffer = view.buffer();
            let iter = buffer.iter_at_offset(location as i32);
            let (strong, _) = view.cursor_locations(Some(&iter));
            let (window_x, window_y) =
                view.buffer_to_window_coords(TextWindowType::Widget, strong.x(), strong.y());
            rect_in_root(
                view.upcast_ref(),
                self.root_widget().as_ref(),
                window_x as f64,
                window_y as f64,
                strong.width().max(1) as f64,
                strong.height().max(1) as f64,
            )
        }

        /// Returns the selected scalar range in a live `TextView`.
        #[must_use]
        pub fn selected_text_range(&self, view: &TextView) -> TextRange {
            let buffer = view.buffer();
            if let Some((start, end)) = buffer.selection_bounds() {
                return TextRange::new(
                    start.offset() as usize,
                    (end.offset() - start.offset()).max(0) as usize,
                );
            }
            let offset = buffer.cursor_position().max(0) as usize;
            TextRange::new(offset, 0)
        }

        /// Clicks the insertion point for `location` in `view`.
        pub fn click_text_position(
            &self,
            view: &TextView,
            location: usize,
        ) -> Result<(), InputSynthesisError> {
            let Some(rect) = self.insertion_caret_rect(view, location) else {
                return Err(InputSynthesisError::MissingTarget);
            };
            self.input().click(Point::new(
                rect.origin.x + (rect.size.width / 2.0).max(0.5),
                rect.origin.y + (rect.size.height / 2.0).max(0.5),
            ))
        }

        /// Registers semantic metadata for a widget so it can be queried later.
        pub fn register_node(&self, widget: &impl IsA<Widget>, descriptor: InstrumentedNode) {
            self.registry.borrow_mut().push(RegisteredWidget {
                widget: widget.as_ref().clone(),
                descriptor,
            });
        }

        /// Compatibility wrapper for the legacy name.
        pub fn register_view(&self, widget: &impl IsA<Widget>, descriptor: InstrumentedNode) {
            self.register_node(widget, descriptor);
        }

        /// Builds the current merged scene from registered native widgets and virtual nodes.
        #[must_use]
        pub fn snapshot_scene(&self) -> Scene {
            self.snapshot_scene_with_registered_indices().0
        }

        fn snapshot_scene_with_registered_indices(&self) -> (Scene, Vec<usize>) {
            let provider_snapshot = self.provider_snapshot();
            let image = provider_snapshot
                .as_ref()
                .and_then(|snapshot| {
                    snapshot
                        .recipes
                        .iter()
                        .any(|recipe| recipe.requires_image())
                        .then(|| self.capture())
                })
                .flatten()
                .map(|image| image.flip_vertical());
            self.snapshot_scene_with_registered_indices_and_provider_snapshot(
                image.as_ref(),
                provider_snapshot,
            )
        }

        fn snapshot_scene_with_registered_indices_and_provider_snapshot(
            &self,
            image: Option<&Image>,
            provider_snapshot: Option<ProviderSnapshot>,
        ) -> (Scene, Vec<usize>) {
            let root_widget = self.root_widget();
            let registry = self.registry.borrow();
            let registered_ids = registry
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    (
                        widget_id(&entry.widget),
                        registered_node_id(index, entry.descriptor.id.as_deref(), "widget"),
                    )
                })
                .collect::<HashMap<usize, String>>();

            let mut registered_indices = Vec::new();
            let mut nodes: Vec<SemanticNode> = registry
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    let rect = rect_of_widget(&entry.widget, root_widget.as_ref())?;
                    let id = registered_node_id(index, entry.descriptor.id.as_deref(), "widget");
                    let visible_rect = visible_rect_of_widget(&entry.widget, root_widget.as_ref());
                    let mut node = SemanticNode::new(
                        id,
                        entry
                            .descriptor
                            .role
                            .clone()
                            .unwrap_or_else(|| infer_role(&entry.widget)),
                        rect,
                    )
                    .with_provenance(NodeProvenanceKind::Native);
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
                    node.visible = entry.widget.is_visible();
                    node.visible_rect = visible_rect;
                    node.hit_testable = entry.widget.can_target();
                    node.opacity = entry.widget.opacity() as f64;
                    if let Some(parent) = entry.widget.parent() {
                        node.child_index = sibling_index(&entry.widget, &parent);
                        node.z_index = node.child_index as i32;
                    } else {
                        node.child_index = index;
                        node.z_index = index as i32;
                    }
                    if let Some(parent_id) = registered_ancestor_id(&entry.widget, &registered_ids)
                    {
                        node.parent_id = Some(parent_id);
                    }
                    registered_indices.push(index);
                    Some(node)
                })
                .collect();

            if let Some(snapshot) = provider_snapshot {
                let native_ids = nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<BTreeSet<_>>();
                nodes.extend(normalize_provider_nodes(snapshot.nodes, &native_ids));
                if snapshot.recipes.is_empty() {
                    return (Scene::new(nodes), registered_indices);
                }
                let resolved_recipes =
                    resolve_node_recipes(nodes, self.root_bounds(), image, &snapshot.recipes);
                return (
                    Scene::with_recipe_errors(resolved_recipes.nodes, resolved_recipes.errors),
                    registered_indices,
                );
            }

            (Scene::new(nodes), registered_indices)
        }

        /// Resolves a semantic region against the current scene.
        pub fn resolve_region(&self, region: &RegionSpec) -> Result<Rect, RegionResolveError> {
            let provider_snapshot = self.provider_snapshot();
            let image = (region.requires_image()
                || provider_snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot
                        .recipes
                        .iter()
                        .any(|recipe| recipe.requires_image())
                }))
            .then(|| self.capture())
            .flatten()
            .map(|image| image.flip_vertical());
            self.snapshot_scene_with_registered_indices_and_provider_snapshot(
                image.as_ref(),
                provider_snapshot,
            )
            .0
            .resolve_region_with_image(self.root_bounds(), image.as_ref(), region)
        }

        /// Sets the host window title.
        pub fn set_title(&self, title: &str) {
            self.window.set_title(Some(title));
        }

        pub(crate) fn root_widget(&self) -> Option<Widget> {
            if self.tracks_window_child {
                *self.root_widget.borrow_mut() = self.window.child();
            }
            self.root_widget.borrow().clone()
        }

        pub(crate) fn root_bounds(&self) -> Rect {
            if let Some(root) = self.root_widget() {
                let width = root.allocated_width().max(1) as f64;
                let height = root.allocated_height().max(1) as f64;
                return Rect::new(Point::new(0.0, 0.0), Size::new(width, height));
            }
            Rect::new(
                Point::new(0.0, 0.0),
                Size::new(
                    self.window.default_width().max(1) as f64,
                    self.window.default_height().max(1) as f64,
                ),
            )
        }

        fn provider_snapshot(&self) -> Option<ProviderSnapshot> {
            self.provider
                .borrow()
                .as_ref()
                .map(|provider| provider.snapshot(&GtkSnapshotContext::new(self)))
        }

        fn registered_widget_for_handle(
            &self,
            handle: glasscheck_core::NodeHandle,
            registered_indices: &[usize],
        ) -> Option<Widget> {
            let registry_index = *registered_indices.get(handle.index())?;
            self.registry
                .borrow()
                .get(registry_index)
                .map(|entry| entry.widget.clone())
        }

        fn resolve_click_target(
            &self,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Result<ResolvedClickTarget, RegionResolveError> {
            if self.detached_root_widget {
                return Err(RegionResolveError::DetachedRootView);
            }

            let (scene, registered_indices) = self.snapshot_scene_with_registered_indices();
            let handle = scene.find(predicate).map_err(map_query_error)?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            let root_widget = self
                .root_widget()
                .ok_or(RegionResolveError::InputUnavailable)?;
            let registered_widget = self.registered_widget_for_handle(handle, &registered_indices);
            let point = resolve_hit_point_for_node(
                &scene,
                handle,
                &root_widget,
                node,
                registered_widget.as_ref(),
                search,
            )
            .ok_or(RegionResolveError::InputUnavailable)?;

            Ok(ResolvedClickTarget {
                root_widget,
                registered_widget,
                point,
            })
        }
    }

    impl Drop for GtkWindowHost {
        fn drop(&mut self) {
            if self.owns_window {
                self.window.close();
            }
        }
    }

    pub(crate) fn capture_widget_image(widget: &Widget) -> Option<glasscheck_core::Image> {
        let width = widget.allocated_width().max(1);
        let height = widget.allocated_height().max(1);
        let snapshot = gtk4::Snapshot::new();
        if let Some(parent) = widget.parent() {
            parent.snapshot_child(widget, &snapshot);
        } else {
            let paintable = gtk4::WidgetPaintable::new(Some(widget));
            paintable.snapshot(&snapshot, width as f64, height as f64);
        }
        let node = snapshot.to_node()?;
        let native = widget.native()?;
        let renderer = native.renderer()?;
        let texture = renderer.render_texture(
            &node,
            Some(&graphene::Rect::new(0.0, 0.0, width as f32, height as f32)),
        );
        let mut data = vec![0; width as usize * height as usize * 4];
        texture.download(&mut data, width as usize * 4);
        Some(glasscheck_core::Image::new(
            width as u32,
            height as u32,
            data,
        ))
    }

    fn managed_window_for_root(widget: &impl IsA<Widget>) -> Window {
        let window = Window::builder()
            .default_width(widget.as_ref().width_request().max(1))
            .default_height(widget.as_ref().height_request().max(1))
            .build();
        window.set_child(Some(widget));
        present_window_offscreen(&window);
        window
    }

    fn rect_of_widget(widget: &Widget, root_widget: Option<&Widget>) -> Option<Rect> {
        rect_in_root(
            widget,
            root_widget,
            0.0,
            0.0,
            widget.allocated_width() as f64,
            widget.allocated_height() as f64,
        )
    }

    fn rect_in_root(
        widget: &Widget,
        root_widget: Option<&Widget>,
        local_x: f64,
        local_y: f64,
        width: f64,
        height: f64,
    ) -> Option<Rect> {
        let root_widget = root_widget?;
        let bounds = widget.compute_bounds(root_widget)?;
        let top_left_x = bounds.x() as f64 + local_x;
        let top_left_y = bounds.y() as f64 + local_y;
        let root_height = root_widget.allocated_height().max(1) as f64;
        Some(Rect::new(
            Point::new(top_left_x, root_height - top_left_y - height),
            Size::new(width.max(1.0), height.max(1.0)),
        ))
    }

    fn visible_rect_of_widget(widget: &Widget, root_widget: Option<&Widget>) -> Option<Rect> {
        let mut visible_rect = rect_of_widget(widget, root_widget)?;
        let root_widget = root_widget?;
        let mut current = Some(widget.clone());
        while let Some(candidate) = current {
            let parent = candidate.parent();
            let clip_source = parent.as_ref().unwrap_or(root_widget);
            let clip_rect = rect_of_widget(clip_source, Some(root_widget))?;
            visible_rect = intersect_rects(visible_rect, clip_rect)?;
            if clip_source == root_widget {
                break;
            }
            current = parent;
        }
        Some(visible_rect)
    }

    fn resolve_hit_point_for_node(
        scene: &Scene,
        handle: glasscheck_core::NodeHandle,
        root_widget: &Widget,
        node: &SemanticNode,
        registered_widget: Option<&Widget>,
        search: &HitPointSearch,
    ) -> Option<Point> {
        let mut candidates = explicit_hit_point(node).into_iter().collect::<Vec<_>>();
        if let Some(hit_bounds) = hit_test_bounds_for_node(node) {
            for point in hit_point_candidates(hit_bounds, search) {
                if !candidates.contains(&point) {
                    candidates.push(point);
                }
            }
        }
        candidates
            .into_iter()
            .find(|point| point_targets_node(scene, handle, root_widget, registered_widget, *point))
    }

    fn point_targets_node(
        scene: &Scene,
        handle: glasscheck_core::NodeHandle,
        root_widget: &Widget,
        registered_widget: Option<&Widget>,
        point: Point,
    ) -> bool {
        if let Some(widget) = registered_widget {
            return widget_accepts_point(root_widget, widget, point);
        }
        scene.topmost_at(point) == Some(handle)
    }

    fn widget_accepts_point(root_widget: &Widget, widget: &Widget, point: Point) -> bool {
        let (x, y) = root_top_left_point(root_widget, point);
        root_widget
            .pick(x, y, gtk4::PickFlags::DEFAULT)
            .is_some_and(|picked| picked == *widget || is_descendant_of_widget(&picked, widget))
    }

    fn dispatch_semantic_click(
        root_widget: &Widget,
        registered_widget: &Widget,
        point: Point,
    ) -> bool {
        let (x, y) = root_top_left_point(root_widget, point);
        let Some(picked) = root_widget.pick(x, y, gtk4::PickFlags::DEFAULT) else {
            return false;
        };
        if !is_descendant_of_widget(&picked, registered_widget) {
            return false;
        }
        let mut current = Some(picked.clone());
        while let Some(candidate) = current {
            let gestures_fired = dispatch_click_to_widget_gestures(root_widget, &candidate, x, y);
            let activated = candidate.activate();
            if gestures_fired || activated {
                return true;
            }
            if candidate == *registered_widget {
                break;
            }
            current = candidate.parent();
        }
        false
    }

    fn dispatch_click_to_widget_gestures(
        root_widget: &Widget,
        widget: &Widget,
        x: f64,
        y: f64,
    ) -> bool {
        let Some((local_x, local_y)) = root_widget.translate_coordinates(widget, x, y) else {
            return false;
        };
        let mut fired = false;
        for object in widget.observe_controllers().snapshot() {
            let Ok(controller) = object.downcast::<gtk4::GestureClick>() else {
                continue;
            };
            let button = controller.button();
            if button != 0 && button != 1 {
                continue;
            }
            controller.emit_by_name::<()>("pressed", &[&1i32, &local_x, &local_y]);
            controller.emit_by_name::<()>("released", &[&1i32, &local_x, &local_y]);
            fired = true;
        }
        fired
    }

    fn dispatch_semantic_context_click(
        root_widget: &Widget,
        registered_widget: &Widget,
        point: Point,
    ) -> bool {
        let (x, y) = root_top_left_point(root_widget, point);
        let Some(picked) = root_widget.pick(x, y, gtk4::PickFlags::DEFAULT) else {
            return false;
        };
        if !is_descendant_of_widget(&picked, registered_widget) {
            return false;
        }
        let mut current = Some(picked.clone());
        while let Some(candidate) = current {
            if dispatch_context_click_to_widget_gestures(root_widget, &candidate, x, y) {
                return true;
            }
            if candidate == *registered_widget {
                break;
            }
            current = candidate.parent();
        }
        false
    }

    fn dispatch_context_click_to_widget_gestures(
        root_widget: &Widget,
        widget: &Widget,
        x: f64,
        y: f64,
    ) -> bool {
        let Some((local_x, local_y)) = root_widget.translate_coordinates(widget, x, y) else {
            return false;
        };
        let mut fired = false;
        for object in widget.observe_controllers().snapshot() {
            let Ok(controller) = object.downcast::<gtk4::GestureClick>() else {
                continue;
            };
            let button = controller.button();
            if button != 0 && button != 3 {
                continue;
            }
            controller.emit_by_name::<()>("pressed", &[&1i32, &local_x, &local_y]);
            controller.emit_by_name::<()>("released", &[&1i32, &local_x, &local_y]);
            fired = true;
        }
        fired
    }

    fn is_descendant_of_widget(widget: &Widget, ancestor: &Widget) -> bool {
        let mut current = Some(widget.clone());
        while let Some(candidate) = current {
            if candidate == *ancestor {
                return true;
            }
            current = candidate.parent();
        }
        false
    }

    fn hit_point_candidates(rect: Rect, search: &HitPointSearch) -> Vec<Point> {
        match search.strategy {
            HitPointStrategy::VisibleCenterFirst => {
                let center = rect_center(rect);
                let mut points = vec![center];
                for point in hit_point_candidates(
                    rect,
                    &HitPointSearch {
                        strategy: HitPointStrategy::Grid,
                        sample_count: search.sample_count.max(9),
                    },
                ) {
                    if !points.contains(&point) {
                        points.push(point);
                    }
                }
                points
            }
            HitPointStrategy::CornersAndCenter => vec![
                Point::new(rect.origin.x, rect.origin.y),
                Point::new(rect.origin.x + rect.size.width, rect.origin.y),
                Point::new(rect.origin.x, rect.origin.y + rect.size.height),
                Point::new(
                    rect.origin.x + rect.size.width,
                    rect.origin.y + rect.size.height,
                ),
                rect_center(rect),
            ],
            HitPointStrategy::Grid => {
                let dimension = (search.sample_count.max(4) as f64).sqrt().ceil() as usize;
                let mut points = Vec::new();
                for row in 0..dimension {
                    for column in 0..dimension {
                        points.push(Point::new(
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

    fn rect_center(rect: Rect) -> Point {
        Point::new(
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        )
    }

    fn root_top_left_point(root_widget: &Widget, point: Point) -> (f64, f64) {
        let height = root_widget.allocated_height().max(1) as f64;
        (point.x, height - point.y)
    }

    fn intersect_rects(left: Rect, right: Rect) -> Option<Rect> {
        let x1 = left.origin.x.max(right.origin.x);
        let y1 = left.origin.y.max(right.origin.y);
        let x2 = (left.origin.x + left.size.width).min(right.origin.x + right.size.width);
        let y2 = (left.origin.y + left.size.height).min(right.origin.y + right.size.height);
        (x2 > x1 && y2 > y1).then_some(Rect::new(Point::new(x1, y1), Size::new(x2 - x1, y2 - y1)))
    }

    fn infer_role(widget: &Widget) -> Role {
        if widget.is::<gtk4::TextView>() {
            Role::TextInput
        } else if widget.is::<gtk4::Button>() {
            Role::Button
        } else if widget.is::<gtk4::Label>() {
            Role::Label
        } else {
            Role::Container
        }
    }

    fn registered_ancestor_id(
        widget: &Widget,
        registered_ids: &HashMap<usize, String>,
    ) -> Option<String> {
        let mut current = widget.parent();
        while let Some(parent) = current {
            if let Some(id) = registered_ids.get(&widget_id(&parent)) {
                return Some(id.clone());
            }
            current = parent.parent();
        }
        None
    }

    fn sibling_index(widget: &Widget, parent: &Widget) -> usize {
        let mut current = parent.first_child();
        let mut index = 0usize;
        while let Some(candidate) = current {
            if candidate == *widget {
                return index;
            }
            current = candidate.next_sibling();
            index += 1;
        }
        0
    }

    fn widget_id(widget: &Widget) -> usize {
        widget.as_ptr() as usize
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

    fn hit_test_bounds_for_node(node: &SemanticNode) -> Option<Rect> {
        explicit_hit_rect(node).or_else(|| {
            let rect = node.visible_rect.unwrap_or(node.rect);
            (rect.size.width > 0.0 && rect.size.height > 0.0).then_some(rect)
        })
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
            ) if *width > 0 && *height > 0 => Some(Rect::new(
                Point::new(*x as f64, *y as f64),
                Size::new(*width as f64, *height as f64),
            )),
            _ => None,
        }
    }

    struct LegacyGtkSceneSource {
        provider: Box<dyn SemanticProvider>,
    }

    impl GtkSceneSource for LegacyGtkSceneSource {
        fn snapshot(&self, _context: &GtkSnapshotContext<'_>) -> SemanticSnapshot {
            self.provider.snapshot()
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub trait GtkSceneSource {}
    pub struct GtkSnapshotContext<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
    pub struct GtkWindowHost;
}

#[cfg(target_os = "linux")]
pub(crate) use imp::capture_widget_image;
pub use imp::{GtkSceneSource, GtkSnapshotContext, GtkWindowHost};
