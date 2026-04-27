#[cfg(target_os = "linux")]
mod imp {
    use std::collections::BTreeSet;
    use std::fmt;

    use glasscheck_core::{
        NodeProvenanceKind, Point, PropertyValue, QueryError, Rect, RegionResolveError, Role,
        Scene, Selector, SemanticNode, Size,
    };
    use glib::object::Cast;
    use gtk4::prelude::*;
    use gtk4::{Button, Label, Popover, Separator, Widget, Window};

    const CONTEXT_MENU_ROOT_ID: &str = "context-menu";
    const MENU_PATH_PROPERTY: &str = "glasscheck:menu_path";
    const WIDGET_PATH_PROPERTY: &str = "gtk:widget_path";
    const MENU_ITEM_HEIGHT: f64 = 24.0;
    const MENU_ITEM_WIDTH: f64 = 280.0;

    /// Retained GTK context menu opened by a semantic context-click call.
    ///
    /// The handle represents a visible GTK `Popover` menu. It snapshots menu
    /// items from the live widget tree and activates items only when they are
    /// backed by visible, sensitive `Button` widgets.
    #[derive(Clone, Debug)]
    pub struct GtkContextMenu {
        popover: Popover,
    }

    /// Errors returned while opening, inspecting, or activating a GTK context menu.
    #[derive(Debug)]
    pub enum GtkContextMenuError {
        /// The host could not resolve the requested node or point for input.
        Resolve(RegionResolveError),
        /// No visible GTK popover menu was available after the context click.
        NoContextMenu,
        /// The requested menu item selector failed.
        Query(QueryError),
        /// The selected semantic node does not describe an activatable menu item.
        InvalidMenuItem,
        /// The live GTK widget for the selected semantic node is unavailable.
        MissingWidget,
        /// The requested item is hidden.
        HiddenMenuItem,
        /// The requested item is disabled.
        DisabledMenuItem,
    }

    impl fmt::Display for GtkContextMenuError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Resolve(error) => {
                    write!(f, "failed to resolve context-click target: {error}")
                }
                Self::NoContextMenu => write!(f, "no GTK context menu was available"),
                Self::Query(error) => write!(f, "failed to resolve menu item selector: {error}"),
                Self::InvalidMenuItem => write!(f, "selected node is not an activatable menu item"),
                Self::MissingWidget => write!(f, "selected menu item widget is unavailable"),
                Self::HiddenMenuItem => write!(f, "selected menu item is hidden"),
                Self::DisabledMenuItem => write!(f, "selected menu item is disabled"),
            }
        }
    }

    impl std::error::Error for GtkContextMenuError {}

    impl From<RegionResolveError> for GtkContextMenuError {
        fn from(value: RegionResolveError) -> Self {
            Self::Resolve(value)
        }
    }

    impl GtkContextMenu {
        pub(crate) fn new(popover: Popover) -> Self {
            Self { popover }
        }

        /// Captures the visible popover menu item hierarchy as a semantic scene.
        #[must_use]
        pub fn snapshot_scene(&self) -> Scene {
            Scene::new(context_menu_nodes(&self.popover))
        }

        /// Activates the unique button-backed menu item matching `selector`.
        pub fn activate_item(&self, selector: &Selector) -> Result<(), GtkContextMenuError> {
            let scene = self.snapshot_scene();
            let handle = scene.find(selector).map_err(GtkContextMenuError::Query)?;
            let node = scene
                .node(handle)
                .ok_or(GtkContextMenuError::InvalidMenuItem)?;
            if node.role != Role::MenuItem {
                return Err(GtkContextMenuError::InvalidMenuItem);
            }
            if !node.visible
                || node
                    .state
                    .get("hidden")
                    .is_some_and(|value| value == &PropertyValue::Bool(true))
            {
                return Err(GtkContextMenuError::HiddenMenuItem);
            }
            if node
                .state
                .get("enabled")
                .is_some_and(|value| value == &PropertyValue::Bool(false))
            {
                return Err(GtkContextMenuError::DisabledMenuItem);
            }
            let path = node
                .properties
                .get(WIDGET_PATH_PROPERTY)
                .and_then(property_string)
                .ok_or(GtkContextMenuError::InvalidMenuItem)?;
            let widget =
                widget_at_path(&self.popover, path).ok_or(GtkContextMenuError::MissingWidget)?;
            let button = widget
                .downcast::<Button>()
                .map_err(|_| GtkContextMenuError::InvalidMenuItem)?;
            if !button.is_visible() {
                return Err(GtkContextMenuError::HiddenMenuItem);
            }
            if !button.is_sensitive() {
                return Err(GtkContextMenuError::DisabledMenuItem);
            }
            button.emit_clicked();
            Ok(())
        }

        /// Hides the retained GTK popover menu.
        pub fn dismiss(&self) {
            self.popover.popdown();
        }
    }

    pub(crate) fn context_click_root_point(
        window: &Window,
        point: Point,
    ) -> Result<GtkContextMenu, GtkContextMenuError> {
        let baseline = visible_context_menu_ids(window);
        context_click_root_point_excluding(window, point, &baseline)
    }

    pub(crate) fn context_click_root_point_excluding(
        window: &Window,
        point: Point,
        baseline: &BTreeSet<usize>,
    ) -> Result<GtkContextMenu, GtkContextMenuError> {
        crate::GtkInputDriver::new(window)
            .context_click(point)
            .map_err(|_| GtkContextMenuError::Resolve(RegionResolveError::InputUnavailable))?;
        flush_main_context(4);
        discover_context_menu_excluding(window, baseline).ok_or(GtkContextMenuError::NoContextMenu)
    }

    pub(crate) fn discover_context_menu_excluding(
        window: &Window,
        baseline: &BTreeSet<usize>,
    ) -> Option<GtkContextMenu> {
        window.child().and_then(|child| {
            visible_popover_in_excluding(&child, baseline).map(GtkContextMenu::new)
        })
    }

    pub(crate) fn visible_context_menu_ids(window: &Window) -> BTreeSet<usize> {
        let mut ids = BTreeSet::new();
        if let Some(child) = window.child() {
            collect_visible_popover_ids(&child, &mut ids);
        }
        ids
    }

    fn visible_popover_in_excluding(
        widget: &Widget,
        baseline: &BTreeSet<usize>,
    ) -> Option<Popover> {
        if let Ok(popover) = widget.clone().downcast::<Popover>() {
            if popover.is_visible() && !baseline.contains(&(popover.as_ptr() as usize)) {
                return Some(popover);
            }
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(popover) = visible_popover_in_excluding(&current, baseline) {
                return Some(popover);
            }
            child = current.next_sibling();
        }
        None
    }

    fn collect_visible_popover_ids(widget: &Widget, ids: &mut BTreeSet<usize>) {
        if let Ok(popover) = widget.clone().downcast::<Popover>() {
            if popover.is_visible() {
                ids.insert(popover.as_ptr() as usize);
            }
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            collect_visible_popover_ids(&current, ids);
            child = current.next_sibling();
        }
    }

    fn context_menu_nodes(popover: &Popover) -> Vec<SemanticNode> {
        let items = popover
            .child()
            .map(|child| collect_menu_items(&child))
            .unwrap_or_default();
        let mut nodes = vec![SemanticNode::new(
            CONTEXT_MENU_ROOT_ID,
            Role::Menu,
            Rect::new(
                Point::new(0.0, 0.0),
                Size::new(
                    MENU_ITEM_WIDTH,
                    (items.len().max(1) as f64) * MENU_ITEM_HEIGHT,
                ),
            ),
        )
        .with_selector(CONTEXT_MENU_ROOT_ID)
        .with_property("glasscheck:menu_native_surface", PropertyValue::Bool(true))
        .with_provenance(NodeProvenanceKind::Native)];

        for (index, item) in items.iter().enumerate() {
            let path = index.to_string();
            let mut node = SemanticNode::new(
                format!("{CONTEXT_MENU_ROOT_ID}/item:{path}"),
                item.role.clone(),
                Rect::new(
                    Point::new(0.0, index as f64 * MENU_ITEM_HEIGHT),
                    Size::new(MENU_ITEM_WIDTH, MENU_ITEM_HEIGHT),
                ),
            )
            .with_parent(CONTEXT_MENU_ROOT_ID, index)
            .with_selector(format!("{CONTEXT_MENU_ROOT_ID}.item[{path}]"))
            .with_property(MENU_PATH_PROPERTY, PropertyValue::String(path.clone()))
            .with_property(
                WIDGET_PATH_PROPERTY,
                PropertyValue::String(item.widget_path.clone()),
            )
            .with_state("enabled", PropertyValue::Bool(item.sensitive))
            .with_state("hidden", PropertyValue::Bool(!item.visible))
            .with_state("has_submenu", PropertyValue::Bool(false))
            .with_provenance(NodeProvenanceKind::Native);
            node.visible = item.visible;
            node.hit_testable = item.visible && item.sensitive && item.role == Role::MenuItem;
            if item.role == Role::Divider {
                node = node
                    .with_selector(format!("{CONTEXT_MENU_ROOT_ID}.separator[{path}]"))
                    .with_property("glasscheck:separator", PropertyValue::Bool(true));
            }
            if let Some(label) = item.label.as_ref() {
                node = node
                    .with_label(label.clone())
                    .with_selector(format!("{CONTEXT_MENU_ROOT_ID}.title:{label}"));
            }
            nodes.push(node);
        }

        nodes
    }

    fn collect_menu_items(root: &Widget) -> Vec<MenuItemWidget> {
        let mut items = Vec::new();
        let mut path = Vec::new();
        collect_menu_items_rec(root, &mut path, &mut items);
        items
    }

    fn collect_menu_items_rec(
        widget: &Widget,
        path: &mut Vec<usize>,
        items: &mut Vec<MenuItemWidget>,
    ) {
        if let Some(item) = menu_item_widget(widget, path) {
            items.push(item);
            return;
        }
        let mut child = widget.first_child();
        let mut index = 0usize;
        while let Some(current) = child {
            path.push(index);
            collect_menu_items_rec(&current, path, items);
            path.pop();
            index += 1;
            child = current.next_sibling();
        }
    }

    fn menu_item_widget(widget: &Widget, path: &[usize]) -> Option<MenuItemWidget> {
        let role = if widget.clone().downcast::<Separator>().is_ok() {
            Role::Divider
        } else if widget.clone().downcast::<Button>().is_ok() {
            Role::MenuItem
        } else {
            return None;
        };
        Some(MenuItemWidget {
            role,
            widget_path: widget_path(path),
            label: widget_label(widget),
            sensitive: widget.is_sensitive(),
            visible: widget.is_visible(),
        })
    }

    fn widget_label(widget: &Widget) -> Option<String> {
        if let Ok(button) = widget.clone().downcast::<Button>() {
            if let Some(label) = button.label() {
                return Some(label.to_string());
            }
            if let Some(child) = button.child() {
                return widget_label(&child);
            }
        }
        if let Ok(label) = widget.clone().downcast::<Label>() {
            return Some(label.text().to_string());
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(label) = widget_label(&current) {
                return Some(label);
            }
            child = current.next_sibling();
        }
        None
    }

    fn widget_at_path(popover: &Popover, path: &str) -> Option<Widget> {
        let mut widget = popover.child()?;
        if path.is_empty() {
            return Some(widget);
        }
        for segment in path.split('/') {
            let index = segment.parse::<usize>().ok()?;
            widget = nth_child(&widget, index)?;
        }
        Some(widget)
    }

    fn nth_child(widget: &Widget, index: usize) -> Option<Widget> {
        let mut child = widget.first_child();
        let mut current_index = 0usize;
        while let Some(current) = child {
            if current_index == index {
                return Some(current);
            }
            current_index += 1;
            child = current.next_sibling();
        }
        None
    }

    fn widget_path(path: &[usize]) -> String {
        path.iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/")
    }

    fn property_string(value: &PropertyValue) -> Option<&str> {
        match value {
            PropertyValue::String(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn flush_main_context(iterations: usize) {
        let context = glib::MainContext::default();
        for _ in 0..iterations.max(1) {
            while context.pending() {
                context.iteration(false);
            }
            context.iteration(false);
        }
    }

    struct MenuItemWidget {
        role: Role,
        widget_path: String,
        label: Option<String>,
        sensitive: bool,
        visible: bool,
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    #[derive(Clone, Debug)]
    pub struct GtkContextMenu;

    #[derive(Debug)]
    pub enum GtkContextMenuError {}
}

#[cfg(target_os = "linux")]
pub(crate) use imp::{
    context_click_root_point, context_click_root_point_excluding, discover_context_menu_excluding,
    flush_main_context, visible_context_menu_ids,
};
pub use imp::{GtkContextMenu, GtkContextMenuError};
