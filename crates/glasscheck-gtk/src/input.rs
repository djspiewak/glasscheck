#[cfg(target_os = "linux")]
mod imp {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uchar, c_ulong};
    use std::sync::OnceLock;

    use glasscheck_core::{InputDriver, InputSynthesisError, KeyModifiers, Point, TextRange};
    use gtk4::gdk;
    use gtk4::glib::translate::ToGlibPtr;
    use gtk4::prelude::*;
    use gtk4::{TextView, Window};

    use crate::screen::present_window_offscreen;

    pub struct GtkInputDriver<'a> {
        window: &'a Window,
    }

    struct X11WindowContext {
        display: *mut std::ffi::c_void,
        xid: c_ulong,
        root: c_ulong,
        height: f64,
    }

    struct XTestApi {
        fake_button_event:
            unsafe extern "C" fn(*mut std::ffi::c_void, c_uint, c_int, c_ulong) -> c_int,
        fake_motion_event:
            unsafe extern "C" fn(*mut std::ffi::c_void, c_int, c_int, c_int, c_ulong) -> c_int,
        fake_key_event:
            unsafe extern "C" fn(*mut std::ffi::c_void, c_uint, c_int, c_ulong) -> c_int,
    }

    struct GdkX11Api {
        display_get_xdisplay:
            unsafe extern "C" fn(*mut gtk4::gdk::ffi::GdkDisplay) -> *mut std::ffi::c_void,
        surface_get_xid: unsafe extern "C" fn(*mut gtk4::gdk::ffi::GdkSurface) -> c_ulong,
    }

    struct X11Api {
        default_root_window: unsafe extern "C" fn(*mut std::ffi::c_void) -> c_ulong,
        sync: unsafe extern "C" fn(*mut std::ffi::c_void, c_int) -> c_int,
        translate_coordinates: unsafe extern "C" fn(
            *mut std::ffi::c_void,
            c_ulong,
            c_ulong,
            c_int,
            c_int,
            *mut c_int,
            *mut c_int,
            *mut c_ulong,
        ) -> c_int,
        keysym_to_keycode: unsafe extern "C" fn(*mut std::ffi::c_void, c_ulong) -> c_uchar,
        string_to_keysym: unsafe extern "C" fn(*const c_char) -> c_ulong,
    }

    impl<'a> GtkInputDriver<'a> {
        /// Creates an input driver for `window`.
        #[must_use]
        pub fn new(window: &'a Window) -> Self {
            Self { window }
        }

        /// Synthesizes a left click at `point` in root coordinates.
        pub fn click(&self, point: Point) -> Result<(), InputSynthesisError> {
            let context = self.x11_context()?;
            let (root_x, root_y) = root_point_to_x11(&context, point)?;
            xtest_motion(context.display, root_x, root_y)?;
            xtest_button(context.display, 1, true)?;
            xtest_button(context.display, 1, false)?;
            sync_display(context.display)?;
            Ok(())
        }

        /// Synthesizes a mouse move at `point` in root coordinates.
        pub fn move_mouse(&self, point: Point) -> Result<(), InputSynthesisError> {
            let context = self.x11_context()?;
            let (root_x, root_y) = root_point_to_x11(&context, point)?;
            xtest_motion(context.display, root_x, root_y)?;
            sync_display(context.display)?;
            Ok(())
        }

        /// Synthesizes a key press via the X11 event queue so root-level and legacy
        /// controllers observe the event before the focused widget's handlers.
        ///
        /// The caller must ensure the target widget has focus (e.g. `widget.grab_focus()`)
        /// before invoking this method; unlike [`key_press`] the queued path does not
        /// locate or refocus the target itself.
        ///
        /// Falls back to `UnsupportedBackend` when the GDK backend is not X11.
        /// See [`key_press`] for the direct (controller-emitting) counterpart.
        pub fn key_press_queued(
            &self,
            key: &str,
            modifiers: KeyModifiers,
        ) -> Result<(), InputSynthesisError> {
            let context = self.x11_context()?;
            let x11 = x11_api()?;

            let key_c = CString::new(key)
                .map_err(|_| InputSynthesisError::UnsupportedKey(key.to_string()))?;
            let keysym = unsafe { (x11.string_to_keysym)(key_c.as_ptr()) };
            if keysym == 0 {
                return Err(InputSynthesisError::UnsupportedKey(key.to_string()));
            }
            let keycode = unsafe { (x11.keysym_to_keycode)(context.display, keysym) } as c_uint;
            if keycode == 0 {
                return Err(InputSynthesisError::UnsupportedKey(key.to_string()));
            }

            let modifier_syms = queued_modifier_keysyms(modifiers);
            let mut modifier_codes: Vec<c_uint> = Vec::with_capacity(modifier_syms.len());
            for name in &modifier_syms {
                let name_c = CString::new(*name)
                    .map_err(|_| InputSynthesisError::TransportFailure("modifier keysym name"))?;
                let sym = unsafe { (x11.string_to_keysym)(name_c.as_ptr()) };
                if sym == 0 {
                    return Err(InputSynthesisError::UnsupportedKey(name.to_string()));
                }
                let code = unsafe { (x11.keysym_to_keycode)(context.display, sym) } as c_uint;
                if code == 0 {
                    return Err(InputSynthesisError::TransportFailure(
                        "queued modifier keycode",
                    ));
                }
                modifier_codes.push(code);
            }

            // Press modifiers, fire the key, then release modifiers in reverse.
            // On any failure the recovery block below releases keys already pressed
            // and flushes via sync_display before propagating the error, so the X
            // server is not left with stuck keys or buffered-but-unflushed events.
            let mut pressed = 0usize;
            for &code in &modifier_codes {
                if let Err(e) = xtest_key(context.display, code, true) {
                    for &c in modifier_codes[..pressed].iter().rev() {
                        let _ = xtest_key(context.display, c, false);
                    }
                    let _ = sync_display(context.display);
                    return Err(e);
                }
                pressed += 1;
            }
            let press_ok = xtest_key(context.display, keycode, true);
            let key_result = press_ok.and_then(|()| xtest_key(context.display, keycode, false));
            // If the press succeeded but the release failed, attempt a recovery release
            // so the primary key does not remain held down and autorepeat into other windows.
            if key_result.is_err() {
                let _ = xtest_key(context.display, keycode, false);
            }
            for &code in modifier_codes.iter().rev() {
                let _ = xtest_key(context.display, code, false);
            }
            if key_result.is_err() {
                let _ = sync_display(context.display);
                return key_result;
            }

            sync_display(context.display)?;
            Ok(())
        }

        /// Synthesizes a key press using backend-neutral modifiers.
        ///
        /// Emits events directly on each `EventControllerKey` in the focused widget's
        /// controller chain, bypassing root-level and legacy controllers. Use
        /// [`key_press_queued`] when the event must be observed by root-level or
        /// `EventControllerLegacy` handlers before the focused widget receives it.
        pub fn key_press(
            &self,
            key: &str,
            modifiers: KeyModifiers,
        ) -> Result<(), InputSynthesisError> {
            self.activate_window();
            let focus = self
                .focus_target()
                .ok_or(InputSynthesisError::MissingFocusTarget)?;
            if dispatch_key_to_controllers(&focus, key, modifiers)? {
                return Ok(());
            }
            if let Ok(text) = focus.downcast::<TextView>() {
                self.type_text_direct(&text, key);
                return Ok(());
            }
            Err(InputSynthesisError::MissingTarget)
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
            gtk4::prelude::WidgetExt::realize(self.window);
        }

        fn x11_context(&self) -> Result<X11WindowContext, InputSynthesisError> {
            self.activate_window();
            let surface = self
                .window
                .surface()
                .ok_or(InputSynthesisError::MissingSurface)?;
            let display = surface.display();
            if !display.backend().is_x11() {
                return Err(InputSynthesisError::UnsupportedBackend);
            }
            let gdk_x11 = gdk_x11_api()?;
            let xdisplay = unsafe { (gdk_x11.display_get_xdisplay)(display.to_glib_none().0) };
            if xdisplay.is_null() {
                return Err(InputSynthesisError::UnsupportedBackend);
            }
            let xid = unsafe { (gdk_x11.surface_get_xid)(surface.to_glib_none().0) };
            if xid == 0 {
                return Err(InputSynthesisError::MissingWindowId);
            }
            let x11 = x11_api()?;
            let height = self
                .window
                .child()
                .map(|root| root.allocated_height().max(1) as f64)
                .unwrap_or_else(|| self.window.default_height().max(1) as f64);
            Ok(X11WindowContext {
                display: xdisplay,
                xid,
                root: unsafe { (x11.default_root_window)(xdisplay) },
                height,
            })
        }

        fn focus_target(&self) -> Option<gtk4::Widget> {
            self.window
                .child()
                .and_then(|root| root.root())
                .and_then(|root| root.focus())
        }
    }

    impl InputDriver for GtkInputDriver<'_> {
        type NativeText = TextView;

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

    fn root_point_to_x11(
        context: &X11WindowContext,
        point: Point,
    ) -> Result<(c_int, c_int), InputSynthesisError> {
        let src_x = point.x.round() as c_int;
        let src_y = (context.height - point.y).round() as c_int;
        let mut root_x = 0;
        let mut root_y = 0;
        let mut child = 0;
        let x11 = x11_api()?;
        let translated = unsafe {
            (x11.translate_coordinates)(
                context.display,
                context.xid,
                context.root,
                src_x,
                src_y,
                &mut root_x,
                &mut root_y,
                &mut child,
            )
        };
        if translated == 0 {
            return Err(InputSynthesisError::TransportFailure(
                "coordinate translation",
            ));
        }
        Ok((root_x, root_y))
    }

    fn dispatch_key_to_controllers(
        widget: &gtk4::Widget,
        key: &str,
        modifiers: KeyModifiers,
    ) -> Result<bool, InputSynthesisError> {
        let Some(keyval) = gdk::Key::from_name(key) else {
            return Err(InputSynthesisError::UnsupportedKey(key.to_string()));
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
                    return Ok(true);
                }
            }
            current = candidate.parent();
        }
        Ok(handled)
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

    fn xtest_motion(
        display: *mut std::ffi::c_void,
        x: c_int,
        y: c_int,
    ) -> Result<(), InputSynthesisError> {
        let api = xtest_api()?;
        let status = unsafe { (api.fake_motion_event)(display, -1, x, y, 0) };
        if status == 0 {
            return Err(InputSynthesisError::TransportFailure("pointer motion"));
        }
        Ok(())
    }

    fn xtest_button(
        display: *mut std::ffi::c_void,
        button: c_uint,
        is_press: bool,
    ) -> Result<(), InputSynthesisError> {
        let api = xtest_api()?;
        let status = unsafe { (api.fake_button_event)(display, button, is_press as c_int, 0) };
        if status == 0 {
            return Err(InputSynthesisError::TransportFailure("pointer button"));
        }
        Ok(())
    }

    fn xtest_key(
        display: *mut std::ffi::c_void,
        keycode: c_uint,
        is_press: bool,
    ) -> Result<(), InputSynthesisError> {
        let api = xtest_api()?;
        let status = unsafe { (api.fake_key_event)(display, keycode, is_press as c_int, 0) };
        if status == 0 {
            return Err(InputSynthesisError::TransportFailure("key event"));
        }
        Ok(())
    }

    fn xtest_api() -> Result<&'static XTestApi, InputSynthesisError> {
        static XTEST_API: OnceLock<Result<XTestApi, InputSynthesisError>> = OnceLock::new();
        XTEST_API
            .get_or_init(load_xtest_api)
            .as_ref()
            .map_err(Clone::clone)
    }

    fn load_xtest_api() -> Result<XTestApi, InputSynthesisError> {
        let handle = open_library("libXtst.so.6")?;
        Ok(XTestApi {
            fake_button_event: load_xtest_symbol(handle, "XTestFakeButtonEvent")?,
            fake_motion_event: load_xtest_symbol(handle, "XTestFakeMotionEvent")?,
            fake_key_event: load_xtest_symbol(handle, "XTestFakeKeyEvent")?,
        })
    }

    fn load_xtest_symbol<T>(
        handle: *mut std::ffi::c_void,
        name: &str,
    ) -> Result<T, InputSynthesisError>
    where
        T: Copy,
    {
        let name = CString::new(name)
            .map_err(|_| InputSynthesisError::TransportFailure("xtest symbol name"))?;
        let symbol = unsafe { dlsym(handle, name.as_ptr()) };
        if symbol.is_null() {
            return Err(InputSynthesisError::UnsupportedBackend);
        }
        Ok(unsafe { std::mem::transmute_copy::<*mut std::ffi::c_void, T>(&symbol) })
    }

    fn sync_display(display: *mut std::ffi::c_void) -> Result<(), InputSynthesisError> {
        let x11 = x11_api()?;
        let status = unsafe { (x11.sync)(display, 0) };
        if status == 0 {
            return Err(InputSynthesisError::TransportFailure("display sync"));
        }
        Ok(())
    }

    fn gdk_x11_api() -> Result<&'static GdkX11Api, InputSynthesisError> {
        static GDK_X11_API: OnceLock<Result<GdkX11Api, InputSynthesisError>> = OnceLock::new();
        GDK_X11_API
            .get_or_init(load_gdk_x11_api)
            .as_ref()
            .map_err(Clone::clone)
    }

    fn load_gdk_x11_api() -> Result<GdkX11Api, InputSynthesisError> {
        let handle = open_library("libgtk-4.so.1")?;
        Ok(GdkX11Api {
            display_get_xdisplay: load_symbol(handle, "gdk_x11_display_get_xdisplay")?,
            surface_get_xid: load_symbol(handle, "gdk_x11_surface_get_xid")?,
        })
    }

    fn x11_api() -> Result<&'static X11Api, InputSynthesisError> {
        static X11_API: OnceLock<Result<X11Api, InputSynthesisError>> = OnceLock::new();
        X11_API
            .get_or_init(load_x11_api)
            .as_ref()
            .map_err(Clone::clone)
    }

    fn load_x11_api() -> Result<X11Api, InputSynthesisError> {
        let handle = open_library("libX11.so.6")?;
        Ok(X11Api {
            default_root_window: load_symbol(handle, "XDefaultRootWindow")?,
            sync: load_symbol(handle, "XSync")?,
            translate_coordinates: load_symbol(handle, "XTranslateCoordinates")?,
            keysym_to_keycode: load_symbol(handle, "XKeysymToKeycode")?,
            string_to_keysym: load_symbol(handle, "XStringToKeysym")?,
        })
    }

    fn open_library(name: &str) -> Result<*mut std::ffi::c_void, InputSynthesisError> {
        let library = CString::new(name)
            .map_err(|_| InputSynthesisError::TransportFailure("library name"))?;
        let handle = unsafe { dlopen(library.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            return Err(InputSynthesisError::UnsupportedBackend);
        }
        Ok(handle)
    }

    fn load_symbol<T>(handle: *mut std::ffi::c_void, name: &str) -> Result<T, InputSynthesisError>
    where
        T: Copy,
    {
        let name =
            CString::new(name).map_err(|_| InputSynthesisError::TransportFailure("symbol name"))?;
        let symbol = unsafe { dlsym(handle, name.as_ptr()) };
        if symbol.is_null() {
            return Err(InputSynthesisError::UnsupportedBackend);
        }
        Ok(unsafe { std::mem::transmute_copy::<*mut std::ffi::c_void, T>(&symbol) })
    }

    use std::os::raw::c_uint;

    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut std::ffi::c_void;
        fn dlsym(handle: *mut std::ffi::c_void, symbol: *const c_char) -> *mut std::ffi::c_void;
    }

    const RTLD_NOW: c_int = 2;

    /// Returns the ordered list of modifier keysym names that must be pressed
    /// (and released in reverse) around a primary key for `modifiers`.
    ///
    /// Uses left-modifier variants (`Shift_L`, `Control_L`, etc.), which are
    /// defined on all standard X keyboards and produce the same modifier bits
    /// as their right-side counterparts.
    fn queued_modifier_keysyms(modifiers: KeyModifiers) -> Vec<&'static str> {
        let mut names = Vec::new();
        if modifiers.shift {
            names.push("Shift_L");
        }
        if modifiers.control {
            names.push("Control_L");
        }
        if modifiers.alt {
            names.push("Alt_L");
        }
        if modifiers.meta {
            names.push("Super_L");
        }
        names
    }

    #[cfg(test)]
    mod tests {
        use super::{modifier_state, queued_modifier_keysyms};
        use glasscheck_core::KeyModifiers;
        use gtk4::gdk;

        #[test]
        fn modifier_state_for_queued_path_covers_all_flags() {
            let all = KeyModifiers {
                shift: true,
                control: true,
                alt: true,
                meta: true,
            };
            let state = modifier_state(all);
            assert!(state.contains(gdk::ModifierType::SHIFT_MASK));
            assert!(state.contains(gdk::ModifierType::CONTROL_MASK));
            assert!(state.contains(gdk::ModifierType::ALT_MASK));
            assert!(state.contains(gdk::ModifierType::META_MASK));
            assert!(state.contains(gdk::ModifierType::SUPER_MASK));

            let none = modifier_state(KeyModifiers::default());
            assert!(none.is_empty());
        }

        #[test]
        fn queued_modifier_keysyms_empty_for_default() {
            assert!(queued_modifier_keysyms(KeyModifiers::default()).is_empty());
        }

        #[test]
        fn xtest_modifier_keysym_for_flag_returns_expected_ordering() {
            let shift_only = queued_modifier_keysyms(KeyModifiers {
                shift: true,
                ..Default::default()
            });
            assert_eq!(shift_only, vec!["Shift_L"]);

            let ctrl_only = queued_modifier_keysyms(KeyModifiers {
                control: true,
                ..Default::default()
            });
            assert_eq!(ctrl_only, vec!["Control_L"]);

            let alt_only = queued_modifier_keysyms(KeyModifiers {
                alt: true,
                ..Default::default()
            });
            assert_eq!(alt_only, vec!["Alt_L"]);

            let meta_only = queued_modifier_keysyms(KeyModifiers {
                meta: true,
                ..Default::default()
            });
            assert_eq!(meta_only, vec!["Super_L"]);

            let multi = queued_modifier_keysyms(KeyModifiers {
                shift: true,
                control: true,
                ..Default::default()
            });
            assert_eq!(multi, vec!["Shift_L", "Control_L"]);

            let all = queued_modifier_keysyms(KeyModifiers {
                shift: true,
                control: true,
                alt: true,
                meta: true,
            });
            assert_eq!(all, vec!["Shift_L", "Control_L", "Alt_L", "Super_L"]);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub struct GtkInputDriver<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::GtkInputDriver;
