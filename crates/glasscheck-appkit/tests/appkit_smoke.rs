#![cfg(target_os = "macos")]

use std::cell::Cell;

use glasscheck_appkit::{AppKitHarness, InstrumentedView};
use glasscheck_core::{
    assert_text_renders, compare_images, load_png, AnchoredTextExpectation, CompareConfig, Point,
    PollOptions, Rect, RegionResolveError, RegionSpec, RelativeBounds, RgbaColor, Role, Selector,
    Size, TextAssertionConfig, TextAssertionError, TextExpectation, TextMatch,
};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSBezierPath, NSClipView, NSColor, NSEvent, NSFont, NSFontManager, NSFontTraitMask, NSGradient,
    NSScreen, NSTextView, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

fn main() {
    let mtm = MainThreadMarker::new().expect("AppKit tests must run on the main thread");
    let harness = AppKitHarness::new(mtm);

    run("capture_returns_non_empty_image", || {
        capture_returns_non_empty_image(harness)
    });
    run(
        "window_creation_starts_offscreen_for_all_connected_displays",
        || window_creation_starts_offscreen_for_all_connected_displays(harness),
    );
    run("query_reports_registered_view_geometry", || {
        query_reports_registered_view_geometry(harness)
    });
    run(
        "capture_resize_keeps_window_offscreen_for_all_connected_displays",
        || capture_resize_keeps_window_offscreen_for_all_connected_displays(harness),
    );
    run("direct_text_input_changes_rendered_content", || {
        direct_text_input_changes_rendered_content(harness)
    });
    run("capture_region_matches_registered_view_bounds", || {
        capture_region_matches_registered_view_bounds(harness)
    });
    run("capture_region_reports_missing_semantic_match", || {
        capture_region_reports_missing_semantic_match(harness)
    });
    run("rendered_text_assertion_matches_live_text", || {
        rendered_text_assertion_matches_live_text(harness)
    });
    run("rendered_text_assertion_reports_visual_regression", || {
        rendered_text_assertion_reports_visual_regression(harness)
    });
    run("anchored_text_assertion_matches_semantic_region", || {
        anchored_text_assertion_matches_semantic_region(harness)
    });
    run("anchored_text_assertion_reports_visual_regression", || {
        anchored_text_assertion_reports_visual_regression(harness)
    });
    run("anchored_text_assertion_reports_ambiguous_match", || {
        anchored_text_assertion_reports_ambiguous_match(harness)
    });
    run(
        "rendered_text_assertion_supports_family_weight_and_italic",
        || rendered_text_assertion_supports_family_weight_and_italic(harness),
    );
    run(
        "rendered_text_assertion_matches_clipped_text_region",
        || rendered_text_assertion_matches_clipped_text_region(harness),
    );
    run(
        "rendered_text_assertion_matches_negative_origin_region",
        || rendered_text_assertion_matches_negative_origin_region(harness),
    );
    run(
        "rendered_text_assertion_reports_negative_origin_regression",
        || rendered_text_assertion_reports_negative_origin_regression(harness),
    );
    run("capture_region_matches_negative_origin_region", || {
        capture_region_matches_negative_origin_region(harness)
    });
    run("document_view_capture_tracks_scrolled_viewport", || {
        document_view_capture_tracks_scrolled_viewport(harness)
    });
    run(
        "click_hotspot_reveals_rounded_gradient_and_anchored_text",
        || click_hotspot_reveals_rounded_gradient_and_anchored_text(harness),
    );
    run(
        "click_outside_hotspot_does_not_activate_gradient_scene",
        || click_outside_hotspot_does_not_activate_gradient_scene(harness),
    );
    run(
        "repeated_hotspot_clicks_leave_gradient_scene_stable",
        || repeated_hotspot_clicks_leave_gradient_scene_stable(harness),
    );
    run(
        "interactive_gradient_scene_reports_text_content_regression",
        || interactive_gradient_scene_reports_text_content_regression(harness),
    );
    run(
        "interactive_gradient_scene_reports_text_size_regression",
        || interactive_gradient_scene_reports_text_size_regression(harness),
    );
    run(
        "interactive_gradient_scene_reports_text_color_regression",
        || interactive_gradient_scene_reports_text_color_regression(harness),
    );
    run("click_dispatches_single_mouse_down", || {
        click_dispatches_single_mouse_down(harness)
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
    harness.settle(4);

    let image = host.capture().expect("window capture should succeed");
    assert!(image.width > 0);
    assert!(image.height > 0);
}

fn window_creation_starts_offscreen_for_all_connected_displays(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 120.0);
    harness.settle(2);

    if let Some(displays) = connected_display_frames(harness.main_thread_marker()) {
        assert_frame_is_offscreen_for_displays(host.window().frame(), &displays);
    }
}

fn capture_resize_keeps_window_offscreen_for_all_connected_displays(harness: AppKitHarness) {
    let host = harness.create_window(10.0, 10.0);
    let view = make_view(harness.main_thread_marker(), NSSize::new(10.0, 10.0));
    host.set_content_view(&view);
    harness.settle(2);

    let _ = host
        .capture()
        .expect("capture should repair undersized windows before rendering");

    let frame = host.window().frame();
    assert!(frame.size.width >= 800.0);
    assert!(frame.size.height >= 600.0);
    if let Some(displays) = connected_display_frames(harness.main_thread_marker()) {
        assert_frame_is_offscreen_for_displays(frame, &displays);
    }
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
            selectors: Vec::new(),
        },
    );

    let scene = host.snapshot_scene();
    let node = scene
        .node(
            scene
                .find(&Selector::id_eq("root"))
                .expect("registered view should exist"),
        )
        .unwrap();
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
            selectors: Vec::new(),
        },
    );

    harness.settle(4);
    let before = host
        .capture_view(&view)
        .expect("empty text view should capture successfully");

    host.input().replace_text(&view, "Functional UI");
    harness.settle(4);

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

fn capture_region_matches_registered_view_bounds(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 160.0);
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(180.0, 80.0));
    view.setFrameOrigin(NSPoint::new(24.0, 18.0));
    host.set_content_view(&view);
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor Panel".into()),
            selectors: Vec::new(),
        },
    );
    host.input().replace_text(&view, "Functional UI");
    harness.settle(4);

    let expected = host
        .capture_view(&view)
        .expect("registered view should capture directly");
    let actual = host
        .capture_region(&RegionSpec::node(Selector::label(TextMatch::contains(
            "Panel",
        ))))
        .expect("semantic region capture should succeed");

    let result = compare_images(
        &actual,
        &expected,
        &CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: false,
        },
    );
    assert!(
        result.passed,
        "semantic region capture should match direct view capture"
    );
}

fn capture_region_reports_missing_semantic_match(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 160.0);
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(180.0, 80.0));
    host.set_content_view(&view);
    harness.settle(2);

    let error = host
        .capture_region(&RegionSpec::node(Selector::label(TextMatch::contains(
            "Missing",
        ))))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::NotFound(_)));
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
        &host.text_renderer(),
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

fn rendered_text_assertion_reports_visual_regression(harness: AppKitHarness) {
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
        "XXXXXXXXXXXX",
        Rect::new(Point::new(0.0, 0.0), Size::new(320.0, 160.0)),
    )
    .with_point_size(18.0)
    .with_foreground(RgbaColor::new(255, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text-regression");
    let error = assert_text_renders(
        &host.text_renderer(),
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
    .unwrap_err();

    match error {
        TextAssertionError::Mismatch {
            expectation: failed_expectation,
            artifacts,
            result,
        } => {
            assert_eq!(failed_expectation.content, "XXXXXXXXXXXX");
            assert!(!result.passed);
            assert!(result.mismatched_pixels > 0);
            assert!(artifacts.actual_path.exists());
            assert!(artifacts.expected_path.exists());
            let actual = load_png(&artifacts.actual_path).expect("actual artifact should load");
            let expected =
                load_png(&artifacts.expected_path).expect("expected artifact should load");
            assert_eq!(actual.width, expected.width);
            assert_eq!(actual.height, expected.height);
            assert!(artifacts
                .diff_path
                .as_ref()
                .is_none_or(|path| path.exists()));
            if let Some(diff_path) = artifacts.diff_path.as_ref() {
                let diff = load_png(diff_path).expect("diff artifact should load");
                assert_eq!(actual.width, diff.width);
                assert_eq!(actual.height, diff.height);
            }
        }
        other => panic!("expected mismatch error, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn anchored_text_assertion_reports_ambiguous_match(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 160.0);
    let container = make_view(harness.main_thread_marker(), NSSize::new(320.0, 160.0));
    let first = make_text_view(harness.main_thread_marker(), NSSize::new(140.0, 60.0));
    let second = make_text_view(harness.main_thread_marker(), NSSize::new(140.0, 60.0));
    second.setFrameOrigin(NSPoint::new(160.0, 0.0));
    container.addSubview(&first);
    container.addSubview(&second);
    host.set_content_view(&container);
    host.register_view(
        &first,
        InstrumentedView {
            id: Some("left-editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &second,
        InstrumentedView {
            id: Some("right-editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
            selectors: Vec::new(),
        },
    );

    let expectation = AnchoredTextExpectation::new(
        "Functional UI",
        RegionSpec::node(Selector::label(TextMatch::exact("Editor"))),
    );

    let artifact_dir = unique_temp_dir("anchored-rendered-text-ambiguous");
    let error = host
        .text_renderer()
        .assert_text_renders_anchored(&expectation, &artifact_dir, &TextAssertionConfig::default())
        .unwrap_err();

    assert!(matches!(
        error,
        glasscheck_appkit::AppKitAnchoredTextError::Resolve(
            RegionResolveError::MultipleMatches { .. }
        )
    ));

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn anchored_text_assertion_matches_semantic_region(harness: AppKitHarness) {
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
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor Canvas".into()),
            selectors: Vec::new(),
        },
    );

    host.input().replace_text(&view, "Functional UI");
    harness.settle(2);

    let expectation = AnchoredTextExpectation::new(
        "Functional UI",
        RegionSpec::node(Selector::and(vec![
            Selector::role_eq(Role::TextInput),
            Selector::label(TextMatch::contains("Canvas")),
        ])),
    )
    .with_point_size(18.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("anchored-rendered-text");
    host.text_renderer()
        .assert_text_renders_anchored(
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
        .expect("anchored rendered text should match the AppKit reference rendering");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn anchored_text_assertion_reports_visual_regression(harness: AppKitHarness) {
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
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor Canvas".into()),
            selectors: Vec::new(),
        },
    );

    host.input().replace_text(&view, "Functional UI");
    harness.settle(2);

    let expectation = AnchoredTextExpectation::new(
        "XXXXXXXXXXXX",
        RegionSpec::node(Selector::and(vec![
            Selector::role_eq(Role::TextInput),
            Selector::label(TextMatch::contains("Canvas")),
        ])),
    )
    .with_point_size(18.0)
    .with_foreground(RgbaColor::new(255, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("anchored-rendered-text-regression");
    let error = host
        .text_renderer()
        .assert_text_renders_anchored(
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
        .unwrap_err();

    match error {
        glasscheck_appkit::AppKitAnchoredTextError::Assert(TextAssertionError::Mismatch {
            expectation: failed_expectation,
            artifacts,
            result,
        }) => {
            assert_eq!(failed_expectation.content, "XXXXXXXXXXXX");
            assert!(!result.passed);
            assert!(result.mismatched_pixels > 0);
            assert!(artifacts.actual_path.exists());
            assert!(artifacts.expected_path.exists());
            assert!(artifacts
                .diff_path
                .as_ref()
                .is_none_or(|path| path.exists()));
        }
        other => panic!("expected mismatch error, got {other:?}"),
    }

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
        &host.text_renderer(),
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
        &host.text_renderer(),
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
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("neg-origin".into()),
            role: Some(Role::TextInput),
            label: Some("Negative Origin".into()),
            selectors: Vec::new(),
        },
    );

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
        &host.text_renderer(),
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

fn rendered_text_assertion_reports_negative_origin_regression(harness: AppKitHarness) {
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
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("neg-origin".into()),
            role: Some(Role::TextInput),
            label: Some("Negative Origin".into()),
            selectors: Vec::new(),
        },
    );

    host.input().replace_text(&view, "I");
    harness.settle(2);

    let expectation = TextExpectation::new(
        "\n    I",
        Rect::new(Point::new(-20.0, -12.0), Size::new(80.0, 80.0)),
    )
    .with_font_name("Menlo-Regular")
    .with_point_size(24.0)
    .with_foreground(RgbaColor::new(255, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("rendered-text-negative-origin");
    let error = assert_text_renders(
        &host.text_renderer(),
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
    .unwrap_err();

    match error {
        TextAssertionError::Mismatch {
            expectation: failed_expectation,
            artifacts,
            result,
        } => {
            assert_eq!(failed_expectation.content, "\n    I");
            assert!(!result.passed);
            assert!(result.mismatched_pixels > 0);
            assert!(artifacts.actual_path.exists());
            assert!(artifacts.expected_path.exists());
            assert!(artifacts
                .diff_path
                .as_ref()
                .is_none_or(|path| path.exists()));
        }
        other => panic!("expected mismatch error, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn capture_region_matches_negative_origin_region(harness: AppKitHarness) {
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
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("neg-origin".into()),
            role: Some(Role::TextInput),
            label: Some("Negative Origin".into()),
            selectors: Vec::new(),
        },
    );

    host.input().replace_text(&view, "I");
    harness.settle(2);

    let window = host.capture().expect("window capture should succeed");
    let rect = host
        .resolve_region(&RegionSpec::node(Selector::label(TextMatch::exact(
            "Negative Origin",
        ))))
        .expect("negative-origin region should resolve");
    let region = host
        .capture_region(&RegionSpec::node(Selector::label(TextMatch::exact(
            "Negative Origin",
        ))))
        .expect("negative-origin region capture should succeed");
    let expected = window.crop(Rect::new(
        Point::new(
            rect.origin.x,
            (f64::from(window.height) - rect.origin.y - rect.size.height).max(0.0),
        ),
        rect.size,
    ));
    let result = compare_images(
        &region,
        &expected,
        &CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: false,
        },
    );
    assert!(result.passed);
}

fn document_view_capture_tracks_scrolled_viewport(harness: AppKitHarness) {
    let host = harness.create_window(140.0, 120.0);
    let clip = make_clip_view(harness.main_thread_marker(), NSSize::new(140.0, 120.0));
    let document = make_text_view(harness.main_thread_marker(), NSSize::new(140.0, 320.0));
    let font_name = NSString::from_str("Menlo-Regular");
    let font = NSFont::fontWithName_size(&font_name, 20.0)
        .expect("Menlo-Regular font should be available on macOS");
    document.setFont(Some(&font));
    if let Some(text_container) = unsafe { document.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    document.setDrawsBackground(true);
    let background = NSColor::whiteColor();
    document.setBackgroundColor(&background);
    let foreground = NSColor::blackColor();
    document.setTextColor(Some(&foreground));
    host.input().replace_text(
        &document,
        "line 01\nline 02\nline 03\nline 04\nline 05\nline 06\nline 07\nline 08\nline 09\nline 10\nline 11\nline 12",
    );

    clip.setDocumentView(Some(&document));
    clip.scrollToPoint(NSPoint::new(0.0, 120.0));
    host.set_content_view(&clip);
    harness.settle(4);

    let expected = host
        .capture_view(&clip)
        .expect("clip view should capture the viewport");
    let actual = host
        .capture_view(&document)
        .expect("document view capture should use the viewport rect");

    let result = compare_images(
        &actual,
        &expected,
        &CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: false,
        },
    );
    assert!(
        result.passed,
        "document capture should match the scrolled clip viewport"
    );
}

fn click_hotspot_reveals_rounded_gradient_and_anchored_text(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 320.0);
    let scene = make_gradient_scene(harness.main_thread_marker());
    host.set_content_view(&scene.root);
    register_gradient_scene(&host, &scene);
    harness.settle(2);

    let before = host
        .capture_region(&RegionSpec::node(Selector::id_eq("gradient-card")))
        .expect("gradient card region should capture before click");

    host.input().click(Point::new(332.0, 70.0));
    harness.settle(2);

    let after = host
        .capture_region(&RegionSpec::node(Selector::id_eq("gradient-card")))
        .expect("gradient card region should capture after click");

    assert_gradient_card_rendering(&after);

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
        "clicking the hotspot should change the rounded gradient card rendering"
    );

    let expectation = AnchoredTextExpectation::new(
        "Connected",
        RegionSpec::node(Selector::id_eq("gradient-card")).subregion(title_region_bounds()),
    )
    .with_font_name("Helvetica-BoldOblique")
    .with_point_size(18.0)
    .with_foreground(RgbaColor::new(255, 255, 255, 255));

    let artifact_dir = unique_temp_dir("interactive-gradient-scene");
    host.text_renderer()
        .assert_text_renders_anchored(
            &expectation,
            &artifact_dir,
            &interactive_gradient_scene_text_config(),
        )
        .expect("styled text should render relative to the gradient card after activation");

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn click_outside_hotspot_does_not_activate_gradient_scene(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 320.0);
    let scene = make_gradient_scene(harness.main_thread_marker());
    host.set_content_view(&scene.root);
    register_gradient_scene(&host, &scene);
    harness.settle(2);

    let before = host
        .capture_region(&RegionSpec::node(Selector::id_eq("gradient-card")))
        .expect("gradient card region should capture before click");

    host.input().click(Point::new(60.0, 60.0));
    harness.settle(2);

    let after = host
        .capture_region(&RegionSpec::node(Selector::id_eq("gradient-card")))
        .expect("gradient card region should capture after outside click");
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
        result.passed,
        "clicking outside the hotspot should not activate the gradient scene"
    );

    let artifact_dir = unique_temp_dir("interactive-gradient-scene-inactive");
    let error = host
        .text_renderer()
        .assert_text_renders_anchored(
            &interactive_gradient_scene_expectation("Connected")
                .with_font_name("Helvetica-BoldOblique")
                .with_point_size(18.0)
                .with_foreground(RgbaColor::new(255, 255, 255, 255)),
            &artifact_dir,
            &interactive_gradient_scene_text_config(),
        )
        .unwrap_err();

    match error {
        glasscheck_appkit::AppKitAnchoredTextError::Assert(TextAssertionError::Mismatch {
            result,
            artifacts,
            ..
        }) => {
            assert!(!result.passed);
            assert!(result.mismatched_pixels > 0);
            assert!(artifacts.actual_path.exists());
            assert!(artifacts.expected_path.exists());
        }
        other => panic!("expected mismatch error, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn repeated_hotspot_clicks_leave_gradient_scene_stable(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 320.0);
    let scene = make_gradient_scene(harness.main_thread_marker());
    host.set_content_view(&scene.root);
    register_gradient_scene(&host, &scene);
    harness.settle(2);

    host.input().click(Point::new(332.0, 70.0));
    harness.settle(2);
    let first = host
        .capture_region(&RegionSpec::node(Selector::id_eq("gradient-card")))
        .expect("gradient card region should capture after first click");

    host.input().click(Point::new(332.0, 70.0));
    harness.settle(2);
    let second = host
        .capture_region(&RegionSpec::node(Selector::id_eq("gradient-card")))
        .expect("gradient card region should capture after second click");

    let result = compare_images(
        &first,
        &second,
        &CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: false,
        },
    );
    assert!(
        result.passed,
        "repeated hotspot clicks should leave the activated scene stable"
    );
}

fn interactive_gradient_scene_reports_text_content_regression(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 320.0);
    let scene = make_gradient_scene(harness.main_thread_marker());
    host.set_content_view(&scene.root);
    register_gradient_scene(&host, &scene);
    harness.settle(2);

    host.input().click(Point::new(332.0, 70.0));
    harness.settle(2);

    let artifact_dir = unique_temp_dir("interactive-gradient-scene-wrong-content");
    let error = host
        .text_renderer()
        .assert_text_renders_anchored(
            &interactive_gradient_scene_expectation("XXXXXXXXXXXX")
                .with_font_name("Helvetica-BoldOblique")
                .with_point_size(18.0)
                .with_foreground(RgbaColor::new(255, 255, 255, 255)),
            &artifact_dir,
            &interactive_gradient_scene_text_config(),
        )
        .unwrap_err();

    assert_interactive_gradient_text_mismatch(error);
    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn interactive_gradient_scene_reports_text_size_regression(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 320.0);
    let scene = make_gradient_scene(harness.main_thread_marker());
    host.set_content_view(&scene.root);
    register_gradient_scene(&host, &scene);
    harness.settle(2);

    host.input().click(Point::new(332.0, 70.0));
    harness.settle(2);

    let artifact_dir = unique_temp_dir("interactive-gradient-scene-wrong-size");
    let error = host
        .text_renderer()
        .assert_text_renders_anchored(
            &interactive_gradient_scene_expectation("Connected")
                .with_font_name("Helvetica-BoldOblique")
                .with_point_size(30.0)
                .with_foreground(RgbaColor::new(255, 255, 255, 255)),
            &artifact_dir,
            &interactive_gradient_scene_text_config(),
        )
        .unwrap_err();

    assert_interactive_gradient_text_mismatch(error);
    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn interactive_gradient_scene_reports_text_color_regression(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 320.0);
    let scene = make_gradient_scene(harness.main_thread_marker());
    host.set_content_view(&scene.root);
    register_gradient_scene(&host, &scene);
    harness.settle(2);

    host.input().click(Point::new(332.0, 70.0));
    harness.settle(2);

    let artifact_dir = unique_temp_dir("interactive-gradient-scene-wrong-color");
    let error = host
        .text_renderer()
        .assert_text_renders_anchored(
            &interactive_gradient_scene_expectation("Connected")
                .with_font_name("Helvetica-BoldOblique")
                .with_point_size(18.0)
                .with_foreground(RgbaColor::new(0, 0, 0, 255)),
            &artifact_dir,
            &interactive_gradient_scene_text_config(),
        )
        .unwrap_err();

    assert_interactive_gradient_text_mismatch(error);
    let _ = std::fs::remove_dir_all(artifact_dir);
}

fn click_dispatches_single_mouse_down(harness: AppKitHarness) {
    let host = harness.create_window(200.0, 120.0);
    let view = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 120.0)),
    );
    host.set_content_view(&view);
    harness.settle(2);

    host.input().click(Point::new(40.0, 40.0));
    harness.settle(2);

    assert_eq!(
        view.ivars().mouse_downs.get(),
        1,
        "a synthesized click should deliver exactly one mouseDown"
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

fn connected_display_frames(mtm: MainThreadMarker) -> Option<Vec<NSRect>> {
    let displays: Vec<NSRect> = NSScreen::screens(mtm)
        .iter()
        .map(|screen| screen.frame())
        .collect();
    if displays.is_empty() {
        eprintln!(
            "headless context detected; skipping live display intersection assertion because NSScreen::screens returned no displays"
        );
        return None;
    }
    Some(displays)
}

fn assert_frame_is_offscreen_for_displays(frame: NSRect, displays: &[NSRect]) {
    assert!(
        !displays
            .iter()
            .copied()
            .any(|display| rects_intersect(frame, display)),
        "window frame {:?} should not intersect any connected display {:?}",
        frame,
        displays
    );
}

fn rects_intersect(lhs: NSRect, rhs: NSRect) -> bool {
    lhs.origin.x < rhs.origin.x + rhs.size.width
        && rhs.origin.x < lhs.origin.x + lhs.size.width
        && lhs.origin.y < rhs.origin.y + rhs.size.height
        && rhs.origin.y < lhs.origin.y + lhs.size.height
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

#[derive(Clone)]
struct InteractiveGradientSceneIvars {
    active: Cell<bool>,
    title_view: Retained<NSTextView>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = InteractiveGradientSceneIvars]
    struct InteractiveGradientSceneView;

    impl InteractiveGradientSceneView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            scene_background_color().setFill();
            NSBezierPath::fillRect(self.bounds());

            let hotspot = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                hotspot_rect(),
                10.0,
                10.0,
            );
            hotspot_outline_color().setStroke();
            hotspot.setLineWidth(2.0);
            hotspot.stroke();

            if !self.ivars().active.get() {
                return;
            }

            let card_rect = gradient_card_rect();
            let card_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                card_rect,
                gradient_card_corner_radius(),
                gradient_card_corner_radius(),
            );

            let gradient = NSGradient::initWithStartingColor_endingColor(
                NSGradient::alloc(),
                &gradient_start_color(),
                &gradient_end_color(),
            )
            .expect("gradient colors should produce an NSGradient");
            gradient.drawInBezierPath_angle(&card_path, -45.0);

            let border = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                card_rect,
                gradient_card_corner_radius(),
                gradient_card_corner_radius(),
            );
            border_color().setStroke();
            border.setLineWidth(1.0);
            border.stroke();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            if !self.mouse_inRect(point, hotspot_rect()) {
                return;
            }

            if self.ivars().active.replace(true) {
                return;
            }

            let title_view = &self.ivars().title_view;
            title_view.setString(&NSString::from_str("Connected"));
            title_view.setNeedsDisplay(true);
            self.setNeedsDisplay(true);
        }
    }
);

impl InteractiveGradientSceneView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        title_view: Retained<NSTextView>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(InteractiveGradientSceneIvars {
            active: Cell::new(false),
            title_view,
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

struct GradientScene {
    root: Retained<InteractiveGradientSceneView>,
    card_anchor: Retained<NSView>,
    title_view: Retained<NSTextView>,
}

#[derive(Clone, Default)]
struct CountingClickIvars {
    mouse_downs: Cell<usize>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = CountingClickIvars]
    struct CountingClickView;

    impl CountingClickView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            let next = self.ivars().mouse_downs.get() + 1;
            self.ivars().mouse_downs.set(next);
        }
    }
);

impl CountingClickView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CountingClickIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

fn register_gradient_scene(host: &glasscheck_appkit::AppKitWindowHost, scene: &GradientScene) {
    host.register_view(
        &scene.card_anchor,
        InstrumentedView {
            id: Some("gradient-card".into()),
            role: Some(Role::Container),
            label: Some("Gradient Card".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &scene.title_view,
        InstrumentedView {
            id: Some("gradient-title".into()),
            role: Some(Role::Label),
            label: Some("Gradient Title".into()),
            selectors: Vec::new(),
        },
    );
}

fn interactive_gradient_scene_expectation(content: &str) -> AnchoredTextExpectation {
    AnchoredTextExpectation::new(
        content,
        RegionSpec::node(Selector::id_eq("gradient-card")).subregion(title_region_bounds()),
    )
}

fn interactive_gradient_scene_text_config() -> TextAssertionConfig {
    TextAssertionConfig {
        compare: CompareConfig {
            channel_tolerance: 24,
            match_threshold: 0.93,
            generate_diff: true,
        },
        write_diff: true,
    }
}

fn assert_interactive_gradient_text_mismatch(error: glasscheck_appkit::AppKitAnchoredTextError) {
    match error {
        glasscheck_appkit::AppKitAnchoredTextError::Assert(TextAssertionError::Mismatch {
            result,
            artifacts,
            ..
        }) => {
            assert!(!result.passed);
            assert!(result.mismatched_pixels > 0);
            assert!(artifacts.actual_path.exists());
            assert!(artifacts.expected_path.exists());
            assert!(artifacts
                .diff_path
                .as_ref()
                .is_none_or(|path| path.exists()));
        }
        other => panic!("expected mismatch error, got {other:?}"),
    }
}

fn make_gradient_scene(mtm: MainThreadMarker) -> GradientScene {
    let card_anchor = make_view(mtm, NSSize::new(180.0, 120.0));
    card_anchor.setFrameOrigin(gradient_card_rect().origin);

    let title_view = make_text_view(mtm, NSSize::new(112.0, 26.0));
    title_view.setFrameOrigin(NSPoint::new(46.0, 241.0));
    title_view.setEditable(false);
    title_view.setSelectable(false);
    title_view.setDrawsBackground(false);
    title_view.setString(&NSString::from_str(""));
    let font_name = NSString::from_str("Helvetica-BoldOblique");
    let font = NSFont::fontWithName_size(&font_name, 18.0)
        .expect("Helvetica-BoldOblique should be available on macOS");
    title_view.setFont(Some(&font));
    let foreground = NSColor::whiteColor();
    title_view.setTextColor(Some(&foreground));
    if let Some(container) = unsafe { title_view.textContainer() } {
        container.setLineFragmentPadding(0.0);
    }
    title_view.setTextContainerInset(NSSize::new(0.0, 0.0));

    let root = InteractiveGradientSceneView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(480.0, 320.0)),
        title_view.clone(),
    );
    root.addSubview(&card_anchor);
    root.addSubview(&title_view);

    GradientScene {
        root,
        card_anchor,
        title_view,
    }
}

fn gradient_card_rect() -> NSRect {
    NSRect::new(NSPoint::new(24.0, 176.0), NSSize::new(180.0, 120.0))
}

fn gradient_card_corner_radius() -> f64 {
    18.0
}

fn hotspot_rect() -> NSRect {
    NSRect::new(NSPoint::new(284.0, 38.0), NSSize::new(96.0, 64.0))
}

fn title_region_bounds() -> RelativeBounds {
    let card = gradient_card_rect();
    let title = NSRect::new(NSPoint::new(46.0, 241.0), NSSize::new(112.0, 26.0));
    RelativeBounds::new(
        (title.origin.x - card.origin.x) / card.size.width,
        (title.origin.y - card.origin.y) / card.size.height,
        title.size.width / card.size.width,
        title.size.height / card.size.height,
    )
}

fn scene_background_color() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.10, 0.12, 0.16, 1.0)
}

fn hotspot_outline_color() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.88, 0.54, 0.29, 1.0)
}

fn gradient_start_color() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.33, 0.83, 0.93, 1.0)
}

fn gradient_end_color() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.09, 0.16, 0.48, 1.0)
}

fn border_color() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.96, 0.99, 1.0, 0.85)
}

fn assert_gradient_card_rendering(image: &glasscheck_core::Image) {
    let outside_corner = image
        .pixel_at(2, 2)
        .expect("top-left corner pixel should exist");
    assert_eq!(outside_corner, [26, 31, 41, 255]);
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
