#[cfg(target_os = "linux")]
mod imp {
    use glasscheck_core::{InputDriver, KeyModifiers, Point, TextRange};
    use gtk4::prelude::*;
    use gtk4::{TextView, Widget, Window};

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
        pub fn key_press(&self, key: &str, _modifiers: KeyModifiers) {
            self.activate_window();
            let Some(root) = self.window.child() else {
                return;
            };
            if let Some(focus) = root.root().and_then(|root| root.focus()) {
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
            self.window.present();
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
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub struct GtkInputDriver<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::GtkInputDriver;
