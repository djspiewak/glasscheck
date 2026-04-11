#[cfg(target_os = "macos")]
mod imp {
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{msg_send, ClassType};
    use objc2_app_kit::{
        NSApplication, NSControl, NSEvent, NSEventModifierFlags, NSEventType, NSTextInputClient,
        NSTextView, NSView, NSWindow,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSString};

    use glasscheck_core::Rect;

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
            self.activate_window();
            let target = self.target_view(point);
            if self.is_control_target(target.as_deref()) {
                if let Some(target) = target.as_deref() {
                    unsafe {
                        let () = msg_send![target, performClick: std::ptr::null::<AnyObject>()];
                    }
                }
                return;
            }
            let window_number = self.window.windowNumber();
            if let Some(target) = target.as_deref() {
                self.click_target(target, point);
            } else if let Some(content) = self.window.contentView() {
                self.click_target(&content, point);
            } else {
                let _ = window_number;
            }
        }

        /// Synthesizes a left click targeted directly at `view`.
        pub fn click_target(&self, view: &NSView, point: NSPoint) {
            self.activate_window();
            let window_number = self.window.windowNumber();
            if let Some(down) =
                NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                    NSEventType::LeftMouseDown,
                    point,
                    NSEventModifierFlags::empty(),
                    0.0,
                    window_number,
                    None,
                    0,
                    1,
                    1.0,
                )
            {
                view.mouseDown(&down);
            }
            if let Some(up) =
                NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
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
                view.mouseUp(&up);
            }
        }

        /// Synthesizes a left click at the center of `rect`.
        pub fn click_rect_center(&self, rect: Rect) {
            self.click(NSPoint::new(
                rect.origin.x + rect.size.width / 2.0,
                rect.origin.y + rect.size.height / 2.0,
            ));
        }

        /// Synthesizes a mouse-move event at `point` in window coordinates.
        pub fn move_mouse(&self, point: NSPoint) {
            self.activate_window();
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
            self.activate_window();
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

        fn activate_window(&self) {
            self.window.makeKeyAndOrderFront(None);
            self.window.makeKeyWindow();
            let mtm =
                MainThreadMarker::new().expect("window activation must run on the main thread");
            let app = NSApplication::sharedApplication(mtm);
            app.activate();
        }

        fn is_control_target(&self, target: Option<&NSView>) -> bool {
            let Some(target) = target else {
                return false;
            };
            unsafe { msg_send![target, isKindOfClass: NSControl::class()] }
        }

        fn target_view(&self, point: NSPoint) -> Option<objc2::rc::Retained<NSView>> {
            let content = self.window.contentView()?;
            let local = content.convertPoint_fromView(point, None);
            content.hitTest(local)
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
