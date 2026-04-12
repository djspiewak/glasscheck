#![cfg(target_os = "linux")]

use std::cell::Cell;
use std::rc::Rc;

use std::time::Duration;

use glasscheck_core::{
    compare_images, AnchoredTextExpectation, CompareConfig, NodePredicate, Point, PollOptions,
    Rect, RegionSpec, RelativeBounds, RgbaColor, Role, Selector, Size, TextAssertionConfig,
};
use glasscheck_gtk::{GtkHarness, InstrumentedWidget};
use gtk4::prelude::*;

fn main() {
    let harness = GtkHarness::new();

    run("capture_returns_non_empty_image", || {
        capture_returns_non_empty_image(harness)
    });
    run("query_reports_registered_widget_geometry", || {
        query_reports_registered_widget_geometry(harness)
    });
    run("direct_text_input_changes_rendered_content", || {
        direct_text_input_changes_rendered_content(harness)
    });
    run("capture_region_matches_registered_widget_bounds", || {
        capture_region_matches_registered_widget_bounds(harness)
    });
    run("rendered_text_assertion_matches_live_text", || {
        rendered_text_assertion_matches_live_text(harness)
    });
    run("anchored_text_assertion_matches_semantic_region", || {
        anchored_text_assertion_matches_semantic_region(harness)
    });
    run("click_button_activates_once", || {
        click_button_activates_once(harness)
    });
}

fn run(name: &str, test: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
    match result {
        Ok(()) => println!("test {name} ... ok"),
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

fn capture_returns_non_empty_image(harness: GtkHarness) {
    let host = harness.create_window(240.0, 120.0);
    let root = fixed_root(240, 120);
    host.set_root(&root);
    harness.settle(4);

    let image = host.capture().expect("window capture should succeed");
    assert!(image.width > 0);
    assert!(image.height > 0);
}

fn query_reports_registered_widget_geometry(harness: GtkHarness) {
    let host = harness.create_window(240.0, 120.0);
    let root = fixed_root(240, 120);
    let button = gtk4::Button::with_label("Run");
    button.set_size_request(80, 24);
    root.put(&button, 10.0, 12.0);
    host.set_root(&root);
    host.register_node(
        &button,
        InstrumentedWidget {
            id: Some("run".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
        },
    );
    harness.settle(4);

    let query_root = host.query_root();
    let node = query_root
        .find(&Selector::by_id("run"))
        .expect("registered widget should exist");
    let allocated_width = button.allocated_width() as f64;
    let allocated_height = button.allocated_height() as f64;
    let root_height = root.allocated_height() as f64;
    assert_eq!(
        node.rect,
        Rect::new(
            Point::new(10.0, root_height - 12.0 - allocated_height),
            Size::new(allocated_width, allocated_height),
        )
    );
}

fn direct_text_input_changes_rendered_content(harness: GtkHarness) {
    let host = harness.create_window(320.0, 160.0);
    let root = fixed_root(320, 160);
    let text_view = styled_text_view("before", "glasscheck-live-text");
    text_view.set_size_request(180, 48);
    root.put(&text_view, 24.0, 24.0);
    host.set_root(&root);
    harness.settle(4);

    let before = host
        .capture_subtree(&text_view)
        .expect("text view should capture before typing");
    host.input().replace_text(&text_view, "after");
    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_millis(250),
                interval: Duration::from_millis(10),
            },
            || host.capture_subtree(&text_view).is_some(),
        )
        .expect("text view should become capturable after typing");
    let after = host
        .capture_subtree(&text_view)
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
    assert!(!result.passed);
}

fn capture_region_matches_registered_widget_bounds(harness: GtkHarness) {
    let host = harness.create_window(320.0, 160.0);
    let root = fixed_root(320, 160);
    let button = gtk4::Button::with_label("Run");
    button.set_size_request(90, 30);
    root.put(&button, 30.0, 20.0);
    host.set_root(&root);
    host.register_node(
        &button,
        InstrumentedWidget {
            id: Some("run".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
        },
    );
    harness.settle(4);

    let region = host
        .capture_region(&RegionSpec::node(NodePredicate::id_eq("run")))
        .expect("button region should capture");
    let expected = host
        .capture_subtree(&button)
        .expect("button subtree should capture");
    let result = compare_images(
        &region,
        &expected,
        &CompareConfig {
            channel_tolerance: 8,
            match_threshold: 0.97,
            generate_diff: false,
        },
    );
    assert!(result.passed);
}

fn rendered_text_assertion_matches_live_text(harness: GtkHarness) {
    let host = harness.create_window(320.0, 160.0);
    let root = fixed_root(320, 160);
    let text_view = styled_text_view("Functional UI", "glasscheck-live-text");
    text_view.set_size_request(180, 48);
    root.put(&text_view, 24.0, 24.0);
    host.set_root(&root);
    harness.settle(4);

    host.text_renderer()
        .assert_text_renders_anchored(
            &AnchoredTextExpectation::new(
                "Functional UI",
                RegionSpec::root().subregion(RelativeBounds::new(
                    24.0 / 320.0,
                    88.0 / 160.0,
                    180.0 / 320.0,
                    48.0 / 160.0,
                )),
            )
            .with_font_family("Sans")
            .with_point_size(14.0)
            .with_foreground(RgbaColor::new(0, 0, 0, 255))
            .with_background(RgbaColor::new(255, 255, 255, 255)),
            &temp_dir("gtk-smoke-anchored"),
            &TextAssertionConfig::default(),
        )
        .expect("anchored text assertion should pass");
}

fn anchored_text_assertion_matches_semantic_region(harness: GtkHarness) {
    let host = harness.create_window(320.0, 160.0);
    let root = fixed_root(320, 160);
    let text_view = styled_text_view("Connected", "glasscheck-live-text");
    text_view.set_size_request(180, 48);
    root.put(&text_view, 24.0, 24.0);
    host.set_root(&root);
    host.register_node(
        &text_view,
        InstrumentedWidget {
            id: Some("status".into()),
            role: Some(Role::TextInput),
            label: Some("Status".into()),
        },
    );
    harness.settle(4);

    let expectation = AnchoredTextExpectation::new(
        "Connected",
        RegionSpec::node(NodePredicate::id_eq("status")).subregion(RelativeBounds::full()),
    )
    .with_font_family("Sans")
    .with_point_size(14.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255));

    host.text_renderer()
        .assert_text_renders_anchored(
            &expectation,
            &temp_dir("gtk-smoke-semantic"),
            &TextAssertionConfig::default(),
        )
        .expect("semantic anchored text assertion should pass");
}

fn click_button_activates_once(harness: GtkHarness) {
    let host = harness.create_window(240.0, 120.0);
    let root = fixed_root(240, 120);
    let button = gtk4::Button::with_label("Run");
    let activations = Rc::new(Cell::new(0usize));
    let seen = activations.clone();
    button.connect_clicked(move |_| seen.set(seen.get() + 1));
    button.set_size_request(90, 30);
    root.put(&button, 20.0, 20.0);
    host.set_root(&root);
    host.register_node(
        &button,
        InstrumentedWidget {
            id: Some("run".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
        },
    );
    harness.settle(4);

    host.click_node(&NodePredicate::id_eq("run"))
        .expect("semantic click should succeed");
    harness.settle(2);

    assert_eq!(activations.get(), 1);
}

fn fixed_root(width: i32, height: i32) -> gtk4::Fixed {
    let root = gtk4::Fixed::new();
    root.set_size_request(width, height);
    install_css(
        &root,
        "#glasscheck-live-root { background-color: rgba(255,255,255,1.0); }",
    );
    root.set_widget_name("glasscheck-live-root");
    root
}

fn styled_text_view(text: &str, name: &str) -> gtk4::TextView {
    let text_view = gtk4::TextView::new();
    text_view.set_widget_name(name);
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(gtk4::WrapMode::None);
    text_view.buffer().set_text(text);
    install_css(
        &text_view,
        &format!(
            "#{name} {{ background-color: rgba(255,255,255,1.0); color: rgba(0,0,0,1.0); font-family: \"Sans\"; font-size: 14pt; }}"
        ),
    );
    text_view
}

fn install_css(widget: &impl IsA<gtk4::Widget>, css: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css);
    gtk4::style_context_add_provider_for_display(
        &widget.display(),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    std::mem::forget(provider);
}

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}
