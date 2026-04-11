#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::{NSScreen, NSWindow, NSWindowStyleMask};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

    const OFFSCREEN_MARGIN: f64 = 32.0;
    const FALLBACK_ORIGIN: NSPoint = NSPoint::new(10_000.0, 10_000.0);

    pub(crate) fn offscreen_window_content_rect(
        mtm: MainThreadMarker,
        width: f64,
        height: f64,
    ) -> NSRect {
        offscreen_rect_for_size(NSSize::new(width, height), screen_frames(mtm))
    }

    pub(crate) fn offscreen_window_frame_rect(
        mtm: MainThreadMarker,
        style_mask: NSWindowStyleMask,
        width: f64,
        height: f64,
    ) -> NSRect {
        let content_rect = offscreen_window_content_rect(mtm, width, height);
        NSWindow::frameRectForContentRect_styleMask(content_rect, style_mask, mtm)
    }

    fn screen_frames(mtm: MainThreadMarker) -> Vec<NSRect> {
        NSScreen::screens(mtm)
            .iter()
            .map(|screen| screen.frame())
            .collect()
    }

    fn offscreen_rect_for_size(
        size: NSSize,
        display_frames: impl IntoIterator<Item = NSRect>,
    ) -> NSRect {
        let origin = offscreen_origin_for_size(size, display_frames);
        NSRect::new(origin, size)
    }

    fn offscreen_origin_for_size(
        _size: NSSize,
        display_frames: impl IntoIterator<Item = NSRect>,
    ) -> NSPoint {
        union_rects(display_frames)
            .map(|bounds| NSPoint::new(max_x(bounds) + OFFSCREEN_MARGIN, bounds.origin.y))
            .unwrap_or(FALLBACK_ORIGIN)
    }

    fn union_rects(rects: impl IntoIterator<Item = NSRect>) -> Option<NSRect> {
        rects.into_iter().reduce(|lhs, rhs| {
            let min_x = lhs.origin.x.min(rhs.origin.x);
            let min_y = lhs.origin.y.min(rhs.origin.y);
            let max_x = max_x(lhs).max(max_x(rhs));
            let max_y = max_y(lhs).max(max_y(rhs));
            NSRect::new(
                NSPoint::new(min_x, min_y),
                NSSize::new(max_x - min_x, max_y - min_y),
            )
        })
    }

    fn max_x(rect: NSRect) -> f64 {
        rect.origin.x + rect.size.width
    }

    fn max_y(rect: NSRect) -> f64 {
        rect.origin.y + rect.size.height
    }

    #[cfg(test)]
    mod tests {
        use super::{offscreen_rect_for_size, union_rects, FALLBACK_ORIGIN};
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        #[test]
        fn offscreen_frame_sits_to_the_right_of_single_display() {
            let frame = offscreen_rect_for_size(
                NSSize::new(240.0, 120.0),
                [NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(1920.0, 1080.0),
                )],
            );

            assert_eq!(frame.origin, NSPoint::new(1952.0, 0.0));
            assert!(!intersects_any(
                frame,
                [NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(1920.0, 1080.0),
                )]
            ));
        }

        #[test]
        fn offscreen_frame_uses_union_of_displays_with_negative_coordinates() {
            let displays = [
                NSRect::new(NSPoint::new(-1440.0, 0.0), NSSize::new(1440.0, 900.0)),
                NSRect::new(NSPoint::new(0.0, -1080.0), NSSize::new(1920.0, 1080.0)),
                NSRect::new(NSPoint::new(1920.0, 200.0), NSSize::new(1280.0, 1024.0)),
            ];

            let frame = offscreen_rect_for_size(NSSize::new(320.0, 160.0), displays);

            assert_eq!(frame.origin, NSPoint::new(3232.0, -1080.0));
            assert!(!intersects_any(frame, displays));
        }

        #[test]
        fn offscreen_frame_uses_fallback_origin_when_no_displays_are_available() {
            let frame = offscreen_rect_for_size(NSSize::new(320.0, 160.0), []);
            assert_eq!(frame.origin, FALLBACK_ORIGIN);
        }

        #[test]
        fn union_rects_covers_vertically_stacked_displays() {
            let union = union_rects([
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1512.0, 982.0)),
                NSRect::new(NSPoint::new(0.0, 982.0), NSSize::new(1728.0, 1117.0)),
            ])
            .expect("display union should exist");

            assert_eq!(union.origin, NSPoint::new(0.0, 0.0));
            assert_eq!(union.size, NSSize::new(1728.0, 2099.0));
        }

        fn intersects_any(frame: NSRect, displays: impl IntoIterator<Item = NSRect>) -> bool {
            displays
                .into_iter()
                .any(|display| intersects(frame, display))
        }

        fn intersects(lhs: NSRect, rhs: NSRect) -> bool {
            lhs.origin.x < rhs.origin.x + rhs.size.width
                && rhs.origin.x < lhs.origin.x + lhs.size.width
                && lhs.origin.y < rhs.origin.y + rhs.size.height
                && rhs.origin.y < lhs.origin.y + lhs.size.height
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {}

#[cfg(target_os = "macos")]
pub(crate) use imp::{offscreen_window_content_rect, offscreen_window_frame_rect};
