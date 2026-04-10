#![cfg(target_os = "macos")]

use glasscheck_appkit::{AppKitHarness, InstrumentedView};
use glasscheck_core::{
    assert_text_renders, compare_images, CompareConfig, Point, PollOptions, Rect, RgbaColor,
    Role, Selector, Size, TextAssertionConfig, TextExpectation,
};
use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSClipView, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSTextView, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

fn main() {
    let mtm = MainThreadMarker::new().expect("AppKit tests must run on the main thread");
    let harness = AppKitHarness::new(mtm);

    run("capture_returns_non_empty_image", || {
        capture_returns_non_empty_image(harness)
    });
    run("query_reports_registered_view_geometry", || {
        query_reports_registered_view_geometry(harness)
    });
    run("direct_text_input_changes_rendered_content", || {
        direct_text_input_changes_rendered_content(harness)
    });
    run("rendered_text_assertion_matches_live_text", || {
        rendered_text_assertion_matches_live_text(harness)
    });
    run("rendered_text_assertion_honors_non_zero_origin", || {
        rendered_text_assertion_honors_non_zero_origin(harness)
    });
    run("rendered_text_assertion_supports_family_weight_and_italic", || {
        rendered_text_assertion_supports_family_weight_and_italic(harness)
    });
    run("rendered_text_assertion_matches_clipped_text_region", || {
        rendered_text_assertion_matches_clipped_text_region(harness)
    });
    run("rendered_text_assertion_matches_negative_origin_region", || {
        rendered_text_assertion_matches_negative_origin_region(harness)
    });
    run("wait_until_flushes_runloop_between_attempts", || {
        wait_until_flushes_runloop_between_attempts(harness)
    });
}

fn run(name: &str, test: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
    match result {
        Ok(()) => {
            println!("test {name} ... ok");
        }
        Err(error) => {
            if let Some(message) = error.downcast_ref::<String>() {
                eprintln!("test {name} ... FAILED\n{message}");
            } else if let Some(message) = error.downcast_ref::<&str>() {
                eprintln!("test {name} ... FAILED\n{message}");
            } else {
                eprintln!("test {name} ... FAILED");
            }
            std::process::exit(1);
        }
    }
}

fn capture_returns_non_empty_image(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 120.0);
    let view = make_view(harness.main_thread_marker(), NSSize::new(240.0, 120.0));
    host.set_content_view(&view);
    harness.settle(2);

    let image = host.capture().expect("window capture should succeed");
    assert!(image.width > 0);
    assert!(image.height > 0);
}

fn query_reports_registered_view_geometry(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 120.0);
    let content_view = make_view(harness.main_thread_marker(), NSSize::new(240.0, 120.0));
    let clip_view = make_clip_view(harness.main_thread_marker(), NSSize::new(100.0, 100.0));
    clip_view.setFrameOrigin(NSPoint::new(10.0, 10.0));
    let child_view = make_view(harness.main_thread_marker(), NSSize::new(20.0, 10.0));
    child_view.setFrameOrigin(NSPoint::new(5.0, 7.0));
    clip_view.addSubview(&child_view);
    content_view.addSubview(&clip_view);
    host.set_content_view(&content_view);
    host.register_view(
        &child_view,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Container),
            label: Some("Root".into()),
        },
    );

    let query_root = host.query_root();
    let node = query_root
        .find(&Selector::by_id("root"))
        .expect("registered view should exist");
    let content_view = host
        .window()
        .contentView()
        .expect("test window should have a content view");
    let expected = content_view.convertRect_fromView(child_view.bounds(), Some(&child_view));
    assert_eq!(
        node.rect,
        glasscheck_core::Rect::new(
            glasscheck_core::Point::new(expected.origin.x, expected.origin.y),
            glasscheck_core::Size::new(expected.size.width, expected.size.height),
        )
    );
}

fn direct_text_input_changes_rendered_content(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 160.0);
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(320.0, 160.0));
    host.set_content_view(&view);
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
        },
    );

    harness.settle(2);
    let before = host
        .capture_view(&view)
        .expect("empty text view should capture successfully");

    host.input().replace_text(&view, "Functional UI");
    harness.settle(2);

    let after = host
        .capture_view(&view)
        .expect("text view should capture after typing");

    let result = compare_images(
        &before,
        &after,
        &CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: false,
        },
    );
    assert!(
        !result.passed,
        "rendered text should change the captured output"
    );
}

fn rendered_text_assertion_matches_live_text(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 160.0);
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(320.0, 160.0));
    let font = NSFont::systemFontOfSize(18.0);
    view.setFont(Some(&font));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    let background = NSColor::whiteColor();
    view.setBackgroundColor(&background);
    let foreground = NSColor::blackColor();
    view.setTextColor(Some(&foreground));
    host.set_content_view(&view);

    host.input().replace_text(&view, "Functional UI");
    harness.settle(2);

    let expectation = TextExpectation::new(
        "Functional UI",
        Rect::new(Point::new(0.0, 0.0), Size::new(320.0, 160.0)),
    )
    .with_point_size(18.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text");
    assert_text_renders(
        &host.text_renderer(harness.main_thread_marker()),
        &expectation,
        &artifact_dir,
        &TextAssertionConfig {
            compare: CompareConfig {
                channel_tolerance: 24,
                match_threshold: 0.97,
                generate_diff: true,
            },
            write_diff: true,
        },
    )
    .expect("rendered text should match the AppKit reference rendering");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn rendered_text_assertion_honors_non_zero_origin(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 120.0);
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(160.0, 80.0));
    let font_name = NSString::from_str("Menlo-Regular");
    let font = NSFont::fontWithName_size(&font_name, 24.0)
        .expect("Menlo-Regular font should be available on macOS");
    view.setFont(Some(&font));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    view.setDrawsBackground(true);
    let background = NSColor::whiteColor();
    view.setBackgroundColor(&background);
    let foreground = NSColor::blackColor();
    view.setTextColor(Some(&foreground));
    view.setFrameOrigin(NSPoint::new(20.0, 20.0));
    host.set_content_view(&view);

    host.input().replace_text(&view, "I");
    harness.settle(2);

    let expectation = TextExpectation::new(
        "I",
        Rect::new(Point::new(20.0, 20.0), Size::new(60.0, 60.0)),
    )
    .with_font_name("Menlo-Regular")
    .with_point_size(24.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text-offset");
    assert_text_renders(
        &host.text_renderer(harness.main_thread_marker()),
        &expectation,
        &artifact_dir,
        &TextAssertionConfig {
            compare: CompareConfig {
                channel_tolerance: 24,
                match_threshold: 0.96,
                generate_diff: true,
            },
            write_diff: true,
        },
    )
    .expect("rendered text with non-zero origin should match the AppKit reference rendering");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn rendered_text_assertion_supports_family_weight_and_italic(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 120.0);
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(240.0, 120.0));
    let font = family_font(harness.main_thread_marker(), "Helvetica", 700, true, 22.0);
    view.setFont(Some(&font));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    view.setDrawsBackground(true);
    let background = NSColor::whiteColor();
    view.setBackgroundColor(&background);
    let foreground = NSColor::blackColor();
    view.setTextColor(Some(&foreground));
    host.set_content_view(&view);

    host.input().replace_text(&view, "Bold Italic");
    harness.settle(2);

    let expectation = TextExpectation::new(
        "Bold Italic",
        Rect::new(Point::new(0.0, 0.0), Size::new(240.0, 120.0)),
    )
    .with_font_family("Helvetica")
    .with_weight(700)
    .italic(true)
    .with_point_size(22.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text-family-weight-italic");
    assert_text_renders(
        &host.text_renderer(harness.main_thread_marker()),
        &expectation,
        &artifact_dir,
        &TextAssertionConfig {
            compare: CompareConfig {
                channel_tolerance: 24,
                match_threshold: 0.96,
                generate_diff: true,
            },
            write_diff: true,
        },
    )
    .expect("family, weight, and italic expectations should match AppKit rendering");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn rendered_text_assertion_matches_clipped_text_region(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let container = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(56.0, 56.0));
    let font_name = NSString::from_str("Menlo-Regular");
    let font = NSFont::fontWithName_size(&font_name, 48.0)
        .expect("Menlo-Regular font should be available on macOS");
    view.setFont(Some(&font));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    view.setDrawsBackground(true);
    let background = NSColor::whiteColor();
    view.setBackgroundColor(&background);
    let foreground = NSColor::blackColor();
    view.setTextColor(Some(&foreground));
    view.setFrameOrigin(NSPoint::new(16.0, 16.0));
    container.addSubview(&view);
    host.set_content_view(&container);

    host.input().replace_text(&view, "W");
    harness.settle(2);

    let expectation = TextExpectation::new(
        "W",
        Rect::new(Point::new(16.0, 16.0), Size::new(56.0, 56.0)),
    )
    .with_font_name("Menlo-Regular")
    .with_point_size(48.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text-clipped");
    assert_text_renders(
        &host.text_renderer(harness.main_thread_marker()),
        &expectation,
        &artifact_dir,
        &TextAssertionConfig {
            compare: CompareConfig {
                channel_tolerance: 24,
                match_threshold: 0.96,
                generate_diff: true,
            },
            write_diff: true,
        },
    )
    .expect("clipped text regions should match the AppKit reference rendering");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn rendered_text_assertion_matches_negative_origin_region(harness: AppKitHarness) {
    let host = harness.create_window(120.0, 120.0);
    let container = make_view(harness.main_thread_marker(), NSSize::new(120.0, 120.0));
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(80.0, 80.0));
    let font_name = NSString::from_str("Menlo-Regular");
    let font = NSFont::fontWithName_size(&font_name, 24.0)
        .expect("Menlo-Regular font should be available on macOS");
    view.setFont(Some(&font));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    view.setDrawsBackground(true);
    let background = NSColor::whiteColor();
    view.setBackgroundColor(&background);
    let foreground = NSColor::blackColor();
    view.setTextColor(Some(&foreground));
    view.setFrameOrigin(NSPoint::new(-20.0, -12.0));
    container.addSubview(&view);
    host.set_content_view(&container);

    host.input().replace_text(&view, "I");
    harness.settle(2);

    let expectation = TextExpectation::new(
        "I",
        Rect::new(Point::new(-20.0, -12.0), Size::new(80.0, 80.0)),
    )
    .with_font_name("Menlo-Regular")
    .with_point_size(24.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text-negative-origin");
    assert_text_renders(
        &host.text_renderer(harness.main_thread_marker()),
        &expectation,
        &artifact_dir,
        &TextAssertionConfig {
            compare: CompareConfig {
                channel_tolerance: 24,
                match_threshold: 0.96,
                generate_diff: true,
            },
            write_diff: true,
        },
    )
    .expect("negative-origin text regions should match the visible AppKit rendering");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn wait_until_flushes_runloop_between_attempts(harness: AppKitHarness) {
    let mut ticks = 0usize;
    let attempts = harness
        .wait_until(
            PollOptions {
                timeout: std::time::Duration::from_millis(120),
                interval: std::time::Duration::from_millis(1),
            },
            || {
                ticks += 1;
                ticks >= 2
            },
        )
        .expect("eventual condition should succeed");

    assert!(attempts >= 2);
}

fn make_view(mtm: MainThreadMarker, size: NSSize) -> Retained<NSView> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
    NSView::initWithFrame(NSView::alloc(mtm), frame)
}

fn make_text_view(mtm: MainThreadMarker, size: NSSize) -> Retained<NSTextView> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
    NSTextView::initWithFrame(NSTextView::alloc(mtm), frame)
}

fn make_clip_view(mtm: MainThreadMarker, size: NSSize) -> Retained<NSClipView> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
    NSClipView::initWithFrame(NSClipView::alloc(mtm), frame)
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "glasscheck-appkit-{prefix}-{}-{}-{}",
        std::process::id(),
        nanos,
        count
    ));
    std::fs::create_dir_all(&path).expect("temporary artifact directory should be creatable");
    path
}

fn family_font(
    mtm: MainThreadMarker,
    family: &str,
    weight: u16,
    italic: bool,
    point_size: f64,
) -> Retained<NSFont> {
    let manager = NSFontManager::sharedFontManager(mtm);
    let family = NSString::from_str(family);
    let mut traits = NSFontTraitMask::empty();
    if italic {
        traits |= NSFontTraitMask::ItalicFontMask;
    }
    if weight >= 600 {
        traits |= NSFontTraitMask::BoldFontMask;
    }
    manager
        .fontWithFamily_traits_weight_size(
            &family,
            traits,
            ((weight.clamp(100, 900) - 100) / 100) as isize,
            point_size,
        )
        .expect("expected family font should be available on macOS")
}
