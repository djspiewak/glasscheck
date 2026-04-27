#![cfg(target_os = "macos")]

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use glasscheck::{
    AnchoredTextExpectation, AnchoredTextHarness, InputDriver, Point, Rect, RegionSpec,
    RelativeBounds, RgbaColor, Role, Selector, SemanticNode, SemanticProvider, Size,
    TextAssertionConfig,
};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSButton, NSColor, NSFont, NSTextView, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

fn main() {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("AppKit tests must run on the main thread");
        std::process::exit(1);
    };
    let harness = glasscheck::Harness::new(mtm);
    run_case("shared_window_host_surface_is_backend_neutral", || {
        let fixture = mount_fixture(&harness);
        exercise_host_contracts(&harness, &fixture.host);
    });
}

struct Fixture {
    host: glasscheck::WindowHost,
    _button_target: Retained<ClickActionTarget>,
}

struct ClickStateProvider {
    clicked: Rc<Cell<bool>>,
}

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

fn mount_fixture(harness: &glasscheck::Harness) -> Fixture {
    let host = harness.create_window(320.0, 160.0);
    let root = make_view(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 160.0)),
    );
    let status = make_text_view(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(24.0, 24.0), NSSize::new(180.0, 48.0)),
    );
    let clicked = Rc::new(Cell::new(false));
    let button_target = ClickActionTarget::new(harness.main_thread_marker(), clicked.clone());
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Run"),
            Some(button_target.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    button.setFrame(NSRect::new(
        NSPoint::new(224.0, 32.0),
        NSSize::new(100.0, 32.0),
    ));
    root.addSubview(&status);
    root.addSubview(&button);

    host.set_content_view(&root);
    host.register_view(
        &status,
        glasscheck::InstrumentedView {
            id: Some("status-text".into()),
            role: Some(Role::TextInput),
            label: Some("Status".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &button,
        glasscheck::InstrumentedView {
            id: Some("run-button".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
            selectors: Vec::new(),
        },
    );
    host.set_scene_source(Box::new(ClickStateProvider { clicked }));
    harness.settle(2);

    Fixture {
        host,
        _button_target: button_target,
    }
}

fn make_view(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSView> {
    NSView::initWithFrame(NSView::alloc(mtm), frame)
}

fn make_text_view(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSTextView> {
    let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    view.setEditable(false);
    view.setSelectable(false);
    view.setDrawsBackground(true);
    view.setBackgroundColor(&NSColor::whiteColor());
    view.setTextColor(Some(&NSColor::blackColor()));
    view.setFont(Some(&NSFont::systemFontOfSize(14.0)));
    view.setString(&NSString::from_str("Ready"));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    view.setTextContainerInset(NSSize::new(0.0, 0.0));
    view
}

fn exercise_host_contracts(harness: &glasscheck::Harness, host: &glasscheck::WindowHost) {
    let _ = std::mem::size_of::<glasscheck::AppKitContextMenu>();
    let _ = std::mem::size_of::<glasscheck::AppKitContextMenuError>();

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
    input
        .move_mouse(Point::new(resolved.origin.x + 2.0, resolved.origin.y + 2.0))
        .expect("mouse move should succeed through the shared facade");
    input
        .key_press_queued("a", glasscheck::KeyModifiers::default())
        .expect("queued key press should succeed through the shared facade");

    let renderer = host.text_renderer();
    assert_text_harness(&renderer);
    renderer
        .assert_text_renders_anchored(
            &status_expectation("Ready"),
            &artifact_dir("appkit-facade-contracts"),
            &TextAssertionConfig::default(),
        )
        .expect("shared text renderer should work without backend-specific arguments");

    host.click_node(&Selector::id_eq("run-button"))
        .expect("semantic click should succeed");
    harness.settle(2);

    let scene = host.snapshot_scene();
    assert_eq!(node_label(&scene, "click-state"), Some("Clicked"));
}

fn status_expectation(content: &str) -> AnchoredTextExpectation {
    AnchoredTextExpectation::new(
        content,
        RegionSpec::node(Selector::id_eq("status-text")).subregion(RelativeBounds::full()),
    )
    .with_point_size(14.0)
    .with_foreground(RgbaColor::new(0, 0, 0, 255))
    .with_background(RgbaColor::new(255, 255, 255, 255))
}

fn node_label<'a>(scene: &'a glasscheck::Scene, id: &str) -> Option<&'a str> {
    let handle = scene.find(&Selector::id_eq(id)).ok()?;
    scene.node(handle)?.label.as_deref()
}

fn assert_input_driver<T: InputDriver>(_: &T) {}

fn assert_text_harness<T: AnchoredTextHarness>(_: &T) {}

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

struct ClickActionIvars {
    clicked: Rc<Cell<bool>>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClickActionIvars]
    struct ClickActionTarget;

    impl ClickActionTarget {
        #[unsafe(method(buttonPressed:))]
        fn button_pressed(&self, _sender: Option<&NSButton>) {
            self.ivars().clicked.set(true);
        }
    }
);

impl ClickActionTarget {
    fn new(mtm: MainThreadMarker, clicked: Rc<Cell<bool>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickActionIvars { clicked });
        unsafe { msg_send![super(this), init] }
    }
}
