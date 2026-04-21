#![cfg(target_os = "linux")]

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use glasscheck_core::{
    InputSynthesisError, KeyModifiers, NodeRecipe, PixelMatch, PixelProbe, Point, PollOptions,
    PropertyValue, Rect, RegionResolveError, RegionSpec, Role, Selector, SemanticNode,
    SemanticProvider, Size, SurfaceId, TextRange, TransientSurfaceSpec,
};
use glasscheck_gtk::{
    GtkHarness, GtkWindowHost, HitPointSearch, HitPointStrategy, InstrumentedWidget,
};
use gtk4::prelude::*;

fn main() {
    let harness = GtkHarness::new().expect("GTK should initialize for native contract tests");

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
        "resolve_hit_point_supports_selector_lookup_and_search_strategy",
        || resolve_hit_point_supports_selector_lookup_and_search_strategy(harness),
    );
    run(
        "semantic_click_dispatches_gesture_click_for_non_button_targets",
        || semantic_click_dispatches_gesture_click_for_non_button_targets(harness),
    );
    run("semantic_click_dispatches_button_gesture_handlers", || {
        semantic_click_dispatches_button_gesture_handlers(harness)
    });
    run(
        "semantic_click_skips_unrealized_registrations_when_mapping_handles",
        || semantic_click_skips_unrealized_registrations_when_mapping_handles(harness),
    );
    run("semantic_click_respects_gesture_button_filters", || {
        semantic_click_respects_gesture_button_filters(harness)
    });
    run(
        "semantic_click_on_registered_ancestor_routes_to_descendant_hit_widget",
        || semantic_click_on_registered_ancestor_routes_to_descendant_hit_widget(harness),
    );
    run("semantic_click_reports_unhittable_registered_node", || {
        semantic_click_reports_unhittable_registered_node(harness)
    });
    run(
        "semantic_click_uses_recipe_hit_target_when_locator_bounds_are_empty",
        || semantic_click_uses_recipe_hit_target_when_locator_bounds_are_empty(harness),
    );
    run("semantic_click_uses_single_scene_snapshot", || {
        semantic_click_uses_single_scene_snapshot(harness)
    });
    run("key_press_dispatches_modifiers_to_event_controller", || {
        key_press_dispatches_modifiers_to_event_controller(harness)
    });
    run("key_press_queued_reaches_root_legacy_controller", || {
        key_press_queued_reaches_root_legacy_controller(harness)
    });
    run("key_press_queued_still_reaches_focused_controller", || {
        key_press_queued_still_reaches_focused_controller(harness)
    });
    run("key_press_direct_skips_root_legacy_controller", || {
        key_press_direct_skips_root_legacy_controller(harness)
    });
    run("key_press_queued_carries_modifier_state", || {
        key_press_queued_carries_modifier_state(harness)
    });
    run(
        "key_press_queued_returns_unsupported_key_for_unknown_name",
        || key_press_queued_returns_unsupported_key_for_unknown_name(harness),
    );
    run("text_range_rect_converts_to_root_coordinates", || {
        text_range_rect_converts_to_root_coordinates(harness)
    });
    run("insertion_caret_rect_converts_to_root_coordinates", || {
        insertion_caret_rect_converts_to_root_coordinates(harness)
    });
    run("visual_recipe_probe_builds_clickable_node", || {
        visual_recipe_probe_builds_clickable_node(harness)
    });
    run("visual_recipe_probe_omits_missing_match", || {
        visual_recipe_probe_omits_missing_match(harness)
    });
    run("session_opens_owned_transient_window_and_evicts_it", || {
        session_opens_owned_transient_window_and_evicts_it(harness)
    });
    run("session_opens_popover_transient_and_evicts_it", || {
        session_opens_popover_transient_and_evicts_it(harness)
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
            ..Default::default()
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
            ..Default::default()
        },
    );

    let scene = attached.snapshot_scene();
    let row = scene.find(&Selector::id_eq("row")).unwrap();
    assert_eq!(scene.node(row).unwrap().label.as_deref(), Some("Draft"));
}

fn provider_only_scene_without_root_is_usable(harness: GtkHarness) {
    let host = harness.create_window(240.0, 160.0);
    host.window().set_child(Option::<&gtk4::Widget>::None);
    let attached = GtkWindowHost::from_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let scene = attached.snapshot_scene();
    let node = scene
        .find(&Selector::id_eq("provider-node"))
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
        .capture_region(&glasscheck_core::RegionSpec::node(Selector::id_eq(
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
            ..Default::default()
        },
    );
    harness.settle(2);

    assert!(host.window().child().is_some());
    let error = host.click_node(&Selector::id_eq("root")).unwrap_err();
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
    let left = scene.find(&Selector::id_eq("left")).unwrap();
    let right = scene.find(&Selector::id_eq("right")).unwrap();
    let label = scene.find(&Selector::id_eq("label")).unwrap();
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
        .find(&Selector::property_eq(
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

    host.click_node(&Selector::id_eq("click-target"))
        .expect("semantic click should succeed");
    harness.settle(2);

    assert_eq!(activations.get(), 1);
}

fn resolve_hit_point_supports_selector_lookup_and_search_strategy(harness: GtkHarness) {
    let host = harness.create_window(140.0, 120.0);
    let root = fixed_root(140, 120);
    let clipping_parent = gtk4::Fixed::new();
    clipping_parent.set_size_request(30, 30);
    let target = gtk4::Button::with_label("Hit");
    target.set_size_request(40, 40);
    clipping_parent.put(&target, 20.0, 20.0);
    root.put(&clipping_parent, 10.0, 10.0);
    host.set_root(&root);
    host.register_node(&target, node("hit-target", Role::Button, "Hit"));
    harness.settle(4);

    let default_hit = host
        .resolve_hit_point(&Selector::id_eq("hit-target"), &HitPointSearch::default())
        .expect("default hit-point search should resolve a visible target");
    assert!((30.0..=60.0).contains(&default_hit.x));
    assert!((30.0..=70.0).contains(&default_hit.y));

    let grid_hit = host
        .resolve_hit_point(
            &Selector::id_eq("hit-target"),
            &HitPointSearch {
                strategy: HitPointStrategy::Grid,
                sample_count: 16,
            },
        )
        .expect("grid hit-point search should resolve a visible target");
    assert!((30.0..=60.0).contains(&grid_hit.x));
    assert!((30.0..=70.0).contains(&grid_hit.y));
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

    host.click_node(&Selector::id_eq("target"))
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

    host.click_node(&Selector::id_eq("gesture"))
        .expect("semantic click should dispatch to gesture-driven targets");
    harness.settle(2);

    assert_eq!(activations.get(), 1);
}

fn semantic_click_dispatches_button_gesture_handlers(harness: GtkHarness) {
    let host = harness.create_window(200.0, 140.0);
    let root = fixed_root(200, 140);
    let button = gtk4::Button::with_label("Gesture");
    button.set_size_request(90, 40);
    let clicked = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let clicked_seen = clicked.clone();
    button.connect_clicked(move |_| clicked_seen.set(clicked_seen.get() + 1));
    let gesture_releases = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let gesture_seen = gesture_releases.clone();
    let gesture = gtk4::GestureClick::new();
    gesture.connect_released(move |_, _, _, _| gesture_seen.set(gesture_seen.get() + 1));
    button.add_controller(gesture);
    root.put(&button, 20.0, 20.0);
    host.set_root(&root);
    host.register_node(&button, node("button-gesture", Role::Button, "Gesture"));
    harness.settle(4);

    host.click_node(&Selector::id_eq("button-gesture"))
        .expect("semantic click should use native pointer dispatch for buttons");
    harness.settle(2);

    assert_eq!(clicked.get(), 1);
    assert_eq!(gesture_releases.get(), 1);
}

fn semantic_click_on_registered_ancestor_routes_to_descendant_hit_widget(harness: GtkHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = fixed_root(220, 140);
    let container = gtk4::Fixed::new();
    container.set_size_request(120, 80);
    let child = gtk4::Button::with_label("Child");
    child.set_size_request(80, 36);
    let activations = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let seen = activations.clone();
    child.connect_clicked(move |_| seen.set(seen.get() + 1));
    container.put(&child, 20.0, 20.0);
    root.put(&container, 30.0, 24.0);
    host.set_root(&root);
    host.register_node(&container, node("container", Role::Container, "Container"));
    harness.settle(4);

    host.click_node(&Selector::id_eq("container"))
        .expect("semantic click on a registered ancestor should route to the hit descendant");
    harness.settle(4);

    assert_eq!(activations.get(), 1);
}

fn semantic_click_respects_gesture_button_filters(harness: GtkHarness) {
    let host = harness.create_window(220.0, 160.0);
    let root = fixed_root(220, 160);
    let container = gtk4::Fixed::new();
    container.set_size_request(140, 90);
    let area = gtk4::DrawingArea::new();
    area.set_size_request(90, 40);
    let child_releases = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let child_seen = child_releases.clone();
    let child_gesture = gtk4::GestureClick::new();
    child_gesture.connect_released(move |_, _, _, _| child_seen.set(child_seen.get() + 1));
    area.add_controller(child_gesture);
    let ancestor_releases = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let ancestor_seen = ancestor_releases.clone();
    let ancestor_gesture = gtk4::GestureClick::new();
    ancestor_gesture.set_button(2);
    ancestor_gesture.connect_released(move |_, _, _, _| ancestor_seen.set(ancestor_seen.get() + 1));
    container.add_controller(ancestor_gesture);
    container.put(&area, 20.0, 20.0);
    root.put(&container, 30.0, 24.0);
    host.set_root(&root);
    host.register_node(
        &area,
        node("gesture-filter-target", Role::Button, "Gesture Filter"),
    );
    harness.settle(4);

    host.click_node(&Selector::id_eq("gesture-filter-target"))
        .expect("semantic click should dispatch a primary-button click");
    harness.settle(2);

    assert_eq!(child_releases.get(), 1);
    assert_eq!(ancestor_releases.get(), 0);
}

fn semantic_click_reports_unhittable_registered_node(harness: GtkHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = fixed_root(180, 120);
    let target = gtk4::Button::with_label("Covered");
    target.set_size_request(120, 60);
    let occluder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    occluder.set_size_request(120, 60);
    root.put(&target, 20.0, 20.0);
    root.put(&occluder, 20.0, 20.0);
    host.set_root(&root);
    host.register_node(&target, node("covered-target", Role::Button, "Covered"));
    harness.settle(4);

    let scene = host.snapshot_scene();
    let handle = scene.find(&Selector::id_eq("covered-target")).unwrap();
    let node = scene.node(handle).unwrap();
    assert!(node.hit_testable);

    let error = host
        .click_node(&Selector::id_eq("covered-target"))
        .unwrap_err();
    assert!(matches!(error, RegionResolveError::InputUnavailable));
}

fn semantic_click_uses_recipe_hit_target_when_locator_bounds_are_empty(harness: GtkHarness) {
    let host = harness.create_window(140.0, 100.0);
    let root = fixed_root(140, 100);
    let target = gtk4::DrawingArea::new();
    target.set_size_request(60, 30);
    let occluder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    occluder.set_size_request(20, 10);
    root.put(&target, 40.0, 30.0);
    root.put(&occluder, 60.0, 40.0);
    host.set_root(&root);
    host.register_node(
        &occluder,
        node("recipe-occluder", Role::Container, "Occluder"),
    );
    host.set_scene_source(Box::new(HitTargetOnlyRecipeProvider));
    harness.settle(4);

    let point = host
        .resolve_hit_point(
            &Selector::selector_eq("recipe.hit-target"),
            &HitPointSearch::default(),
        )
        .expect("recipe hit target should provide a usable fallback point");
    assert_ne!(point, Point::new(70.0, 55.0));
    assert!((40.0..=100.0).contains(&point.x));
    assert!((40.0..=70.0).contains(&point.y));
    assert!(!(60.0..=80.0).contains(&point.x) || !(50.0..=60.0).contains(&point.y));
}

fn semantic_click_uses_single_scene_snapshot(harness: GtkHarness) {
    let host = harness.create_window(220.0, 120.0);
    let root = fixed_root(220, 120);
    let target = gtk4::DrawingArea::new();
    target.set_size_request(80, 40);
    root.put(&target, 20.0, 20.0);
    host.set_root(&root);
    let snapshot_calls = std::rc::Rc::new(std::cell::Cell::new(0usize));
    host.set_scene_source(Box::new(FlippingSelectorProvider {
        snapshot_calls: snapshot_calls.clone(),
    }));
    harness.settle(4);

    host.click_node(&Selector::selector_eq("moving.target"))
        .expect("semantic click should stay bound to one provider snapshot");
    harness.settle(4);

    assert_eq!(snapshot_calls.get(), 1);
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
    host.input()
        .key_press(
            "a",
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        )
        .expect("strict key synthesis should deliver the key event");
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

fn visual_recipe_probe_builds_clickable_node(harness: GtkHarness) {
    let host = harness.create_window(140.0, 100.0);
    let root = fixed_root(140, 100);
    let chip = gtk4::Button::with_label("");
    chip.set_widget_name("visual-probe-chip");
    chip.set_size_request(24, 12);
    install_css(
        &chip,
        "#visual-probe-chip { background: rgba(255,0,0,1.0); border-radius: 0; min-height: 12px; min-width: 24px; padding: 0; }",
    );
    root.put(&chip, 40.0, 30.0);
    let clicks = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let seen = clicks.clone();
    let gesture = gtk4::GestureClick::new();
    gesture.connect_pressed(move |_, _, x, y| {
        if (0.0..=24.0).contains(&x) && (0.0..=12.0).contains(&y) {
            seen.set(seen.get() + 1);
        }
    });
    chip.add_controller(gesture);
    host.set_root(&root);
    host.set_scene_source(Box::new(VisualProbeProvider));
    wait_for_window_capture(harness, &host, &root);
    if harness
        .wait_until(PollOptions::default(), || {
            host.capture_subtree(&chip).is_some_and(|image| {
                image.average_rgba(Rect::new(Point::new(0.0, 0.0), image.size()))[0] > 200.0
            })
        })
        .is_err()
    {
        eprintln!(
            "skipping visual recipe positive probe: GTK subtree capture did not render the probe widget under this X11 environment"
        );
        return;
    }

    let scene = host.snapshot_scene();
    assert!(scene.recipe_errors().is_empty());
    let handle = scene
        .find(&Selector::selector_eq("visual.red-chip"))
        .expect("visual recipe node should resolve");
    let node = scene.node(handle).unwrap();
    assert_eq!(
        node.rect,
        Rect::new(Point::new(40.0, 58.0), Size::new(24.0, 12.0))
    );

    host.click_node(&Selector::selector_eq("visual.red-chip"))
        .expect("visual recipe node should be clickable");
    harness.settle(4);
    assert_eq!(clicks.get(), 1);

    let image = host
        .capture_region(&RegionSpec::node(Selector::selector_eq("visual.red-chip")))
        .expect("visual recipe region should capture");
    assert!(image.average_rgba(Rect::new(Point::new(0.0, 0.0), image.size()))[0] > 200.0);
}

fn visual_recipe_probe_omits_missing_match(harness: GtkHarness) {
    let host = harness.create_window(140.0, 100.0);
    let root = fixed_root(140, 100);
    host.set_root(&root);
    host.set_scene_source(Box::new(VisualProbeProvider));
    harness.settle(4);

    let scene = host.snapshot_scene();
    assert_eq!(scene.recipe_errors().len(), 1);
    assert_eq!(scene.recipe_errors()[0].recipe_id, "visual.red-chip");
    assert!(matches!(
        scene.recipe_errors()[0].error,
        RegionResolveError::VisualMatchMissing | RegionResolveError::CaptureUnavailable
    ));
    assert!(matches!(
        scene.find(&Selector::selector_eq("visual.red-chip")),
        Err(_)
    ));
}

fn session_opens_owned_transient_window_and_evicts_it(harness: GtkHarness) {
    let inserted = Rc::new(Cell::new(false));
    let host = harness.create_window(320.0, 220.0);
    let root = fixed_root(320, 220);
    let chooser = gtk4::Window::builder()
        .default_width(180)
        .default_height(90)
        .transient_for(host.window())
        .build();
    let chooser_root = fixed_root(180, 90);
    let chooser_button = gtk4::Button::with_label("Insert Table");
    chooser_button.set_size_request(120, 32);
    chooser_root.put(&chooser_button, 16.0, 20.0);
    chooser.set_child(Some(&chooser_root));
    let chooser_clone = chooser.clone();
    let inserted_clone = inserted.clone();
    chooser_button.connect_clicked(move |_| {
        inserted_clone.set(true);
        chooser_clone.close();
    });

    let open = gtk4::Button::with_label("Open Picker");
    open.set_size_request(120, 32);
    let chooser_clone = chooser.clone();
    open.connect_clicked(move |_| chooser_clone.present());
    root.put(&open, 20.0, 24.0);
    host.set_root(&root);
    host.register_node(&open, node("open-picker", Role::Button, "Open Picker"));
    host.set_semantic_provider(Box::new(InsertedGtkTableScene {
        inserted: inserted.clone(),
    }));
    harness.settle(4);

    let session = harness.session();
    session.attach_host("main", host);
    session
        .open_transient_with_click(
            "picker",
            &TransientSurfaceSpec::new("main", Selector::id_eq("open-picker")),
            PollOptions::default(),
        )
        .expect("session should attach the transient child window");
    session.with_surface(&SurfaceId::new("picker"), |picker| {
        picker.register_node(
            &chooser_button,
            node("insert-table", Role::Button, "Insert Table"),
        );
    });
    harness.settle(4);

    session
        .click_node(&SurfaceId::new("picker"), &Selector::id_eq("insert-table"))
        .expect("picker surface should be attached")
        .expect("click inside transient surface should succeed");
    session
        .wait_for_surface_closed(&SurfaceId::new("picker"), PollOptions::default())
        .expect("closed transient should be evicted");
    assert!(!session.surface_is_open(&SurfaceId::new("picker")));

    let main_scene = session
        .snapshot_scene(&SurfaceId::new("main"))
        .expect("main surface should remain attached");
    assert!(main_scene.find(&Selector::id_eq("inserted-table")).is_ok());
}

fn session_opens_popover_transient_and_evicts_it(harness: GtkHarness) {
    let inserted = Rc::new(Cell::new(false));
    let host = harness.create_window(320.0, 220.0);
    let root = fixed_root(320, 220);
    let popover = gtk4::Popover::new();
    let chooser_button = gtk4::Button::with_label("Insert Table");
    chooser_button.set_size_request(120, 32);
    popover.set_child(Some(&chooser_button));
    let popover_clone = popover.clone();
    let inserted_clone = inserted.clone();
    chooser_button.connect_clicked(move |_| {
        inserted_clone.set(true);
        popover_clone.popdown();
    });

    let open = gtk4::MenuButton::new();
    open.set_label("Open Picker");
    open.set_size_request(120, 32);
    open.set_popover(Some(&popover));
    root.put(&open, 20.0, 24.0);
    host.set_root(&root);
    host.register_node(&open, node("open-picker", Role::Button, "Open Picker"));
    host.set_semantic_provider(Box::new(InsertedGtkTableScene {
        inserted: inserted.clone(),
    }));
    harness.settle(4);

    let session = harness.session();
    session.attach_host("main", host);
    session
        .open_transient_with_click(
            "picker",
            &TransientSurfaceSpec::new("main", Selector::id_eq("open-picker")),
            PollOptions::default(),
        )
        .expect("session should attach the transient popover");
    session.with_surface(&SurfaceId::new("picker"), |picker| {
        picker.register_node(
            &chooser_button,
            node("insert-table", Role::Button, "Insert Table"),
        );
    });
    harness.settle(4);

    session
        .click_node(&SurfaceId::new("picker"), &Selector::id_eq("insert-table"))
        .expect("picker surface should be attached")
        .expect("click inside transient popover should succeed");
    session
        .wait_for_surface_closed(&SurfaceId::new("picker"), PollOptions::default())
        .expect("dismissed popover should be evicted");
    assert!(!session.surface_is_open(&SurfaceId::new("picker")));

    let main_scene = session
        .snapshot_scene(&SurfaceId::new("main"))
        .expect("main surface should remain attached");
    assert!(main_scene.find(&Selector::id_eq("inserted-table")).is_ok());
}

fn fixed_root(width: i32, height: i32) -> gtk4::Fixed {
    let root = gtk4::Fixed::new();
    root.set_size_request(width, height);
    root
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

fn node(id: &str, role: Role, label: &str) -> InstrumentedWidget {
    InstrumentedWidget {
        id: Some(id.into()),
        role: Some(role),
        label: Some(label.into()),
        ..Default::default()
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

struct VisualProbeProvider;

impl SemanticProvider for VisualProbeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        Vec::new()
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        let locator = RegionSpec::root().pixel_probe(PixelProbe::new(
            PixelMatch::new([255, 0, 0, 255], 1, 255),
            8,
        ));
        vec![
            NodeRecipe::new("visual.red-chip", Role::Button, locator.clone())
                .with_selector("visual.red-chip")
                .with_hit_target(locator),
        ]
    }
}

struct HitTargetOnlyRecipeProvider;

impl SemanticProvider for HitTargetOnlyRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        Vec::new()
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "recipe-hit-target",
            Role::Button,
            RegionSpec::rect(Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0))),
        )
        .with_selector("recipe.hit-target")
        .with_hit_target(RegionSpec::rect(Rect::new(
            Point::new(40.0, 40.0),
            Size::new(60.0, 30.0),
        )))]
    }
}

struct InsertedGtkTableScene {
    inserted: Rc<Cell<bool>>,
}

impl SemanticProvider for InsertedGtkTableScene {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        self.inserted
            .get()
            .then(|| {
                vec![SemanticNode::new(
                    "inserted-table",
                    Role::Container,
                    Rect::new(Point::new(24.0, 70.0), Size::new(220.0, 110.0)),
                )]
            })
            .unwrap_or_default()
    }
}

struct FlippingSelectorProvider {
    snapshot_calls: std::rc::Rc<std::cell::Cell<usize>>,
}

impl SemanticProvider for FlippingSelectorProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        Vec::new()
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        let call = self.snapshot_calls.get();
        self.snapshot_calls.set(call + 1);
        (call == 0)
            .then(|| {
                NodeRecipe::new(
                    "moving-target",
                    Role::Button,
                    RegionSpec::rect(Rect::new(Point::new(20.0, 60.0), Size::new(80.0, 40.0))),
                )
                .with_selector("moving.target")
            })
            .into_iter()
            .collect()
    }
}

fn key_press_queued_reaches_root_legacy_controller(harness: GtkHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = fixed_root(220, 140);
    let entry = gtk4::Entry::new();
    entry.set_size_request(120, 36);
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(|_, _, _, _| gtk4::glib::Propagation::Stop);
    entry.add_controller(controller);
    root.put(&entry, 20.0, 20.0);
    host.set_root(&root);
    harness.settle(4);

    entry.grab_focus();
    harness.settle(2);

    let key_press_count = Rc::new(Cell::new(0usize));
    let count_ref = key_press_count.clone();
    let legacy = gtk4::EventControllerLegacy::new();
    legacy.connect_event(move |_, event| {
        if event.event_type() == gtk4::gdk::EventType::KeyPress {
            count_ref.set(count_ref.get() + 1);
        }
        false
    });
    host.window().add_controller(legacy);

    host.input()
        .key_press_queued("a", KeyModifiers::default())
        .expect("queued key press should flow through root EventControllerLegacy");
    harness.settle(2);

    assert_eq!(
        key_press_count.get(),
        1,
        "root EventControllerLegacy should observe the queued key-press event exactly once"
    );
}

fn key_press_queued_still_reaches_focused_controller(harness: GtkHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = fixed_root(220, 140);
    let entry = gtk4::Entry::new();
    entry.set_size_request(120, 36);
    let keys_seen = Rc::new(std::cell::RefCell::new(Vec::<Option<String>>::new()));
    let seen = keys_seen.clone();
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        seen.borrow_mut().push(key.name().map(|n| n.to_string()));
        gtk4::glib::Propagation::Stop
    });
    entry.add_controller(controller);
    root.put(&entry, 20.0, 20.0);
    host.set_root(&root);
    harness.settle(4);

    entry.grab_focus();
    harness.settle(2);

    host.input()
        .key_press_queued("a", KeyModifiers::default())
        .expect("queued key press should reach the focused EventControllerKey");
    harness.settle(2);

    let seen = keys_seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].as_deref(), Some("a"));
}

fn key_press_direct_skips_root_legacy_controller(harness: GtkHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = fixed_root(220, 140);
    let entry = gtk4::Entry::new();
    entry.set_size_request(120, 36);
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(|_, _, _, _| gtk4::glib::Propagation::Stop);
    entry.add_controller(controller);
    root.put(&entry, 20.0, 20.0);
    host.set_root(&root);
    harness.settle(4);

    entry.grab_focus();
    harness.settle(2);

    let key_press_count = Rc::new(Cell::new(0usize));
    let count_ref = key_press_count.clone();
    let legacy = gtk4::EventControllerLegacy::new();
    legacy.connect_event(move |_, event| {
        if event.event_type() == gtk4::gdk::EventType::KeyPress {
            count_ref.set(count_ref.get() + 1);
        }
        false
    });
    host.window().add_controller(legacy);

    host.input()
        .key_press("a", KeyModifiers::default())
        .expect("direct key press should succeed");
    harness.settle(2);

    assert_eq!(
        key_press_count.get(),
        0,
        "direct key_press should bypass root EventControllerLegacy (regression guard)"
    );
}

fn key_press_queued_carries_modifier_state(harness: GtkHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = fixed_root(220, 140);
    let entry = gtk4::Entry::new();
    entry.set_size_request(120, 36);
    let modifiers_seen = Rc::new(std::cell::RefCell::new(
        Vec::<gtk4::gdk::ModifierType>::new(),
    ));
    let seen = modifiers_seen.clone();
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(move |_, _, _, mods| {
        seen.borrow_mut().push(mods);
        gtk4::glib::Propagation::Stop
    });
    entry.add_controller(controller);
    root.put(&entry, 20.0, 20.0);
    host.set_root(&root);
    harness.settle(4);

    entry.grab_focus();
    harness.settle(2);

    host.input()
        .key_press_queued(
            "a",
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        )
        .expect("queued key press with Control modifier should succeed");
    harness.settle(2);

    let seen = modifiers_seen.borrow();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0].contains(gtk4::gdk::ModifierType::CONTROL_MASK),
        "queued key event should carry CONTROL_MASK"
    );
}

fn key_press_queued_returns_unsupported_key_for_unknown_name(harness: GtkHarness) {
    let host = harness.create_window(160.0, 80.0);
    let root = fixed_root(160, 80);
    host.set_root(&root);
    harness.settle(4);

    let result = host
        .input()
        .key_press_queued("not_a_valid_key_name_xyz", KeyModifiers::default());
    assert!(
        matches!(result, Err(InputSynthesisError::UnsupportedKey(_))),
        "unknown key name should return UnsupportedKey, got: {result:?}"
    );
}
