#[cfg(target_os = "linux")]
mod imp {
    use glasscheck_core::{InputDriver, KeyModifiers, Point, TextRange};
    use gtk4::gdk;
    use gtk4::prelude::*;
    use gtk4::{TextView, Widget, Window};

    use crate::screen::present_window_offscreen;

    pub struct GtkInputDriver<'a> {
        window: &'a Window,
    }

    impl<'a> GtkInputDriver<'a> {
        /// Creates an input driver for `window`.
        #[must_use]
        pub fn new(window: &'a Window) -> Self {
            Self { window }
        }

        /// Synthesizes a left click at `point` in root coordinates.
        pub fn click(&self, point: Point) {
            self.activate_window();
            let Some(root) = self.window.child() else {
                return;
            };
            let (x, y) = root_top_left_point(&root, point);
            let Some(target) = root.pick(x, y, gtk4::PickFlags::DEFAULT) else {
                return;
            };
            if let Ok(button) = target.clone().downcast::<gtk4::Button>() {
                button.emit_clicked();
                return;
            }
            if activate_target_chain(&target) {
                return;
            }
            if dispatch_click_to_gestures(&root, &target, x, y) {
                return;
            }
            target.grab_focus();
        }

        /// Synthesizes a mouse move. GTK parity currently only guarantees focus routing.
        pub fn move_mouse(&self, point: Point) {
            self.activate_window();
            let Some(root) = self.window.child() else {
                return;
            };
            let (x, y) = root_top_left_point(&root, point);
            if let Some(target) = root.pick(x, y, gtk4::PickFlags::DEFAULT) {
                target.grab_focus();
            }
        }

        /// Synthesizes a key press using backend-neutral modifiers.
        pub fn key_press(&self, key: &str, modifiers: KeyModifiers) {
            self.activate_window();
            let Some(root) = self.window.child() else {
                return;
            };
            if let Some(focus) = root.root().and_then(|root| root.focus()) {
                if dispatch_key_to_controllers(&focus, key, modifiers) {
                    return;
                }
                if let Ok(text) = focus.downcast::<TextView>() {
                    self.type_text_direct(&text, key);
                }
            }
        }

        /// Inserts `text` directly through the `TextBuffer` API.
        pub fn type_text_direct(&self, view: &TextView, text: &str) {
            let buffer = view.buffer();
            buffer.insert_at_cursor(text);
        }

        /// Replaces the entire text contents of `view`.
        pub fn replace_text(&self, view: &TextView, text: &str) {
            let buffer = view.buffer();
            buffer.set_text(text);
            let iter = buffer.iter_at_offset(text.chars().count() as i32);
            buffer.place_cursor(&iter);
        }

        /// Sets the selected range in `view`.
        pub fn set_selection(&self, view: &TextView, range: TextRange) {
            let buffer = view.buffer();
            let start = buffer.iter_at_offset(range.start as i32);
            let end = buffer.iter_at_offset((range.start + range.len) as i32);
            buffer.select_range(&start, &end);
            view.grab_focus();
        }

        fn activate_window(&self) {
            present_window_offscreen(self.window);
            self.window.grab_focus();
        }
    }

    impl InputDriver for GtkInputDriver<'_> {
        type NativeText = TextView;

        fn click(&self, point: Point) {
            Self::click(self, point);
        }

        fn move_mouse(&self, point: Point) {
            Self::move_mouse(self, point);
        }

        fn key_press(&self, key: &str, modifiers: KeyModifiers) {
            Self::key_press(self, key, modifiers);
        }

        fn type_text_direct(&self, view: &Self::NativeText, text: &str) {
            Self::type_text_direct(self, view, text);
        }

        fn replace_text(&self, view: &Self::NativeText, text: &str) {
            Self::replace_text(self, view, text);
        }

        fn set_selection(&self, view: &Self::NativeText, range: TextRange) {
            Self::set_selection(self, view, range);
        }
    }

    fn root_top_left_point(root: &Widget, point: Point) -> (f64, f64) {
        let height = root.allocated_height().max(1) as f64;
        (point.x, height - point.y)
    }

    fn activate_target_chain(target: &Widget) -> bool {
        let mut current = Some(target.clone());
        while let Some(widget) = current {
            if widget.activate() {
                return true;
            }
            current = widget.parent();
        }
        false
    }

    fn dispatch_click_to_gestures(root: &Widget, target: &Widget, x: f64, y: f64) -> bool {
        let mut current = Some(target.clone());
        let mut fired = false;
        while let Some(widget) = current {
            let Some((local_x, local_y)) = root.translate_coordinates(&widget, x, y) else {
                current = widget.parent();
                continue;
            };
            for object in widget.observe_controllers().snapshot() {
                let Ok(controller) = object.downcast::<gtk4::GestureClick>() else {
                    continue;
                };
                controller.emit_by_name::<()>("pressed", &[&1i32, &local_x, &local_y]);
                controller.emit_by_name::<()>("released", &[&1i32, &local_x, &local_y]);
                fired = true;
            }
            current = widget.parent();
        }
        fired
    }

    fn dispatch_key_to_controllers(widget: &Widget, key: &str, modifiers: KeyModifiers) -> bool {
        let Some(keyval) = gdk::Key::from_name(key) else {
            return false;
        };
        let state = modifier_state(modifiers);
        let mut current = Some(widget.clone());
        let mut handled = false;
        while let Some(candidate) = current {
            for object in candidate.observe_controllers().snapshot() {
                let Ok(controller) = object.downcast::<gtk4::EventControllerKey>() else {
                    continue;
                };
                let propagation =
                    controller.emit_by_name::<bool>("key-pressed", &[&keyval, &0u32, &state]);
                controller.emit_by_name::<()>("key-released", &[&keyval, &0u32, &state]);
                handled = true;
                if propagation {
                    return true;
                }
            }
            current = candidate.parent();
        }
        handled
    }

    fn modifier_state(modifiers: KeyModifiers) -> gdk::ModifierType {
        let mut state = gdk::ModifierType::empty();
        if modifiers.shift {
            state |= gdk::ModifierType::SHIFT_MASK;
        }
        if modifiers.control {
            state |= gdk::ModifierType::CONTROL_MASK;
        }
        if modifiers.alt {
            state |= gdk::ModifierType::ALT_MASK;
        }
        if modifiers.meta {
            state |= gdk::ModifierType::META_MASK | gdk::ModifierType::SUPER_MASK;
        }
        state
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub struct GtkInputDriver<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::GtkInputDriver;
