#![cfg(target_os = "linux")]

use std::cell::Cell;
use std::rc::Rc;

use std::time::Duration;

use glasscheck_core::{
    compare_images, AnchoredTextExpectation, CompareConfig, NodePredicate, Point, PollOptions,
    Rect, RegionSpec, RelativeBounds, RgbaColor, Role, Selector, Size, TextAssertionConfig,
};
use glasscheck_gtk::{GtkHarness, InstrumentedWidget};
use gtk4::gdk;
use gtk4::prelude::*;

fn main() {
    let harness = GtkHarness::new();

    run("capture_returns_non_empty_image", || {
        capture_returns_non_empty_image(harness)
    });
    run(
        "root_attachment_starts_window_offscreen_for_all_connected_displays",
        || root_attachment_starts_window_offscreen_for_all_connected_displays(harness),
    );
    run(
        "input_activation_keeps_window_offscreen_for_all_connected_displays",
        || input_activation_keeps_window_offscreen_for_all_connected_displays(harness),
    );
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
    wait_for_window_capture(harness, &host, &root);
    let image = host.capture().expect("window capture should succeed");
    assert!(image.width > 0);
    assert!(image.height > 0);
}

fn root_attachment_starts_window_offscreen_for_all_connected_displays(harness: GtkHarness) {
    let host = harness.create_window(240.0, 120.0);
    let root = fixed_root(240, 120);
    host.set_root(&root);
    wait_for_window_map(harness, host.window());

    let display = gtk4::prelude::WidgetExt::display(host.window());
    if !display.backend().is_x11() {
        eprintln!("non-X11 backend detected; skipping live frame placement assertion");
        return;
    }

    if let Some(displays) = connected_display_rects(&display) {
        let frame = x11_window_frame(host.window()).expect("X11 toplevel frame should be readable");
        assert_frame_is_offscreen_for_displays(frame, &displays);
    }
}

fn input_activation_keeps_window_offscreen_for_all_connected_displays(harness: GtkHarness) {
    let host = harness.create_window(240.0, 120.0);
    let root = fixed_root(240, 120);
    let button = gtk4::Button::with_label("Run");
    button.set_size_request(90, 30);
    root.put(&button, 20.0, 20.0);
    host.set_root(&root);
    wait_for_window_map(harness, host.window());

    host.input().move_mouse(Point::new(24.0, 24.0));
    wait_for_window_map(harness, host.window());

    let display = gtk4::prelude::WidgetExt::display(host.window());
    if !display.backend().is_x11() {
        eprintln!("non-X11 backend detected; skipping live frame placement assertion");
        return;
    }

    if let Some(displays) = connected_display_rects(&display) {
        let frame = x11_window_frame(host.window()).expect("X11 toplevel frame should be readable");
        assert_frame_is_offscreen_for_displays(frame, &displays);
    }
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
    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_secs(1),
                interval: Duration::from_millis(10),
            },
            || host.capture_subtree(&text_view).is_some(),
        )
        .expect("text view should become capturable before typing");

    let before = host
        .capture_subtree(&text_view)
        .expect("text view should capture before typing");
    host.input().replace_text(&text_view, "after");
    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_secs(1),
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
    wait_for_window_capture(harness, &host, &root);

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
    wait_for_window_capture(harness, &host, &root);

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
    wait_for_window_capture(harness, &host, &root);

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

fn wait_for_window_map(harness: GtkHarness, window: &gtk4::Window) {
    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_secs(1),
                interval: Duration::from_millis(10),
            },
            || window.surface().is_some_and(|surface| surface.is_mapped()),
        )
        .expect("GTK toplevel surface should become mapped");
}

fn wait_for_window_capture(
    harness: GtkHarness,
    host: &glasscheck_gtk::GtkWindowHost,
    root: &impl IsA<gtk4::Widget>,
) {
    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_secs(1),
                interval: Duration::from_millis(10),
            },
            || root.as_ref().allocated_width() > 1 && root.as_ref().allocated_height() > 1,
        )
        .expect("root should receive a real allocation before capture");
    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_secs(1),
                interval: Duration::from_millis(10),
            },
            || host.capture().is_some(),
        )
        .expect("window capture should become available");
}

fn connected_display_rects(display: &gdk::Display) -> Option<Vec<DisplayRect>> {
    let displays: Vec<DisplayRect> = display
        .monitors()
        .snapshot()
        .into_iter()
        .filter_map(|object| object.downcast::<gdk::Monitor>().ok())
        .map(|monitor| {
            let geometry = monitor.geometry();
            DisplayRect::new(
                geometry.x(),
                geometry.y(),
                geometry.width(),
                geometry.height(),
            )
        })
        .collect();
    if displays.is_empty() {
        eprintln!(
            "headless context detected; skipping live display intersection assertion because GDK reported no connected monitors"
        );
        return None;
    }
    Some(displays)
}

fn assert_frame_is_offscreen_for_displays(frame: DisplayRect, displays: &[DisplayRect]) {
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

fn rects_intersect(lhs: DisplayRect, rhs: DisplayRect) -> bool {
    lhs.x < rhs.x + rhs.width
        && rhs.x < lhs.x + lhs.width
        && lhs.y < rhs.y + rhs.height
        && rhs.y < lhs.y + lhs.height
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DisplayRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl DisplayRect {
    fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

fn x11_window_frame(window: &gtk4::Window) -> Option<DisplayRect> {
    use std::mem::MaybeUninit;
    use std::os::raw::{c_int, c_uint, c_ulong};

    use gtk4::glib::translate::ToGlibPtr;

    gtk4::prelude::WidgetExt::realize(window);
    let surface = window.surface()?;
    let display = surface.display();
    if !display.backend().is_x11() {
        return None;
    }

    unsafe {
        let xdisplay = gdk_x11_display_get_xdisplay(display.to_glib_none().0);
        let xid = gdk_x11_surface_get_xid(surface.to_glib_none().0);
        if xdisplay.is_null() || xid == 0 {
            return None;
        }
        XSync(xdisplay, 0);

        let mut root = MaybeUninit::<c_ulong>::uninit();
        let mut x = MaybeUninit::<c_int>::uninit();
        let mut y = MaybeUninit::<c_int>::uninit();
        let mut width = MaybeUninit::<c_uint>::uninit();
        let mut height = MaybeUninit::<c_uint>::uninit();
        let mut border = MaybeUninit::<c_uint>::uninit();
        let mut depth = MaybeUninit::<c_uint>::uninit();

        if XGetGeometry(
            xdisplay,
            xid,
            root.as_mut_ptr(),
            x.as_mut_ptr(),
            y.as_mut_ptr(),
            width.as_mut_ptr(),
            height.as_mut_ptr(),
            border.as_mut_ptr(),
            depth.as_mut_ptr(),
        ) == 0
        {
            return None;
        }

        let root = root.assume_init();
        let mut root_x = 0;
        let mut root_y = 0;
        let mut child = 0;
        if XTranslateCoordinates(
            xdisplay,
            xid,
            root,
            0,
            0,
            &mut root_x,
            &mut root_y,
            &mut child,
        ) == 0
        {
            return None;
        }

        Some(DisplayRect::new(
            root_x,
            root_y,
            width.assume_init() as i32,
            height.assume_init() as i32,
        ))
    }
}

#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gdk_x11_display_get_xdisplay(
        display: *mut gtk4::gdk::ffi::GdkDisplay,
    ) -> *mut std::ffi::c_void;
    fn gdk_x11_surface_get_xid(surface: *mut gtk4::gdk::ffi::GdkSurface) -> std::os::raw::c_ulong;
}

#[link(name = "X11")]
unsafe extern "C" {
    fn XGetGeometry(
        display: *mut std::ffi::c_void,
        drawable: std::os::raw::c_ulong,
        root_return: *mut std::os::raw::c_ulong,
        x_return: *mut std::os::raw::c_int,
        y_return: *mut std::os::raw::c_int,
        width_return: *mut std::os::raw::c_uint,
        height_return: *mut std::os::raw::c_uint,
        border_width_return: *mut std::os::raw::c_uint,
        depth_return: *mut std::os::raw::c_uint,
    ) -> std::os::raw::c_int;
    fn XTranslateCoordinates(
        display: *mut std::ffi::c_void,
        src_w: std::os::raw::c_ulong,
        dest_w: std::os::raw::c_ulong,
        src_x: std::os::raw::c_int,
        src_y: std::os::raw::c_int,
        dest_x_return: *mut std::os::raw::c_int,
        dest_y_return: *mut std::os::raw::c_int,
        child_return: *mut std::os::raw::c_ulong,
    ) -> std::os::raw::c_int;
    fn XSync(display: *mut std::ffi::c_void, discard: std::os::raw::c_int) -> std::os::raw::c_int;
}
