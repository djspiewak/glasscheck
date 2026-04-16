#[cfg(target_os = "linux")]
use std::cell::Cell;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::rc::Rc;
#[cfg(target_os = "linux")]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use glasscheck::{
    AnchoredTextExpectation, AnchoredTextHarness, InputDriver, Point, PollOptions, Rect,
    RegionSpec, RelativeBounds, RgbaColor, Role, Selector, SemanticNode, SemanticProvider, Size,
    TextAssertionConfig,
};
#[cfg(target_os = "linux")]
use gtk4::prelude::*;

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    if !native_gtk_session_available() {
        eprintln!("skipping shared GTK facade contracts without a native GTK display session");
        return;
    }
    let harness =
        glasscheck::Harness::new().expect("GTK should initialize for shared facade contract tests");
    run_case("shared_window_host_surface_is_backend_neutral", || {
        let fixture = mount_fixture(&harness);
        exercise_host_contracts(&harness, &fixture.host);
    });
}

#[cfg(target_os = "linux")]
fn native_gtk_session_available() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(target_os = "linux")]
struct Fixture {
    host: glasscheck::WindowHost,
}

#[cfg(target_os = "linux")]
struct ClickStateProvider {
    clicked: Rc<Cell<bool>>,
}

#[cfg(target_os = "linux")]
impl SemanticProvider for ClickStateProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        let label = if self.clicked.get() {
            "Clicked"
        } else {
            "Idle"
        };
        vec![SemanticNode::new(
            "click-state",
            Role::Label,
            Rect::new(Point::new(24.0, 100.0), Size::new(120.0, 24.0)),
        )
        .with_label(label)]
    }
}

#[cfg(target_os = "linux")]
fn mount_fixture(harness: &glasscheck::Harness) -> Fixture {
    let host = harness.create_window(320.0, 160.0);
    let root = gtk4::Fixed::new();
    root.set_size_request(320, 160);

    let status = gtk4::TextView::new();
    status.set_size_request(180, 48);
    status.set_editable(false);
    status.set_cursor_visible(false);
    status.set_wrap_mode(gtk4::WrapMode::None);
    status.set_left_margin(0);
    status.set_right_margin(0);
    status.set_top_margin(0);
    status.set_bottom_margin(0);
    status.buffer().set_text("Ready");
    install_css(
        &status,
        ".shared-status { font-size: 14pt; color: #000000; background: #ffffff; }",
    );
    status.add_css_class("shared-status");

    let button = gtk4::Button::with_label("Run");
    button.set_size_request(100, 32);

    let clicked = Rc::new(Cell::new(false));
    let seen = clicked.clone();
    button.connect_clicked(move |_| seen.set(true));

    root.put(&status, 24.0, 24.0);
    root.put(&button, 224.0, 32.0);
    host.set_root(&root);
    host.register_node(
        &status,
        glasscheck::InstrumentedNode {
            id: Some("status-text".into()),
            role: Some(Role::TextInput),
            label: Some("Status".into()),
            ..Default::default()
        },
    );
    host.register_node(
        &button,
        glasscheck::InstrumentedNode {
            id: Some("run-button".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
            ..Default::default()
        },
    );
    host.set_scene_source(Box::new(ClickStateProvider { clicked }));

    harness
        .wait_until(
            PollOptions {
                timeout: Duration::from_secs(1),
                interval: Duration::from_millis(10),
            },
            || host.capture().is_some(),
        )
        .expect("fixture should become capturable");

    Fixture { host }
}

#[cfg(target_os = "linux")]
fn install_css(widget: &impl IsA<gtk4::Widget>, css: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css);
    gtk4::style_context_add_provider_for_display(
        &widget.display(),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[cfg(target_os = "linux")]
fn exercise_host_contracts(harness: &glasscheck::Harness, host: &glasscheck::WindowHost) {
    let scene = host.snapshot_scene();
    assert_eq!(node_label(&scene, "click-state"), Some("Idle"));
    assert_eq!(node_label(&scene, "run-button"), Some("Run"));

    let image = host.capture().expect("full-window capture should succeed");
    assert!(image.width > 0);
    assert!(image.height > 0);

    let text_region = RegionSpec::node(Selector::id_eq("status-text"));
    let resolved = host
        .resolve_region(&text_region)
        .expect("status text should resolve to a region");
    assert!(resolved.size.width > 0.0);
    assert!(resolved.size.height > 0.0);

    let crop = host
        .capture_region(&text_region)
        .expect("status text region capture should succeed");
    assert!(crop.width > 0);
    assert!(crop.height > 0);

    let input = host.input();
    assert_input_driver(&input);
    input.move_mouse(Point::new(resolved.origin.x + 2.0, resolved.origin.y + 2.0));

    let renderer = host.text_renderer();
    assert_text_harness(&renderer);
    renderer
        .assert_text_renders_anchored(
            &status_expectation("Ready"),
            &artifact_dir("gtk-facade-contracts"),
            &TextAssertionConfig::default(),
        )
        .expect("shared text renderer should work without backend-specific arguments");

    host.click_node(&Selector::id_eq("run-button"))
        .expect("semantic click should succeed");
    harness.settle(2);

    let scene = host.snapshot_scene();
    assert_eq!(node_label(&scene, "click-state"), Some("Clicked"));
}

#[cfg(target_os = "linux")]
fn status_expectation(content: &str) -> AnchoredTextExpectation {
    AnchoredTextExpectation::new(
        content,
        RegionSpec::node(Selector::id_eq("status-text")).subregion(RelativeBounds::full()),
    )
    .with_point_size(14.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255))
}

#[cfg(target_os = "linux")]
fn node_label<'a>(scene: &'a glasscheck::Scene, id: &str) -> Option<&'a str> {
    let handle = scene.find(&Selector::id_eq(id)).ok()?;
    scene.node(handle)?.label.as_deref()
}

#[cfg(target_os = "linux")]
fn assert_input_driver<T: InputDriver>(_: &T) {}

#[cfg(target_os = "linux")]
fn assert_text_harness<T: AnchoredTextHarness>(_: &T) {}

#[cfg(target_os = "linux")]
fn artifact_dir(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    path.push(format!("{label}-{stamp}"));
    fs::create_dir_all(&path).expect("artifact directory should be creatable");
    path
}

#[cfg(target_os = "linux")]
fn run_case(name: &str, test: impl FnOnce()) {
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
