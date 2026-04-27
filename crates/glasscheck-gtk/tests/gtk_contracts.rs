#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    linux::run();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use glasscheck_core::{
        DialogCapability, DialogError, DialogKind, DialogQuery, InputSynthesisError, KeyModifiers,
        NodeRecipe, PixelMatch, PixelProbe, Point, PollOptions, PropertyValue, Rect,
        RegionResolveError, RegionSpec, Role, Selector, SemanticNode, SemanticProvider, Size,
        SurfaceId, TextRange, TransientSurfaceSpec,
    };
    use glasscheck_gtk::{
        GtkDialogController, GtkHarness, GtkWindowHost, HitPointSearch, HitPointStrategy,
        InstrumentedWidget,
    };
    use gtk4::prelude::*;

    pub(super) fn run() {
        let harness = GtkHarness::new().expect("GTK should initialize for native contract tests");

        run_case("attach_to_existing_window_builds_scene_snapshot", || {
            attach_to_existing_window_builds_scene_snapshot(harness)
        });
        run_case("provider_only_scene_without_root_is_usable", || {
            provider_only_scene_without_root_is_usable(harness)
        });
        run_case("provider_only_region_capture_fails_cleanly", || {
            provider_only_region_capture_fails_cleanly(harness)
        });
        run_case("root_widget_only_host_without_window_is_safe", || {
            root_widget_only_host_without_window_is_safe(harness)
        });
        run_case(
            "registered_native_hierarchy_supports_parent_and_child_indexes",
            || registered_native_hierarchy_supports_parent_and_child_indexes(harness),
        );
        run_case(
            "provider_ids_are_namespaced_when_they_collide_with_native_ids",
            || provider_ids_are_namespaced_when_they_collide_with_native_ids(harness),
        );
        run_case("semantic_click_targets_registered_node", || {
            semantic_click_targets_registered_node(harness)
        });
        run_case(
            "resolve_hit_point_supports_selector_lookup_and_search_strategy",
            || resolve_hit_point_supports_selector_lookup_and_search_strategy(harness),
        );
        run_case(
            "semantic_click_dispatches_gesture_click_for_non_button_targets",
            || semantic_click_dispatches_gesture_click_for_non_button_targets(harness),
        );
        run_case("semantic_click_dispatches_button_gesture_handlers", || {
            semantic_click_dispatches_button_gesture_handlers(harness)
        });
        run_case(
            "semantic_click_skips_unrealized_registrations_when_mapping_handles",
            || semantic_click_skips_unrealized_registrations_when_mapping_handles(harness),
        );
        run_case("semantic_click_respects_gesture_button_filters", || {
            semantic_click_respects_gesture_button_filters(harness)
        });
        run_case(
            "semantic_click_on_registered_ancestor_routes_to_descendant_hit_widget",
            || semantic_click_on_registered_ancestor_routes_to_descendant_hit_widget(harness),
        );
        run_case("semantic_click_reports_unhittable_registered_node", || {
            semantic_click_reports_unhittable_registered_node(harness)
        });
        run_case(
            "semantic_click_uses_recipe_hit_target_when_locator_bounds_are_empty",
            || semantic_click_uses_recipe_hit_target_when_locator_bounds_are_empty(harness),
        );
        run_case("semantic_click_uses_single_scene_snapshot", || {
            semantic_click_uses_single_scene_snapshot(harness)
        });
        run_case("key_press_dispatches_modifiers_to_event_controller", || {
            key_press_dispatches_modifiers_to_event_controller(harness)
        });
        run_case("key_press_queued_reaches_root_legacy_controller", || {
            key_press_queued_reaches_root_legacy_controller(harness)
        });
        run_case("key_press_queued_still_reaches_focused_controller", || {
            key_press_queued_still_reaches_focused_controller(harness)
        });
        run_case("key_press_direct_skips_root_legacy_controller", || {
            key_press_direct_skips_root_legacy_controller(harness)
        });
        run_case("key_press_queued_carries_modifier_state", || {
            key_press_queued_carries_modifier_state(harness)
        });
        run_case(
            "key_press_queued_returns_unsupported_key_for_unknown_name",
            || key_press_queued_returns_unsupported_key_for_unknown_name(harness),
        );
        run_case("text_range_rect_converts_to_root_coordinates", || {
            text_range_rect_converts_to_root_coordinates(harness)
        });
        run_case("insertion_caret_rect_converts_to_root_coordinates", || {
            insertion_caret_rect_converts_to_root_coordinates(harness)
        });
        run_case("visual_recipe_probe_builds_clickable_node", || {
            visual_recipe_probe_builds_clickable_node(harness)
        });
        run_case("visual_recipe_probe_omits_missing_match", || {
            visual_recipe_probe_omits_missing_match(harness)
        });
        run_case("session_rejects_duplicate_attach_host_ids", || {
            session_rejects_duplicate_attach_host_ids(harness)
        });
        run_case("session_rejects_transient_owner_id_reuse", || {
            session_rejects_transient_owner_id_reuse(harness)
        });
        run_case("session_opens_owned_transient_window_and_evicts_it", || {
            session_opens_owned_transient_window_and_evicts_it(harness)
        });
        run_case("session_opens_popover_transient_and_evicts_it", || {
            session_opens_popover_transient_and_evicts_it(harness)
        });
        run_case(
            "context_click_node_opens_popover_menu_and_activates_item",
            || context_click_node_opens_popover_menu_and_activates_item(harness),
        );
        run_case(
            "context_click_node_without_menu_does_not_fall_back_after_semantic_gesture",
            || context_click_node_without_menu_does_not_fall_back_after_semantic_gesture(harness),
        );
        run_case("dialog_methods_report_missing_surface", || {
            dialog_methods_report_missing_surface(harness)
        });
        run_case("cancel_dialog_reports_vetoed_close_as_poll_error", || {
            cancel_dialog_reports_vetoed_close_as_poll_error(harness)
        });
        run_case("wait_for_dialog_discovers_matching_window", || {
            wait_for_dialog_discovers_matching_window(harness)
        });
        run_case("wait_for_dialog_classifies_alerts", || {
            wait_for_dialog_classifies_alerts(harness)
        });
        run_case("file_chooser_dialogs_classify_and_select_paths", || {
            file_chooser_dialogs_classify_and_select_paths(harness)
        });
        run_case("custom_dialog_snapshots_and_drives_widgets", || {
            custom_dialog_snapshots_and_drives_widgets(harness)
        });
        run_case("dialog_response_buttons_expose_button_roles", || {
            dialog_response_buttons_expose_button_roles(harness)
        });
        run_case(
            "async_dialog_controller_reports_capability_boundary",
            || async_dialog_controller_reports_capability_boundary(harness),
        );
    }

    fn run_case(name: &str, test: impl FnOnce()) {
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

    fn assert_panics_with(expected: &str, test: impl FnOnce()) {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
        std::panic::set_hook(previous_hook);
        let Err(error) = result else {
            panic!("expected panic containing {expected:?}");
        };
        let message = if let Some(message) = error.downcast_ref::<String>() {
            message.as_str()
        } else if let Some(message) = error.downcast_ref::<&'static str>() {
            message
        } else {
            "<non-string panic>"
        };
        assert!(
            message.contains(expected),
            "expected panic containing {expected:?}, got {message:?}"
        );
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
        ancestor_gesture
            .connect_released(move |_, _, _, _| ancestor_seen.set(ancestor_seen.get() + 1));
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

    fn session_rejects_duplicate_attach_host_ids(harness: GtkHarness) {
        let first = harness.create_window(120.0, 80.0);
        let second = harness.create_window(120.0, 80.0);
        let session = harness.session();

        session.attach_host("main", first);
        assert_panics_with("surface id 'main' is already registered", || {
            session.attach_host("main", second);
        });
    }

    fn session_rejects_transient_owner_id_reuse(harness: GtkHarness) {
        let session = harness.session();
        let spec = TransientSurfaceSpec::new("main", Selector::id_eq("open-picker"));

        assert_panics_with("transient id must not equal the owner surface id", || {
            let _ = session.open_transient_with_click("main", &spec, PollOptions::default());
        });
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

    fn context_click_node_opens_popover_menu_and_activates_item(harness: GtkHarness) {
        let activated = Rc::new(Cell::new(false));
        let stale_host = harness.create_window(220.0, 140.0);
        let stale_root = fixed_root(220, 140);
        let stale_target = gtk4::Button::with_label("Stale Editor");
        stale_target.set_size_request(120, 32);
        stale_root.put(&stale_target, 16.0, 16.0);
        let stale_popover = gtk4::Popover::new();
        stale_popover.set_parent(&stale_target);
        stale_popover.set_child(Some(&gtk4::Button::with_label("Stale Command")));
        stale_host.set_root(&stale_root);
        stale_popover.popup();

        let host = harness.create_window(320.0, 220.0);
        let root = fixed_root(320, 220);
        let target = gtk4::Button::with_label("Editor");
        target.set_size_request(160, 40);
        root.put(&target, 24.0, 24.0);

        let popover = gtk4::Popover::new();
        popover.set_parent(&target);
        let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let insert = gtk4::Button::with_label("Insert Table");
        let disabled = gtk4::Button::with_label("Disabled Command");
        let hidden = gtk4::Button::with_label("Hidden Command");
        disabled.set_sensitive(false);
        hidden.set_visible(false);
        menu_box.append(&insert);
        menu_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        menu_box.append(&disabled);
        menu_box.append(&hidden);
        popover.set_child(Some(&menu_box));

        let activated_clone = activated.clone();
        let popover_for_insert = popover.clone();
        insert.connect_clicked(move |_| {
            activated_clone.set(true);
            popover_for_insert.popdown();
        });
        let hidden_activated = Rc::new(Cell::new(false));
        let hidden_seen = hidden_activated.clone();
        hidden.connect_clicked(move |_| hidden_seen.set(true));
        let popover_for_gesture = popover.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        gesture.connect_pressed(move |_, _, _, _| popover_for_gesture.popup());
        target.add_controller(gesture);

        host.set_root(&root);
        host.register_node(&target, node("editor", Role::Button, "Editor"));
        harness.settle(4);

        let session = harness.session();
        session.attach_host("main", host);
        let menu = session
            .context_click_node(&SurfaceId::new("main"), &Selector::id_eq("editor"))
            .expect("main surface should be attached")
            .expect("context click should open the GTK popover menu");
        let scene = menu.snapshot_scene();
        assert!(scene
            .find(&Selector::selector_eq("context-menu.title:Stale Command"))
            .is_err());
        assert!(scene
            .find(&Selector::selector_eq("context-menu.title:Insert Table"))
            .is_ok());
        assert!(scene
            .find(&Selector::selector_eq("context-menu.separator[1]"))
            .is_ok());

        assert!(matches!(
            menu.activate_item(&Selector::selector_eq(
                "context-menu.title:Disabled Command"
            )),
            Err(glasscheck_gtk::GtkContextMenuError::DisabledMenuItem)
        ));
        assert!(matches!(
            menu.activate_item(&Selector::selector_eq("context-menu.title:Hidden Command")),
            Err(glasscheck_gtk::GtkContextMenuError::HiddenMenuItem)
        ));
        assert!(!hidden_activated.get());

        menu.activate_item(&Selector::selector_eq("context-menu.title:Insert Table"))
            .expect("enabled GTK context menu item should activate");
        assert!(activated.get());
        menu.dismiss();
        popover.unparent();
        stale_popover.unparent();
    }

    fn context_click_node_without_menu_does_not_fall_back_after_semantic_gesture(
        harness: GtkHarness,
    ) {
        let host = harness.create_window(220.0, 140.0);
        let root = fixed_root(220, 140);
        let target = gtk4::DrawingArea::new();
        target.set_size_request(120, 48);
        root.put(&target, 24.0, 24.0);

        let releases = Rc::new(Cell::new(0usize));
        let seen_releases = releases.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        gesture.connect_released(move |_, _, _, _| {
            seen_releases.set(seen_releases.get() + 1);
        });
        target.add_controller(gesture);

        host.set_root(&root);
        host.register_node(
            &target,
            node("gesture-no-menu", Role::Button, "Gesture No Menu"),
        );
        harness.settle(4);

        let error = host
            .context_click_node(&Selector::id_eq("gesture-no-menu"))
            .expect_err("context-click gesture without a popover should report no menu");
        harness.settle(4);

        assert!(matches!(
            error,
            glasscheck_gtk::GtkContextMenuError::NoContextMenu
        ));
        assert_eq!(releases.get(), 1);
    }

    fn context_click_node_ignores_same_window_stale_popover(harness: GtkHarness) {
        let host = harness.create_window(360.0, 180.0);
        let root = fixed_root(360, 180);
        let stale_target = gtk4::Button::with_label("Stale Editor");
        stale_target.set_size_request(120, 32);
        root.put(&stale_target, 16.0, 16.0);
        let stale_popover = gtk4::Popover::new();
        stale_popover.set_parent(&stale_target);
        stale_popover.set_child(Some(&gtk4::Button::with_label("Stale Command")));

        let target = gtk4::Button::with_label("Editor");
        target.set_size_request(120, 32);
        root.put(&target, 180.0, 16.0);
        let target_popover = gtk4::Popover::new();
        target_popover.set_parent(&target);
        target_popover.set_child(Some(&gtk4::Button::with_label("Target Command")));
        let popover_for_gesture = target_popover.clone();
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        gesture.connect_pressed(move |_, _, _, _| popover_for_gesture.popup());
        target.add_controller(gesture);

        host.set_root(&root);
        host.register_node(&target, node("editor", Role::Button, "Editor"));
        harness.settle(4);
        stale_popover.popup();
        harness.settle(4);

        let session = harness.session();
        session.attach_host("main", host);
        let menu = session
            .context_click_node(&SurfaceId::new("main"), &Selector::id_eq("editor"))
            .expect("main surface should be attached")
            .expect("context click should open the target GTK popover menu");
        let scene = menu.snapshot_scene();
        assert!(scene
            .find(&Selector::selector_eq("context-menu.title:Stale Command"))
            .is_err());
        assert!(scene
            .find(&Selector::selector_eq("context-menu.title:Target Command"))
            .is_ok());

        menu.dismiss();
        target_popover.unparent();
        stale_popover.unparent();
    }

    fn dialog_methods_report_missing_surface(harness: GtkHarness) {
        let session = harness.session();
        let missing = SurfaceId::new("missing-dialog");

        assert!(matches!(
            session.dialog_kind(&missing),
            Err(DialogError::MissingSurface)
        ));
        assert!(matches!(
            session.snapshot_dialog_scene(&missing),
            Err(DialogError::MissingSurface)
        ));
        assert!(matches!(
            session.click_dialog_button(&missing, &Selector::role_eq(Role::Button)),
            Err(DialogError::MissingSurface)
        ));
        assert!(matches!(
            session.set_dialog_text(&missing, &Selector::role_eq(Role::TextInput), "ignored"),
            Err(DialogError::MissingSurface)
        ));
        let save_path = std::env::temp_dir().join("glasscheck-missing-save.txt");
        assert!(matches!(
            session.choose_save_dialog_path(&missing, &save_path, PollOptions::default()),
            Err(DialogError::MissingSurface)
        ));
        let open_paths = vec![std::env::temp_dir().join("glasscheck-missing-open.txt")];
        assert!(matches!(
            session.choose_open_dialog_paths(&missing, &open_paths, PollOptions::default()),
            Err(DialogError::MissingSurface)
        ));
        assert!(matches!(
            session.cancel_dialog(&missing, PollOptions::default()),
            Err(DialogError::MissingSurface)
        ));
    }

    fn cancel_dialog_reports_vetoed_close_as_poll_error(harness: GtkHarness) {
        let dialog = gtk4::Dialog::builder()
            .title("Vetoed GTK Dialog")
            .default_width(160)
            .default_height(80)
            .build();
        let veto_close = Rc::new(Cell::new(true));
        let veto_count = Rc::new(Cell::new(0usize));
        let veto_close_for_signal = veto_close.clone();
        let veto_count_for_signal = veto_count.clone();
        dialog.connect_close_request(move |_| {
            if veto_close_for_signal.get() {
                veto_count_for_signal.set(veto_count_for_signal.get() + 1);
                gtk4::glib::Propagation::Stop
            } else {
                gtk4::glib::Propagation::Proceed
            }
        });
        dialog.present();
        harness.settle(4);

        let session = harness.session();
        session.attach_window("vetoed-dialog", dialog.upcast_ref::<gtk4::Window>());
        assert!(matches!(
            session.cancel_dialog(
                &SurfaceId::new("vetoed-dialog"),
                PollOptions {
                    timeout: Duration::from_millis(40),
                    interval: Duration::from_millis(5),
                },
            ),
            Err(DialogError::Poll(_))
        ));
        assert!(session.surface_is_open(&SurfaceId::new("vetoed-dialog")));
        assert_eq!(veto_count.get(), 1);

        veto_close.set(false);
        dialog.close();
        harness.settle(4);
    }

    fn wait_for_dialog_discovers_matching_window(harness: GtkHarness) {
        let hidden = gtk4::Dialog::builder()
            .title("Target GTK Dialog")
            .default_width(160)
            .default_height(80)
            .build();
        let ignored = gtk4::Dialog::builder()
            .title("Ignored GTK Dialog")
            .default_width(160)
            .default_height(80)
            .build();
        let target = gtk4::Dialog::builder()
            .title("Target GTK Dialog")
            .default_width(160)
            .default_height(80)
            .build();
        ignored.present();
        target.present();
        harness.settle(4);

        let session = harness.session();
        session
            .wait_for_dialog(
                "target-dialog",
                &DialogQuery::kind(DialogKind::Panel).title_contains("Target"),
                PollOptions::default(),
            )
            .expect("session should discover only the matching dialog");
        assert_eq!(
            session
                .dialog_kind(&SurfaceId::new("target-dialog"))
                .unwrap(),
            DialogKind::Panel
        );
        let scene = session
            .snapshot_dialog_scene(&SurfaceId::new("target-dialog"))
            .expect("discovered GTK dialog should snapshot");
        assert_eq!(
            scene
                .node(scene.find(&Selector::id_eq("gtk.dialog")).unwrap())
                .unwrap()
                .label
                .as_deref(),
            Some("Target GTK Dialog")
        );

        session
            .cancel_dialog(&SurfaceId::new("target-dialog"), PollOptions::default())
            .expect("target dialog should close");
        session
            .wait_for_surface_closed(&SurfaceId::new("target-dialog"), PollOptions::default())
            .expect("target dialog should be evicted after close");
        assert!(!session.surface_is_open(&SurfaceId::new("target-dialog")));
        hidden.close();
        ignored.close();
        harness.settle(2);
    }

    fn wait_for_dialog_classifies_alerts(harness: GtkHarness) {
        let ordinary = gtk4::Window::builder()
            .title("Target GTK Alert")
            .default_width(160)
            .default_height(80)
            .build();
        let alert = gtk4::MessageDialog::builder()
            .title("Target GTK Alert")
            .text("Discard changes?")
            .modal(true)
            .build();
        let text_only_alert = gtk4::MessageDialog::builder()
            .text("Unsaved changes")
            .modal(true)
            .build();
        ordinary.present();
        alert.present();
        text_only_alert.present();
        harness.settle(4);

        let session = harness.session();
        session
            .wait_for_dialog(
                "text-only-alert",
                &DialogQuery::alert().title_eq("Unsaved changes"),
                PollOptions::default(),
            )
            .expect("message text should be usable for alert discovery when title is absent");
        assert_eq!(
            session
                .dialog_kind(&SurfaceId::new("text-only-alert"))
                .unwrap(),
            DialogKind::Alert
        );
        session
            .cancel_dialog(&SurfaceId::new("text-only-alert"), PollOptions::default())
            .expect("text-only alert should close");

        session
            .wait_for_dialog(
                "target-alert",
                &DialogQuery::alert().title_eq("Target GTK Alert"),
                PollOptions::default(),
            )
            .expect("session should discover the alert and ignore ordinary windows");
        assert_eq!(
            session
                .dialog_kind(&SurfaceId::new("target-alert"))
                .unwrap(),
            DialogKind::Alert
        );

        session
            .cancel_dialog(&SurfaceId::new("target-alert"), PollOptions::default())
            .expect("target alert should close");
        ordinary.close();
        harness.settle(2);
    }

    fn file_chooser_dialogs_classify_and_select_paths(harness: GtkHarness) {
        let session = harness.session();
        let temp = temp_test_dir("gtk-file-chooser");
        let open_path = temp.join("input.txt");
        fs::write(&open_path, "fixture").expect("open fixture should be writable");
        let save_path = temp.join("output.txt");

        let save_dialog = gtk4::FileChooserDialog::new(
            Some("Save Fixture"),
            None::<&gtk4::Window>,
            gtk4::FileChooserAction::Save,
            &[],
        );
        save_dialog.present();
        harness.settle(4);
        session
            .wait_for_dialog(
                "save-dialog",
                &DialogQuery::save_panel().title_eq("Save Fixture"),
                PollOptions::default(),
            )
            .expect("session should discover save file chooser dialogs");
        assert_eq!(
            session.dialog_kind(&SurfaceId::new("save-dialog")).unwrap(),
            DialogKind::SavePanel
        );
        session
            .choose_save_dialog_path(
                &SurfaceId::new("save-dialog"),
                &save_path,
                PollOptions::default(),
            )
            .expect("save file chooser should accept deterministic path");
        harness.settle(4);
        assert_eq!(
            save_dialog.current_folder().and_then(|file| file.path()),
            Some(temp.clone())
        );
        assert_eq!(
            save_dialog.current_name().map(|name| name.to_string()),
            Some(String::from("output.txt"))
        );
        session
            .cancel_dialog(&SurfaceId::new("save-dialog"), PollOptions::default())
            .expect("save dialog should close");
        session
            .wait_for_surface_closed(&SurfaceId::new("save-dialog"), PollOptions::default())
            .expect("save dialog should be evicted");

        let open_dialog = gtk4::FileChooserDialog::new(
            Some("Open Fixture"),
            None::<&gtk4::Window>,
            gtk4::FileChooserAction::Open,
            &[],
        );
        open_dialog.present();
        harness.settle(4);
        session
            .wait_for_dialog(
                "open-dialog",
                &DialogQuery::open_panel().title_eq("Open Fixture"),
                PollOptions::default(),
            )
            .expect("session should discover open file chooser dialogs");
        assert_eq!(
            session.dialog_kind(&SurfaceId::new("open-dialog")).unwrap(),
            DialogKind::OpenPanel
        );
        session
            .choose_open_dialog_paths(
                &SurfaceId::new("open-dialog"),
                std::slice::from_ref(&open_path),
                PollOptions::default(),
            )
            .expect("open file chooser should accept an existing path");
        assert_eq!(
            open_dialog.file().and_then(|file| file.path()),
            Some(open_path.clone())
        );

        let missing = temp.join("missing.txt");
        assert!(matches!(
            session.choose_open_dialog_paths(
                &SurfaceId::new("open-dialog"),
                &[missing.clone()],
                PollOptions::default(),
            ),
            Err(DialogError::MissingRequestedPath(path)) if path == missing
        ));
        let second = temp.join("second.txt");
        fs::write(&second, "second").expect("second fixture should be writable");
        assert!(matches!(
            session.choose_open_dialog_paths(
                &SurfaceId::new("open-dialog"),
                &[open_path.clone(), second],
                PollOptions::default(),
            ),
            Err(DialogError::UnsupportedLiveSelection(_))
        ));
        assert!(matches!(
            session.choose_save_dialog_path(
                &SurfaceId::new("open-dialog"),
                &save_path,
                PollOptions::default(),
            ),
            Err(DialogError::KindMismatch {
                expected: DialogKind::SavePanel,
                actual: DialogKind::OpenPanel
            })
        ));
        session
            .cancel_dialog(&SurfaceId::new("open-dialog"), PollOptions::default())
            .expect("open dialog should close");
        session
            .wait_for_surface_closed(&SurfaceId::new("open-dialog"), PollOptions::default())
            .expect("open dialog should be evicted");
    }

    fn custom_dialog_snapshots_and_drives_widgets(harness: GtkHarness) {
        let dialog = gtk4::Dialog::builder()
            .title("GTK Custom Dialog")
            .default_width(240)
            .default_height(120)
            .build();
        let root = fixed_root(240, 120);
        let label = gtk4::Label::new(Some("Read Only Dialog Label"));
        let entry = gtk4::Entry::new();
        entry.set_text("draft");
        let disabled_entry = gtk4::Entry::new();
        disabled_entry.set_text("locked");
        disabled_entry.set_sensitive(false);
        let hidden_entry = gtk4::Entry::new();
        hidden_entry.set_text("concealed");
        hidden_entry.set_visible(false);
        let button = gtk4::Button::with_label("Apply");
        let disabled_button = gtk4::Button::with_label("Disabled Apply");
        disabled_button.set_sensitive(false);
        let hidden_button = gtk4::Button::with_label("Hidden Apply");
        hidden_button.set_visible(false);
        let activations = Rc::new(Cell::new(0usize));
        let seen = activations.clone();
        button.connect_clicked(move |_| seen.set(seen.get() + 1));
        let disabled_activations = Rc::new(Cell::new(0usize));
        let disabled_seen = disabled_activations.clone();
        disabled_button.connect_clicked(move |_| disabled_seen.set(disabled_seen.get() + 1));
        let hidden_activations = Rc::new(Cell::new(0usize));
        let hidden_seen = hidden_activations.clone();
        hidden_button.connect_clicked(move |_| hidden_seen.set(hidden_seen.get() + 1));
        root.put(&label, 16.0, 16.0);
        root.put(&entry, 16.0, 44.0);
        root.put(&disabled_entry, 16.0, 78.0);
        root.put(&hidden_entry, 16.0, 112.0);
        root.put(&button, 16.0, 146.0);
        root.put(&disabled_button, 16.0, 180.0);
        root.put(&hidden_button, 16.0, 214.0);
        dialog.set_child(Some(&root));
        dialog.present();
        harness.settle(4);

        let session = harness.session();
        session.attach_window("custom-dialog", dialog.upcast_ref::<gtk4::Window>());
        assert_eq!(
            session
                .dialog_kind(&SurfaceId::new("custom-dialog"))
                .unwrap(),
            DialogKind::Panel
        );

        let scene = session
            .snapshot_dialog_scene(&SurfaceId::new("custom-dialog"))
            .expect("custom GTK dialog should snapshot");
        let root_node = scene
            .node(scene.find(&Selector::id_eq("gtk.dialog")).unwrap())
            .unwrap();
        assert_eq!(
            root_node.properties.get("gtk:dialog_kind"),
            Some(&PropertyValue::string("panel"))
        );
        assert!(scene
            .find(&Selector::selector_eq(
                "gtk.dialog.label.read.only.dialog.label"
            ))
            .is_ok());
        assert_eq!(
            scene
                .resolve(&Selector::property_eq(
                    "gtk:name_field",
                    PropertyValue::string("draft"),
                ))
                .unwrap()
                .node
                .value
                .as_deref(),
            Some("draft")
        );

        session
            .set_dialog_text(
                &SurfaceId::new("custom-dialog"),
                &Selector::property_eq("gtk:name_field", PropertyValue::string("draft")),
                "ready",
            )
            .expect("dialog text field should be editable");
        assert_eq!(entry.text().as_str(), "ready");

        assert!(matches!(
            session.set_dialog_text(
                &SurfaceId::new("custom-dialog"),
                &Selector::property_eq("gtk:name_field", PropertyValue::string("locked")),
                "mutated",
            ),
            Err(DialogError::InputUnavailable)
        ));
        assert_eq!(disabled_entry.text().as_str(), "locked");

        assert!(matches!(
            session.set_dialog_text(
                &SurfaceId::new("custom-dialog"),
                &Selector::property_eq("gtk:name_field", PropertyValue::string("concealed")),
                "mutated",
            ),
            Err(DialogError::InputUnavailable)
        ));
        assert_eq!(hidden_entry.text().as_str(), "concealed");

        assert!(matches!(
            session.click_dialog_button(
                &SurfaceId::new("custom-dialog"),
                &Selector::and(vec![
                    Selector::role_eq(Role::Button),
                    Selector::selector_eq("gtk.dialog.label.disabled.apply"),
                ]),
            ),
            Err(DialogError::InputUnavailable)
        ));
        assert_eq!(disabled_activations.get(), 0);

        assert!(matches!(
            session.click_dialog_button(
                &SurfaceId::new("custom-dialog"),
                &Selector::and(vec![
                    Selector::role_eq(Role::Button),
                    Selector::selector_eq("gtk.dialog.label.hidden.apply"),
                ]),
            ),
            Err(DialogError::InputUnavailable)
        ));
        assert_eq!(hidden_activations.get(), 0);

        session
            .click_dialog_button(
                &SurfaceId::new("custom-dialog"),
                &Selector::and(vec![
                    Selector::role_eq(Role::Button),
                    Selector::selector_eq("gtk.dialog.label.apply"),
                ]),
            )
            .expect("dialog button should activate");
        assert_eq!(activations.get(), 1);

        session
            .cancel_dialog(&SurfaceId::new("custom-dialog"), PollOptions::default())
            .expect("custom GTK dialog should cancel");
        session
            .wait_for_surface_closed(&SurfaceId::new("custom-dialog"), PollOptions::default())
            .expect("custom GTK dialog should be evicted after close");
        assert!(!session.surface_is_open(&SurfaceId::new("custom-dialog")));
    }

    fn dialog_response_buttons_expose_button_roles(harness: GtkHarness) {
        let dialog = gtk4::Dialog::builder()
            .title("GTK Response Roles")
            .default_width(240)
            .default_height(120)
            .build();
        dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
        dialog.add_button("OK", gtk4::ResponseType::Ok);
        dialog.present();
        harness.settle(4);

        let session = harness.session();
        session.attach_window("response-dialog", dialog.upcast_ref::<gtk4::Window>());
        let scene = session
            .snapshot_dialog_scene(&SurfaceId::new("response-dialog"))
            .expect("response dialog should snapshot");
        let confirm = scene
            .resolve(&Selector::property_eq(
                "gtk:button_role",
                PropertyValue::string("confirm"),
            ))
            .expect("confirm response button should be classified");
        assert_eq!(confirm.node.label.as_deref(), Some("OK"));
        let cancel = scene
            .resolve(&Selector::property_eq(
                "gtk:button_role",
                PropertyValue::string("cancel"),
            ))
            .expect("cancel response button should be classified");
        assert_eq!(cancel.node.label.as_deref(), Some("Cancel"));

        session
            .cancel_dialog(&SurfaceId::new("response-dialog"), PollOptions::default())
            .expect("response dialog should close");
        session
            .wait_for_surface_closed(&SurfaceId::new("response-dialog"), PollOptions::default())
            .expect("response dialog should be evicted after close");
    }

    fn async_dialog_controller_reports_capability_boundary(harness: GtkHarness) {
        let session = harness.session();
        session.attach_dialog_controller(
            "async-alert",
            GtkDialogController::new(DialogKind::Alert, "Async Alert"),
        );

        assert_eq!(
            session.dialog_kind(&SurfaceId::new("async-alert")).unwrap(),
            DialogKind::Alert
        );
        assert!(matches!(
            session.snapshot_dialog_scene(&SurfaceId::new("async-alert")),
            Err(DialogError::UnsupportedCapability(
                DialogCapability::SceneSnapshot
            ))
        ));
        assert!(matches!(
            session.click_dialog_button(
                &SurfaceId::new("async-alert"),
                &Selector::role_eq(Role::Button)
            ),
            Err(DialogError::UnsupportedCapability(
                DialogCapability::ButtonClick
            ))
        ));
        assert!(matches!(
            session.set_dialog_text(
                &SurfaceId::new("async-alert"),
                &Selector::role_eq(Role::TextInput),
                "ignored",
            ),
            Err(DialogError::UnsupportedCapability(
                DialogCapability::TextEdit
            ))
        ));
        let save_path = std::env::temp_dir().join("glasscheck-async-save.txt");
        assert!(matches!(
            session.choose_save_dialog_path(
                &SurfaceId::new("async-alert"),
                &save_path,
                PollOptions::default(),
            ),
            Err(DialogError::UnsupportedCapability(
                DialogCapability::SavePathSelection
            ))
        ));
        let open_paths = vec![std::env::temp_dir().join("glasscheck-async-open.txt")];
        assert!(matches!(
            session.choose_open_dialog_paths(
                &SurfaceId::new("async-alert"),
                &open_paths,
                PollOptions::default(),
            ),
            Err(DialogError::UnsupportedCapability(
                DialogCapability::OpenPathSelection
            ))
        ));
        assert!(matches!(
            session.cancel_dialog(&SurfaceId::new("async-alert"), PollOptions::default()),
            Err(DialogError::UnsupportedCapability(DialogCapability::Cancel))
        ));
        assert!(session.surface_is_open(&SurfaceId::new("async-alert")));
        assert_eq!(
            session
                .wait_for_surface_closed(&SurfaceId::new("async-alert"), PollOptions::default())
                .expect("controller-only dialog metadata should evict immediately"),
            0
        );
        assert!(!session.surface_is_open(&SurfaceId::new("async-alert")));
        assert!(matches!(
            session.dialog_kind(&SurfaceId::new("async-alert")),
            Err(DialogError::MissingSurface)
        ));
    }

    fn fixed_root(width: i32, height: i32) -> gtk4::Fixed {
        let root = gtk4::Fixed::new();
        root.set_size_request(width, height);
        root
    }

    fn temp_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("glasscheck-{label}-{stamp}"));
        fs::create_dir_all(&path).expect("temp test directory should be creatable");
        path
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
        legacy.set_propagation_phase(gtk4::PropagationPhase::Capture);
        legacy.connect_event(move |_, event| {
            if event.event_type() == gtk4::gdk::EventType::KeyPress {
                count_ref.set(count_ref.get() + 1);
            }
            gtk4::glib::Propagation::Proceed
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
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
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
        legacy.set_propagation_phase(gtk4::PropagationPhase::Capture);
        legacy.connect_event(move |_, event| {
            if event.event_type() == gtk4::gdk::EventType::KeyPress {
                count_ref.set(count_ref.get() + 1);
            }
            gtk4::glib::Propagation::Proceed
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
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        controller.connect_key_pressed(move |_, key, _, mods| {
            // Only record modifier state for the primary key, not for modifier
            // keys injected separately by XTest (e.g. Control_L press).
            if key.name().as_deref() == Some("a") {
                seen.borrow_mut().push(mods);
            }
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
        assert_eq!(
            seen.len(),
            1,
            "exactly one key_pressed event for \"a\" should be observed"
        );
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
}
