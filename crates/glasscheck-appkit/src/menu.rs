#[cfg(target_os = "macos")]
mod imp {
    use std::collections::BTreeMap;
    use std::ffi::c_uchar;
    use std::fmt;
    use std::ptr;

    use glasscheck_core::{
        Image, NodeProvenanceKind, Point, PropertyValue, Rect, Role, Scene, Selector, SemanticNode,
        Size,
    };
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{AnyThread, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSBezierPath, NSBitmapImageRep, NSColor, NSControlStateValueMixed,
        NSControlStateValueOn, NSEventModifierFlags, NSGraphicsContext, NSMenu, NSMenuItem,
        NSMenuItemCell, NSView,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

    const MENU_BAR_ID: &str = "menu:bar";
    const MENU_BAR_SELECTOR: &str = "menu.bar";
    const DIRECT_ROW_HEIGHT: f64 = 24.0;
    const SEPARATOR_ROW_HEIGHT: f64 = 9.0;
    const HORIZONTAL_PADDING: f64 = 28.0;
    const MIN_MENU_WIDTH: f64 = 180.0;
    const MENU_MARGIN: f64 = 6.0;

    /// Main-thread driver for the process-wide AppKit menu bar.
    #[derive(Clone, Copy)]
    pub struct AppKitMenuBar {
        mtm: MainThreadMarker,
    }

    /// A top-level AppKit menu selected from [`AppKitMenuBar`].
    pub struct AppKitOpenedMenu {
        mtm: MainThreadMarker,
        menu: Retained<NSMenu>,
        title: String,
        top_level_index: usize,
    }

    /// Visual and semantic capture of an opened AppKit menu.
    #[derive(Clone, Debug, PartialEq)]
    pub struct AppKitMenuCapture {
        /// Native-rendered menu pixels in RGBA8 order.
        pub image: Image,
        /// Semantic scene whose node rectangles match `image` coordinates.
        pub scene: Scene,
    }

    /// Top-level menu lookup target.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum AppKitMenuTarget {
        /// Match a top-level menu by visible title.
        Title(String),
        /// Match a top-level menu by `NSApplication.mainMenu` item index.
        Index(usize),
        /// Match exactly one top-level menu node in the menu-bar snapshot.
        Selector(Selector),
    }

    /// Options for offscreen menu rendering.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct AppKitMenuCaptureOptions {
        /// Optional item to render in the highlighted state.
        pub highlighted_item: Option<Selector>,
        /// Opts into a future visible fallback if offscreen rendering is unavailable.
        ///
        /// The default is `false`, and the current renderer never opens a native popup.
        pub allow_visible_fallback: bool,
    }

    /// Errors returned by AppKit menu-bar testing helpers.
    #[derive(Clone, Debug, PartialEq)]
    pub enum AppKitMenuError {
        /// `NSApplication.mainMenu` is unset.
        MissingMainMenu,
        /// The requested top-level menu was not found.
        MenuNotFound(AppKitMenuTarget),
        /// The requested top-level menu selector matched more than one menu.
        AmbiguousMenuTarget {
            target: AppKitMenuTarget,
            count: usize,
        },
        /// The requested top-level target did not resolve to a top-level menu.
        InvalidMenuTarget(AppKitMenuTarget),
        /// The top-level menu item had no submenu to inspect or render.
        MissingSubmenu(String),
        /// The requested menu item was not found.
        ItemNotFound(Selector),
        /// The requested menu item selector matched more than one item.
        AmbiguousItemTarget { selector: Selector, count: usize },
        /// The requested item target did not resolve to an activatable item.
        InvalidItemTarget(Selector),
        /// The requested item is disabled.
        ItemDisabled(String),
        /// The requested item is hidden.
        ItemHidden(String),
        /// The requested item is a separator.
        SeparatorActivation(String),
        /// The requested item is not an actionable command.
        NonActionableItem(String),
        /// AppKit could not resolve a receiver for the requested action.
        ActionTargetNotFound(String),
        /// AppKit declined to dispatch the requested action.
        ActionDispatchFailed(String),
        /// The requested highlight target is not a directly rendered visible row.
        HighlightTargetNotRendered(Selector),
        /// The requested activation target is not a directly rendered visible row.
        ActivationTargetNotRendered(Selector),
        /// Native offscreen rendering is unsupported in the current environment.
        OffscreenRenderingUnavailable,
        /// Offscreen rendering was unavailable and visible fallback was not enabled.
        VisibleFallbackDisabled,
        /// AppKit did not provide a usable bitmap capture.
        CaptureUnavailable,
    }

    impl fmt::Display for AppKitMenuError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::MissingMainMenu => write!(f, "NSApplication has no main menu"),
                Self::MenuNotFound(_) => write!(f, "menu target was not found"),
                Self::AmbiguousMenuTarget { count, .. } => {
                    write!(f, "menu target matched {count} menus")
                }
                Self::InvalidMenuTarget(_) => write!(f, "menu target did not resolve to a menu"),
                Self::MissingSubmenu(title) => write!(f, "menu item '{title}' has no submenu"),
                Self::ItemNotFound(_) => write!(f, "menu item target was not found"),
                Self::AmbiguousItemTarget { count, .. } => {
                    write!(f, "menu item target matched {count} items")
                }
                Self::InvalidItemTarget(_) => {
                    write!(f, "menu item target did not resolve to an actionable item")
                }
                Self::ItemDisabled(title) => write!(f, "menu item '{title}' is disabled"),
                Self::ItemHidden(title) => write!(f, "menu item '{title}' is hidden"),
                Self::SeparatorActivation(id) => write!(f, "menu separator '{id}' cannot activate"),
                Self::NonActionableItem(title) => {
                    write!(f, "menu item '{title}' is not an actionable command")
                }
                Self::ActionTargetNotFound(title) => {
                    write!(f, "menu item '{title}' action had no AppKit target")
                }
                Self::ActionDispatchFailed(title) => {
                    write!(f, "menu item '{title}' action was not dispatched by AppKit")
                }
                Self::HighlightTargetNotRendered(_) => {
                    write!(f, "highlight target is not a rendered menu row")
                }
                Self::ActivationTargetNotRendered(_) => {
                    write!(f, "activation target is not a rendered menu row")
                }
                Self::OffscreenRenderingUnavailable => {
                    write!(f, "offscreen menu rendering is unavailable")
                }
                Self::VisibleFallbackDisabled => {
                    write!(f, "visible menu fallback is disabled")
                }
                Self::CaptureUnavailable => write!(f, "menu capture is unavailable"),
            }
        }
    }

    impl std::error::Error for AppKitMenuError {}

    impl AppKitMenuBar {
        #[must_use]
        pub(crate) fn new(mtm: MainThreadMarker) -> Self {
            Self { mtm }
        }

        /// Returns a semantic snapshot of `NSApplication.mainMenu`.
        pub fn snapshot(&self) -> Result<Scene, AppKitMenuError> {
            Ok(build_menu_bar_snapshot(self.mtm)?.scene)
        }

        /// Resolves and opens a top-level main-menu entry.
        pub fn open(&self, target: AppKitMenuTarget) -> Result<AppKitOpenedMenu, AppKitMenuError> {
            let main_menu = main_menu(self.mtm)?;
            let index = resolve_top_level_menu_index(&target, self.mtm)?;
            let item = main_menu
                .itemAtIndex(index as isize)
                .ok_or_else(|| AppKitMenuError::MenuNotFound(target.clone()))?;
            let title = item.title().to_string();
            validate_top_level_item_state(&item, &title)?;
            let submenu = item
                .submenu()
                .ok_or_else(|| AppKitMenuError::MissingSubmenu(title.clone()))?;
            Ok(AppKitOpenedMenu {
                mtm: self.mtm,
                menu: submenu,
                title,
                top_level_index: index,
            })
        }
    }

    impl AppKitOpenedMenu {
        /// Returns a semantic snapshot of the opened menu subtree.
        pub fn snapshot(&self) -> Result<Scene, AppKitMenuError> {
            Ok(self.snapshot_with_highlight(None)?.scene)
        }

        /// Captures the opened menu using non-visible offscreen rendering by default.
        pub fn capture(
            &self,
            options: &AppKitMenuCaptureOptions,
        ) -> Result<AppKitMenuCapture, AppKitMenuError> {
            let highlighted_id = options
                .highlighted_item
                .as_ref()
                .map(|selector| self.resolve_rendered_row_id(selector))
                .transpose()?;
            let snapshot = self.snapshot_with_highlight(highlighted_id.as_deref())?;
            let layout = snapshot
                .layout
                .as_ref()
                .ok_or(AppKitMenuError::OffscreenRenderingUnavailable)?;
            match render_menu(self.mtm, layout, highlighted_id.as_deref()) {
                Ok(image) => Ok(AppKitMenuCapture {
                    image,
                    scene: snapshot.scene,
                }),
                Err(AppKitMenuError::OffscreenRenderingUnavailable)
                    if options.allow_visible_fallback =>
                {
                    Err(AppKitMenuError::OffscreenRenderingUnavailable)
                }
                Err(AppKitMenuError::OffscreenRenderingUnavailable) => {
                    Err(AppKitMenuError::VisibleFallbackDisabled)
                }
                Err(error) => Err(error),
            }
        }

        /// Activates the unique visible, enabled menu item matching `selector`.
        pub fn activate(&self, selector: &Selector) -> Result<(), AppKitMenuError> {
            let snapshot = self.snapshot_with_highlight(None)?;
            let handle = match snapshot.scene.find_all(selector).as_slice() {
                [] => return Err(AppKitMenuError::ItemNotFound(selector.clone())),
                [handle] => *handle,
                matches => {
                    return Err(AppKitMenuError::AmbiguousItemTarget {
                        selector: selector.clone(),
                        count: matches.len(),
                    });
                }
            };
            let node = snapshot
                .scene
                .node(handle)
                .ok_or_else(|| AppKitMenuError::InvalidItemTarget(selector.clone()))?;
            let item_ref = snapshot
                .items
                .iter()
                .find(|item| item.id == node.id)
                .ok_or_else(|| AppKitMenuError::InvalidItemTarget(selector.clone()))?;
            if item_ref.separator {
                return Err(AppKitMenuError::SeparatorActivation(item_ref.id.clone()));
            }
            if item_ref.hidden {
                return Err(AppKitMenuError::ItemHidden(item_ref.title.clone()));
            }
            if !item_ref.enabled {
                return Err(AppKitMenuError::ItemDisabled(item_ref.title.clone()));
            }
            if !item_ref.rendered {
                return Err(AppKitMenuError::ActivationTargetNotRendered(
                    selector.clone(),
                ));
            }
            if item_ref.has_submenu || !item_ref.actionable {
                return Err(AppKitMenuError::NonActionableItem(item_ref.title.clone()));
            }
            let action = item_ref
                .action
                .ok_or_else(|| AppKitMenuError::NonActionableItem(item_ref.title.clone()))?;
            let app = NSApplication::sharedApplication(self.mtm);
            let sender = Some(item_ref.item.as_ref());
            let target = item_ref.target.as_deref();
            if unsafe { app.targetForAction_to_from(action, target, sender) }.is_none() {
                return Err(AppKitMenuError::ActionTargetNotFound(
                    item_ref.title.clone(),
                ));
            }
            item_ref
                .owning_menu
                .performActionForItemAtIndex(item_ref.index as isize);
            Ok(())
        }

        fn resolve_rendered_row_id(&self, selector: &Selector) -> Result<String, AppKitMenuError> {
            let snapshot = self.snapshot_with_highlight(None)?;
            let id = match snapshot.scene.find_all(selector).as_slice() {
                [] => Err(AppKitMenuError::ItemNotFound(selector.clone())),
                [handle] => snapshot
                    .scene
                    .node(*handle)
                    .map(|node| node.id.clone())
                    .ok_or_else(|| AppKitMenuError::InvalidItemTarget(selector.clone())),
                matches => Err(AppKitMenuError::AmbiguousItemTarget {
                    selector: selector.clone(),
                    count: matches.len(),
                }),
            }?;
            let layout = snapshot
                .layout
                .as_ref()
                .ok_or(AppKitMenuError::OffscreenRenderingUnavailable)?;
            if layout.rows.iter().any(|row| row.id == id) {
                Ok(id)
            } else {
                Err(AppKitMenuError::HighlightTargetNotRendered(
                    selector.clone(),
                ))
            }
        }

        fn snapshot_with_highlight(
            &self,
            highlighted_id: Option<&str>,
        ) -> Result<MenuSnapshot, AppKitMenuError> {
            let layout = MenuLayout::for_opened_menu(self.mtm, self.top_level_index, &self.menu);
            Ok(build_opened_menu_snapshot(
                self.top_level_index,
                &self.title,
                &self.menu,
                Some(layout),
                highlighted_id,
            ))
        }
    }

    struct MenuSnapshot {
        scene: Scene,
        items: Vec<MenuItemRef>,
        layout: Option<MenuLayout>,
    }

    struct MenuItemRef {
        id: String,
        title: String,
        owning_menu: Retained<NSMenu>,
        index: usize,
        item: Retained<NSMenuItem>,
        separator: bool,
        enabled: bool,
        hidden: bool,
        has_submenu: bool,
        actionable: bool,
        action: Option<Sel>,
        target: Option<Retained<AnyObject>>,
        rendered: bool,
    }

    struct MenuLayout {
        width: f64,
        height: f64,
        rows: Vec<MenuRow>,
    }

    struct MenuRow {
        id: String,
        item: Retained<NSMenuItem>,
        rect: Rect,
        separator: bool,
    }

    impl MenuLayout {
        fn for_opened_menu(mtm: MainThreadMarker, top_level_index: usize, menu: &NSMenu) -> Self {
            menu.update();
            let mut measured = Vec::new();
            let mut width = MIN_MENU_WIDTH;
            let count = menu.numberOfItems().max(0) as usize;
            for index in 0..count {
                let Some(item) = menu.itemAtIndex(index as isize) else {
                    continue;
                };
                if item.isHiddenOrHasHiddenAncestor() {
                    continue;
                }
                let separator = item.isSeparatorItem();
                let id = menu_item_id(top_level_index, &[], index);
                let row_height = if separator {
                    SEPARATOR_ROW_HEIGHT
                } else {
                    measured_cell_size(mtm, &item).height.max(DIRECT_ROW_HEIGHT)
                };
                let title_width = estimated_title_width(&item.title().to_string());
                let key_width = estimated_title_width(&item.keyEquivalent().to_string());
                width = width.max(title_width + key_width + HORIZONTAL_PADDING * 3.0);
                measured.push((id, item, row_height, separator));
            }

            let row_heights = measured
                .iter()
                .map(|(_, _, row_height, _)| *row_height)
                .collect::<Vec<_>>();
            let (height, row_rects) = row_rects_from_heights(width, &row_heights);
            let rows = measured
                .into_iter()
                .zip(row_rects)
                .map(|((id, item, _, separator), rect)| MenuRow {
                    id,
                    item,
                    rect,
                    separator,
                })
                .collect();

            Self {
                width: width.ceil(),
                height: height.ceil(),
                rows,
            }
        }
    }

    fn row_rects_from_heights(width: f64, row_heights: &[f64]) -> (f64, Vec<Rect>) {
        let content_height: f64 = row_heights.iter().sum();
        let height = (content_height + MENU_MARGIN * 2.0).max(DIRECT_ROW_HEIGHT);
        let mut y = height - MENU_MARGIN;
        let rects = row_heights
            .iter()
            .map(|row_height| {
                y -= row_height;
                Rect::new(
                    Point::new(MENU_MARGIN, y),
                    Size::new(width - MENU_MARGIN * 2.0, *row_height),
                )
            })
            .collect();
        (height, rects)
    }

    fn image_rect_from_appkit_rect(capture_height: f64, rect: Rect) -> Rect {
        Rect::new(
            Point::new(
                rect.origin.x,
                capture_height - rect.origin.y - rect.size.height,
            ),
            rect.size,
        )
    }

    fn main_menu(mtm: MainThreadMarker) -> Result<Retained<NSMenu>, AppKitMenuError> {
        NSApplication::sharedApplication(mtm)
            .mainMenu()
            .ok_or(AppKitMenuError::MissingMainMenu)
    }

    fn resolve_top_level_menu_index(
        target: &AppKitMenuTarget,
        mtm: MainThreadMarker,
    ) -> Result<usize, AppKitMenuError> {
        let main_menu = main_menu(mtm)?;
        main_menu.update();
        match target {
            AppKitMenuTarget::Index(index) => {
                if *index < main_menu.numberOfItems().max(0) as usize {
                    Ok(*index)
                } else {
                    Err(AppKitMenuError::MenuNotFound(target.clone()))
                }
            }
            AppKitMenuTarget::Title(title) => (0..main_menu.numberOfItems().max(0) as usize)
                .find(|index| {
                    main_menu
                        .itemAtIndex(*index as isize)
                        .is_some_and(|item| item.title().to_string() == *title)
                })
                .ok_or_else(|| AppKitMenuError::MenuNotFound(target.clone())),
            AppKitMenuTarget::Selector(selector) => {
                let scene = build_menu_bar_snapshot(mtm)?.scene;
                match scene.find_all(selector).as_slice() {
                    [] => Err(AppKitMenuError::MenuNotFound(target.clone())),
                    [handle] => {
                        let node = scene
                            .node(*handle)
                            .ok_or_else(|| AppKitMenuError::InvalidMenuTarget(target.clone()))?;
                        if node.role != Role::Menu {
                            return Err(AppKitMenuError::InvalidMenuTarget(target.clone()));
                        }
                        match node.properties.get("top_level_index") {
                            Some(PropertyValue::Integer(index)) => Ok(*index as usize),
                            _ => Err(AppKitMenuError::InvalidMenuTarget(target.clone())),
                        }
                    }
                    matches => Err(AppKitMenuError::AmbiguousMenuTarget {
                        target: target.clone(),
                        count: matches.len(),
                    }),
                }
            }
        }
    }

    fn build_menu_bar_snapshot(mtm: MainThreadMarker) -> Result<MenuSnapshot, AppKitMenuError> {
        let main_menu = main_menu(mtm)?;
        main_menu.update();
        let count = main_menu.numberOfItems().max(0) as usize;
        let mut root = SemanticNode::new(
            MENU_BAR_ID,
            Role::MenuBar,
            Rect::new(Point::new(0.0, 0.0), Size::new(count as f64, 1.0)),
        )
        .with_selector(MENU_BAR_SELECTOR)
        .with_provenance(NodeProvenanceKind::Native);
        root.visible = true;
        root.visible_rect = Some(root.rect);
        root.hit_testable = true;
        let mut nodes = vec![root];
        let mut items = Vec::new();

        for index in 0..count {
            let Some(item) = main_menu.itemAtIndex(index as isize) else {
                continue;
            };
            let title = item.title().to_string();
            let id = top_menu_id(index);
            let mut node = SemanticNode::new(
                id.clone(),
                Role::Menu,
                Rect::new(Point::new(index as f64, 0.0), Size::new(1.0, 1.0)),
            )
            .with_label(title.clone())
            .with_parent(MENU_BAR_ID, index)
            .with_selector(format!("menu.index.{index}"))
            .with_selector(format!("menu.title.{}", slug(&title)))
            .with_property("top_level_index", PropertyValue::Integer(index as i64))
            .with_provenance(NodeProvenanceKind::Native);
            apply_item_state(&mut node, &item, None);
            apply_top_level_item_semantics(&mut node, &item);
            nodes.push(node);
            if let Some(submenu) = item.submenu() {
                append_menu_items(
                    index,
                    &submenu,
                    &id,
                    &[title],
                    None,
                    None,
                    &mut nodes,
                    &mut items,
                );
            }
        }

        Ok(MenuSnapshot {
            scene: Scene::new(nodes),
            items,
            layout: None,
        })
    }

    fn build_opened_menu_snapshot(
        top_level_index: usize,
        title: &str,
        menu: &NSMenu,
        layout: Option<MenuLayout>,
        highlighted_id: Option<&str>,
    ) -> MenuSnapshot {
        menu.update();
        let size = layout
            .as_ref()
            .map(|layout| Size::new(layout.width, layout.height))
            .unwrap_or_else(|| Size::new(0.0, 0.0));
        let id = top_menu_id(top_level_index);
        let mut nodes = vec![SemanticNode::new(
            id.clone(),
            Role::Menu,
            Rect::new(Point::new(0.0, 0.0), size),
        )
        .with_label(title.to_string())
        .with_selector(format!("menu.index.{top_level_index}"))
        .with_selector(format!("menu.title.{}", slug(title)))
        .with_property(
            "top_level_index",
            PropertyValue::Integer(top_level_index as i64),
        )
        .with_provenance(NodeProvenanceKind::Native)];
        let mut items = Vec::new();
        let row_rects = layout
            .as_ref()
            .map(|layout| {
                layout
                    .rows
                    .iter()
                    .map(|row| {
                        (
                            row.id.clone(),
                            image_rect_from_appkit_rect(layout.height, row.rect),
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        append_menu_items(
            top_level_index,
            menu,
            &id,
            &[title.to_string()],
            Some(&row_rects),
            highlighted_id,
            &mut nodes,
            &mut items,
        );

        MenuSnapshot {
            scene: Scene::new(nodes),
            items,
            layout,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn append_menu_items(
        top_level_index: usize,
        menu: &NSMenu,
        parent_id: &str,
        path: &[String],
        row_rects: Option<&BTreeMap<String, Rect>>,
        highlighted_id: Option<&str>,
        nodes: &mut Vec<SemanticNode>,
        items: &mut Vec<MenuItemRef>,
    ) {
        menu.update();
        let count = menu.numberOfItems().max(0) as usize;
        for index in 0..count {
            let Some(item) = menu.itemAtIndex(index as isize) else {
                continue;
            };
            let id = menu_item_id(top_level_index, path, index);
            let title = menu_item_title(&item, index);
            let separator = item.isSeparatorItem();
            let hidden = item.isHiddenOrHasHiddenAncestor();
            let enabled = item.isEnabled();
            let has_submenu = item.hasSubmenu();
            let action = item.action();
            let target = item.target();
            let actionable = action.is_some();
            let rendered_rect = row_rects.and_then(|rects| rects.get(&id).copied());
            let rendered = rendered_rect.is_some();
            let rect = rendered_rect
                .unwrap_or_else(|| Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)));
            let role = if separator {
                Role::Divider
            } else {
                Role::MenuItem
            };
            let mut node = SemanticNode::new(id.clone(), role, rect)
                .with_parent(parent_id, index)
                .with_selector(format!("menu.item.index.{top_level_index}.{index}"))
                .with_selector(format!("menu.item.title.{}", slug(&title)))
                .with_selector(menu_item_path_selector(path, &title))
                .with_property("has_submenu", PropertyValue::Bool(item.hasSubmenu()))
                .with_property("tag", PropertyValue::Integer(item.tag() as i64))
                .with_provenance(NodeProvenanceKind::Native);
            if !separator {
                node.label = Some(title.clone());
            }
            if let Some(action) = item.action() {
                node = node.with_property(
                    "action",
                    PropertyValue::string(action.name().to_string_lossy()),
                );
            }
            apply_item_state(&mut node, &item, highlighted_id);
            if hidden || !rendered {
                node.visible = false;
                node.visible_rect = None;
            } else if rect.size.width > 0.0 && rect.size.height > 0.0 {
                node.visible_rect = Some(rect);
            }
            node.hit_testable = enabled && !hidden && !separator && rendered;
            nodes.push(node);

            items.push(MenuItemRef {
                id: id.clone(),
                title: title.clone(),
                owning_menu: unsafe {
                    Retained::retain(menu as *const NSMenu as *mut NSMenu)
                        .expect("snapshot menu should remain retainable")
                },
                index,
                item: item.clone(),
                separator,
                enabled,
                hidden,
                has_submenu,
                actionable,
                action,
                target,
                rendered,
            });

            if let Some(submenu) = item.submenu() {
                let submenu_id = format!("{id}/submenu");
                let submenu_path = path_with_title(path, &title);
                let mut submenu_node = SemanticNode::new(
                    submenu_id.clone(),
                    Role::Menu,
                    Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)),
                );
                submenu_node = submenu_node
                    .with_label(title.clone())
                    .with_parent(id, 0)
                    .with_selector(format!("menu.title.{}", slug(&title)))
                    .with_provenance(NodeProvenanceKind::Native);
                submenu_node.visible = false;
                submenu_node.visible_rect = None;
                submenu_node.hit_testable = false;
                nodes.push(submenu_node);
                append_menu_items(
                    top_level_index,
                    &submenu,
                    &submenu_id,
                    &submenu_path,
                    None,
                    highlighted_id,
                    nodes,
                    items,
                );
            }
        }
    }

    fn apply_top_level_item_semantics(node: &mut SemanticNode, item: &NSMenuItem) {
        let hidden = item.isHiddenOrHasHiddenAncestor();
        let enabled = item.isEnabled();
        if hidden {
            node.visible = false;
            node.visible_rect = None;
        } else {
            node.visible = true;
            node.visible_rect = Some(node.rect);
        }
        node.hit_testable = enabled && !hidden;
    }

    fn validate_top_level_item_state(
        item: &NSMenuItem,
        title: &str,
    ) -> Result<(), AppKitMenuError> {
        if item.isHiddenOrHasHiddenAncestor() {
            return Err(AppKitMenuError::ItemHidden(title.to_string()));
        }
        if !item.isEnabled() {
            return Err(AppKitMenuError::ItemDisabled(title.to_string()));
        }
        Ok(())
    }

    fn apply_item_state(node: &mut SemanticNode, item: &NSMenuItem, highlighted_id: Option<&str>) {
        let state = item.state();
        let checked = state == NSControlStateValueOn;
        let mixed = state == NSControlStateValueMixed;
        let highlighted = highlighted_id == Some(node.id.as_str());
        node.state
            .insert("enabled".into(), PropertyValue::Bool(item.isEnabled()));
        node.state.insert(
            "hidden".into(),
            PropertyValue::Bool(item.isHiddenOrHasHiddenAncestor()),
        );
        node.state
            .insert("checked".into(), PropertyValue::Bool(checked));
        node.state
            .insert("mixed".into(), PropertyValue::Bool(mixed));
        node.state
            .insert("highlighted".into(), PropertyValue::Bool(highlighted));
        for key in ["enabled", "hidden", "checked", "mixed", "highlighted"] {
            node.state_provenance
                .insert(key.into(), NodeProvenanceKind::Native);
        }
        node.properties.insert(
            "key_equivalent".into(),
            PropertyValue::string(item.keyEquivalent().to_string()),
        );
        node.properties.insert(
            "key_modifiers".into(),
            PropertyValue::string(format_modifier_flags(item.keyEquivalentModifierMask())),
        );
        node.property_provenance
            .insert("key_equivalent".into(), NodeProvenanceKind::Native);
        node.property_provenance
            .insert("key_modifiers".into(), NodeProvenanceKind::Native);
    }

    fn render_menu(
        mtm: MainThreadMarker,
        layout: &MenuLayout,
        highlighted_id: Option<&str>,
    ) -> Result<Image, AppKitMenuError> {
        let width = layout.width.ceil().max(1.0) as u32;
        let height = layout.height.ceil().max(1.0) as u32;
        let bitmap = create_bitmap_rep(width, height).ok_or(AppKitMenuError::CaptureUnavailable)?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)
            .ok_or(AppKitMenuError::CaptureUnavailable)?;
        let view = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(layout.width, layout.height),
            ),
        );

        let previous = NSGraphicsContext::currentContext();
        NSGraphicsContext::setCurrentContext(Some(&context));
        NSGraphicsContext::saveGraphicsState_class();
        NSColor::windowBackgroundColor().setFill();
        NSBezierPath::fillRect(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(layout.width, layout.height),
        ));

        for row in &layout.rows {
            let ns_rect = ns_rect(row.rect);
            if highlighted_id == Some(row.id.as_str()) {
                NSColor::controlAccentColor().setFill();
                NSBezierPath::fillRect(ns_rect);
            }
            let cell = NSMenuItemCell::new(mtm);
            cell.setMenuItem(Some(&row.item));
            cell.setHighlighted(highlighted_id == Some(row.id.as_str()));
            cell.calcSize();
            if row.separator {
                cell.drawSeparatorItemWithFrame_inView(ns_rect, &view);
            } else {
                cell.drawWithFrame_inView(ns_rect, &view);
            }
        }

        context.flushGraphics();
        NSGraphicsContext::restoreGraphicsState_class();
        NSGraphicsContext::setCurrentContext(previous.as_deref());
        extract_pixels(&bitmap, width, height).ok_or(AppKitMenuError::CaptureUnavailable)
    }

    fn measured_cell_size(mtm: MainThreadMarker, item: &NSMenuItem) -> NSSize {
        let cell = NSMenuItemCell::new(mtm);
        cell.setMenuItem(Some(item));
        cell.calcSize();
        let size = cell.cellSize();
        NSSize::new(size.width.max(0.0), size.height.max(0.0))
    }

    fn create_bitmap_rep(width: u32, height: u32) -> Option<Retained<NSBitmapImageRep>> {
        let color_space = NSString::from_str("NSDeviceRGBColorSpace");
        let width = width as isize;
        let height = height as isize;
        let bytes_per_row = width.checked_mul(4)?;

        unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                ptr::null_mut::<*mut c_uchar>(),
                width,
                height,
                8,
                4,
                true,
                false,
                &color_space,
                bytes_per_row,
                32,
            )
        }
    }

    fn extract_pixels(rep: &NSBitmapImageRep, width: u32, height: u32) -> Option<Image> {
        let ptr = rep.bitmapData();
        if ptr.is_null() {
            return None;
        }

        let row_bytes = width as usize * 4;
        let bytes_per_row = rep.bytesPerRow() as usize;
        let count = bytes_per_row.checked_mul(height as usize)?;
        let bytes = unsafe { std::slice::from_raw_parts(ptr, count) };
        let data = copy_bitmap_rows(bytes, row_bytes, height as usize, bytes_per_row)?;

        Some(Image::new(width, height, data))
    }

    fn copy_bitmap_rows(
        bytes: &[u8],
        row_bytes: usize,
        height: usize,
        bytes_per_row: usize,
    ) -> Option<Vec<u8>> {
        if bytes_per_row < row_bytes {
            return None;
        }
        let required = bytes_per_row.checked_mul(height)?;
        if bytes.len() < required {
            return None;
        }
        if row_bytes == bytes_per_row {
            return Some(bytes[..required].to_vec());
        }

        let mut data = Vec::with_capacity(row_bytes.checked_mul(height)?);
        for row in 0..height {
            let offset = row.checked_mul(bytes_per_row)?;
            data.extend_from_slice(bytes.get(offset..offset + row_bytes)?);
        }
        Some(data)
    }

    fn top_menu_id(index: usize) -> String {
        format!("menu:{index}")
    }

    fn menu_item_id(top_level_index: usize, path: &[String], index: usize) -> String {
        let mut id = top_menu_id(top_level_index);
        for segment in path.iter().skip(1) {
            id.push_str("/submenu:");
            id.push_str(&slug(segment));
        }
        id.push_str("/item:");
        id.push_str(&index.to_string());
        id
    }

    fn menu_item_path_selector(path: &[String], title: &str) -> String {
        let mut selector = String::from("menu.item.path");
        for segment in path_with_title(path, title) {
            selector.push('.');
            selector.push_str(&slug(&segment));
        }
        selector
    }

    fn path_with_title(path: &[String], title: &str) -> Vec<String> {
        let mut path = path.to_vec();
        path.push(title.to_string());
        path
    }

    fn menu_item_title(item: &NSMenuItem, index: usize) -> String {
        if item.isSeparatorItem() {
            format!("separator-{index}")
        } else {
            item.title().to_string()
        }
    }

    fn estimated_title_width(title: &str) -> f64 {
        title.chars().count() as f64 * 8.0
    }

    fn format_modifier_flags(flags: NSEventModifierFlags) -> String {
        let mut parts = Vec::new();
        if flags.contains(NSEventModifierFlags::Command) {
            parts.push("cmd");
        }
        if flags.contains(NSEventModifierFlags::Shift) {
            parts.push("shift");
        }
        if flags.contains(NSEventModifierFlags::Option) {
            parts.push("option");
        }
        if flags.contains(NSEventModifierFlags::Control) {
            parts.push("control");
        }
        if flags.contains(NSEventModifierFlags::Function) {
            parts.push("function");
        }
        parts.join("+")
    }

    fn slug(value: &str) -> String {
        let mut slug = String::new();
        let mut last_dash = false;
        for ch in value.chars().flat_map(char::to_lowercase) {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch);
                last_dash = false;
            } else if !last_dash {
                slug.push('-');
                last_dash = true;
            }
        }
        let slug = slug.trim_matches('-').to_string();
        if slug.is_empty() {
            "item".into()
        } else {
            slug
        }
    }

    fn ns_rect(rect: Rect) -> NSRect {
        NSRect::new(
            NSPoint::new(rect.origin.x, rect.origin.y),
            NSSize::new(rect.size.width, rect.size.height),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn slug_normalizes_menu_titles_for_selectors() {
            assert_eq!(slug("Open..."), "open");
            assert_eq!(slug("Mixed State"), "mixed-state");
            assert_eq!(slug("  !!!  "), "item");
        }

        #[test]
        fn ids_and_path_selectors_are_stable() {
            let path = vec!["File".to_string(), "Export".to_string()];

            assert_eq!(top_menu_id(2), "menu:2");
            assert_eq!(menu_item_id(2, &path, 4), "menu:2/submenu:export/item:4");
            assert_eq!(
                menu_item_path_selector(&path, "PDF Document"),
                "menu.item.path.file.export.pdf-document"
            );
        }

        #[test]
        fn modifier_flags_use_stable_order() {
            let flags = NSEventModifierFlags::Command
                | NSEventModifierFlags::Shift
                | NSEventModifierFlags::Option
                | NSEventModifierFlags::Control
                | NSEventModifierFlags::Function;

            assert_eq!(
                format_modifier_flags(flags),
                "cmd+shift+option+control+function"
            );
            assert_eq!(format_modifier_flags(NSEventModifierFlags::empty()), "");
        }

        #[test]
        fn row_layout_uses_appkit_draw_coordinates() {
            let (height, rects) = row_rects_from_heights(200.0, &[20.0, 10.0]);

            assert_eq!(height, 42.0);
            assert_eq!(
                rects,
                vec![
                    Rect::new(Point::new(6.0, 16.0), Size::new(188.0, 20.0)),
                    Rect::new(Point::new(6.0, 6.0), Size::new(188.0, 10.0)),
                ]
            );
        }

        #[test]
        fn image_rects_use_top_left_image_coordinates() {
            let (height, rects) = row_rects_from_heights(200.0, &[20.0, 10.0]);
            let image_rects = rects
                .into_iter()
                .map(|rect| image_rect_from_appkit_rect(height, rect))
                .collect::<Vec<_>>();

            assert_eq!(
                image_rects,
                vec![
                    Rect::new(Point::new(6.0, 6.0), Size::new(188.0, 20.0)),
                    Rect::new(Point::new(6.0, 26.0), Size::new(188.0, 10.0)),
                ]
            );
        }

        #[test]
        fn bitmap_row_copy_removes_stride_padding() {
            let bytes = [
                1, 2, 3, 4, 5, 6, 7, 8, 99, 98, 97, 96, 9, 10, 11, 12, 13, 14, 15, 16, 95, 94, 93,
                92,
            ];

            assert_eq!(
                copy_bitmap_rows(&bytes, 8, 2, 12),
                Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
            );
        }

        #[test]
        fn bitmap_row_copy_rejects_narrow_or_short_rows() {
            assert_eq!(copy_bitmap_rows(&[1, 2, 3, 4], 8, 1, 4), None);
            assert_eq!(copy_bitmap_rows(&[1, 2, 3, 4], 4, 2, 4), None);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::fmt;

    use glasscheck_core::{Image, Scene, Selector};

    #[derive(Clone, Copy)]
    pub struct AppKitMenuBar;

    pub struct AppKitOpenedMenu;

    #[derive(Clone, Debug, PartialEq)]
    pub struct AppKitMenuCapture {
        pub image: Image,
        pub scene: Scene,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum AppKitMenuTarget {
        Title(String),
        Index(usize),
        Selector(Selector),
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct AppKitMenuCaptureOptions {
        pub highlighted_item: Option<Selector>,
        pub allow_visible_fallback: bool,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum AppKitMenuError {
        UnsupportedPlatform,
    }

    impl fmt::Display for AppKitMenuError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::UnsupportedPlatform => write!(f, "AppKit menu testing requires macOS"),
            }
        }
    }

    impl std::error::Error for AppKitMenuError {}
}

pub use imp::{
    AppKitMenuBar, AppKitMenuCapture, AppKitMenuCaptureOptions, AppKitMenuError, AppKitMenuTarget,
    AppKitOpenedMenu,
};
