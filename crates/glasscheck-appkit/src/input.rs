#[cfg(target_os = "macos")]
mod imp {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{msg_send, ClassType};
    use objc2_app_kit::{
        NSApplication, NSControl, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSMenu,
        NSTextInputClient, NSTextView, NSTrackingAreaOptions, NSView, NSWindow,
    };
    use objc2_foundation::{
        MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSPoint, NSRange, NSRunLoop, NSString,
    };

    use glasscheck_core::{InputDriver, InputSynthesisError, KeyModifiers, Point, Rect, TextRange};

    pub struct AppKitInputDriver<'a> {
        window: &'a NSWindow,
        mtm: MainThreadMarker,
        attached_child_window: bool,
    }

    #[derive(Clone, Copy)]
    enum ContextClickGesture {
        Secondary,
        ControlPrimary,
    }

    impl<'a> AppKitInputDriver<'a> {
        /// Creates an input driver for `window`.
        #[must_use]
        pub fn new(
            window: &'a NSWindow,
            mtm: MainThreadMarker,
            attached_child_window: bool,
        ) -> Self {
            Self {
                window,
                mtm,
                attached_child_window,
            }
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
            let window_number = self.window.windowNumber().max(0);
            let Some(down) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseDown,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-down event creation"));
            };
            view.mouseDown(&down);
            let Some(up) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-up event creation"));
            };
            view.mouseUp(&up);
            Ok(())
        }

        /// Resolves the native AppKit context menu at `point` in window coordinates.
        ///
        /// AppKit treats secondary click and Control-primary click as equivalent
        /// contextual intent, so this method keeps that platform detail behind
        /// the backend-specific `context_click` API.
        pub fn context_click(
            &self,
            point: Point,
        ) -> Result<Option<Retained<NSMenu>>, InputSynthesisError> {
            self.activate_window();
            let point = ns_point(point);
            let target = self
                .target_view(point)
                .or_else(|| self.window.contentView())
                .ok_or(InputSynthesisError::MissingTarget)?;
            self.context_click_target(&target, point)
        }

        /// Resolves the native AppKit context menu for `view` at `point`.
        pub fn context_click_target(
            &self,
            view: &NSView,
            point: NSPoint,
        ) -> Result<Option<Retained<NSMenu>>, InputSynthesisError> {
            self.activate_window();
            for gesture in [
                ContextClickGesture::Secondary,
                ContextClickGesture::ControlPrimary,
            ] {
                let event = self.context_click_event(gesture, point)?;
                if let Some(menu) = view.menuForEvent(&event) {
                    view.willOpenMenu_withEvent(&menu, &event);
                    menu.update();
                    return Ok(Some(menu));
                }
            }
            Ok(None)
        }

        /// Synthesizes a click that preserves NSApplication local mouse-up monitors.
        pub fn click_window_point_with_local_mouse_up_monitor(
            &self,
            point: Point,
        ) -> Result<(), InputSynthesisError> {
            self.activate_window();
            let point = ns_point(point);
            let target = self
                .target_view(point)
                .or_else(|| self.window.contentView())
                .ok_or(InputSynthesisError::MissingTarget)?;
            if let Some(text_view) = self.text_view_target(&target) {
                return self
                    .click_text_view_with_posted_mouse_up(text_view, Point::new(point.x, point.y));
            }
            let window_number = self.window.windowNumber().max(0);
            let Some(down) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseDown,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-down event creation"));
            };
            target.mouseDown(&down);
            let Some(up) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-up event creation"));
            };
            self.app().sendEvent(&up);
            // Hidden background test windows let local monitors observe the mouse-up,
            // but AppKit does not reliably route that event back into generic views.
            if !self.window.isVisible() {
                target.mouseUp(&up);
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

        /// Synthesizes a left click at `point` by routing through the window event path.
        pub fn click_window_point(&self, point: Point) -> Result<(), InputSynthesisError> {
            self.activate_window();
            let point = ns_point(point);
            self.dispatch_window_mouse_click(point)
        }

        /// Synthesizes a mouse-move event at `point` in window coordinates.
        pub fn move_mouse(&self, point: Point) -> Result<(), InputSynthesisError> {
            self.activate_window();
            let accepts_mouse_moved_events = self.window.acceptsMouseMovedEvents();
            self.window.setAcceptsMouseMovedEvents(true);
            let result = self.dispatch_mouse_move(
                NSEventType::MouseMoved,
                ns_point(point),
                NSEventModifierFlags::empty(),
                0,
                0.0,
            );
            self.window
                .setAcceptsMouseMovedEvents(accepts_mouse_moved_events);
            result
        }

        /// Synthesizes a key press and release with the provided metadata.
        ///
        /// Dispatches directly into the window's responder chain via `sendEvent`,
        /// bypassing local event monitors. Use [`key_press_raw_queued`] when the
        /// event must flow through monitors first.
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
        ///
        /// Bypasses local event monitors; use [`key_press_queued`] when the event
        /// must be observed by application-level monitors before the responder chain.
        pub fn key_press(
            &self,
            characters: &str,
            modifiers: KeyModifiers,
        ) -> Result<(), InputSynthesisError> {
            self.key_press_raw(0, ns_modifiers(modifiers), characters)
        }

        /// Synthesizes a key press and release via the application event queue,
        /// so the event flows through local event monitors before the responder chain.
        ///
        /// Posts both `KeyDown` and `KeyUp` events via `postEvent_atStart` and then
        /// drains them through `nextEventMatchingMask` so that any registered local
        /// event monitors observe them before the first responder receives the event.
        /// The caller must ensure the desired first responder is set on the window
        /// (via `window.makeFirstResponder`) before invoking this method.
        /// Use [`key_press_raw`] for direct responder-chain delivery without monitor
        /// involvement.
        pub fn key_press_raw_queued(
            &self,
            key_code: u16,
            modifiers: NSEventModifierFlags,
            characters: &str,
        ) -> Result<(), InputSynthesisError> {
            let chars = NSString::from_str(characters);
            let chars_ignoring = NSString::from_str(characters);
            let point = NSPoint::new(0.0, 0.0);
            self.activate_window();
            let window_number = self.window.windowNumber().max(0);

            let down = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
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
            )
            .ok_or(InputSynthesisError::TransportFailure("key-down event creation"))?;

            let up = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
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
            )
            .ok_or(InputSynthesisError::TransportFailure("key-up event creation"))?;

            // Post both events then drain them through nextEventMatchingMask so that
            // application-level local event monitors observe them before the responder chain.
            self.app().postEvent_atStart(&down, false);
            self.app().postEvent_atStart(&up, false);
            self.drain_queued_key_events();
            Ok(())
        }

        /// Synthesizes a queued key press using backend-neutral modifiers.
        ///
        /// The event passes through local event monitors before reaching the first
        /// responder. Use [`key_press`] for direct responder-chain delivery when
        /// monitor observation is not needed.
        pub fn key_press_queued(
            &self,
            characters: &str,
            modifiers: KeyModifiers,
        ) -> Result<(), InputSynthesisError> {
            self.key_press_raw_queued(0, ns_modifiers(modifiers), characters)
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

        /// Synthesizes a left click targeted at a text view's insertion point.
        pub fn click_text_view(
            &self,
            view: &NSTextView,
            point: Point,
        ) -> Result<(), InputSynthesisError> {
            self.activate_window();
            self.window.makeFirstResponder(Some(view));
            let point = ns_point(point);
            let window_number = self.window.windowNumber().max(0);
            let Some(down) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseDown,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-down event creation"));
            };
            let Some(up) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-up event creation"));
            };
            // Child windows may still be tracking-eligible; post at front so NSTextView's
            // tracking loop dequeues the up event before any becomeFirstResponder events.
            let at_start = self.attached_child_window || self.window.parentWindow().is_some();
            self.app().postEvent_atStart(&up, at_start);
            view.mouseDown(&down);
            self.drain_run_loop();
            Ok(())
        }

        fn click_text_view_with_posted_mouse_up(
            &self,
            view: &NSTextView,
            point: Point,
        ) -> Result<(), InputSynthesisError> {
            self.activate_window();
            self.window.makeFirstResponder(Some(view));
            self.click_text_view_with_posted_mouse_up_impl(view, point)
        }

        fn click_text_view_with_posted_mouse_up_impl(
            &self,
            view: &NSTextView,
            point: Point,
        ) -> Result<(), InputSynthesisError> {
            let point = ns_point(point);
            let window_number = self.window.windowNumber().max(0);
            let Some(down) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseDown,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-down event creation"));
            };
            let Some(up) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                point,
                NSEventModifierFlags::empty(),
                0.0,
                window_number,
                None,
                0,
                1,
                1.0,
            ) else {
                return Err(InputSynthesisError::TransportFailure("mouse-up event creation"));
            };
            // Child windows may remain tracking-eligible (isVisible-like) even when
            // ordered-out, so NSTextView's tracking loop blocks waiting for mouse-up.
            // Post the up event at the front of the queue (atStart: true) before
            // calling mouseDown, so the tracking loop dequeues it before any events
            // that becomeFirstResponder may have appended. Plain ordered-out windows
            // without a parent don't have this problem and continue to use sendEvent.
            if self.window.parentWindow().is_some() {
                self.app().postEvent_atStart(&up, true);
                view.mouseDown(&down);
                self.drain_run_loop();
                return Ok(());
            }
            view.mouseDown(&down);
            self.app().sendEvent(&up);
            // Hidden background test windows let local monitors observe the mouse-up,
            // but AppKit does not reliably route that event back into the text view.
            if !self.window.isVisible() {
                view.mouseUp(&up);
            }
            Ok(())
        }

        /// Sends an Objective-C action message to `target`.
        pub fn send_action(&self, target: &AnyObject, action: Sel) {
            unsafe {
                let () = msg_send![target, performSelector: action, withObject: std::ptr::null::<AnyObject>()];
            }
        }

        /// Intentionally a no-op: background test windows must not be brought to
        /// front. Callers are responsible for ordering test windows as needed before
        /// performing input synthesis.
        fn activate_window(&self) {}

        fn app(&self) -> Retained<NSApplication> {
            NSApplication::sharedApplication(self.mtm)
        }

        fn dispatch_mouse_move(
            &self,
            event_type: NSEventType,
            point: NSPoint,
            modifiers: NSEventModifierFlags,
            click_count: isize,
            pressure: f32,
        ) -> Result<(), InputSynthesisError> {
            let window_number = self.window.windowNumber();
            let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                event_type,
                point,
                modifiers,
                0.0,
                window_number,
                None,
                0,
                click_count,
                pressure,
            ) else {
                return Err(InputSynthesisError::TransportFailure(
                    "mouse event creation",
                ));
            };
            self.window.sendEvent(&event);
            self.dispatch_owner_tracking_mouse_moved_fallback(&event, point);
            Ok(())
        }

        fn dispatch_window_mouse_event(
            &self,
            event_type: NSEventType,
            point: NSPoint,
            modifiers: NSEventModifierFlags,
            click_count: isize,
            pressure: f32,
        ) -> Result<(), InputSynthesisError> {
            let window_number = self.window.windowNumber();
            let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                event_type,
                point,
                modifiers,
                0.0,
                window_number,
                None,
                0,
                click_count,
                pressure,
            ) else {
                return Err(InputSynthesisError::TransportFailure(
                    "mouse event creation",
                ));
            };
            self.window.sendEvent(&event);
            Ok(())
        }

        fn dispatch_window_mouse_click(&self, point: NSPoint) -> Result<(), InputSynthesisError> {
            self.dispatch_window_mouse_event(
                NSEventType::LeftMouseDown,
                point,
                NSEventModifierFlags::empty(),
                1,
                1.0,
            )?;
            self.dispatch_window_mouse_event(
                NSEventType::LeftMouseUp,
                point,
                NSEventModifierFlags::empty(),
                1,
                1.0,
            )
        }

        fn context_click_event(
            &self,
            gesture: ContextClickGesture,
            point: NSPoint,
        ) -> Result<Retained<NSEvent>, InputSynthesisError> {
            let (event_type, modifiers) = match gesture {
                ContextClickGesture::Secondary => {
                    (NSEventType::RightMouseDown, NSEventModifierFlags::empty())
                }
                ContextClickGesture::ControlPrimary => {
                    (NSEventType::LeftMouseDown, NSEventModifierFlags::Control)
                }
            };
            NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                event_type,
                point,
                modifiers,
                0.0,
                self.window.windowNumber().max(0),
                None,
                0,
                1,
                1.0,
            )
            .ok_or(InputSynthesisError::TransportFailure("context-click event creation"))
        }

        // AppKit does not reliably deliver owner-backed tracking-area mouseMoved callbacks for
        // these synthetic window-targeted moves, so provide a narrow mouseMoved-only fallback.
        fn dispatch_owner_tracking_mouse_moved_fallback(&self, event: &NSEvent, point: NSPoint) {
            let Some(content) = self.window.contentView() else {
                return;
            };
            self.dispatch_owner_tracking_mouse_moved_fallback_in_view(&content, event, point);
        }

        fn dispatch_owner_tracking_mouse_moved_fallback_in_view(
            &self,
            view: &NSView,
            event: &NSEvent,
            window_point: NSPoint,
        ) {
            let local_point = view.convertPoint_fromView(window_point, None);
            for area in view.trackingAreas().iter() {
                let options = area.options();
                if !self.supports_owner_tracking_mouse_moved_fallback(options) {
                    continue;
                }
                let rect = if options.contains(NSTrackingAreaOptions::InVisibleRect) {
                    view.visibleRect()
                } else {
                    area.rect()
                };
                if !point_in_rect(local_point, rect) {
                    continue;
                }
                let Some(owner) = area.owner() else {
                    continue;
                };
                unsafe {
                    let sel = objc2::sel!(mouseMoved:);
                    if msg_send![&*owner, respondsToSelector: sel] {
                        let () = msg_send![&*owner, mouseMoved: event];
                    }
                }
            }

            for subview in view.subviews().iter() {
                self.dispatch_owner_tracking_mouse_moved_fallback_in_view(
                    &subview,
                    event,
                    window_point,
                );
            }
        }

        fn supports_owner_tracking_mouse_moved_fallback(
            &self,
            options: NSTrackingAreaOptions,
        ) -> bool {
            if !options.contains(NSTrackingAreaOptions::MouseMoved) {
                return false;
            }
            if options.contains(NSTrackingAreaOptions::ActiveAlways) {
                return true;
            }
            if options.contains(NSTrackingAreaOptions::ActiveInKeyWindow) {
                return self.window.isKeyWindow();
            }
            if options.contains(NSTrackingAreaOptions::ActiveInActiveApp) {
                // Intentionally always true: for hidden background test windows we synthesize
                // events regardless of whether the app is currently active.
                return true;
            }
            false
        }

        fn drain_run_loop(&self) {
            let date = NSDate::dateWithTimeIntervalSinceNow(0.01);
            let _ = NSRunLoop::currentRunLoop()
                .runMode_beforeDate(unsafe { NSDefaultRunLoopMode }, &date);
        }

        /// Dequeues pending key events through `nextEventMatchingMask` (which invokes local
        /// event monitors), then dispatches each via `app.sendEvent` for standard routing.
        ///
        /// Drains queued key events through `nextEventMatchingMask` so that local
        /// event monitors observe them before the responder chain.
        ///
        /// Local event monitors fire inside `app.sendEvent`, not during the dequeue
        /// step. `app.sendEvent` routes to the current key window's responder; for
        /// background (non-key) test windows that routes to a different window, so we
        /// also call `window.sendEvent` to reach our own first responder. For key
        /// windows `app.sendEvent` already targets our window, so the direct call is
        /// skipped to avoid double dispatch.
        fn drain_queued_key_events(&self) {
            let mask = NSEventMask::KeyDown | NSEventMask::KeyUp;
            for _ in 0..2 {
                let expiry = NSDate::dateWithTimeIntervalSinceNow(0.05);
                if let Some(event) = self.app().nextEventMatchingMask_untilDate_inMode_dequeue(
                    mask,
                    Some(&expiry),
                    unsafe { NSDefaultRunLoopMode },
                    true,
                ) {
                    if self.window.isKeyWindow() {
                        self.app().sendEvent(&event);
                    } else {
                        self.app().sendEvent(&event);
                        self.window.sendEvent(&event);
                    }
                }
            }
        }

        fn is_control_target(&self, target: Option<&NSView>) -> bool {
            let Some(target) = target else {
                return false;
            };
            unsafe { msg_send![target, isKindOfClass: NSControl::class()] }
        }

        fn text_view_target<'b>(&self, view: &'b NSView) -> Option<&'b NSTextView> {
            if unsafe { msg_send![view, isKindOfClass: NSTextView::class()] } {
                Some(unsafe { &*(view as *const NSView).cast::<NSTextView>() })
            } else {
                None
            }
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

        fn key_press_queued(
            &self,
            key: &str,
            modifiers: KeyModifiers,
        ) -> Result<(), InputSynthesisError> {
            Self::key_press_queued(self, key, modifiers)
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

    fn point_in_rect(point: NSPoint, rect: objc2_foundation::NSRect) -> bool {
        point.x >= rect.origin.x
            && point.x <= rect.origin.x + rect.size.width
            && point.y >= rect.origin.y
            && point.y <= rect.origin.y + rect.size.height
    }
    #[cfg(test)]
    mod tests {
        use super::ns_modifiers;
        use glasscheck_core::KeyModifiers;
        use objc2_app_kit::NSEventModifierFlags;

        #[test]
        fn ns_modifiers_covers_every_key_modifier() {
            let all = KeyModifiers {
                shift: true,
                control: true,
                alt: true,
                meta: true,
            };
            let flags = ns_modifiers(all);
            assert!(flags.contains(NSEventModifierFlags::Shift));
            assert!(flags.contains(NSEventModifierFlags::Control));
            assert!(flags.contains(NSEventModifierFlags::Option));
            assert!(flags.contains(NSEventModifierFlags::Command));

            let none = ns_modifiers(KeyModifiers::default());
            assert!(none.is_empty());

            assert!(ns_modifiers(KeyModifiers {
                shift: true,
                ..Default::default()
            })
            .contains(NSEventModifierFlags::Shift));
            assert!(ns_modifiers(KeyModifiers {
                control: true,
                ..Default::default()
            })
            .contains(NSEventModifierFlags::Control));
            assert!(ns_modifiers(KeyModifiers {
                alt: true,
                ..Default::default()
            })
            .contains(NSEventModifierFlags::Option));
            assert!(ns_modifiers(KeyModifiers {
                meta: true,
                ..Default::default()
            })
            .contains(NSEventModifierFlags::Command));
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
