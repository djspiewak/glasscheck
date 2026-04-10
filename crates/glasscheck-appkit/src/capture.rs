#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_uchar;
    use std::ptr;

    use glasscheck_core::Image;
    use objc2::{AnyThread, ClassType};
    use objc2_app_kit::{NSBitmapImageRep, NSClipView, NSSplitView, NSView, NSWindow};
    use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

    const MIN_CAPTURE_DIM: f64 = 50.0;
    const DEFAULT_CAPTURE_FRAME: NSRect =
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0));

    pub fn capture_view_image(view: &NSView) -> Option<Image> {
        if let Some(window) = view.window() {
            ensure_window_has_frame(&window);
            window.display();
        }

        let rect = effective_capture_rect(view);
        capture_rect(view, rect)
    }

    fn ensure_window_has_frame(window: &NSWindow) {
        let frame = window.frame();
        if frame.size.width < MIN_CAPTURE_DIM || frame.size.height < MIN_CAPTURE_DIM {
            window.setMinSize(DEFAULT_CAPTURE_FRAME.size);
            window.setFrame_display(DEFAULT_CAPTURE_FRAME, true);
            if let Some(content) = window.contentView() {
                force_split_view_layout(&content);
            }
        }
    }

    fn force_split_view_layout(view: &NSView) {
        if view.isKindOfClass(NSSplitView::class()) {
            let split: &NSSplitView = unsafe { &*(ptr::from_ref(view).cast()) };
            split.adjustSubviews();
        }
        for subview in view.subviews().to_vec() {
            force_split_view_layout(&subview);
        }
    }

    fn effective_capture_rect(view: &NSView) -> NSRect {
        let clip_view = unsafe { view.superview() };
        let is_document_view = clip_view
            .as_ref()
            .is_some_and(|clip| clip.isKindOfClass(NSClipView::class()));

        if is_document_view {
            let scroll_view = clip_view.and_then(|clip| unsafe { clip.superview() });
            if let Some(scroll_view) = &scroll_view {
                let frame = scroll_view.frame();
                if frame.size.width >= MIN_CAPTURE_DIM && frame.size.height >= MIN_CAPTURE_DIM {
                    return NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
                }
            }
            if let Some(window) = view.window() {
                let frame = window.frame();
                if frame.size.width >= MIN_CAPTURE_DIM && frame.size.height >= MIN_CAPTURE_DIM {
                    return NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
                }
            }
        }

        view.bounds()
    }

    fn capture_rect(view: &NSView, rect: NSRect) -> Option<Image> {
        let width = rect.size.width as u32;
        let height = rect.size.height as u32;
        if width == 0 || height == 0 {
            return None;
        }

        view.display();
        let bitmap = create_bitmap_rep(width, height)?;
        view.cacheDisplayInRect_toBitmapImageRep(rect, &bitmap);
        extract_pixels(&bitmap, width, height)
    }

    fn create_bitmap_rep(width: u32, height: u32) -> Option<objc2::rc::Retained<NSBitmapImageRep>> {
        let color_space = NSString::from_str("NSDeviceRGBColorSpace");
        let width = width as isize;
        let height = height as isize;
        let bytes_per_row = width.checked_mul(4)?;

        unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                ptr::null_mut::<*mut c_uchar>(),
                width,
                height,
                8,
                4,
                true,
                false,
                &color_space,
                bytes_per_row,
                32,
            )
        }
    }

    fn extract_pixels(rep: &NSBitmapImageRep, width: u32, height: u32) -> Option<Image> {
        let ptr = rep.bitmapData();
        if ptr.is_null() {
            return None;
        }

        let row_bytes = width as usize * 4;
        let bytes_per_row = rep.bytesPerRow() as usize;
        let data = if row_bytes == bytes_per_row {
            let count = row_bytes.checked_mul(height as usize)?;
            unsafe { std::slice::from_raw_parts(ptr, count) }.to_vec()
        } else {
            let mut data = Vec::with_capacity(row_bytes * height as usize);
            for row in 0..height as usize {
                let offset = row * bytes_per_row;
                let row = unsafe { std::slice::from_raw_parts(ptr.add(offset), row_bytes) };
                data.extend_from_slice(row);
            }
            data
        };

        Some(Image::new(width, height, data))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct NSView;
    pub fn capture_view_image(_: &NSView) -> Option<glasscheck_core::Image> {
        None
    }
}

pub use imp::capture_view_image;
