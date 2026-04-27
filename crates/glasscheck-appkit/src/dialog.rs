#[cfg(target_os = "macos")]
mod imp {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use glasscheck_core::{
        InputSynthesisError, NodeProvenanceKind, Point, PollError, PollOptions, PropertyValue,
        Rect, RegionResolveError, Role, Scene, Selector, SemanticNode, SemanticSnapshot, Size,
    };
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn,
        NSApplication, NSButton, NSControl, NSModalResponseCancel, NSModalResponseOK, NSOpenPanel,
        NSPanel, NSSavePanel, NSTextField, NSTextView, NSView, NSWindow,
    };
    use objc2_foundation::{MainThreadMarker, NSArray, NSPoint, NSRect, NSString, NSURL};

    use crate::{AppKitHarness, AppKitSceneSource, AppKitSnapshotContext, AppKitWindowHost};

    const DIALOG_ROOT_ID: &str = "appkit.dialog";
    const DIALOG_KIND_PROPERTY: &str = "appkit:dialog_kind";
    const CLASS_PROPERTY: &str = "appkit:class";
    const VIEW_PATH_PROPERTY: &str = "appkit:view_path";
    const BUTTON_ROLE_PROPERTY: &str = "appkit:button_role";
    const DIRECTORY_URL_PROPERTY: &str = "appkit:directory_url";
    const NAME_FIELD_PROPERTY: &str = "appkit:name_field";
    const PROMPT_PROPERTY: &str = "appkit:prompt";
    const MESSAGE_PROPERTY: &str = "appkit:message";
    const SELECTED_PATH_PROPERTY: &str = "appkit:selected_path";

    /// Native AppKit dialog or panel kind.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AppKitDialogKind {
        /// Standard `NSAlert` dialog or sheet.
        Alert,
        /// Standard `NSOpenPanel`.
        OpenPanel,
        /// Standard `NSSavePanel`.
        SavePanel,
        /// Other AppKit `NSPanel`-backed dialog.
        Panel,
    }

    impl AppKitDialogKind {
        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Alert => "alert",
                Self::OpenPanel => "open_panel",
                Self::SavePanel => "save_panel",
                Self::Panel => "panel",
            }
        }
    }

    /// Query used to discover native AppKit dialogs and panels.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct AppKitDialogQuery {
        kind: Option<AppKitDialogKind>,
        title: Option<DialogTitleMatch>,
    }

    impl AppKitDialogQuery {
        /// Matches a standard AppKit alert.
        #[must_use]
        pub fn alert() -> Self {
            Self::kind(AppKitDialogKind::Alert)
        }

        /// Matches a standard AppKit open panel.
        #[must_use]
        pub fn open_panel() -> Self {
            Self::kind(AppKitDialogKind::OpenPanel)
        }

        /// Matches a standard AppKit save panel.
        #[must_use]
        pub fn save_panel() -> Self {
            Self::kind(AppKitDialogKind::SavePanel)
        }

        /// Matches a dialog or panel by kind.
        #[must_use]
        pub fn kind(kind: AppKitDialogKind) -> Self {
            Self {
                kind: Some(kind),
                title: None,
            }
        }

        /// Adds an exact title constraint.
        #[must_use]
        pub fn title_eq(mut self, title: impl Into<String>) -> Self {
            self.title = Some(DialogTitleMatch::Exact(title.into()));
            self
        }

        /// Adds a substring title constraint.
        #[must_use]
        pub fn title_contains(mut self, title: impl Into<String>) -> Self {
            self.title = Some(DialogTitleMatch::Contains(title.into()));
            self
        }

        #[must_use]
        pub(crate) fn matches_window(&self, window: &NSWindow) -> bool {
            let Some(kind) = classify_window(window) else {
                return false;
            };
            if self.kind.is_some_and(|expected| expected != kind) {
                return false;
            }
            self.title.as_ref().is_none_or(|title| {
                title.matches(&dialog_title(window, kind))
                    || title.matches(&window.title().to_string())
            })
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum DialogTitleMatch {
        Exact(String),
        Contains(String),
    }

    impl DialogTitleMatch {
        fn matches(&self, title: &str) -> bool {
            match self {
                Self::Exact(expected) => title == expected,
                Self::Contains(expected) => title.contains(expected),
            }
        }
    }

    /// Errors returned by AppKit dialog helpers.
    #[derive(Debug)]
    pub enum AppKitDialogError {
        /// The requested surface is absent or already closed.
        MissingSurface,
        /// The requested surface is an ordinary window, not a native dialog or panel.
        NotDialog,
        /// The attached surface was not the required dialog kind.
        KindMismatch {
            expected: AppKitDialogKind,
            actual: AppKitDialogKind,
        },
        /// The live OS panel did not expose enough public structure to drive the request.
        UnsupportedLiveSelection(&'static str),
        /// A requested file-system path was invalid or missing.
        MissingRequestedPath(PathBuf),
        /// Native capture or input failed.
        InputUnavailable,
        /// The requested selector did not resolve.
        Resolve(RegionResolveError),
        /// Polling timed out.
        Poll(PollError),
        /// A completed panel did not report the expected selected path.
        SelectionMismatch {
            expected: Vec<PathBuf>,
            actual: Vec<PathBuf>,
        },
    }

    impl std::fmt::Display for AppKitDialogError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MissingSurface => write!(f, "dialog surface is missing"),
                Self::NotDialog => write!(f, "surface is not an AppKit dialog or panel"),
                Self::KindMismatch { expected, actual } => {
                    write!(f, "expected {:?}, found {:?}", expected, actual)
                }
                Self::UnsupportedLiveSelection(reason) => {
                    write!(f, "live panel selection is unsupported: {reason}")
                }
                Self::MissingRequestedPath(path) => {
                    write!(f, "requested path is unavailable: {}", path.display())
                }
                Self::InputUnavailable => write!(f, "dialog input is unavailable"),
                Self::Resolve(error) => write!(f, "{error}"),
                Self::Poll(error) => write!(f, "{error}"),
                Self::SelectionMismatch { expected, actual } => {
                    write!(
                        f,
                        "selection mismatch: expected {expected:?}, found {actual:?}"
                    )
                }
            }
        }
    }

    impl std::error::Error for AppKitDialogError {}

    impl From<PollError> for AppKitDialogError {
        fn from(value: PollError) -> Self {
            Self::Poll(value)
        }
    }

    impl From<RegionResolveError> for AppKitDialogError {
        fn from(value: RegionResolveError) -> Self {
            Self::Resolve(value)
        }
    }

    pub(crate) fn attach_dialog_window(
        harness: AppKitHarness,
        window: &NSWindow,
    ) -> AppKitWindowHost {
        configure_background_dialog_window(window);
        let host = harness.attach_window(window);
        let retained = retain_window(window);
        host.set_contextual_scene_source(Box::new(DialogSceneSource { window: retained }));
        host
    }

    fn configure_background_dialog_window(window: &NSWindow) {
        window.setExcludedFromWindowsMenu(true);
        window.setCanHide(false);
        window.setHidesOnDeactivate(true);
        window.setAlphaValue(0.0);
    }

    pub(crate) fn classify_window(window: &NSWindow) -> Option<AppKitDialogKind> {
        if window_is_kind_of::<NSOpenPanel>(window) {
            Some(AppKitDialogKind::OpenPanel)
        } else if window_is_kind_of::<NSSavePanel>(window) {
            Some(AppKitDialogKind::SavePanel)
        } else if window_is_kind_of::<NSPanel>(window) && class_name(window).contains("Alert") {
            Some(AppKitDialogKind::Alert)
        } else if window_is_kind_of::<NSPanel>(window) {
            Some(AppKitDialogKind::Panel)
        } else {
            None
        }
    }

    pub(crate) fn snapshot_dialog_scene(
        host: &AppKitWindowHost,
    ) -> Result<Scene, AppKitDialogError> {
        build_dialog_scene(host.window())
    }

    pub(crate) fn set_dialog_text(
        host: &AppKitWindowHost,
        predicate: &Selector,
        text: &str,
    ) -> Result<(), AppKitDialogError> {
        let scene = snapshot_dialog_scene(host)?;
        let handle = scene.find(predicate).map_err(map_query_error)?;
        let node = scene
            .node(handle)
            .ok_or(AppKitDialogError::InputUnavailable)?;
        let Some(PropertyValue::String(path)) = node.properties.get(VIEW_PATH_PROPERTY) else {
            return Err(AppKitDialogError::InputUnavailable);
        };
        if node.role != Role::TextInput {
            return Err(AppKitDialogError::InputUnavailable);
        }
        let Some(view) = view_at_path(host.window(), path) else {
            return Err(AppKitDialogError::InputUnavailable);
        };
        set_view_text(&view, text)
    }

    pub(crate) fn choose_save_panel_path(
        host: &AppKitWindowHost,
        path: &Path,
        options: PollOptions,
    ) -> Result<usize, AppKitDialogError> {
        let actual = classify_window(host.window()).ok_or(AppKitDialogError::NotDialog)?;
        if actual != AppKitDialogKind::SavePanel {
            return Err(AppKitDialogError::KindMismatch {
                expected: AppKitDialogKind::SavePanel,
                actual,
            });
        }
        let intent = save_panel_path_intent(path)?;
        let panel = retained_save_panel(host.window()).ok_or(
            AppKitDialogError::UnsupportedLiveSelection("surface is not NSSavePanel"),
        )?;
        let directory = file_url(&intent.directory, true);
        panel.setDirectoryURL(Some(&directory));
        panel.setNameFieldStringValue(&NSString::from_str(&intent.file_name));
        unsafe {
            panel.ok(None);
        }
        let attempts = wait_for_panel_result(host, options)?;
        let actual = panel_url_paths(&panel);
        if paths_match(&actual, &intent.expected) {
            Ok(attempts)
        } else {
            Err(AppKitDialogError::SelectionMismatch {
                expected: intent.expected,
                actual,
            })
        }
    }

    pub(crate) fn choose_open_panel_paths(
        host: &AppKitWindowHost,
        paths: &[PathBuf],
        options: PollOptions,
    ) -> Result<usize, AppKitDialogError> {
        let actual = classify_window(host.window()).ok_or(AppKitDialogError::NotDialog)?;
        if actual != AppKitDialogKind::OpenPanel {
            return Err(AppKitDialogError::KindMismatch {
                expected: AppKitDialogKind::OpenPanel,
                actual,
            });
        }
        let intent = open_panel_path_intent(paths)?;
        let panel = retained_open_panel(host.window()).ok_or(
            AppKitDialogError::UnsupportedLiveSelection("surface is not NSOpenPanel"),
        )?;
        panel.setDirectoryURL(Some(&file_url(&intent.directory, true)));
        wait_for_directory(host, &intent.directory, options)?;
        if paths.len() > 1 {
            panel.setAllowsMultipleSelection(true);
        }
        for (index, path) in paths.iter().enumerate() {
            click_open_panel_path(host, path, index > 0)?;
        }
        unsafe {
            panel.ok(None);
        }
        let attempts = wait_for_panel_result(host, options)?;
        let actual = open_panel_url_paths(&panel);
        if paths_match(&actual, &intent.expected) {
            Ok(attempts)
        } else {
            Err(AppKitDialogError::SelectionMismatch {
                expected: intent.expected,
                actual,
            })
        }
    }

    pub(crate) fn cancel_dialog(
        host: &AppKitWindowHost,
        options: PollOptions,
    ) -> Result<usize, AppKitDialogError> {
        match classify_window(host.window()).ok_or(AppKitDialogError::NotDialog)? {
            AppKitDialogKind::OpenPanel => {
                let panel = retained_open_panel(host.window())
                    .ok_or(AppKitDialogError::InputUnavailable)?;
                unsafe {
                    panel.cancel(None);
                }
            }
            AppKitDialogKind::SavePanel => {
                let panel = retained_save_panel(host.window())
                    .ok_or(AppKitDialogError::InputUnavailable)?;
                unsafe {
                    panel.cancel(None);
                }
            }
            AppKitDialogKind::Alert | AppKitDialogKind::Panel => {
                if click_first_button_by_role(host, "cancel")
                    .or_else(|_| click_fallback_button_by_role(host, "cancel"))
                    .is_err()
                {
                    host.window().close();
                }
            }
        }
        wait_for_panel_result(host, options)
    }

    pub(crate) fn click_dialog_button(
        host: &AppKitWindowHost,
        predicate: &Selector,
    ) -> Result<(), AppKitDialogError> {
        let scene = snapshot_dialog_scene(host)?;
        let handle = scene.find(predicate).map_err(map_query_error)?;
        let node = scene
            .node(handle)
            .ok_or(AppKitDialogError::InputUnavailable)?;
        if node.role != Role::Button {
            return Err(AppKitDialogError::InputUnavailable);
        }
        if let Some(PropertyValue::String(path)) = node.properties.get(VIEW_PATH_PROPERTY) {
            let Some(view) = view_at_path(host.window(), path) else {
                return Err(AppKitDialogError::InputUnavailable);
            };
            if let Ok(button) = view.clone().downcast::<NSButton>() {
                unsafe {
                    button.performClick(None);
                }
                return Ok(());
            }
            return host.click_node(predicate).map_err(AppKitDialogError::from);
        }
        if classify_window(host.window()).ok_or(AppKitDialogError::NotDialog)?
            == AppKitDialogKind::Alert
        {
            return click_alert_fallback(host, predicate);
        }
        Err(AppKitDialogError::InputUnavailable)
    }

    fn build_dialog_scene(window: &NSWindow) -> Result<Scene, AppKitDialogError> {
        Ok(Scene::new(build_dialog_snapshot(window)?.nodes))
    }

    fn build_dialog_snapshot(window: &NSWindow) -> Result<SemanticSnapshot, AppKitDialogError> {
        let kind = classify_window(window).ok_or(AppKitDialogError::NotDialog)?;
        let root_bounds = window_root_bounds(window);
        let mut root = SemanticNode::new(DIALOG_ROOT_ID, Role::Window, root_bounds)
            .with_selector("appkit.dialog")
            .with_selector(format!("appkit.dialog.{}", kind.as_str()))
            .with_property(DIALOG_KIND_PROPERTY, PropertyValue::string(kind.as_str()))
            .with_property(CLASS_PROPERTY, PropertyValue::string(class_name(window)))
            .with_provenance(NodeProvenanceKind::Native);
        let title = dialog_title(window, kind);
        if !title.is_empty() {
            root.label = Some(title);
        }
        add_panel_properties(window, kind, &mut root);
        let mut nodes = vec![root];
        if let Some(root_view) = window.contentView() {
            let root_view_index = next_child_index(&nodes, DIALOG_ROOT_ID);
            collect_view_nodes(
                &root_view,
                &root_view,
                DIALOG_ROOT_ID,
                "0".into(),
                root_view_index,
                &mut nodes,
            );
        }
        if kind == AppKitDialogKind::Alert {
            add_alert_fallback_buttons(&mut nodes);
        }
        add_panel_selection_nodes(window, kind, &mut nodes);
        Ok(SemanticSnapshot::from_nodes(nodes))
    }

    fn add_alert_fallback_buttons(nodes: &mut Vec<SemanticNode>) {
        let has_confirm = nodes.iter().any(|node| {
            node.properties
                .get(BUTTON_ROLE_PROPERTY)
                .is_some_and(|value| value == &PropertyValue::string("confirm"))
        });
        if has_confirm {
            return;
        }
        let has_cancel = nodes.iter().any(|node| {
            node.properties
                .get(BUTTON_ROLE_PROPERTY)
                .is_some_and(|value| value == &PropertyValue::string("cancel"))
        });
        let confirm_index = next_child_index(nodes, DIALOG_ROOT_ID);
        nodes.push(
            SemanticNode::new(
                "appkit.dialog.alert.confirm",
                Role::Button,
                Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)),
            )
            .with_label("OK")
            .with_parent(DIALOG_ROOT_ID, confirm_index)
            .with_selector("appkit.dialog.button")
            .with_selector("appkit.dialog.button.confirm")
            .with_property(BUTTON_ROLE_PROPERTY, PropertyValue::string("confirm"))
            .with_provenance(NodeProvenanceKind::Native),
        );
        if !has_cancel {
            let cancel_index = next_child_index(nodes, DIALOG_ROOT_ID);
            nodes.push(
                SemanticNode::new(
                    "appkit.dialog.alert.cancel",
                    Role::Button,
                    Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)),
                )
                .with_label("Cancel")
                .with_parent(DIALOG_ROOT_ID, cancel_index)
                .with_selector("appkit.dialog.button")
                .with_selector("appkit.dialog.button.cancel")
                .with_property(BUTTON_ROLE_PROPERTY, PropertyValue::string("cancel"))
                .with_provenance(NodeProvenanceKind::Native),
            )
        }
    }

    fn add_panel_properties(window: &NSWindow, kind: AppKitDialogKind, node: &mut SemanticNode) {
        let Some(panel) = retained_save_panel(window) else {
            return;
        };
        if let Some(directory) = panel.directoryURL().and_then(|url| url.path()) {
            insert_native_property(
                node,
                DIRECTORY_URL_PROPERTY.into(),
                PropertyValue::string(directory.to_string()),
            );
        }
        if matches!(
            kind,
            AppKitDialogKind::SavePanel | AppKitDialogKind::OpenPanel
        ) {
            insert_native_property(
                node,
                PROMPT_PROPERTY.into(),
                PropertyValue::string(panel.prompt().to_string()),
            );
            insert_native_property(
                node,
                MESSAGE_PROPERTY.into(),
                PropertyValue::string(panel.message().to_string()),
            );
            if kind == AppKitDialogKind::SavePanel {
                insert_native_property(
                    node,
                    NAME_FIELD_PROPERTY.into(),
                    PropertyValue::string(panel.nameFieldStringValue().to_string()),
                );
            }
        }
    }

    fn add_panel_selection_nodes(
        window: &NSWindow,
        kind: AppKitDialogKind,
        nodes: &mut Vec<SemanticNode>,
    ) {
        let paths = match kind {
            AppKitDialogKind::OpenPanel => retained_open_panel(window)
                .map(|panel| open_panel_url_paths(&panel))
                .unwrap_or_default(),
            AppKitDialogKind::SavePanel => retained_save_panel(window)
                .map(|panel| panel_url_paths(&panel))
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        for (index, path) in paths.into_iter().enumerate() {
            let child_index = next_child_index(nodes, DIALOG_ROOT_ID);
            let label = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            nodes.push(
                SemanticNode::new(
                    format!("appkit.dialog.selection.{index}"),
                    Role::ListItem,
                    Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)),
                )
                .with_label(label)
                .with_parent(DIALOG_ROOT_ID, child_index)
                .with_selector("appkit.dialog.selection")
                .with_property(
                    SELECTED_PATH_PROPERTY,
                    PropertyValue::string(path.display().to_string()),
                )
                .with_provenance(NodeProvenanceKind::Native),
            );
        }
    }

    fn next_child_index(nodes: &[SemanticNode], parent_id: &str) -> usize {
        nodes
            .iter()
            .filter(|node| node.parent_id.as_deref() == Some(parent_id))
            .count()
    }

    fn collect_view_nodes(
        view: &NSView,
        root_view: &NSView,
        parent_id: &str,
        path: String,
        child_index: usize,
        nodes: &mut Vec<SemanticNode>,
    ) {
        let id = format!("appkit.dialog.view.{path}");
        let mut node =
            SemanticNode::new(id.clone(), view_role(view), rect_in_root(view, root_view))
                .with_parent(parent_id, child_index)
                .with_selector("appkit.dialog.view")
                .with_property(CLASS_PROPERTY, PropertyValue::string(class_name(view)))
                .with_property(VIEW_PATH_PROPERTY, PropertyValue::string(path.clone()))
                .with_provenance(NodeProvenanceKind::Native);
        node.z_index = child_index as i32;
        node.visible = !view.isHiddenOrHasHiddenAncestor();
        if node.visible && node.rect.size.width > 0.0 && node.rect.size.height > 0.0 {
            node.visible_rect = Some(node.rect);
            node.hit_testable = true;
        } else {
            node.hit_testable = false;
        }
        apply_view_semantics(view, &mut node);
        nodes.push(node);

        let subviews = view.subviews();
        for (index, child) in subviews.iter().enumerate() {
            collect_view_nodes(
                &child,
                root_view,
                &id,
                format!("{path}.{index}"),
                index,
                nodes,
            );
        }
    }

    fn apply_view_semantics(view: &NSView, node: &mut SemanticNode) {
        let object = any_object(view);
        if let Some(button) = object.downcast_ref::<NSButton>() {
            let title = button.title().to_string();
            if !title.is_empty() {
                node.label = Some(title.clone());
                node.selectors.insert(format!(
                    "appkit.dialog.button.{}",
                    selector_fragment(&title)
                ));
            }
            let role = button_role(button);
            node.selectors.insert("appkit.dialog.button".into());
            node.selectors
                .insert(format!("appkit.dialog.button.{role}"));
            insert_native_property(
                node,
                BUTTON_ROLE_PROPERTY.into(),
                PropertyValue::string(role),
            );
            insert_native_state(
                node,
                "enabled".into(),
                PropertyValue::Bool(button.isEnabled()),
            );
            node.role = Role::Button;
            return;
        }
        if let Some(field) = object.downcast_ref::<NSTextField>() {
            let value = field.stringValue().to_string();
            if field.isEditable() {
                node.role = Role::TextInput;
                node.value = Some(value.clone());
                node.selectors.insert("appkit.dialog.text_input".into());
            } else {
                node.role = Role::Label;
                node.label = usable_visible_text(&value);
                node.selectors.insert("appkit.dialog.label".into());
            }
            if let Some(value) = usable_selector_text(&value) {
                node.selectors
                    .insert(format!("appkit.dialog.text.{}", selector_fragment(&value)));
            }
            return;
        }
        if let Some(text_view) = object.downcast_ref::<NSTextView>() {
            let value = text_view.string().to_string();
            if text_view.isEditable() {
                node.role = Role::TextInput;
                node.value = Some(value.clone());
                node.selectors.insert("appkit.dialog.text_input".into());
            } else {
                node.role = Role::Label;
                node.label = usable_visible_text(&value);
                node.selectors.insert("appkit.dialog.label".into());
            }
            if let Some(value) = usable_selector_text(&value) {
                node.selectors
                    .insert(format!("appkit.dialog.text.{}", selector_fragment(&value)));
            }
            return;
        }
        if let Some(control) = object.downcast_ref::<NSControl>() {
            let value = control.stringValue().to_string();
            if let Some(value) = usable_visible_text(&value) {
                node.label = Some(value.clone());
                if let Some(value) = usable_selector_text(&value) {
                    node.selectors
                        .insert(format!("appkit.dialog.text.{}", selector_fragment(&value)));
                }
            }
            insert_native_state(
                node,
                "enabled".into(),
                PropertyValue::Bool(control.isEnabled()),
            );
        }
    }

    fn insert_native_property(node: &mut SemanticNode, key: String, value: PropertyValue) {
        node.properties.insert(key.clone(), value);
        node.property_provenance
            .insert(key, NodeProvenanceKind::Native);
    }

    fn insert_native_state(node: &mut SemanticNode, key: String, value: PropertyValue) {
        node.state.insert(key.clone(), value);
        node.state_provenance
            .insert(key, NodeProvenanceKind::Native);
    }

    fn view_role(view: &NSView) -> Role {
        let object = any_object(view);
        if object.downcast_ref::<NSButton>().is_some() {
            Role::Button
        } else if object
            .downcast_ref::<NSTextField>()
            .is_some_and(NSTextField::isEditable)
            || object
                .downcast_ref::<NSTextView>()
                .is_some_and(NSTextView::isEditable)
        {
            Role::TextInput
        } else if object.downcast_ref::<NSTextField>().is_some()
            || object.downcast_ref::<NSTextView>().is_some()
        {
            Role::Label
        } else {
            Role::Container
        }
    }

    fn set_view_text(view: &NSView, text: &str) -> Result<(), AppKitDialogError> {
        let text = NSString::from_str(text);
        let object = any_object(view);
        if let Some(field) = object.downcast_ref::<NSTextField>() {
            if !field.isEditable() {
                return Err(AppKitDialogError::InputUnavailable);
            }
            field.setStringValue(&text);
            return Ok(());
        }
        if let Some(text_view) = object.downcast_ref::<NSTextView>() {
            if !text_view.isEditable() {
                return Err(AppKitDialogError::InputUnavailable);
            }
            text_view.setString(&text);
            return Ok(());
        }
        Err(AppKitDialogError::InputUnavailable)
    }

    fn click_open_panel_path(
        host: &AppKitWindowHost,
        path: &Path,
        additive: bool,
    ) -> Result<(), AppKitDialogError> {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| AppKitDialogError::MissingRequestedPath(path.to_path_buf()))?;
        let predicate = Selector::or(vec![
            Selector::label(glasscheck_core::TextMatch::exact(name.clone())),
            Selector::Value(glasscheck_core::TextMatch::exact(name)),
        ]);
        if additive {
            return Err(AppKitDialogError::UnsupportedLiveSelection(
                "multi-selection requires OS file-browser rows to support additive clicks",
            ));
        }
        host.click_node(&predicate).map_err(|error| match error {
            RegionResolveError::NotFound(_) => AppKitDialogError::UnsupportedLiveSelection(
                "requested file was not exposed in the open-panel view tree",
            ),
            other => AppKitDialogError::Resolve(other),
        })
    }

    fn click_first_button_by_role(
        host: &AppKitWindowHost,
        role: &str,
    ) -> Result<(), AppKitDialogError> {
        host.click_node(&Selector::property_eq(
            BUTTON_ROLE_PROPERTY,
            PropertyValue::string(role),
        ))
        .map_err(AppKitDialogError::from)
    }

    fn click_fallback_button_by_role(
        host: &AppKitWindowHost,
        role: &str,
    ) -> Result<(), AppKitDialogError> {
        click_alert_fallback(
            host,
            &Selector::property_eq(BUTTON_ROLE_PROPERTY, PropertyValue::string(role)),
        )
    }

    fn click_alert_fallback(
        host: &AppKitWindowHost,
        predicate: &Selector,
    ) -> Result<(), AppKitDialogError> {
        let scene = snapshot_dialog_scene(host)?;
        let handle = scene.find(predicate).map_err(map_query_error)?;
        let node = scene
            .node(handle)
            .ok_or(AppKitDialogError::InputUnavailable)?;
        if node.properties.contains_key(VIEW_PATH_PROPERTY) {
            return Err(AppKitDialogError::InputUnavailable);
        }
        let role = match node.properties.get(BUTTON_ROLE_PROPERTY) {
            Some(PropertyValue::String(role)) => role.as_str(),
            _ => return Err(AppKitDialogError::InputUnavailable),
        };
        let response = match role {
            "cancel" => NSAlertSecondButtonReturn,
            "confirm" => NSAlertFirstButtonReturn,
            _ => NSAlertThirdButtonReturn,
        };
        if let Some(parent) = host.window().sheetParent() {
            parent.endSheet_returnCode(host.window(), response);
        } else {
            let app = NSApplication::sharedApplication(host.main_thread_marker());
            app.stopModalWithCode(response);
            host.window().orderOut(None);
        }
        Ok(())
    }

    fn wait_for_panel_result(
        host: &AppKitWindowHost,
        options: PollOptions,
    ) -> Result<usize, AppKitDialogError> {
        glasscheck_core::wait_for_condition(options, || {
            !host.window().isVisible() && host.window().parentWindow().is_none()
        })
        .map_err(AppKitDialogError::from)
    }

    fn wait_for_directory(
        host: &AppKitWindowHost,
        directory: &Path,
        options: PollOptions,
    ) -> Result<usize, AppKitDialogError> {
        glasscheck_core::wait_for_condition(options, || {
            let Some(panel) = retained_save_panel(host.window()) else {
                return false;
            };
            panel
                .directoryURL()
                .and_then(|url| url.path())
                .is_some_and(|path| {
                    PathBuf::from(path.to_string()) == canonical_intent_path(directory)
                })
        })
        .map_err(AppKitDialogError::from)
    }

    fn retained_open_panel(window: &NSWindow) -> Option<Retained<NSOpenPanel>> {
        retain_window(window).downcast::<NSOpenPanel>().ok()
    }

    fn retained_save_panel(window: &NSWindow) -> Option<Retained<NSSavePanel>> {
        retain_window(window).downcast::<NSSavePanel>().ok()
    }

    fn retain_window(window: &NSWindow) -> Retained<NSWindow> {
        unsafe {
            Retained::retain(window as *const NSWindow as *mut NSWindow)
                .expect("dialog window should retain successfully")
        }
    }

    fn file_url(path: &Path, is_dir: bool) -> Retained<NSURL> {
        NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&path.to_string_lossy()), is_dir)
    }

    fn panel_url_paths(panel: &NSSavePanel) -> Vec<PathBuf> {
        panel
            .URL()
            .and_then(|url| url.path())
            .map(|path| vec![PathBuf::from(path.to_string())])
            .unwrap_or_default()
    }

    fn open_panel_url_paths(panel: &NSOpenPanel) -> Vec<PathBuf> {
        panel
            .URLs()
            .iter()
            .filter_map(|url| url.path())
            .map(|path| PathBuf::from(path.to_string()))
            .collect()
    }

    fn paths_match(actual: &[PathBuf], expected: &[PathBuf]) -> bool {
        let mut actual = actual
            .iter()
            .map(|path| canonical_intent_path(path))
            .collect::<Vec<_>>();
        let mut expected = expected
            .iter()
            .map(|path| canonical_intent_path(path))
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        actual == expected
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SavePanelPathIntent {
        directory: PathBuf,
        file_name: String,
        expected: Vec<PathBuf>,
    }

    fn save_panel_path_intent(path: &Path) -> Result<SavePanelPathIntent, AppKitDialogError> {
        let directory = path
            .parent()
            .filter(|parent| parent.exists())
            .map(Path::to_path_buf)
            .ok_or_else(|| AppKitDialogError::MissingRequestedPath(path.to_path_buf()))?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| AppKitDialogError::MissingRequestedPath(path.to_path_buf()))?;
        Ok(SavePanelPathIntent {
            directory,
            file_name,
            expected: vec![canonical_intent_path(path)],
        })
    }

    #[derive(Debug, PartialEq, Eq)]
    struct OpenPanelPathIntent {
        directory: PathBuf,
        expected: Vec<PathBuf>,
    }

    fn open_panel_path_intent(paths: &[PathBuf]) -> Result<OpenPanelPathIntent, AppKitDialogError> {
        let Some(first) = paths.first() else {
            return Err(AppKitDialogError::UnsupportedLiveSelection(
                "at least one open-panel path is required",
            ));
        };
        if paths.len() > 1 {
            return Err(AppKitDialogError::UnsupportedLiveSelection(
                "multi-selection requires deterministic additive selection support",
            ));
        }
        for path in paths {
            if !path.exists() {
                return Err(AppKitDialogError::MissingRequestedPath(path.clone()));
            }
        }
        let directory = first
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppKitDialogError::MissingRequestedPath(first.clone()))?;
        if paths
            .iter()
            .any(|path| path.parent() != Some(directory.as_path()))
        {
            return Err(AppKitDialogError::UnsupportedLiveSelection(
                "all requested open-panel paths must share a parent directory",
            ));
        }
        Ok(OpenPanelPathIntent {
            directory,
            expected: paths
                .iter()
                .map(|path| canonical_intent_path(path))
                .collect(),
        })
    }

    fn canonical_intent_path(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn view_at_path(window: &NSWindow, path: &str) -> Option<Retained<NSView>> {
        let root = window.contentView()?;
        if path == "0" {
            return Some(root);
        }
        let mut current = root;
        for segment in path.split('.').skip(1) {
            let index = segment.parse::<usize>().ok()?;
            let subviews = current.subviews().to_vec();
            current = subviews.get(index).cloned()?;
        }
        Some(current)
    }

    fn rect_in_root(view: &NSView, root_view: &NSView) -> Rect {
        let rect = root_view.convertRect_fromView(view.bounds(), Some(view));
        ns_rect_to_rect(rect)
    }

    fn window_root_bounds(window: &NSWindow) -> Rect {
        let rect = window.contentLayoutRect();
        Rect::new(
            Point::new(0.0, 0.0),
            Size::new(rect.size.width, rect.size.height),
        )
    }

    fn ns_rect_to_rect(rect: NSRect) -> Rect {
        Rect::new(
            Point::new(rect.origin.x, rect.origin.y),
            Size::new(rect.size.width, rect.size.height),
        )
    }

    fn class_name<T>(value: &T) -> String {
        format!("{:?}", any_object(value).class())
    }

    fn window_is_kind_of<T>(window: &NSWindow) -> bool
    where
        T: objc2::DowncastTarget,
    {
        any_object(window).downcast_ref::<T>().is_some()
    }

    fn any_object<T>(value: &T) -> &AnyObject {
        unsafe { &*(std::ptr::from_ref(value).cast()) }
    }

    fn dialog_title(window: &NSWindow, kind: AppKitDialogKind) -> String {
        if kind == AppKitDialogKind::SavePanel || kind == AppKitDialogKind::OpenPanel {
            if let Some(panel) = retained_save_panel(window) {
                let title = panel.title().to_string();
                if !title.is_empty() {
                    return title;
                }
                let message = panel.message().to_string();
                if !message.is_empty() {
                    return message;
                }
            }
        }
        let title = window.title().to_string();
        if !title.is_empty() {
            return title;
        }
        first_label(window.contentView().as_deref()).unwrap_or_default()
    }

    fn first_label(view: Option<&NSView>) -> Option<String> {
        let view = view?;
        let object = any_object(view);
        if let Some(field) = object.downcast_ref::<NSTextField>() {
            let value = field.stringValue().to_string();
            if let Some(value) = usable_visible_text(&value) {
                return Some(value);
            }
        }
        if let Some(text_view) = object.downcast_ref::<NSTextView>() {
            let value = text_view.string().to_string();
            if let Some(value) = usable_visible_text(&value) {
                return Some(value);
            }
        }
        view.subviews()
            .iter()
            .find_map(|child| first_label(Some(&child)))
    }

    fn usable_visible_text(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.starts_with("<NSImage") || trimmed.len() > 512 {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn usable_selector_text(value: &str) -> Option<String> {
        let visible = usable_visible_text(value)?;
        if visible.len() > 120 || visible.contains('\n') {
            None
        } else {
            Some(visible)
        }
    }

    fn button_role(button: &NSButton) -> &'static str {
        let title = button.title().to_string().to_lowercase();
        let key_equivalent = button.keyEquivalent().to_string();
        if title == "cancel" || key_equivalent == "\u{1b}" {
            "cancel"
        } else if title == "ok" || title == "open" || title == "save" || key_equivalent == "\r" {
            "confirm"
        } else {
            "other"
        }
    }

    fn selector_fragment(value: &str) -> String {
        let mut fragment = value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '.'
                }
            })
            .collect::<String>();
        while fragment.contains("..") {
            fragment = fragment.replace("..", ".");
        }
        fragment.trim_matches('.').to_string()
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

    struct DialogSceneSource {
        window: Retained<NSWindow>,
    }

    impl AppKitSceneSource for DialogSceneSource {
        fn snapshot(&self, _context: &AppKitSnapshotContext<'_>) -> SemanticSnapshot {
            build_dialog_snapshot(&self.window)
                .expect("attached dialog scene source requires a dialog or panel")
        }
    }

    #[cfg(test)]
    mod tests {
        use std::fs;
        use std::path::PathBuf;
        use std::time::{SystemTime, UNIX_EPOCH};

        use glasscheck_core::{Point, Rect, Role, SemanticNode, Size};

        use super::{
            next_child_index, open_panel_path_intent, save_panel_path_intent, selector_fragment,
            usable_selector_text, usable_visible_text, AppKitDialogError, AppKitDialogKind,
            AppKitDialogQuery, DialogTitleMatch, OpenPanelPathIntent, SavePanelPathIntent,
        };

        #[test]
        fn dialog_query_builders_capture_kind_and_title() {
            assert_eq!(
                AppKitDialogQuery::alert().kind,
                Some(AppKitDialogKind::Alert)
            );
            assert_eq!(
                AppKitDialogQuery::open_panel().kind,
                Some(AppKitDialogKind::OpenPanel)
            );
            assert_eq!(
                AppKitDialogQuery::save_panel().kind,
                Some(AppKitDialogKind::SavePanel)
            );
        }

        #[test]
        fn dialog_title_matches_cover_exact_contains_and_misses() {
            let exact = DialogTitleMatch::Exact("Export Document".into());
            assert!(exact.matches("Export Document"));
            assert!(!exact.matches("Export"));

            let contains = DialogTitleMatch::Contains("Document".into());
            assert!(contains.matches("Export Document"));
            assert!(!contains.matches("Open Fixture"));
        }

        #[test]
        fn selector_fragment_normalizes_visible_text() {
            assert_eq!(selector_fragment("Don't Save"), "don.t.save");
            assert_eq!(selector_fragment("  Open…  "), "open");
        }

        #[test]
        fn unusable_control_text_is_not_promoted_to_semantics() {
            assert_eq!(usable_visible_text("  Export  "), Some("Export".into()));
            assert!(usable_visible_text("<NSImage 0x123>").is_none());
            assert!(usable_selector_text(&"x".repeat(121)).is_none());
            assert!(usable_selector_text("line one\nline two").is_none());
        }

        #[test]
        fn next_child_index_counts_siblings_under_the_same_parent() {
            let rect = Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0));
            let nodes = vec![
                SemanticNode::new("root", Role::Window, rect),
                SemanticNode::new("left", Role::Container, rect).with_parent("root", 0),
                SemanticNode::new("nested", Role::Label, rect).with_parent("left", 0),
                SemanticNode::new("right", Role::Container, rect).with_parent("root", 1),
            ];

            assert_eq!(next_child_index(&nodes, "root"), 2);
            assert_eq!(next_child_index(&nodes, "left"), 1);
            assert_eq!(next_child_index(&nodes, "missing"), 0);
        }

        #[test]
        fn save_panel_path_intent_allows_new_file_in_existing_directory() {
            let dir = temp_test_dir("save-intent");
            let path = dir.join("export.txt");

            let intent = save_panel_path_intent(&path).unwrap();

            assert_eq!(
                intent,
                SavePanelPathIntent {
                    directory: dir,
                    file_name: "export.txt".into(),
                    expected: vec![path],
                }
            );
        }

        #[test]
        fn save_panel_path_intent_rejects_missing_directory() {
            let path = temp_test_dir("save-missing-parent")
                .join("missing")
                .join("export.txt");

            let error = save_panel_path_intent(&path).unwrap_err();

            assert!(matches!(
                error,
                AppKitDialogError::MissingRequestedPath(missing) if missing == path
            ));
        }

        #[test]
        fn open_panel_path_intent_requires_existing_paths_in_one_directory() {
            let dir = temp_test_dir("open-intent");
            let first = dir.join("first.txt");
            fs::write(&first, "first").unwrap();

            let intent = open_panel_path_intent(std::slice::from_ref(&first)).unwrap();

            assert_eq!(
                intent,
                OpenPanelPathIntent {
                    directory: dir,
                    expected: vec![first.canonicalize().unwrap()],
                }
            );
        }

        #[test]
        fn open_panel_path_intent_rejects_empty_missing_and_mixed_directories() {
            assert!(matches!(
                open_panel_path_intent(&[]).unwrap_err(),
                AppKitDialogError::UnsupportedLiveSelection(_)
            ));

            let dir = temp_test_dir("open-invalid");
            let missing = dir.join("missing.txt");
            assert!(matches!(
                open_panel_path_intent(std::slice::from_ref(&missing)).unwrap_err(),
                AppKitDialogError::MissingRequestedPath(path) if path == missing
            ));
        }

        #[test]
        fn open_panel_path_intent_rejects_multi_selection_before_live_selection() {
            let dir = temp_test_dir("open-multi-invalid");
            let first = dir.join("first.txt");
            let second = dir.join("second.txt");
            fs::write(&first, "first").unwrap();
            fs::write(&second, "second").unwrap();

            assert!(matches!(
                open_panel_path_intent(&[first.clone(), second]).unwrap_err(),
                AppKitDialogError::UnsupportedLiveSelection(_)
            ));

            let other = temp_test_dir("open-invalid-other");
            let second = other.join("second.txt");
            fs::write(&second, "second").unwrap();
            assert!(matches!(
                open_panel_path_intent(&[first, second]).unwrap_err(),
                AppKitDialogError::UnsupportedLiveSelection(_)
            ));
        }

        fn temp_test_dir(label: &str) -> PathBuf {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("glasscheck-dialog-{label}-{stamp}"));
            fs::create_dir_all(&path).unwrap();
            path
        }
    }

    #[allow(dead_code)]
    fn _assert_no_private_panel_apis(_: MainThreadMarker, _: Duration, _: NSPoint) {
        let _ = (
            NSAlertFirstButtonReturn,
            NSAlertSecondButtonReturn,
            NSAlertThirdButtonReturn,
            NSModalResponseOK,
            NSModalResponseCancel,
            NSArray::<AnyObject>::new(),
            InputSynthesisError::UnsupportedBackend,
        );
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::path::PathBuf;

    /// Native AppKit dialog or panel kind.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AppKitDialogKind {
        Alert,
        OpenPanel,
        SavePanel,
        Panel,
    }

    impl AppKitDialogKind {
        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Alert => "alert",
                Self::OpenPanel => "open_panel",
                Self::SavePanel => "save_panel",
                Self::Panel => "panel",
            }
        }
    }

    /// Query used to discover native AppKit dialogs and panels.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct AppKitDialogQuery;

    impl AppKitDialogQuery {
        #[must_use]
        pub fn alert() -> Self {
            Self
        }

        #[must_use]
        pub fn open_panel() -> Self {
            Self
        }

        #[must_use]
        pub fn save_panel() -> Self {
            Self
        }

        #[must_use]
        pub fn kind(_: AppKitDialogKind) -> Self {
            Self
        }

        #[must_use]
        pub fn title_eq(self, _: impl Into<String>) -> Self {
            self
        }

        #[must_use]
        pub fn title_contains(self, _: impl Into<String>) -> Self {
            self
        }
    }

    /// Errors returned by AppKit dialog helpers.
    #[derive(Debug)]
    pub enum AppKitDialogError {
        MissingSurface,
        NotDialog,
        KindMismatch {
            expected: AppKitDialogKind,
            actual: AppKitDialogKind,
        },
        UnsupportedLiveSelection(&'static str),
        MissingRequestedPath(PathBuf),
        InputUnavailable,
        Resolve(glasscheck_core::RegionResolveError),
        Poll(glasscheck_core::PollError),
        SelectionMismatch {
            expected: Vec<PathBuf>,
            actual: Vec<PathBuf>,
        },
    }

    impl std::fmt::Display for AppKitDialogError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "AppKit dialogs are unavailable on this target")
        }
    }

    impl std::error::Error for AppKitDialogError {}
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{
    attach_dialog_window, cancel_dialog, choose_open_panel_paths, choose_save_panel_path,
    classify_window, click_dialog_button, set_dialog_text, snapshot_dialog_scene,
};

pub use imp::{AppKitDialogError, AppKitDialogKind, AppKitDialogQuery};
