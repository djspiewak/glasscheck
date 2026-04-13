#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::collections::{BTreeSet, HashMap};

    use glasscheck_core::{
        crop_image_bottom_left, normalize_provider_nodes, registered_node_id, InstrumentedNode,
        NodePredicate, Point, QueryRoot, Rect, RegionResolveError, RegionSpec, Role, SceneSnapshot,
        SemanticNode, SemanticProvider, Size, TextRange,
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

    /// GTK4 window host used to build, capture, query, and drive a test scene.
    ///
    /// Use it to attach semantic metadata to live widgets, capture rendered
    /// pixels, and query the current widget tree through a backend-neutral
    /// scene model.
    pub struct GtkWindowHost {
        window: Option<Window>,
        root_widget: RefCell<Option<Widget>>,
        registry: RefCell<Vec<RegisteredWidget>>,
        provider: RefCell<Option<Box<dyn SemanticProvider>>>,
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
                window: Some(window),
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
                window: Some(window.clone()),
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
                .or_else(|| Some(managed_window_for_root(widget)));
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
            self.window
                .as_ref()
                .expect("window access requires an attached GTK window")
        }

        /// Sets the window child/root widget.
        pub fn set_root(&self, widget: &impl IsA<Widget>) {
            if let Some(window) = self.window.as_ref() {
                window.set_child(Some(widget));
                present_window_offscreen(window);
            }
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
            let rect = self.resolve_region(region)?;
            let image = self
                .capture()
                .ok_or(RegionResolveError::CaptureUnavailable)?;
            Ok(crop_image_bottom_left(&image, rect))
        }

        /// Returns an input driver scoped to this window.
        #[must_use]
        pub fn input(&self) -> GtkInputDriver<'_> {
            let window = self
                .window
                .as_ref()
                .expect("input requires an attached GTK window");
            GtkInputDriver::new(window)
        }

        /// Clicks the visual center of the unique node matching `predicate`.
        ///
        /// GTK input synthesis is still best-effort in some paths. Prefer
        /// semantic assertions and direct widget APIs when interaction fidelity
        /// matters more than exercising the click route itself.
        pub fn click_node(&self, predicate: &NodePredicate) -> Result<(), RegionResolveError> {
            if self.detached_root_widget {
                return Err(RegionResolveError::DetachedRootView);
            }
            let (scene, registered_indices) = self.snapshot_scene_with_registered_indices();
            let handle = scene.find(predicate).map_err(map_query_error)?;
            let node = scene
                .node(handle)
                .ok_or(RegionResolveError::InvalidHandle(handle))?;
            if let Some(widget) = self.registered_widget_for_handle(handle, &registered_indices) {
                if let Ok(button) = widget.clone().downcast::<gtk4::Button>() {
                    button.emit_clicked();
                    return Ok(());
                }
            }
            if self.root_widget().is_none() {
                return Err(RegionResolveError::InputUnavailable);
            }
            self.input().click(Point::new(
                node.rect.origin.x + node.rect.size.width / 2.0,
                node.rect.origin.y + node.rect.size.height / 2.0,
            ));
            Ok(())
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

        /// Builds a merged scene snapshot from registered native widgets and virtual nodes.
        #[must_use]
        pub fn snapshot_scene(&self) -> SceneSnapshot {
            self.snapshot_scene_with_registered_indices().0
        }

        fn snapshot_scene_with_registered_indices(&self) -> (SceneSnapshot, Vec<usize>) {
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
                    let mut node = SemanticNode::new(
                        id,
                        entry
                            .descriptor
                            .role
                            .clone()
                            .unwrap_or_else(|| infer_role(&entry.widget)),
                        rect,
                    );
                    node.label = entry.descriptor.label.clone();
                    node.visible = entry.widget.is_visible();
                    node.visible_rect = Some(node.rect);
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

            (SceneSnapshot::new(nodes), registered_indices)
        }

        /// Builds a query root from the current scene snapshot.
        #[must_use]
        pub fn query_root(&self) -> QueryRoot {
            QueryRoot::from_scene(self.snapshot_scene())
        }

        /// Resolves a semantic region against the current scene snapshot.
        pub fn resolve_region(&self, region: &RegionSpec) -> Result<Rect, RegionResolveError> {
            self.snapshot_scene()
                .resolve_region(self.root_bounds(), region)
        }

        /// Sets the window title when a window is attached.
        pub fn set_title(&self, title: &str) {
            if let Some(window) = self.window.as_ref() {
                window.set_title(Some(title));
            }
        }

        pub(crate) fn root_widget(&self) -> Option<Widget> {
            if self.tracks_window_child {
                if let Some(window) = self.window.as_ref() {
                    *self.root_widget.borrow_mut() = window.child();
                }
            }
            self.root_widget.borrow().clone()
        }

        pub(crate) fn root_bounds(&self) -> Rect {
            if let Some(root) = self.root_widget() {
                let width = root.allocated_width().max(1) as f64;
                let height = root.allocated_height().max(1) as f64;
                return Rect::new(Point::new(0.0, 0.0), Size::new(width, height));
            }
            let window = self
                .window
                .as_ref()
                .expect("host should have either a root widget or a window");
            Rect::new(
                Point::new(0.0, 0.0),
                Size::new(
                    window.default_width().max(1) as f64,
                    window.default_height().max(1) as f64,
                ),
            )
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
    }

    impl Drop for GtkWindowHost {
        fn drop(&mut self) {
            if self.owns_window {
                if let Some(window) = self.window.take() {
                    window.close();
                }
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
            glasscheck_core::QueryError::NotFoundPredicate(predicate) => {
                RegionResolveError::NotFound(predicate)
            }
            glasscheck_core::QueryError::MultiplePredicateMatches { predicate, count } => {
                RegionResolveError::MultipleMatches { predicate, count }
            }
            glasscheck_core::QueryError::NotFound(selector) => {
                RegionResolveError::NotFound(NodePredicate::id_eq(selector.id.unwrap_or_default()))
            }
            glasscheck_core::QueryError::MultipleMatches { selector, count } => {
                RegionResolveError::MultipleMatches {
                    predicate: NodePredicate::id_eq(selector.id.unwrap_or_default()),
                    count,
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub struct GtkWindowHost;
}

#[cfg(target_os = "linux")]
pub(crate) use imp::capture_widget_image;
pub use imp::GtkWindowHost;
