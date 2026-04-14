#[cfg(target_os = "macos")]
mod imp {
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{msg_send, ClassType};
    use objc2_app_kit::{
        NSApplication, NSControl, NSEvent, NSEventModifierFlags, NSEventType, NSTextInputClient,
        NSTextView, NSView, NSWindow,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSString};

    use glasscheck_core::{InputDriver, InputSynthesisError, KeyModifiers, Point, Rect, TextRange};

    pub struct AppKitInputDriver<'a> {
        window: &'a NSWindow,
        mtm: MainThreadMarker,
    }

    impl<'a> AppKitInputDriver<'a> {
        /// Creates an input driver for `window`.
        #[must_use]
        pub fn new(window: &'a NSWindow, mtm: MainThreadMarker) -> Self {
            Self { window, mtm }
        }

        /// Synthesizes a left mouse click at `point` in window coordinates.
        pub fn click(&self, point: Point) -> Result<(), InputSynthesisError> {
            self.activate_window();
            let point = ns_point(point);
            let target = self.target_view(point);
            if self.is_control_target(target.as_deref()) {
                if let Some(target) = target.as_deref() {
                    unsafe {
                        let () = msg_send![target, performClick: std::ptr::null::<AnyObject>()];
                    }
                }
                return Ok(());
            }
            if let Some(target) = target.as_deref() {
                self.click_target(target, point)?;
            } else if let Some(content) = self.window.contentView() {
                self.click_target(&content, point)?;
            } else {
                return Err(InputSynthesisError::MissingTarget);
            }
            Ok(())
        }

        /// Synthesizes a left click targeted directly at `view`.
        pub fn click_target(
            &self,
            view: &NSView,
            point: NSPoint,
        ) -> Result<(), InputSynthesisError> {
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
            } else {
                return Err(InputSynthesisError::TransportFailure("mouse-down event creation"));
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
            } else {
                return Err(InputSynthesisError::TransportFailure("mouse-up event creation"));
            }
            Ok(())
        }

        /// Synthesizes a left click at the center of `rect`.
        pub fn click_rect_center(&self, rect: Rect) -> Result<(), InputSynthesisError> {
            self.click(Point::new(
                rect.origin.x + rect.size.width / 2.0,
                rect.origin.y + rect.size.height / 2.0,
            ))
        }

        /// Synthesizes a mouse-move event at `point` in window coordinates.
        pub fn move_mouse(&self, point: Point) -> Result<(), InputSynthesisError> {
            self.activate_window();
            let point = ns_point(point);
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
                Ok(())
            } else {
                Err(InputSynthesisError::TransportFailure("mouse-move event creation"))
            }
        }

        /// Synthesizes a key press and release with the provided metadata.
        pub fn key_press_raw(
            &self,
            key_code: u16,
            modifiers: NSEventModifierFlags,
            characters: &str,
        ) -> Result<(), InputSynthesisError> {
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
            } else {
                return Err(InputSynthesisError::TransportFailure("key-down event creation"));
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
            } else {
                return Err(InputSynthesisError::TransportFailure("key-up event creation"));
            }
            Ok(())
        }

        /// Synthesizes a key press and release using backend-neutral modifiers.
        pub fn key_press(
            &self,
            characters: &str,
            modifiers: KeyModifiers,
        ) -> Result<(), InputSynthesisError> {
            self.key_press_raw(0, ns_modifiers(modifiers), characters)
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
        pub fn set_selection(&self, view: &NSTextView, range: TextRange) {
            view.setSelectedRange(ns_range_for_scalar_range(view, range));
        }

        /// Sends an Objective-C action message to `target`.
        pub fn send_action(&self, target: &AnyObject, action: Sel) {
            unsafe {
                let () = msg_send![target, performSelector: action, withObject: std::ptr::null::<AnyObject>()];
            }
        }

        fn activate_window(&self) {
            let app = NSApplication::sharedApplication(self.mtm);
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

    impl InputDriver for AppKitInputDriver<'_> {
        type NativeText = NSTextView;

        fn click(&self, point: Point) -> Result<(), InputSynthesisError> {
            Self::click(self, point)
        }

        fn move_mouse(&self, point: Point) -> Result<(), InputSynthesisError> {
            Self::move_mouse(self, point)
        }

        fn key_press(&self, key: &str, modifiers: KeyModifiers) -> Result<(), InputSynthesisError> {
            Self::key_press(self, key, modifiers)
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

    fn ns_point(point: Point) -> NSPoint {
        NSPoint::new(point.x, point.y)
    }

    fn ns_modifiers(modifiers: KeyModifiers) -> NSEventModifierFlags {
        let mut flags = NSEventModifierFlags::empty();
        if modifiers.shift {
            flags |= NSEventModifierFlags::Shift;
        }
        if modifiers.control {
            flags |= NSEventModifierFlags::Control;
        }
        if modifiers.alt {
            flags |= NSEventModifierFlags::Option;
        }
        if modifiers.meta {
            flags |= NSEventModifierFlags::Command;
        }
        flags
    }

    fn scalar_index_to_utf16_offset(text: &str, scalar_index: usize) -> usize {
        text.chars()
            .take(scalar_index)
            .map(char::len_utf16)
            .sum::<usize>()
    }

    fn ns_range_for_scalar_range(view: &NSTextView, range: TextRange) -> NSRange {
        let content = view.string().to_string();
        let start = scalar_index_to_utf16_offset(&content, range.start);
        let end = scalar_index_to_utf16_offset(&content, range.start + range.len);
        NSRange::new(start, end.saturating_sub(start))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct AppKitInputDriver<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::AppKitInputDriver;
