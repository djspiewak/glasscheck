#[cfg(target_os = "linux")]
mod imp {
    use std::ffi::c_void;
    use std::os::raw::{c_int, c_long, c_ulong};

    use gtk4::gdk;
    use gtk4::glib::translate::ToGlibPtr;
    use gtk4::prelude::*;
    use gtk4::Window;

    const OFFSCREEN_MARGIN: i32 = 32;
    const FALLBACK_ORIGIN: DisplayPoint = DisplayPoint {
        x: 10_000,
        y: 10_000,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct DisplayPoint {
        pub(crate) x: i32,
        pub(crate) y: i32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct DisplayRect {
        pub(crate) x: i32,
        pub(crate) y: i32,
        pub(crate) width: i32,
        pub(crate) height: i32,
    }

    impl DisplayRect {
        fn max_x(self) -> i32 {
            self.x + self.width
        }

        fn max_y(self) -> i32 {
            self.y + self.height
        }
    }

    pub(crate) fn present_window_offscreen(window: &Window) {
        if should_hide_during_initial_map(window) {
            let original_opacity = window.opacity();
            window.set_opacity(0.0);
            apply_offscreen_position(window);
            window.present();
            wait_for_window_ready(window);
            apply_offscreen_position(window);
            window.set_opacity(original_opacity);
            return;
        }

        apply_offscreen_position(window);
        window.present();
        apply_offscreen_position(window);
    }

    pub(crate) fn connected_display_rects(display: &gdk::Display) -> Vec<DisplayRect> {
        display
            .monitors()
            .snapshot()
            .into_iter()
            .filter_map(|object| object.downcast::<gdk::Monitor>().ok())
            .map(|monitor| {
                let geometry = monitor.geometry();
                DisplayRect {
                    x: geometry.x(),
                    y: geometry.y(),
                    width: geometry.width(),
                    height: geometry.height(),
                }
            })
            .collect()
    }

    pub(crate) fn offscreen_origin(
        display_rects: impl IntoIterator<Item = DisplayRect>,
    ) -> DisplayPoint {
        union_rects(display_rects)
            .map(|bounds| DisplayPoint {
                x: bounds.max_x() + OFFSCREEN_MARGIN,
                y: bounds.y,
            })
            .unwrap_or(FALLBACK_ORIGIN)
    }

    fn apply_offscreen_position(window: &Window) {
        gtk4::prelude::WidgetExt::realize(window);
        let Some(surface) = window.surface() else {
            return;
        };
        let display = surface.display();
        if !display.backend().is_x11() {
            return;
        }

        let origin = offscreen_origin(connected_display_rects(&display));
        unsafe {
            let xdisplay = gdk_x11_display_get_xdisplay(display.to_glib_none().0);
            let xid = gdk_x11_surface_get_xid(surface.to_glib_none().0);
            if !xdisplay.is_null() && xid != 0 {
                let width = surface.width().max(window.default_width()).max(1);
                let height = surface.height().max(window.default_height()).max(1);
                let mut attributes = XSetWindowAttributes {
                    background_pixmap: 0,
                    background_pixel: 0,
                    border_pixmap: 0,
                    border_pixel: 0,
                    bit_gravity: 0,
                    win_gravity: 0,
                    backing_store: 0,
                    backing_planes: 0,
                    backing_pixel: 0,
                    save_under: 0,
                    event_mask: 0,
                    do_not_propagate_mask: 0,
                    override_redirect: 1,
                    colormap: 0,
                    cursor: 0,
                };
                let mut hints = XSizeHints {
                    flags: P_POSITION | P_SIZE,
                    x: origin.x,
                    y: origin.y,
                    width,
                    height,
                    min_width: 0,
                    min_height: 0,
                    max_width: 0,
                    max_height: 0,
                    width_inc: 0,
                    height_inc: 0,
                    min_aspect: XAspect { x: 0, y: 0 },
                    max_aspect: XAspect { x: 0, y: 0 },
                    base_width: 0,
                    base_height: 0,
                    win_gravity: 0,
                };
                XChangeWindowAttributes(xdisplay, xid, CW_OVERRIDE_REDIRECT, &mut attributes);
                XSetWMNormalHints(xdisplay, xid, &mut hints);
                XMoveResizeWindow(
                    xdisplay,
                    xid,
                    origin.x,
                    origin.y,
                    width as u32,
                    height as u32,
                );
                XSync(xdisplay, 0);
            }
        }
    }

    #[repr(C)]
    struct XSetWindowAttributes {
        background_pixmap: c_ulong,
        background_pixel: c_ulong,
        border_pixmap: c_ulong,
        border_pixel: c_ulong,
        bit_gravity: c_int,
        win_gravity: c_int,
        backing_store: c_int,
        backing_planes: c_ulong,
        backing_pixel: c_ulong,
        save_under: c_int,
        event_mask: c_long,
        do_not_propagate_mask: c_long,
        override_redirect: c_int,
        colormap: c_ulong,
        cursor: c_ulong,
    }

    fn should_hide_during_initial_map(window: &Window) -> bool {
        gtk4::prelude::WidgetExt::display(window).backend().is_x11()
    }

    fn wait_for_window_ready(window: &Window) {
        let started = std::time::Instant::now();
        let context = glib::MainContext::default();
        while started.elapsed() < std::time::Duration::from_secs(1) {
            while context.pending() {
                context.iteration(false);
            }
            context.iteration(false);

            let child_ready = window
                .child()
                .is_some_and(|child| child.allocated_width() > 1 && child.allocated_height() > 1);
            let surface_ready = window.surface().is_some_and(|surface| surface.is_mapped());
            if child_ready && surface_ready {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[repr(C)]
    struct XAspect {
        x: c_int,
        y: c_int,
    }

    #[repr(C)]
    struct XSizeHints {
        flags: c_long,
        x: c_int,
        y: c_int,
        width: c_int,
        height: c_int,
        min_width: c_int,
        min_height: c_int,
        max_width: c_int,
        max_height: c_int,
        width_inc: c_int,
        height_inc: c_int,
        min_aspect: XAspect,
        max_aspect: XAspect,
        base_width: c_int,
        base_height: c_int,
        win_gravity: c_int,
    }

    const P_POSITION: c_long = 1 << 2;
    const P_SIZE: c_long = 1 << 3;
    const CW_OVERRIDE_REDIRECT: c_ulong = 1 << 9;

    #[link(name = "gtk-4")]
    unsafe extern "C" {
        fn gdk_x11_display_get_xdisplay(display: *mut gtk4::gdk::ffi::GdkDisplay) -> *mut c_void;
        fn gdk_x11_surface_get_xid(surface: *mut gtk4::gdk::ffi::GdkSurface) -> c_ulong;
    }

    #[link(name = "X11")]
    unsafe extern "C" {
        fn XChangeWindowAttributes(
            display: *mut c_void,
            w: c_ulong,
            valuemask: c_ulong,
            attributes: *mut XSetWindowAttributes,
        ) -> c_int;
        fn XMoveResizeWindow(
            display: *mut c_void,
            w: c_ulong,
            x: c_int,
            y: c_int,
            width: u32,
            height: u32,
        ) -> c_int;
        fn XSetWMNormalHints(display: *mut c_void, w: c_ulong, hints: *mut XSizeHints);
        fn XSync(display: *mut c_void, discard: c_int) -> c_int;
    }

    fn union_rects(rects: impl IntoIterator<Item = DisplayRect>) -> Option<DisplayRect> {
        rects.into_iter().reduce(|lhs, rhs| {
            let min_x = lhs.x.min(rhs.x);
            let min_y = lhs.y.min(rhs.y);
            let max_x = lhs.max_x().max(rhs.max_x());
            let max_y = lhs.max_y().max(rhs.max_y());
            DisplayRect {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            }
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{offscreen_origin, DisplayPoint, DisplayRect, FALLBACK_ORIGIN};

        #[test]
        fn offscreen_origin_sits_to_the_right_of_single_display() {
            let origin = offscreen_origin([DisplayRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }]);

            assert_eq!(origin, DisplayPoint { x: 1952, y: 0 });
            assert!(!intersects_any(
                DisplayRect {
                    x: origin.x,
                    y: origin.y,
                    width: 240,
                    height: 120,
                },
                [DisplayRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                }]
            ));
        }

        #[test]
        fn offscreen_origin_uses_union_of_displays_with_negative_coordinates() {
            let displays = [
                DisplayRect {
                    x: -1440,
                    y: 0,
                    width: 1440,
                    height: 900,
                },
                DisplayRect {
                    x: 0,
                    y: -1080,
                    width: 1920,
                    height: 1080,
                },
                DisplayRect {
                    x: 1920,
                    y: 200,
                    width: 1280,
                    height: 1024,
                },
            ];

            let origin = offscreen_origin(displays);

            assert_eq!(origin, DisplayPoint { x: 3232, y: -1080 });
            assert!(!intersects_any(
                DisplayRect {
                    x: origin.x,
                    y: origin.y,
                    width: 320,
                    height: 160,
                },
                displays
            ));
        }

        #[test]
        fn offscreen_origin_uses_fallback_when_no_displays_are_available() {
            assert_eq!(offscreen_origin([]), FALLBACK_ORIGIN);
        }

        fn intersects_any(
            frame: DisplayRect,
            displays: impl IntoIterator<Item = DisplayRect>,
        ) -> bool {
            displays
                .into_iter()
                .any(|display| intersects(frame, display))
        }

        fn intersects(lhs: DisplayRect, rhs: DisplayRect) -> bool {
            lhs.x < rhs.x + rhs.width
                && rhs.x < lhs.x + lhs.width
                && lhs.y < rhs.y + rhs.height
                && rhs.y < lhs.y + lhs.height
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {}

#[cfg(target_os = "linux")]
pub(crate) use imp::present_window_offscreen;
