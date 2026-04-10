#[cfg(target_os = "macos")]
mod imp {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Sel};
    use objc2_app_kit::{
        NSEvent, NSEventModifierFlags, NSEventType, NSTextInputClient, NSTextView, NSWindow,
    };
    use objc2_foundation::{NSPoint, NSRange, NSString};

    pub struct AppKitInputDriver<'a> {
        window: &'a NSWindow,
    }

    impl<'a> AppKitInputDriver<'a> {
        /// Creates an input driver for `window`.
        #[must_use]
        pub fn new(window: &'a NSWindow) -> Self {
            Self { window }
        }

        /// Synthesizes a left mouse click at `point` in window coordinates.
        pub fn click(&self, point: NSPoint) {
            let window_number = self.window.windowNumber();
            if let Some(down) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseDown,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) {
                self.window.sendEvent(&down);
            }
            if let Some(up) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) {
                self.window.sendEvent(&up);
            }
        }

        /// Synthesizes a mouse-move event at `point` in window coordinates.
        pub fn move_mouse(&self, point: NSPoint) {
            let window_number = self.window.windowNumber();
            if let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::MouseMoved,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                0,
                0.0,
            ) {
                self.window.sendEvent(&event);
            }
        }

        /// Synthesizes a key press and release with the provided metadata.
        pub fn key_press(&self, key_code: u16, modifiers: NSEventModifierFlags, characters: &str) {
            let chars = NSString::from_str(characters);
            let chars_ignoring = NSString::from_str(characters);
            let point = NSPoint::new(0.0, 0.0);
            let window_number = self.window.windowNumber();

            if let Some(down) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                NSEventType::KeyDown,
                point,
                modifiers,
                0.0,
                window_number,
                None,
                &chars,
                &chars_ignoring,
                false,
                key_code,
            ) {
                self.window.sendEvent(&down);
            }
            if let Some(up) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                NSEventType::KeyUp,
                point,
                modifiers,
                0.0,
                window_number,
                None,
                &chars,
                &chars_ignoring,
                false,
                key_code,
            ) {
                self.window.sendEvent(&up);
            }
        }

        /// Inserts `text` directly through the `NSTextInputClient` API.
        pub fn type_text_direct(&self, view: &NSTextView, text: &str) {
            for ch in text.chars() {
                let position = NSTextInputClient::selectedRange(view).location;
                let string = NSString::from_str(&ch.to_string());
                unsafe {
                    NSTextInputClient::insertText_replacementRange(
                        view,
                        &string,
                        NSRange::new(position, 0),
                    );
                }
            }
        }

        /// Replaces the entire text contents of `view`.
        pub fn replace_text(&self, view: &NSTextView, text: &str) {
            let storage = unsafe { view.textStorage() };
            let length = storage.as_ref().map_or(0, |storage| storage.length());
            view.setSelectedRange(NSRange::new(0, length));
            let text = NSString::from_str(text);
            unsafe {
                NSTextInputClient::insertText_replacementRange(
                    view,
                    &text,
                    NSRange::new(0, length),
                );
            }
        }

        /// Sets the selected range in `view`.
        pub fn set_selection(&self, view: &NSTextView, location: usize, length: usize) {
            view.setSelectedRange(NSRange::new(location, length));
        }

        /// Sends an Objective-C action message to `target`.
        pub fn send_action(&self, target: &AnyObject, action: Sel) {
            unsafe {
                let () = msg_send![target, performSelector: action, withObject: std::ptr::null::<AnyObject>()];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct AppKitInputDriver<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::AppKitInputDriver;
