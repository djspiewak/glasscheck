#![cfg(target_os = "linux")]

use glasscheck_core::{
    KeyModifiers, NodePredicate, Point, PropertyValue, Rect, RegionResolveError, Role,
    SemanticNode, SemanticProvider, Size, TextRange,
};
use glasscheck_gtk::{GtkHarness, GtkWindowHost, InstrumentedWidget};
use gtk4::prelude::*;

fn main() {
    let harness = GtkHarness::new();

    run("attach_to_existing_window_builds_scene_snapshot", || {
        attach_to_existing_window_builds_scene_snapshot(harness)
    });
    run("provider_only_scene_without_root_is_usable", || {
        provider_only_scene_without_root_is_usable(harness)
    });
    run("provider_only_region_capture_fails_cleanly", || {
        provider_only_region_capture_fails_cleanly(harness)
    });
    run("root_widget_only_host_without_window_is_safe", || {
        root_widget_only_host_without_window_is_safe(harness)
    });
    run(
        "registered_native_hierarchy_supports_parent_and_child_indexes",
        || registered_native_hierarchy_supports_parent_and_child_indexes(harness),
    );
    run(
        "provider_ids_are_namespaced_when_they_collide_with_native_ids",
        || provider_ids_are_namespaced_when_they_collide_with_native_ids(harness),
    );
    run("semantic_click_targets_registered_node", || {
        semantic_click_targets_registered_node(harness)
    });
    run(
        "semantic_click_dispatches_gesture_click_for_non_button_targets",
        || semantic_click_dispatches_gesture_click_for_non_button_targets(harness),
    );
    run(
        "semantic_click_skips_unrealized_registrations_when_mapping_handles",
        || semantic_click_skips_unrealized_registrations_when_mapping_handles(harness),
    );
    run("key_press_dispatches_modifiers_to_event_controller", || {
        key_press_dispatches_modifiers_to_event_controller(harness)
    });
    run("text_range_rect_converts_to_root_coordinates", || {
        text_range_rect_converts_to_root_coordinates(harness)
    });
    run("insertion_caret_rect_converts_to_root_coordinates", || {
        insertion_caret_rect_converts_to_root_coordinates(harness)
    });
}

fn run(name: &str, test: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
    match result {
        Ok(()) => println!("test {name} ... ok"),
        Err(error) => {
            if let Some(message) = error.downcast_ref::<String>() {
                println!("test {name} ... FAILED\n{message}");
            } else if let Some(message) = error.downcast_ref::<&'static str>() {
                println!("test {name} ... FAILED\n{message}");
            } else {
                println!("test {name} ... FAILED");
            }
            std::panic::resume_unwind(error);
        }
    }
}

fn attach_to_existing_window_builds_scene_snapshot(harness: GtkHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = fixed_root(320, 200);
    let row = gtk4::Button::with_label("Draft");
    row.set_size_request(180, 32);
    root.put(&row, 12.0, 18.0);
    host.set_root(&root);
    host.register_node(
        &row,
        InstrumentedWidget {
            id: Some("row".into()),
            role: Some(Role::ListItem),
            label: Some("Draft".into()),
        },
    );
    harness.settle(4);

    let attached = GtkWindowHost::from_window(host.window());
    attached.register_node(
        &row,
        InstrumentedWidget {
            id: Some("row".into()),
            role: Some(Role::ListItem),
            label: Some("Draft".into()),
        },
    );

    let scene = attached.snapshot_scene();
    let row = scene.find(&NodePredicate::id_eq("row")).unwrap();
    assert_eq!(scene.node(row).unwrap().label.as_deref(), Some("Draft"));
}

fn provider_only_scene_without_root_is_usable(harness: GtkHarness) {
    let host = harness.create_window(240.0, 160.0);
    host.window().set_child(Option::<&gtk4::Widget>::None);
    let attached = GtkWindowHost::from_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let scene = attached.snapshot_scene();
    let node = scene
        .find(&NodePredicate::id_eq("provider-node"))
        .expect("provider node should be queryable without a root widget");
    assert_eq!(
        scene.node(node).unwrap().label.as_deref(),
        Some("Provider Node")
    );
}

fn provider_only_region_capture_fails_cleanly(harness: GtkHarness) {
    let host = harness.create_window(240.0, 160.0);
    host.window().set_child(Option::<&gtk4::Widget>::None);
    let attached = GtkWindowHost::from_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let error = attached
        .capture_region(&glasscheck_core::RegionSpec::node(NodePredicate::id_eq(
            "provider-node",
        )))
        .unwrap_err();
    assert!(matches!(error, RegionResolveError::CaptureUnavailable));
}

fn root_widget_only_host_without_window_is_safe(harness: GtkHarness) {
    let button = gtk4::Button::with_label("Root");
    button.set_size_request(120, 40);
    let host = GtkWindowHost::from_root(&button, None);
    host.register_node(
        &button,
        InstrumentedWidget {
            id: Some("root".into()),
            role: Some(Role::Button),
            label: Some("Root".into()),
        },
    );
    harness.settle(2);

    assert!(host.window().child().is_some());
    let error = host.click_node(&NodePredicate::id_eq("root")).unwrap_err();
    assert!(matches!(error, RegionResolveError::DetachedRootView));
}

fn registered_native_hierarchy_supports_parent_and_child_indexes(harness: GtkHarness) {
    let host = harness.create_window(240.0, 160.0);
    let root = fixed_root(240, 160);
    let left = gtk4::Button::with_label("Left");
    left.set_size_request(90, 40);
    root.put(&left, 10.0, 20.0);
    let right = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    right.set_size_request(90, 40);
    right.set_widget_name("right");
    let label = gtk4::Label::new(Some("Nested"));
    right.append(&label);
    root.put(&right, 110.0, 20.0);
    host.set_root(&root);
    host.register_node(&root, node("root", Role::Container, "Root"));
    host.register_node(&left, node("left", Role::Button, "Left"));
    host.register_node(&right, node("right", Role::Container, "Right"));
    host.register_node(&label, node("label", Role::Label, "Nested"));
    harness.settle(4);

    let scene = host.snapshot_scene();
    let left = scene.find(&NodePredicate::id_eq("left")).unwrap();
    let right = scene.find(&NodePredicate::id_eq("right")).unwrap();
    let label = scene.find(&NodePredicate::id_eq("label")).unwrap();
    assert_eq!(scene.node(left).unwrap().parent_id.as_deref(), Some("root"));
    assert_eq!(
        scene.node(right).unwrap().parent_id.as_deref(),
        Some("root")
    );
    assert_eq!(
        scene.node(label).unwrap().parent_id.as_deref(),
        Some("right")
    );
}

fn provider_ids_are_namespaced_when_they_collide_with_native_ids(harness: GtkHarness) {
    let host = harness.create_window(240.0, 160.0);
    let root = fixed_root(240, 160);
    let native = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    native.set_size_request(80, 40);
    root.put(&native, 10.0, 10.0);
    host.set_root(&root);
    host.register_node(&native, node("battlefield", Role::Container, "Native"));
    host.set_semantic_provider(Box::new(CollidingProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider = scene
        .find(&NodePredicate::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("battlefield"),
        ))
        .unwrap();
    assert_eq!(scene.node(provider).unwrap().id, "provider::battlefield");
}

fn semantic_click_targets_registered_node(harness: GtkHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = fixed_root(180, 120);
    let button = gtk4::Button::with_label("Click");
    button.set_size_request(80, 30);
    let activations = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let seen = activations.clone();
    button.connect_clicked(move |_| seen.set(seen.get() + 1));
    root.put(&button, 20.0, 20.0);
    host.set_root(&root);
    host.register_node(&button, node("click-target", Role::Button, "Click"));
    harness.settle(4);

    host.click_node(&NodePredicate::id_eq("click-target"))
        .expect("semantic click should succeed");
    harness.settle(2);

    assert_eq!(activations.get(), 1);
}

fn semantic_click_skips_unrealized_registrations_when_mapping_handles(harness: GtkHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = fixed_root(180, 120);
    let skipped = gtk4::Button::with_label("Skipped");
    skipped.set_size_request(80, 30);
    let target = gtk4::Button::with_label("Target");
    target.set_size_request(80, 30);
    let skipped_activations = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let seen_skipped = skipped_activations.clone();
    skipped.connect_clicked(move |_| seen_skipped.set(seen_skipped.get() + 1));
    let target_activations = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let seen_target = target_activations.clone();
    target.connect_clicked(move |_| seen_target.set(seen_target.get() + 1));
    root.put(&target, 20.0, 20.0);
    host.set_root(&root);
    host.register_node(&skipped, node("skipped", Role::Button, "Skipped"));
    host.register_node(&target, node("target", Role::Button, "Target"));
    harness.settle(4);

    host.click_node(&NodePredicate::id_eq("target"))
        .expect("semantic click should resolve the realized target widget");
    harness.settle(2);

    assert_eq!(skipped_activations.get(), 0);
    assert_eq!(target_activations.get(), 1);
}

fn semantic_click_dispatches_gesture_click_for_non_button_targets(harness: GtkHarness) {
    let host = harness.create_window(200.0, 140.0);
    let root = fixed_root(200, 140);
    let area = gtk4::DrawingArea::new();
    area.set_size_request(90, 40);
    let activations = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let seen = activations.clone();
    let gesture = gtk4::GestureClick::new();
    gesture.connect_released(move |_, _, _, _| seen.set(seen.get() + 1));
    area.add_controller(gesture);
    root.put(&area, 20.0, 20.0);
    host.set_root(&root);
    host.register_node(&area, node("gesture", Role::Button, "Gesture"));
    harness.settle(4);

    host.click_node(&NodePredicate::id_eq("gesture"))
        .expect("semantic click should dispatch to gesture-driven targets");
    harness.settle(2);

    assert_eq!(activations.get(), 1);
}

fn key_press_dispatches_modifiers_to_event_controller(harness: GtkHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = fixed_root(220, 140);
    let entry = gtk4::Entry::new();
    entry.set_size_request(120, 36);
    let states = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = states.clone();
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, modifiers| {
        seen.borrow_mut()
            .push((key.name().map(|name| name.to_string()), modifiers));
        gtk4::glib::Propagation::Stop
    });
    entry.add_controller(controller);
    root.put(&entry, 20.0, 20.0);
    host.set_root(&root);
    harness.settle(4);

    entry.grab_focus();
    harness.settle(2);
    host.input().key_press(
        "a",
        KeyModifiers {
            control: true,
            ..KeyModifiers::default()
        },
    );
    harness.settle(2);

    let states = states.borrow();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].0.as_deref(), Some("a"));
    assert!(states[0].1.contains(gtk4::gdk::ModifierType::CONTROL_MASK));
}

fn text_range_rect_converts_to_root_coordinates(harness: GtkHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = fixed_root(320, 200);
    let view = gtk4::TextView::new();
    view.buffer().set_text("Coordinate space");
    view.set_size_request(160, 80);
    root.put(&view, 28.0, 34.0);
    host.set_root(&root);
    harness.settle(4);

    let rect = host
        .text_range_rect(&view, TextRange::new(0, 10))
        .expect("text range rect should resolve");
    assert!(rect.size.width > 0.0);
    assert!(rect.size.height > 0.0);
}

fn insertion_caret_rect_converts_to_root_coordinates(harness: GtkHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = fixed_root(320, 200);
    let view = gtk4::TextView::new();
    view.buffer().set_text("Caret conversion");
    view.set_size_request(160, 80);
    root.put(&view, 42.0, 26.0);
    host.set_root(&root);
    harness.settle(4);

    let buffer = view.buffer();
    let iter = buffer.iter_at_offset(3);
    buffer.place_cursor(&iter);
    let rect = host
        .insertion_caret_rect(&view, 3)
        .expect("caret rect should resolve");
    assert!(rect.size.height > 0.0);
}

fn fixed_root(width: i32, height: i32) -> gtk4::Fixed {
    let root = gtk4::Fixed::new();
    root.set_size_request(width, height);
    root
}

fn node(id: &str, role: Role, label: &str) -> InstrumentedWidget {
    InstrumentedWidget {
        id: Some(id.into()),
        role: Some(role),
        label: Some(label.into()),
    }
}

struct ProviderOnlySceneProvider;

impl SemanticProvider for ProviderOnlySceneProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![SemanticNode::new(
            "provider-node",
            Role::Container,
            Rect::new(Point::new(0.0, 0.0), Size::new(40.0, 20.0)),
        )
        .with_label("Provider Node")]
    }
}

struct CollidingProvider;

impl SemanticProvider for CollidingProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "battlefield",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0)),
            ),
            SemanticNode::new(
                "battlefield/card",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(80.0, 20.0)),
            )
            .with_parent("battlefield", 0),
        ]
    }
}
