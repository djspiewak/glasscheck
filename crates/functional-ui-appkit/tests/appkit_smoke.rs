#![cfg(target_os = "macos")]

use functional_ui_appkit::{AppKitHarness, InstrumentedView};
use functional_ui_core::{compare_images, CompareConfig, PollOptions, Role, Selector};
use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSClipView, NSTextView, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

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
        functional_ui_core::Rect::new(
            functional_ui_core::Point::new(expected.origin.x, expected.origin.y),
            functional_ui_core::Size::new(expected.size.width, expected.size.height),
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
