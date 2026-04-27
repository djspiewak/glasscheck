#[cfg(target_os = "linux")]
mod imp {
    use std::path::{Path, PathBuf};
    use std::thread::sleep;
    use std::time::Instant;

    use glasscheck_core::{
        DialogCapability, DialogError, DialogKind, NodeProvenanceKind, PollError, PollOptions,
        PropertyValue, QueryError, Rect, RegionResolveError, Role, Scene, Selector, SemanticNode,
        SemanticSnapshot, Size,
    };
    use glib::object::{Cast, ObjectExt};
    use gtk4::prelude::*;
    use gtk4::{
        Button, Dialog, Editable, Entry, FileChooserAction, FileChooserDialog, FileChooserNative,
        Label, MessageDialog, ResponseType, Separator, TextView, Widget, Window,
    };

    use crate::{GtkSceneSource, GtkSnapshotContext, GtkWindowHost};

    const DIALOG_ROOT_ID: &str = "gtk.dialog";
    const DIALOG_KIND_PROPERTY: &str = "gtk:dialog_kind";
    const CLASS_PROPERTY: &str = "gtk:class";
    const WIDGET_PATH_PROPERTY: &str = "gtk:widget_path";
    const BUTTON_ROLE_PROPERTY: &str = "gtk:button_role";
    const DIRECTORY_PATH_PROPERTY: &str = "gtk:directory_path";
    const NAME_FIELD_PROPERTY: &str = "gtk:name_field";
    const MESSAGE_PROPERTY: &str = "gtk:message";

    type GtkDialogError = DialogError;

    /// Metadata handle for GTK async dialog objects that do not expose a widget tree.
    ///
    /// Controllers participate in session discovery and `dialog_kind` queries,
    /// but they cannot snapshot or drive live dialog UI because there is no
    /// retained GTK surface. Session operations therefore return
    /// `DialogError::UnsupportedCapability` for every live capability.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct GtkDialogController {
        kind: DialogKind,
        title: String,
    }

    impl GtkDialogController {
        /// Creates a controller record for a GTK async dialog object.
        #[must_use]
        pub fn new(kind: DialogKind, title: impl Into<String>) -> Self {
            Self {
                kind,
                title: title.into(),
            }
        }

        /// Creates a controller record for a `GtkFileChooserNative`.
        #[must_use]
        pub fn file_chooser_native(dialog: &FileChooserNative) -> Self {
            Self {
                kind: file_chooser_action_kind(dialog.action()),
                title: dialog
                    .title()
                    .map_or_else(String::new, |title| title.to_string()),
            }
        }

        /// Returns the controller's dialog kind.
        #[must_use]
        pub fn kind(&self) -> DialogKind {
            self.kind
        }

        /// Returns the controller title used for query matching.
        #[must_use]
        pub fn title(&self) -> &str {
            &self.title
        }

        /// Returns whether this non-surface controller supports `capability`.
        ///
        /// This currently returns `false` for every live capability.
        #[must_use]
        pub fn supports(&self, capability: DialogCapability) -> bool {
            let _ = capability;
            false
        }
    }

    pub(crate) fn attach_dialog_window(window: &Window) -> Result<GtkWindowHost, GtkDialogError> {
        let kind = classify_window(window).ok_or(GtkDialogError::NotDialog)?;
        let host = GtkWindowHost::from_window(window);
        host.set_contextual_scene_source(Box::new(GtkDialogSceneSource { kind }));
        Ok(host)
    }

    pub(crate) fn classify_window(window: &Window) -> Option<DialogKind> {
        if let Ok(dialog) = window.clone().downcast::<FileChooserDialog>() {
            Some(file_chooser_action_kind(dialog.action()))
        } else if window.clone().downcast::<MessageDialog>().is_ok() {
            Some(DialogKind::Alert)
        } else if window.clone().downcast::<Dialog>().is_ok() {
            Some(DialogKind::Panel)
        } else {
            None
        }
    }

    pub(crate) fn dialog_title(window: &Window, kind: DialogKind) -> String {
        if let Some(title) = window.title() {
            if !title.is_empty() {
                return title.to_string();
            }
        }
        if kind == DialogKind::Alert {
            if let Ok(dialog) = window.clone().downcast::<MessageDialog>() {
                if let Some(text) = dialog.text() {
                    if !text.is_empty() {
                        return text.to_string();
                    }
                }
            }
        }
        String::new()
    }

    pub(crate) fn snapshot_dialog_scene(host: &GtkWindowHost) -> Result<Scene, GtkDialogError> {
        let kind = classify_window(host.window()).ok_or(GtkDialogError::NotDialog)?;
        Ok(Scene::new(dialog_snapshot(host, kind).nodes))
    }

    pub(crate) fn click_dialog_button(
        host: &GtkWindowHost,
        predicate: &Selector,
    ) -> Result<(), GtkDialogError> {
        let scene = snapshot_dialog_scene(host)?;
        let handle = scene.find(predicate).map_err(map_query_error)?;
        let node = scene.node(handle).ok_or(GtkDialogError::InputUnavailable)?;
        if node.role != Role::Button {
            return Err(GtkDialogError::InputUnavailable);
        }
        if !node.visible || !node.hit_testable {
            return Err(GtkDialogError::InputUnavailable);
        }
        let path = node
            .properties
            .get(WIDGET_PATH_PROPERTY)
            .and_then(property_string)
            .ok_or(GtkDialogError::InputUnavailable)?;
        let widget = widget_at_path(host, path).ok_or(GtkDialogError::InputUnavailable)?;
        let button = widget
            .downcast::<Button>()
            .map_err(|_| GtkDialogError::InputUnavailable)?;
        if !button.is_visible() || !button.is_sensitive() {
            return Err(GtkDialogError::InputUnavailable);
        }
        button.emit_clicked();
        Ok(())
    }

    pub(crate) fn set_dialog_text(
        host: &GtkWindowHost,
        predicate: &Selector,
        text: &str,
    ) -> Result<(), GtkDialogError> {
        let scene = snapshot_dialog_scene(host)?;
        let handle = scene.find(predicate).map_err(map_query_error)?;
        let node = scene.node(handle).ok_or(GtkDialogError::InputUnavailable)?;
        if node.role != Role::TextInput {
            return Err(GtkDialogError::InputUnavailable);
        }
        if !node.visible || !node.hit_testable {
            return Err(GtkDialogError::InputUnavailable);
        }
        let path = node
            .properties
            .get(WIDGET_PATH_PROPERTY)
            .and_then(property_string)
            .ok_or(GtkDialogError::InputUnavailable)?;
        let widget = widget_at_path(host, path).ok_or(GtkDialogError::InputUnavailable)?;
        if !widget.is_visible() || !widget.is_sensitive() {
            return Err(GtkDialogError::InputUnavailable);
        }
        if let Ok(editable) = widget.clone().downcast::<Editable>() {
            if !editable.is_editable() {
                return Err(GtkDialogError::InputUnavailable);
            }
            editable.set_text(text);
            return Ok(());
        }
        if let Ok(text_view) = widget.downcast::<TextView>() {
            if !text_view.is_editable() {
                return Err(GtkDialogError::InputUnavailable);
            }
            text_view.buffer().set_text(text);
            return Ok(());
        }
        Err(GtkDialogError::InputUnavailable)
    }

    pub(crate) fn choose_save_dialog_path(
        host: &GtkWindowHost,
        path: &Path,
        options: PollOptions,
    ) -> Result<usize, GtkDialogError> {
        let kind = classify_window(host.window()).ok_or(GtkDialogError::NotDialog)?;
        if kind != DialogKind::SavePanel {
            return Err(GtkDialogError::KindMismatch {
                expected: DialogKind::SavePanel,
                actual: kind,
            });
        }
        let dialog = host
            .window()
            .clone()
            .downcast::<FileChooserDialog>()
            .map_err(|_| GtkDialogError::NotDialog)?;
        if dialog.action() != FileChooserAction::Save {
            return Err(GtkDialogError::UnsupportedCapability(
                DialogCapability::SavePathSelection,
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| GtkDialogError::MissingRequestedPath(PathBuf::from(path)))?;
        if !parent.is_dir() {
            return Err(GtkDialogError::MissingRequestedPath(PathBuf::from(path)));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| GtkDialogError::MissingRequestedPath(PathBuf::from(path)))?;
        dialog
            .set_current_folder(Some(&gtk4::gio::File::for_path(parent)))
            .map_err(|_| GtkDialogError::InputUnavailable)?;
        dialog.set_current_name(file_name);
        let expected = vec![path.to_path_buf()];
        wait_for_file_chooser_selection(
            options,
            || save_dialog_reflects_path(&dialog, parent, file_name),
            || save_dialog_path(&dialog),
            expected,
        )
    }

    pub(crate) fn choose_open_dialog_paths(
        host: &GtkWindowHost,
        paths: &[PathBuf],
        options: PollOptions,
    ) -> Result<usize, GtkDialogError> {
        let kind = classify_window(host.window()).ok_or(GtkDialogError::NotDialog)?;
        if kind != DialogKind::OpenPanel {
            return Err(GtkDialogError::KindMismatch {
                expected: DialogKind::OpenPanel,
                actual: kind,
            });
        }
        let dialog = host
            .window()
            .clone()
            .downcast::<FileChooserDialog>()
            .map_err(|_| GtkDialogError::NotDialog)?;
        if dialog.action() != FileChooserAction::Open {
            return Err(GtkDialogError::UnsupportedCapability(
                DialogCapability::OpenPathSelection,
            ));
        }
        if paths.len() != 1 {
            return Err(GtkDialogError::UnsupportedLiveSelection(
                "GTK open dialog path selection currently supports exactly one path",
            ));
        }
        let path = &paths[0];
        if !path.exists() {
            return Err(GtkDialogError::MissingRequestedPath(path.clone()));
        }
        dialog
            .set_file(&gtk4::gio::File::for_path(path))
            .map_err(|_| GtkDialogError::InputUnavailable)?;
        let expected = vec![path.clone()];
        wait_for_file_chooser_selection(
            options,
            || {
                let actual = open_dialog_paths(&dialog);
                actual.len() == 1 && actual.first() == Some(path)
            },
            || open_dialog_paths(&dialog),
            expected,
        )
    }

    pub(crate) fn cancel_dialog(
        host: &GtkWindowHost,
        options: PollOptions,
    ) -> Result<usize, GtkDialogError> {
        let window = host.window().clone();
        window.close();
        wait_for_window_closed(&window, options).map_err(GtkDialogError::Poll)
    }

    fn wait_for_window_closed(window: &Window, options: PollOptions) -> Result<usize, PollError> {
        let started = Instant::now();
        let mut attempts = 0usize;
        loop {
            attempts += 1;
            crate::menu::flush_main_context(1);
            if !window.is_visible() {
                return Ok(attempts);
            }
            if started.elapsed() >= options.timeout {
                return Err(PollError::Timeout {
                    elapsed: started.elapsed(),
                    attempts,
                });
            }
            let remaining = options.timeout.saturating_sub(started.elapsed());
            sleep(remaining.min(options.interval));
        }
    }

    fn wait_for_file_chooser_selection<Reflects, Actual>(
        options: PollOptions,
        mut reflects: Reflects,
        mut actual: Actual,
        expected: Vec<PathBuf>,
    ) -> Result<usize, GtkDialogError>
    where
        Reflects: FnMut() -> bool,
        Actual: FnMut() -> Vec<PathBuf>,
    {
        let started = Instant::now();
        let mut attempts = 0usize;
        loop {
            attempts += 1;
            crate::menu::flush_main_context(1);
            if reflects() {
                let actual = actual();
                return if actual == expected {
                    Ok(attempts)
                } else {
                    Err(GtkDialogError::SelectionMismatch { expected, actual })
                };
            }
            if started.elapsed() >= options.timeout {
                let actual = actual();
                return if actual.is_empty() {
                    Err(GtkDialogError::Poll(PollError::Timeout {
                        elapsed: started.elapsed(),
                        attempts,
                    }))
                } else {
                    Err(GtkDialogError::SelectionMismatch { expected, actual })
                };
            }
            let remaining = options.timeout.saturating_sub(started.elapsed());
            sleep(remaining.min(options.interval));
        }
    }

    fn save_dialog_reflects_path(
        dialog: &FileChooserDialog,
        parent: &Path,
        file_name: &str,
    ) -> bool {
        dialog
            .current_folder()
            .and_then(|file| file.path())
            .as_deref()
            == Some(parent)
            && dialog
                .current_name()
                .is_some_and(|name| name.as_str() == file_name)
    }

    fn save_dialog_path(dialog: &FileChooserDialog) -> Vec<PathBuf> {
        let folder = dialog.current_folder().and_then(|file| file.path());
        let name = dialog.current_name().map(|name| name.to_string());
        match (folder, name) {
            (Some(folder), Some(name)) if !name.is_empty() => vec![folder.join(name)],
            (Some(folder), _) => vec![folder],
            _ => Vec::new(),
        }
    }

    fn open_dialog_paths(dialog: &FileChooserDialog) -> Vec<PathBuf> {
        dialog
            .file()
            .and_then(|file| file.path())
            .into_iter()
            .collect()
    }

    pub(crate) fn dialog_snapshot(host: &GtkWindowHost, kind: DialogKind) -> SemanticSnapshot {
        let root_bounds = host.root_bounds();
        let mut root = SemanticNode::new(DIALOG_ROOT_ID, Role::Window, root_bounds)
            .with_selector("gtk.dialog")
            .with_selector(format!("gtk.dialog.{}", kind.as_str()))
            .with_property(DIALOG_KIND_PROPERTY, PropertyValue::string(kind.as_str()))
            .with_property(
                CLASS_PROPERTY,
                PropertyValue::string(class_name(host.window().upcast_ref::<Widget>())),
            )
            .with_provenance(NodeProvenanceKind::Native);
        if let Some(title) = host.window().title() {
            if !title.is_empty() {
                root.label = Some(title.to_string());
            }
        }
        add_dialog_properties(host.window(), kind, &mut root);
        let mut nodes = vec![root];
        if let Some(root_widget) = host.root_widget() {
            let path = "0".to_string();
            collect_widget_nodes(host, &root_widget, DIALOG_ROOT_ID, path, 0, &mut nodes);
        }
        SemanticSnapshot::from_nodes(nodes)
    }

    fn add_dialog_properties(window: &Window, kind: DialogKind, root: &mut SemanticNode) {
        if kind == DialogKind::Alert {
            if let Ok(dialog) = window.clone().downcast::<MessageDialog>() {
                if let Some(text) = dialog.text() {
                    root.properties.insert(
                        MESSAGE_PROPERTY.into(),
                        PropertyValue::string(text.to_string()),
                    );
                    root.property_provenance
                        .insert(MESSAGE_PROPERTY.into(), NodeProvenanceKind::Native);
                }
            }
        }
        if kind == DialogKind::SavePanel || kind == DialogKind::OpenPanel {
            if let Ok(dialog) = window.clone().downcast::<FileChooserDialog>() {
                if let Some(file) = dialog.current_folder() {
                    if let Some(path) = file.path() {
                        root.properties.insert(
                            DIRECTORY_PATH_PROPERTY.into(),
                            PropertyValue::string(path.display().to_string()),
                        );
                        root.property_provenance
                            .insert(DIRECTORY_PATH_PROPERTY.into(), NodeProvenanceKind::Native);
                    }
                }
            }
        }
    }

    fn collect_widget_nodes(
        host: &GtkWindowHost,
        widget: &Widget,
        parent_id: &str,
        path: String,
        child_index: usize,
        nodes: &mut Vec<SemanticNode>,
    ) {
        let id = format!("{DIALOG_ROOT_ID}.widget.{path}");
        let rect = widget_rect(host, widget).unwrap_or_else(|| {
            Rect::new(glasscheck_core::Point::new(0.0, 0.0), Size::new(0.0, 0.0))
        });
        let mut node = SemanticNode::new(id.clone(), infer_role(widget), rect)
            .with_parent(parent_id, child_index)
            .with_property(WIDGET_PATH_PROPERTY, PropertyValue::string(path.clone()))
            .with_property(CLASS_PROPERTY, PropertyValue::string(class_name(widget)))
            .with_provenance(NodeProvenanceKind::Native);
        node.visible = widget.is_visible();
        node.visible_rect = if node.visible { Some(rect) } else { None };
        node.hit_testable = node.visible && widget.is_sensitive();
        if let Some(label) = widget_label(widget) {
            node.label = Some(label.clone());
            if let Some(selector) = usable_selector_fragment(&label) {
                node.selectors
                    .insert(format!("gtk.dialog.label.{selector}"));
            }
        }
        if let Some(value) = widget_value(widget) {
            node.value = Some(value);
        }
        if let Ok(button) = widget.clone().downcast::<Button>() {
            node.properties.insert(
                BUTTON_ROLE_PROPERTY.into(),
                PropertyValue::string(button_role(&button)),
            );
            node.property_provenance
                .insert(BUTTON_ROLE_PROPERTY.into(), NodeProvenanceKind::Native);
        }
        if let Ok(entry) = widget.clone().downcast::<Entry>() {
            node.properties.insert(
                NAME_FIELD_PROPERTY.into(),
                PropertyValue::string(entry.text().to_string()),
            );
            node.property_provenance
                .insert(NAME_FIELD_PROPERTY.into(), NodeProvenanceKind::Native);
        }
        nodes.push(node);

        let mut next_index = 0usize;
        let mut child = widget.first_child();
        while let Some(current) = child {
            let next = current.next_sibling();
            let child_path = format!("{path}.{next_index}");
            collect_widget_nodes(host, &current, &id, child_path, next_index, nodes);
            next_index += 1;
            child = next;
        }
    }

    fn widget_at_path(host: &GtkWindowHost, path: &str) -> Option<Widget> {
        let mut segments = path.split('.');
        let first = segments.next()?;
        if first != "0" {
            return None;
        }
        let mut current = host.root_widget()?;
        for segment in segments {
            let target = segment.parse::<usize>().ok()?;
            let mut index = 0usize;
            let mut child = current.first_child();
            loop {
                let candidate = child?;
                if index == target {
                    current = candidate;
                    break;
                }
                index += 1;
                child = candidate.next_sibling();
            }
        }
        Some(current)
    }

    fn infer_role(widget: &Widget) -> Role {
        if widget.clone().downcast::<Button>().is_ok() {
            Role::Button
        } else if widget.clone().downcast::<Editable>().is_ok()
            || widget.clone().downcast::<TextView>().is_ok()
        {
            Role::TextInput
        } else if widget.clone().downcast::<Label>().is_ok() {
            Role::Label
        } else if widget.clone().downcast::<Separator>().is_ok() {
            Role::Divider
        } else {
            Role::Container
        }
    }

    fn widget_label(widget: &Widget) -> Option<String> {
        if let Ok(button) = widget.clone().downcast::<Button>() {
            return button.label().map(|label| label.to_string());
        }
        if let Ok(label) = widget.clone().downcast::<Label>() {
            let value = label.text().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
        None
    }

    fn widget_value(widget: &Widget) -> Option<String> {
        if let Ok(editable) = widget.clone().downcast::<Editable>() {
            return Some(editable.text().to_string());
        }
        if let Ok(text_view) = widget.clone().downcast::<TextView>() {
            let buffer = text_view.buffer();
            let (start, end) = buffer.bounds();
            return Some(buffer.text(&start, &end, true).to_string());
        }
        None
    }

    fn button_role(button: &Button) -> &'static str {
        let Some(dialog) = button
            .root()
            .and_then(|root| root.downcast::<Dialog>().ok())
            .or_else(|| {
                button
                    .ancestor(Dialog::static_type())
                    .and_then(|widget| widget.downcast::<Dialog>().ok())
            })
        else {
            return "other";
        };
        response_button_role(dialog.response_for_widget(button))
    }

    fn response_button_role(response: ResponseType) -> &'static str {
        match response {
            ResponseType::Accept | ResponseType::Ok | ResponseType::Yes | ResponseType::Apply => {
                "confirm"
            }
            ResponseType::Reject
            | ResponseType::Cancel
            | ResponseType::Close
            | ResponseType::DeleteEvent
            | ResponseType::No => "cancel",
            ResponseType::None | ResponseType::Help | ResponseType::Other(_) => "other",
            ResponseType::__Unknown(_) => "other",
        }
    }

    fn file_chooser_action_kind(action: FileChooserAction) -> DialogKind {
        match action {
            FileChooserAction::Save => DialogKind::SavePanel,
            _ => DialogKind::OpenPanel,
        }
    }

    fn class_name(widget: &Widget) -> String {
        widget.type_().name().to_string()
    }

    fn property_string(value: &PropertyValue) -> Option<&str> {
        match value {
            PropertyValue::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    fn widget_rect(host: &GtkWindowHost, widget: &Widget) -> Option<Rect> {
        let root = host.root_widget()?;
        let bounds = widget.compute_bounds(&root)?;
        let width = widget.allocated_width() as f64;
        let height = widget.allocated_height() as f64;
        let root_height = root.allocated_height().max(1) as f64;
        Some(Rect::new(
            glasscheck_core::Point::new(
                bounds.x() as f64,
                root_height - bounds.y() as f64 - height,
            ),
            Size::new(width.max(1.0), height.max(1.0)),
        ))
    }

    fn map_query_error(error: QueryError) -> GtkDialogError {
        match error {
            QueryError::NotFound(selector) => {
                GtkDialogError::Resolve(RegionResolveError::NotFound(selector))
            }
            QueryError::MultipleMatches { selector, count } => {
                GtkDialogError::Resolve(RegionResolveError::MultipleMatches {
                    predicate: selector,
                    count,
                })
            }
        }
    }

    fn usable_selector_fragment(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > 120 || trimmed.contains('\n') {
            return None;
        }
        let mut out = String::new();
        let mut last_dot = false;
        for ch in trimmed.chars().flat_map(char::to_lowercase) {
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
                last_dot = false;
            } else if !last_dot {
                out.push('.');
                last_dot = true;
            }
        }
        while out.ends_with('.') {
            out.pop();
        }
        (!out.is_empty()).then_some(out)
    }

    struct GtkDialogSceneSource {
        kind: DialogKind,
    }

    impl GtkSceneSource for GtkDialogSceneSource {
        fn snapshot(&self, context: &GtkSnapshotContext<'_>) -> SemanticSnapshot {
            dialog_snapshot(context.host(), self.kind)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn selector_fragment_normalizes_visible_text() {
            assert_eq!(
                usable_selector_fragment("  Save As...  "),
                Some("save.as".into())
            );
            assert!(usable_selector_fragment("").is_none());
            assert!(usable_selector_fragment(&"x".repeat(121)).is_none());
        }

        #[test]
        fn async_controller_reports_capability_boundary() {
            let controller = GtkDialogController {
                kind: DialogKind::Alert,
                title: "Message".into(),
            };
            assert_eq!(controller.kind(), DialogKind::Alert);
            assert!(!controller.supports(DialogCapability::Cancel));
            assert!(!controller.supports(DialogCapability::SceneSnapshot));
        }

        #[test]
        fn response_button_role_classifies_standard_response_types() {
            assert_eq!(response_button_role(ResponseType::Ok), "confirm");
            assert_eq!(response_button_role(ResponseType::Accept), "confirm");
            assert_eq!(response_button_role(ResponseType::Yes), "confirm");
            assert_eq!(response_button_role(ResponseType::Apply), "confirm");
            assert_eq!(response_button_role(ResponseType::Cancel), "cancel");
            assert_eq!(response_button_role(ResponseType::Reject), "cancel");
            assert_eq!(response_button_role(ResponseType::Close), "cancel");
            assert_eq!(response_button_role(ResponseType::DeleteEvent), "cancel");
            assert_eq!(response_button_role(ResponseType::No), "cancel");
            assert_eq!(response_button_role(ResponseType::Help), "other");
            assert_eq!(response_button_role(ResponseType::Other(42)), "other");
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct GtkDialogController;
}

#[cfg(target_os = "linux")]
pub(crate) use imp::{
    attach_dialog_window, cancel_dialog, choose_open_dialog_paths, choose_save_dialog_path,
    classify_window, click_dialog_button, dialog_title, set_dialog_text, snapshot_dialog_scene,
};

pub use imp::GtkDialogController;
