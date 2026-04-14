#[cfg(target_os = "linux")]
mod imp {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_ulong};
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

        /// Synthesizes a key press using backend-neutral modifiers.
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
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub struct GtkInputDriver<'a> {
        _marker: std::marker::PhantomData<&'a ()>,
    }
}

pub use imp::GtkInputDriver;
