#![cfg(target_os = "macos")]

use std::cell::Cell;
use std::rc::Rc;

use glasscheck_appkit::{AppKitHarness, AppKitWindowHost, InstrumentedView};
use glasscheck_core::{
    assert_above, assert_vertical_alignment, compare_images, CompareConfig, LayoutTolerance,
    NodePredicate, Point, PropertyValue, QueryError, Rect, RegionResolveError, RelativeBounds,
    Role, SemanticNode, SemanticProvider, Size,
};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSButton, NSEvent, NSEventModifierFlags, NSFont, NSTextInputClient, NSTextView, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSRect, NSSize, NSString};

fn main() {
    let mtm = MainThreadMarker::new().expect("AppKit tests must run on the main thread");
    let harness = AppKitHarness::new(mtm);

    run("attach_to_existing_window_builds_scene_snapshot", || {
        attach_to_existing_window_builds_scene_snapshot(harness)
    });
    run("provider_only_scene_without_content_view_is_usable", || {
        provider_only_scene_without_content_view_is_usable(harness)
    });
    run("provider_only_region_capture_fails_cleanly", || {
        provider_only_region_capture_fails_cleanly(harness)
    });
    run("root_view_only_host_without_window_is_safe", || {
        root_view_only_host_without_window_is_safe(harness)
    });
    run(
        "root_view_attachment_with_window_stays_pinned_to_supplied_view",
        || root_view_attachment_with_window_stays_pinned_to_supplied_view(harness),
    );
    run(
        "pinned_root_view_semantic_click_uses_window_coordinates",
        || pinned_root_view_semantic_click_uses_window_coordinates(harness),
    );
    run("text_range_rect_converts_to_root_coordinates", || {
        text_range_rect_converts_to_root_coordinates(harness)
    });
    run(
        "insertion_caret_rect_converts_from_screen_to_root_coordinates",
        || insertion_caret_rect_converts_from_screen_to_root_coordinates(harness),
    );
    run("attached_window_refreshes_after_content_view_swap", || {
        attached_window_refreshes_after_content_view_swap(harness)
    });
    run("attached_window_reports_missing_node", || {
        attached_window_reports_missing_node(harness)
    });
    run("query_root_is_scene_backed", || {
        query_root_is_scene_backed(harness)
    });
    run("registered_views_leave_active_root_subtree_cleanly", || {
        registered_views_leave_active_root_subtree_cleanly(harness)
    });
    run(
        "attached_window_registry_drops_nodes_after_content_swap",
        || attached_window_registry_drops_nodes_after_content_swap(harness),
    );
    run(
        "native_snapshot_visibility_tracks_hidden_ancestors_and_clipping",
        || native_snapshot_visibility_tracks_hidden_ancestors_and_clipping(harness),
    );
    run(
        "provider_only_root_relative_region_uses_content_bounds",
        || provider_only_root_relative_region_uses_content_bounds(harness),
    );
    run("sidebar_row_alignment_assertion_passes", || {
        sidebar_row_alignment_assertion_passes(harness)
    });
    run(
        "sidebar_row_alignment_assertion_fails_for_misaligned_text",
        || sidebar_row_alignment_assertion_fails_for_misaligned_text(harness),
    );
    run(
        "virtual_semantic_provider_supports_stable_card_queries",
        || virtual_semantic_provider_supports_stable_card_queries(harness),
    );
    run("virtual_semantic_provider_reports_missing_card", || {
        virtual_semantic_provider_reports_missing_card(harness)
    });
    run("virtual_semantic_provider_layout_assertions_pass", || {
        virtual_semantic_provider_layout_assertions_pass(harness)
    });
    run(
        "virtual_semantic_provider_layout_assertions_fail_for_regression",
        || virtual_semantic_provider_layout_assertions_fail_for_regression(harness),
    );
    run(
        "provider_ids_are_namespaced_when_they_collide_with_native_ids",
        || provider_ids_are_namespaced_when_they_collide_with_native_ids(harness),
    );
    run("colliding_provider_id_is_not_left_ambiguous", || {
        colliding_provider_id_is_not_left_ambiguous(harness)
    });
    run(
        "provider_namespacing_preserves_existing_source_id_property",
        || provider_namespacing_preserves_existing_source_id_property(harness),
    );
    run(
        "provider_namespacing_preserves_unique_native_parent_relationships",
        || provider_namespacing_preserves_unique_native_parent_relationships(harness),
    );
    run(
        "provider_namespacing_marks_ambiguous_native_parents",
        || provider_namespacing_marks_ambiguous_native_parents(harness),
    );
    run(
        "provider_parent_repair_marks_ambiguous_native_parents_without_namespacing",
        || provider_parent_repair_marks_ambiguous_native_parents_without_namespacing(harness),
    );
    run(
        "duplicate_provider_ids_do_not_invent_parent_relationships",
        || duplicate_provider_ids_do_not_invent_parent_relationships(harness),
    );
    run(
        "semantic_click_uses_matched_handle_when_native_ids_duplicate",
        || semantic_click_uses_matched_handle_when_native_ids_duplicate(harness),
    );
    run("semantic_click_targets_registered_node", || {
        semantic_click_targets_registered_node(harness)
    });
    run(
        "provider_click_after_content_swap_does_not_dispatch_to_stale_registered_view",
        || provider_click_after_content_swap_does_not_dispatch_to_stale_registered_view(harness),
    );
    run(
        "semantic_click_on_registered_ancestor_routes_to_descendant_hit_view",
        || semantic_click_on_registered_ancestor_routes_to_descendant_hit_view(harness),
    );
    run("semantic_click_reports_missing_node", || {
        semantic_click_reports_missing_node(harness)
    });
    run("semantic_click_uses_visible_hit_testable_point", || {
        semantic_click_uses_visible_hit_testable_point(harness)
    });
    run("semantic_click_reports_unhittable_registered_node", || {
        semantic_click_reports_unhittable_registered_node(harness)
    });
    run(
        "semantic_click_reports_unhittable_registered_root_node",
        || semantic_click_reports_unhittable_registered_root_node(harness),
    );
    run(
        "provider_only_semantic_click_reports_unavailable_input",
        || provider_only_semantic_click_reports_unavailable_input(harness),
    );
    run(
        "semantic_click_stress_does_not_duplicate_mouse_downs",
        || semantic_click_stress_does_not_duplicate_mouse_downs(harness),
    );
    run(
        "semantic_click_rejects_registered_node_when_hit_test_returns_ancestor",
        || semantic_click_rejects_registered_node_when_hit_test_returns_ancestor(harness),
    );
    run(
        "semantic_click_rejects_registered_node_when_hit_test_is_unknown",
        || semantic_click_rejects_registered_node_when_hit_test_is_unknown(harness),
    );
    run(
        "attached_window_prunes_stale_registered_views_after_content_swap",
        || attached_window_prunes_stale_registered_views_after_content_swap(harness),
    );
    run(
        "move_mouse_targets_attached_window_even_when_another_window_is_key",
        || move_mouse_targets_attached_window_even_when_another_window_is_key(harness),
    );
    run(
        "key_press_targets_attached_window_even_when_another_window_is_key",
        || key_press_targets_attached_window_even_when_another_window_is_key(harness),
    );
    run(
        "registered_native_hierarchy_supports_parent_and_child_indexes",
        || registered_native_hierarchy_supports_parent_and_child_indexes(harness),
    );
    run(
        "registered_native_hierarchy_reports_missing_ancestor",
        || registered_native_hierarchy_reports_missing_ancestor(harness),
    );
    run(
        "registered_native_hierarchy_uses_nearest_registered_ancestor",
        || registered_native_hierarchy_uses_nearest_registered_ancestor(harness),
    );
    run(
        "duplicate_native_ids_do_not_drop_ancestor_relationships",
        || duplicate_native_ids_do_not_drop_ancestor_relationships(harness),
    );
    run("nested_child_click_routes_to_hit_tested_view", || {
        nested_child_click_routes_to_hit_tested_view(harness)
    });
    run("stock_button_click_invokes_action_once", || {
        stock_button_click_invokes_action_once(harness)
    });
    run("stock_button_click_outside_does_not_invoke_action", || {
        stock_button_click_outside_does_not_invoke_action(harness)
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

fn attach_to_existing_window_builds_scene_snapshot(harness: AppKitHarness) {
    let host = harness.create_window(480.0, 240.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(480.0, 240.0));
    let row = make_view(harness.main_thread_marker(), NSSize::new(220.0, 24.0));
    row.setFrameOrigin(NSPoint::new(12.0, 18.0));
    let label = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(180.0, 18.0),
        "Draft",
    );
    label.setFrameOrigin(NSPoint::new(0.0, 3.0));
    row.addSubview(&label);
    root.addSubview(&row);
    host.set_content_view(&root);
    host.register_view(
        &row,
        InstrumentedView {
            id: Some("sidebar-row".into()),
            role: Some(Role::ListItem),
            label: Some("Draft".into()),
        },
    );
    host.register_view(
        &label,
        InstrumentedView {
            id: Some("sidebar-label".into()),
            role: Some(Role::Label),
            label: Some("Draft".into()),
        },
    );
    harness.settle(2);

    let attached = AppKitWindowHost::from_window(host.window());
    attached.register_view(
        &row,
        InstrumentedView {
            id: Some("sidebar-row".into()),
            role: Some(Role::ListItem),
            label: Some("Draft".into()),
        },
    );
    attached.register_view(
        &label,
        InstrumentedView {
            id: Some("sidebar-label".into()),
            role: Some(Role::Label),
            label: Some("Draft".into()),
        },
    );

    let scene = attached.snapshot_scene();
    let row = scene.find(&NodePredicate::id_eq("sidebar-row")).unwrap();
    let label = scene.find(&NodePredicate::id_eq("sidebar-label")).unwrap();
    assert_eq!(scene.node(row).unwrap().label.as_deref(), Some("Draft"));
    assert_eq!(scene.node(label).unwrap().role, Role::Label);
}

fn attached_window_reports_missing_node(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    host.set_content_view(&root);
    let attached = AppKitWindowHost::from_window(host.window());
    let error = attached
        .snapshot_scene()
        .find(&NodePredicate::id_eq("missing"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFoundPredicate(_)));
}

fn query_root_is_scene_backed(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    let child = make_view(harness.main_thread_marker(), NSSize::new(80.0, 24.0));
    root.addSubview(&child);
    host.set_content_view(&root);
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("native-child".into()),
            role: Some(Role::ListItem),
            label: Some("Loose".into()),
        },
    );
    host.set_semantic_provider(Box::new(ProviderOnlySceneProvider));
    harness.settle(2);

    let query_root = host.query_root();
    let provider = query_root
        .find_by_predicate(&NodePredicate::property_eq(
            "provider",
            PropertyValue::Bool(true),
        ))
        .expect("scene-backed query root should include provider nodes");
    let native = query_root
        .find_by_predicate(&NodePredicate::id_eq("native-child"))
        .expect("scene-backed query root should include registered native nodes");

    assert!(query_root.scene().is_some());
    assert_eq!(provider.id.as_deref(), Some("provider-node"));
    assert_eq!(native.id.as_deref(), Some("native-child"));
    assert!(query_root
        .find_by_predicate(&NodePredicate::parent(NodePredicate::id_eq("native-child")))
        .is_err());
}

fn registered_views_leave_active_root_subtree_cleanly(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let child = make_view(harness.main_thread_marker(), NSSize::new(80.0, 24.0));
    root.addSubview(&child);
    host.set_content_view(&root);
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("detached-child".into()),
            role: Some(Role::Container),
            label: Some("Detached".into()),
        },
    );
    harness.settle(2);
    assert!(host
        .snapshot_scene()
        .find(&NodePredicate::id_eq("detached-child"))
        .is_ok());

    child.removeFromSuperview();
    harness.settle(2);

    let error = host
        .snapshot_scene()
        .find(&NodePredicate::id_eq("detached-child"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFoundPredicate(_)));
}

fn attached_window_registry_drops_nodes_after_content_swap(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let initial_root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let initial_child = make_view(harness.main_thread_marker(), NSSize::new(80.0, 24.0));
    initial_root.addSubview(&initial_child);
    host.set_content_view(&initial_root);
    harness.settle(2);

    let attached = AppKitWindowHost::from_window(host.window());
    attached.register_view(
        &initial_child,
        InstrumentedView {
            id: Some("stale-child".into()),
            role: Some(Role::Container),
            label: Some("Stale".into()),
        },
    );
    assert!(attached
        .snapshot_scene()
        .find(&NodePredicate::id_eq("stale-child"))
        .is_ok());

    let replacement_root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    host.set_content_view(&replacement_root);
    harness.settle(2);

    let error = attached
        .snapshot_scene()
        .find(&NodePredicate::id_eq("stale-child"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFoundPredicate(_)));
}

fn native_snapshot_visibility_tracks_hidden_ancestors_and_clipping(harness: AppKitHarness) {
    let host = harness.create_window(120.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(120.0, 120.0));

    let hidden_parent = make_view(harness.main_thread_marker(), NSSize::new(60.0, 60.0));
    hidden_parent.setFrameOrigin(NSPoint::new(10.0, 10.0));
    hidden_parent.setHidden(true);
    let hidden_child = make_view(harness.main_thread_marker(), NSSize::new(30.0, 30.0));
    hidden_child.setFrameOrigin(NSPoint::new(5.0, 5.0));
    hidden_parent.addSubview(&hidden_child);
    root.addSubview(&hidden_parent);

    let clipping_parent = make_view(harness.main_thread_marker(), NSSize::new(40.0, 40.0));
    clipping_parent.setFrameOrigin(NSPoint::new(10.0, 10.0));
    let clipped_child = make_view(harness.main_thread_marker(), NSSize::new(30.0, 30.0));
    clipped_child.setFrameOrigin(NSPoint::new(30.0, 30.0));
    clipping_parent.addSubview(&clipped_child);
    root.addSubview(&clipping_parent);

    host.set_content_view(&root);
    host.register_view(
        &hidden_child,
        InstrumentedView {
            id: Some("hidden-child".into()),
            role: Some(Role::Container),
            label: Some("Hidden".into()),
        },
    );
    host.register_view(
        &clipped_child,
        InstrumentedView {
            id: Some("clipped-child".into()),
            role: Some(Role::Container),
            label: Some("Clipped".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let hidden = scene
        .node(scene.find(&NodePredicate::id_eq("hidden-child")).unwrap())
        .unwrap();
    let clipped = scene
        .node(scene.find(&NodePredicate::id_eq("clipped-child")).unwrap())
        .unwrap();

    assert!(!hidden.visible);
    assert!(!hidden.hit_testable);
    assert_eq!(
        clipped.visible_rect,
        Some(Rect::new(Point::new(40.0, 40.0), Size::new(10.0, 10.0),))
    );
}

fn provider_only_root_relative_region_uses_content_bounds(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 160.0);
    host.window().setContentView(None);
    let attached = AppKitWindowHost::from_window(host.window());

    let rect = attached
        .resolve_region(
            &glasscheck_core::RegionSpec::root()
                .subregion(RelativeBounds::new(0.25, 0.5, 0.5, 0.25)),
        )
        .expect("root-relative regions should resolve without a content view");
    let content = host.window().contentLayoutRect();

    assert_eq!(
        rect,
        Rect::new(
            Point::new(0.25 * content.size.width, 0.5 * content.size.height),
            Size::new(0.5 * content.size.width, 0.25 * content.size.height),
        )
    );
}

fn provider_only_scene_without_content_view_is_usable(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 160.0);
    let attached = AppKitWindowHost::from_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let scene = attached.snapshot_scene();
    let node = scene
        .find(&NodePredicate::id_eq("provider-node"))
        .expect("provider node should be queryable without a content view");

    assert_eq!(
        scene.node(node).unwrap().label.as_deref(),
        Some("Provider Node")
    );
}

fn provider_only_region_capture_fails_cleanly(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 160.0);
    host.window().setContentView(None);
    let attached = AppKitWindowHost::from_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let error = attached
        .capture_region(&glasscheck_core::RegionSpec::node(NodePredicate::id_eq(
            "provider-node",
        )))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::CaptureUnavailable));
}

fn root_view_only_host_without_window_is_safe(harness: AppKitHarness) {
    let root = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    let host = AppKitWindowHost::from_root_view(&root, None);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Button),
            label: Some("Root".into()),
        },
    );
    harness.settle(2);

    assert!(host.window().contentView().is_some());
    let _ = host.input();
    let error = host.click_node(&NodePredicate::id_eq("root")).unwrap_err();
    assert!(matches!(error, RegionResolveError::DetachedRootView));
    assert_eq!(root.ivars().mouse_downs.get(), 0);
}

fn root_view_attachment_with_window_stays_pinned_to_supplied_view(harness: AppKitHarness) {
    let host = harness.create_window(260.0, 180.0);
    let content = make_view(harness.main_thread_marker(), NSSize::new(260.0, 180.0));
    let pinned = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(120.0, 40.0),
        "Pinned Root",
    );
    pinned.setFrameOrigin(NSPoint::new(40.0, 30.0));
    content.addSubview(&pinned);
    host.set_content_view(&content);
    harness.settle(2);

    let attached = AppKitWindowHost::from_root_view(&pinned, Some(host.window()));
    let actual = attached
        .capture()
        .expect("attached root view should capture");
    let expected = attached
        .capture_view(&pinned)
        .expect("pinned root view should capture directly");
    let result = compare_images(
        &actual,
        &expected,
        &CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: false,
        },
    );

    assert!(result.passed, "attached root capture should stay pinned");
}

fn pinned_root_view_semantic_click_uses_window_coordinates(harness: AppKitHarness) {
    let host = harness.create_window(260.0, 180.0);
    let content = make_view(harness.main_thread_marker(), NSSize::new(260.0, 180.0));
    let pinned = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(40.0, 30.0), NSSize::new(120.0, 60.0)),
    );
    content.addSubview(&pinned);
    host.set_content_view(&content);
    host.register_view(
        &pinned,
        InstrumentedView {
            id: Some("pinned-click".into()),
            role: Some(Role::Button),
            label: Some("Pinned".into()),
        },
    );
    harness.settle(2);

    let attached = AppKitWindowHost::from_root_view(&pinned, Some(host.window()));
    attached.register_view(
        &pinned,
        InstrumentedView {
            id: Some("pinned-click".into()),
            role: Some(Role::Button),
            label: Some("Pinned".into()),
        },
    );

    attached
        .click_node(&NodePredicate::id_eq("pinned-click"))
        .unwrap();
    harness.settle(2);

    assert_eq!(pinned.ivars().mouse_downs.get(), 1);
}

fn text_range_rect_converts_to_root_coordinates(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(160.0, 80.0),
        "Coordinate space",
    );
    view.setFrameOrigin(NSPoint::new(28.0, 34.0));
    view.setTextContainerInset(NSSize::new(7.0, 11.0));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    root.addSubview(&view);
    host.set_content_view(&root);
    harness.settle(2);

    let range = NSRange::new(0, 10);
    let actual = host
        .text_range_rect(&view, range)
        .expect("text range rect should resolve");

    let layout_manager =
        unsafe { view.layoutManager() }.expect("text view should have a layout manager");
    let text_container =
        unsafe { view.textContainer() }.expect("text view should have a text container");
    let glyph_range = unsafe {
        layout_manager.glyphRangeForCharacterRange_actualCharacterRange(range, std::ptr::null_mut())
    };
    let bounding =
        layout_manager.boundingRectForGlyphRange_inTextContainer(glyph_range, &text_container);
    let origin = view.textContainerOrigin();
    let local = NSRect::new(
        NSPoint::new(bounding.origin.x + origin.x, bounding.origin.y + origin.y),
        bounding.size,
    );
    let expected = root.convertRect_fromView(local, Some(&view));

    assert_eq!(
        actual,
        Rect::new(
            Point::new(expected.origin.x, expected.origin.y),
            Size::new(expected.size.width, expected.size.height),
        )
    );
}

fn insertion_caret_rect_converts_from_screen_to_root_coordinates(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(160.0, 80.0),
        "Caret conversion",
    );
    view.setFrameOrigin(NSPoint::new(42.0, 26.0));
    view.setTextContainerInset(NSSize::new(8.0, 9.0));
    if let Some(text_container) = unsafe { view.textContainer() } {
        text_container.setLineFragmentPadding(0.0);
    }
    root.addSubview(&view);
    host.set_content_view(&root);
    harness.settle(2);

    let location = 3;
    let actual = host
        .insertion_caret_rect(&view, location)
        .expect("caret rect should resolve");
    let screen_rect = unsafe {
        view.firstRectForCharacterRange_actualRange(NSRange::new(location, 0), std::ptr::null_mut())
    };
    let expected =
        root.convertRect_fromView(host.window().convertRectFromScreen(screen_rect), None);

    assert_eq!(
        actual,
        Rect::new(
            Point::new(expected.origin.x, expected.origin.y),
            Size::new(expected.size.width, expected.size.height),
        )
    );
}

fn attached_window_refreshes_after_content_view_swap(harness: AppKitHarness) {
    let host = harness.create_window(260.0, 140.0);
    let initial_root = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(260.0, 140.0),
        "Initial root",
    );
    let replacement_root = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(260.0, 140.0),
        "Replacement root",
    );
    host.set_content_view(&initial_root);
    harness.settle(2);

    let attached = AppKitWindowHost::from_window(host.window());
    host.window().setContentView(Some(&replacement_root));
    harness.settle(2);

    let actual = attached.capture().expect("attached host should capture");
    let expected = attached
        .capture_view(&replacement_root)
        .expect("replacement root should capture");
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
        "attached host should track the swapped content view"
    );
}

fn move_mouse_targets_attached_window_even_when_another_window_is_key(harness: AppKitHarness) {
    let target = harness.create_window(180.0, 120.0);
    let target_view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    target.set_content_view(&target_view);
    target.window().setAcceptsMouseMovedEvents(true);
    target.window().makeFirstResponder(Some(&target_view));

    let other = harness.create_window(180.0, 120.0);
    let other_view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    other.set_content_view(&other_view);
    other.window().setAcceptsMouseMovedEvents(true);
    other.window().makeFirstResponder(Some(&other_view));
    other.window().makeKeyAndOrderFront(None);
    other.window().makeKeyWindow();
    harness.settle(2);

    target.input().move_mouse(NSPoint::new(40.0, 40.0));
    harness.settle(2);

    assert_eq!(target_view.ivars().mouse_moves.get(), 1);
    assert_eq!(other_view.ivars().mouse_moves.get(), 0);
}

fn key_press_targets_attached_window_even_when_another_window_is_key(harness: AppKitHarness) {
    let target = harness.create_window(180.0, 120.0);
    let target_view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    target.set_content_view(&target_view);
    target.window().makeFirstResponder(Some(&target_view));

    let other = harness.create_window(180.0, 120.0);
    let other_view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    other.set_content_view(&other_view);
    other.window().makeFirstResponder(Some(&other_view));
    other.window().makeKeyAndOrderFront(None);
    other.window().makeKeyWindow();
    harness.settle(2);

    target
        .input()
        .key_press(0, NSEventModifierFlags::empty(), "a");
    harness.settle(2);

    assert_eq!(target_view.ivars().key_downs.get(), 1);
    assert_eq!(other_view.ivars().key_downs.get(), 0);
}

fn registered_native_hierarchy_supports_parent_and_child_indexes(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 160.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(240.0, 160.0));
    let left = make_view(harness.main_thread_marker(), NSSize::new(90.0, 40.0));
    left.setFrameOrigin(NSPoint::new(10.0, 20.0));
    let right = make_view(harness.main_thread_marker(), NSSize::new(90.0, 40.0));
    right.setFrameOrigin(NSPoint::new(110.0, 20.0));
    let label = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(70.0, 18.0),
        "Nested",
    );
    label.setFrameOrigin(NSPoint::new(0.0, 6.0));

    right.addSubview(&label);
    root.addSubview(&left);
    root.addSubview(&right);
    host.set_content_view(&root);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Container),
            label: Some("Root".into()),
        },
    );
    host.register_view(
        &left,
        InstrumentedView {
            id: Some("left".into()),
            role: Some(Role::Container),
            label: Some("Left".into()),
        },
    );
    host.register_view(
        &right,
        InstrumentedView {
            id: Some("right".into()),
            role: Some(Role::Container),
            label: Some("Right".into()),
        },
    );
    host.register_view(
        &label,
        InstrumentedView {
            id: Some("label".into()),
            role: Some(Role::Label),
            label: Some("Nested".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let left_handle = scene.find(&NodePredicate::id_eq("left")).unwrap();
    let right_handle = scene.find(&NodePredicate::id_eq("right")).unwrap();
    let label_handle = scene.find(&NodePredicate::id_eq("label")).unwrap();

    assert_eq!(
        scene.node(left_handle).unwrap().parent_id.as_deref(),
        Some("root")
    );
    assert_eq!(scene.node(left_handle).unwrap().child_index, 0);
    assert_eq!(scene.node(left_handle).unwrap().z_index, 0);
    assert_eq!(
        scene.node(right_handle).unwrap().parent_id.as_deref(),
        Some("root")
    );
    assert_eq!(scene.node(right_handle).unwrap().child_index, 1);
    assert_eq!(scene.node(right_handle).unwrap().z_index, 1);
    assert_eq!(
        scene.node(label_handle).unwrap().parent_id.as_deref(),
        Some("right")
    );
    assert_eq!(scene.node(label_handle).unwrap().child_index, 0);
    assert_eq!(scene.node(label_handle).unwrap().z_index, 0);

    let root_children = scene.find_all(&NodePredicate::parent(NodePredicate::id_eq("root")));
    assert_eq!(root_children.len(), 2);
    assert_eq!(scene.node(root_children[0]).unwrap().id, "left");
    assert_eq!(scene.node(root_children[1]).unwrap().id, "right");
    assert_eq!(
        scene
            .find(&NodePredicate::parent(NodePredicate::id_eq("right")))
            .unwrap()
            .index(),
        label_handle.index()
    );
    assert_eq!(
        scene
            .find(&NodePredicate::ancestor(NodePredicate::id_eq("right")))
            .unwrap()
            .index(),
        label_handle.index()
    );
}

fn registered_native_hierarchy_reports_missing_ancestor(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    let child = make_view(harness.main_thread_marker(), NSSize::new(80.0, 24.0));
    root.addSubview(&child);
    host.set_content_view(&root);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Container),
            label: Some("Root".into()),
        },
    );
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("child".into()),
            role: Some(Role::Container),
            label: Some("Child".into()),
        },
    );
    harness.settle(2);

    let error = host
        .snapshot_scene()
        .find(&NodePredicate::ancestor(NodePredicate::id_eq("missing")))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFoundPredicate(_)));
}

fn registered_native_hierarchy_uses_nearest_registered_ancestor(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 160.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(240.0, 160.0));
    let wrapper = make_view(harness.main_thread_marker(), NSSize::new(120.0, 80.0));
    wrapper.setFrameOrigin(NSPoint::new(20.0, 20.0));
    let label = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(80.0, 20.0),
        "Nested",
    );
    label.setFrameOrigin(NSPoint::new(10.0, 12.0));
    wrapper.addSubview(&label);
    root.addSubview(&wrapper);
    host.set_content_view(&root);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Container),
            label: Some("Root".into()),
        },
    );
    host.register_view(
        &label,
        InstrumentedView {
            id: Some("label".into()),
            role: Some(Role::Label),
            label: Some("Nested".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let label_handle = scene.find(&NodePredicate::id_eq("label")).unwrap();
    assert_eq!(
        scene.node(label_handle).unwrap().parent_id.as_deref(),
        Some("root")
    );
    let children = scene.find_all(&NodePredicate::parent(NodePredicate::id_eq("root")));
    assert_eq!(children.len(), 1);
    assert_eq!(scene.node(children[0]).unwrap().id, "label");
}

fn duplicate_native_ids_do_not_drop_ancestor_relationships(harness: AppKitHarness) {
    let host = harness.create_window(280.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(280.0, 180.0));
    let left = make_view(harness.main_thread_marker(), NSSize::new(100.0, 60.0));
    left.setFrameOrigin(NSPoint::new(20.0, 20.0));
    let right = make_view(harness.main_thread_marker(), NSSize::new(100.0, 60.0));
    right.setFrameOrigin(NSPoint::new(140.0, 20.0));
    let left_label = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(60.0, 18.0),
        "Left",
    );
    left_label.setFrameOrigin(NSPoint::new(8.0, 8.0));
    let right_label = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(60.0, 18.0),
        "Right",
    );
    right_label.setFrameOrigin(NSPoint::new(8.0, 8.0));

    left.addSubview(&left_label);
    right.addSubview(&right_label);
    root.addSubview(&left);
    root.addSubview(&right);
    host.set_content_view(&root);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Container),
            label: Some("Root".into()),
        },
    );
    host.register_view(
        &left,
        InstrumentedView {
            id: Some("duplicate-parent".into()),
            role: Some(Role::Container),
            label: Some("Left Parent".into()),
        },
    );
    host.register_view(
        &right,
        InstrumentedView {
            id: Some("duplicate-parent".into()),
            role: Some(Role::Container),
            label: Some("Right Parent".into()),
        },
    );
    host.register_view(
        &left_label,
        InstrumentedView {
            id: Some("left-label".into()),
            role: Some(Role::Label),
            label: Some("Left".into()),
        },
    );
    host.register_view(
        &right_label,
        InstrumentedView {
            id: Some("right-label".into()),
            role: Some(Role::Label),
            label: Some("Right".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let parents = scene.find_all(&NodePredicate::id_eq("duplicate-parent"));
    assert_eq!(parents.len(), 0);

    let disambiguated_parents = scene.find_all(&NodePredicate::id_eq("native::duplicate-parent"));
    assert_eq!(disambiguated_parents.len(), 1);
    assert_eq!(
        scene
            .find_all(&NodePredicate::id_eq("native::duplicate-parent#1"))
            .len(),
        1
    );

    let left_label = scene.find(&NodePredicate::id_eq("left-label")).unwrap();
    let right_label = scene.find(&NodePredicate::id_eq("right-label")).unwrap();
    assert_eq!(
        scene.node(left_label).unwrap().parent_id.as_deref(),
        Some("native::duplicate-parent")
    );
    assert_eq!(
        scene.node(right_label).unwrap().parent_id.as_deref(),
        Some("native::duplicate-parent#1")
    );
    assert_eq!(
        scene
            .find(&NodePredicate::ancestor(NodePredicate::id_eq(
                "native::duplicate-parent"
            )))
            .unwrap()
            .index(),
        left_label.index()
    );
    assert_eq!(
        scene
            .find(&NodePredicate::ancestor(NodePredicate::id_eq(
                "native::duplicate-parent#1"
            )))
            .unwrap()
            .index(),
        right_label.index()
    );
    let original_id_matches =
        scene.find_all(&NodePredicate::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("duplicate-parent"),
        ));
    assert_eq!(original_id_matches.len(), 2);
}

fn nested_child_click_routes_to_hit_tested_view(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let parent = ClickTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, 140.0)),
    );
    let child = ClickTrackingChildView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(24.0, 20.0), NSSize::new(80.0, 36.0)),
    );
    parent.addSubview(&child);
    host.set_content_view(&parent);
    harness.settle(2);

    host.input().click(NSPoint::new(40.0, 32.0));
    harness.settle(2);

    let child_mouse_downs = child.ivars().mouse_downs.get();
    let parent_mouse_downs = parent.ivars().mouse_downs.get();
    assert_eq!(
        child_mouse_downs, 1,
        "child={} parent={}",
        child_mouse_downs, parent_mouse_downs
    );
    assert_eq!(
        parent_mouse_downs, 0,
        "child={} parent={}",
        child_mouse_downs, parent_mouse_downs
    );
}

fn stock_button_click_invokes_action_once(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    let target = ButtonActionTarget::new(harness.main_thread_marker());
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Run"),
            Some(target.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    button.setFrame(NSRect::new(
        NSPoint::new(24.0, 24.0),
        NSSize::new(120.0, 32.0),
    ));
    root.addSubview(&button);
    host.set_content_view(&root);
    host.register_view(
        &button,
        InstrumentedView {
            id: Some("run-button".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
        },
    );
    harness.settle(2);

    host.click_node(&NodePredicate::id_eq("run-button"))
        .unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().actions.get(), 1);
}

fn stock_button_click_outside_does_not_invoke_action(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    let target = ButtonActionTarget::new(harness.main_thread_marker());
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Run"),
            Some(target.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    button.setFrame(NSRect::new(
        NSPoint::new(24.0, 24.0),
        NSSize::new(120.0, 32.0),
    ));
    root.addSubview(&button);
    host.set_content_view(&root);
    harness.settle(2);

    host.input().click(NSPoint::new(190.0, 110.0));
    harness.settle(2);

    assert_eq!(target.ivars().actions.get(), 0);
}

fn sidebar_row_alignment_assertion_passes(harness: AppKitHarness) {
    let row_rect = Rect::new(Point::new(12.0, 18.0), Size::new(220.0, 24.0));
    let text_rect = Rect::new(Point::new(16.0, 21.0), Size::new(180.0, 18.0));
    assert_vertical_alignment(text_rect, row_rect, LayoutTolerance::default()).unwrap();
    let _ = harness;
}

fn sidebar_row_alignment_assertion_fails_for_misaligned_text(harness: AppKitHarness) {
    let row_rect = Rect::new(Point::new(12.0, 18.0), Size::new(220.0, 24.0));
    let text_rect = Rect::new(Point::new(16.0, 28.0), Size::new(180.0, 18.0));
    assert!(assert_vertical_alignment(text_rect, row_rect, LayoutTolerance::default()).is_err());
    let _ = harness;
}

fn virtual_semantic_provider_supports_stable_card_queries(harness: AppKitHarness) {
    let host = harness.create_window(600.0, 360.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(600.0, 360.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(CardSceneProvider::default()));
    let scene = host.snapshot_scene();
    let card = scene
        .find(&NodePredicate::property_eq(
            "session_id",
            PropertyValue::Integer(3),
        ))
        .unwrap();
    assert_eq!(scene.node(card).unwrap().id, "battlefield/card:session-3");
}

fn virtual_semantic_provider_reports_missing_card(harness: AppKitHarness) {
    let host = harness.create_window(600.0, 360.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(600.0, 360.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(CardSceneProvider::default()));
    let error = host
        .snapshot_scene()
        .find(&NodePredicate::property_eq(
            "session_id",
            PropertyValue::Integer(99),
        ))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFoundPredicate(_)));
}

fn virtual_semantic_provider_layout_assertions_pass(harness: AppKitHarness) {
    let host = harness.create_window(600.0, 360.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(600.0, 360.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(CardSceneProvider::default()));
    let scene = host.snapshot_scene();

    let title = scene
        .node(
            scene
                .find(&NodePredicate::id_eq("battlefield/card:session-3/title"))
                .unwrap(),
        )
        .unwrap();
    let chip = scene
        .node(
            scene
                .find(&NodePredicate::id_eq(
                    "battlefield/card:session-3/status-chip",
                ))
                .unwrap(),
        )
        .unwrap();
    assert_above(title.rect, chip.rect, LayoutTolerance::default()).unwrap();
}

fn virtual_semantic_provider_layout_assertions_fail_for_regression(harness: AppKitHarness) {
    let host = harness.create_window(600.0, 360.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(600.0, 360.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(BrokenCardSceneProvider));
    let scene = host.snapshot_scene();

    let title = scene
        .node(
            scene
                .find(&NodePredicate::id_eq("battlefield/card:session-3/title"))
                .unwrap(),
        )
        .unwrap();
    let chip = scene
        .node(
            scene
                .find(&NodePredicate::id_eq(
                    "battlefield/card:session-3/status-chip",
                ))
                .unwrap(),
        )
        .unwrap();
    assert!(assert_above(title.rect, chip.rect, LayoutTolerance::default()).is_err());
}

fn provider_ids_are_namespaced_when_they_collide_with_native_ids(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    let native = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    root.addSubview(&native);
    host.set_content_view(&root);
    host.register_view(
        &native,
        InstrumentedView {
            id: Some("battlefield".into()),
            role: Some(Role::Container),
            label: Some("Native Battlefield".into()),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_root = scene
        .find(&NodePredicate::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("battlefield"),
        ))
        .unwrap();
    let provider_child = scene
        .find(&NodePredicate::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("battlefield/card:session-3"),
        ))
        .unwrap();

    assert_eq!(
        scene.node(provider_root).unwrap().id,
        "provider::battlefield"
    );
    assert_eq!(
        scene.node(provider_child).unwrap().parent_id.as_deref(),
        Some("provider::battlefield")
    );
}

fn colliding_provider_id_is_not_left_ambiguous(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    let native = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    root.addSubview(&native);
    host.set_content_view(&root);
    host.register_view(
        &native,
        InstrumentedView {
            id: Some("battlefield".into()),
            role: Some(Role::Container),
            label: Some("Native Battlefield".into()),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let native_handle = scene.find(&NodePredicate::id_eq("battlefield")).unwrap();

    assert_eq!(
        scene.node(native_handle).unwrap().label.as_deref(),
        Some("Native Battlefield")
    );
    assert!(scene
        .find(&NodePredicate::id_eq(
            "provider::battlefield/card:session-3",
        ))
        .is_ok());
}

fn provider_namespacing_preserves_existing_source_id_property(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    let native = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    root.addSubview(&native);
    host.set_content_view(&root);
    host.register_view(
        &native,
        InstrumentedView {
            id: Some("battlefield".into()),
            role: Some(Role::Container),
            label: Some("Native Battlefield".into()),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProviderWithSourceId));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider = scene
        .find(&NodePredicate::property_eq(
            "source_id",
            PropertyValue::string("provider-owned"),
        ))
        .unwrap();

    assert_eq!(scene.node(provider).unwrap().id, "provider::battlefield");
    assert_eq!(
        scene
            .node(provider)
            .unwrap()
            .properties
            .get("glasscheck:source_id"),
        Some(&PropertyValue::string("battlefield"))
    );
}

fn provider_namespacing_preserves_unique_native_parent_relationships(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    let native = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    root.addSubview(&native);
    host.set_content_view(&root);
    host.register_view(
        &native,
        InstrumentedView {
            id: Some("battlefield".into()),
            role: Some(Role::Container),
            label: Some("Native Battlefield".into()),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProviderWithNativeChild));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_child = scene
        .find(&NodePredicate::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("battlefield/native-child"),
        ))
        .unwrap();

    assert_eq!(
        scene.node(provider_child).unwrap().parent_id.as_deref(),
        Some("battlefield")
    );
    assert_eq!(
        scene
            .node(provider_child)
            .unwrap()
            .properties
            .get("glasscheck:ambiguous_parent_id"),
        None
    );
}

fn provider_namespacing_marks_ambiguous_native_parents(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    let first = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    let second = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    root.addSubview(&first);
    root.addSubview(&second);
    host.set_content_view(&root);
    for native in [&first, &second] {
        host.register_view(
            native,
            InstrumentedView {
                id: Some("battlefield".into()),
                role: Some(Role::Container),
                label: Some("Native Battlefield".into()),
            },
        );
    }
    host.set_semantic_provider(Box::new(CollidingProviderWithNativeChild));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_child = scene
        .find(&NodePredicate::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("battlefield/native-child"),
        ))
        .unwrap();

    assert_eq!(scene.node(provider_child).unwrap().parent_id, None);
    assert_eq!(
        scene
            .node(provider_child)
            .unwrap()
            .properties
            .get("glasscheck:ambiguous_parent_id"),
        Some(&PropertyValue::string("battlefield"))
    );
}

fn duplicate_provider_ids_do_not_invent_parent_relationships(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(DuplicateParentProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let child = scene
        .find(&NodePredicate::id_eq("provider::parent/child"))
        .unwrap();

    assert_eq!(scene.node(child).unwrap().parent_id, None);
    assert_eq!(
        scene
            .node(child)
            .unwrap()
            .properties
            .get("glasscheck:ambiguous_parent_id"),
        Some(&PropertyValue::string("parent"))
    );
    assert!(matches!(
        scene.find(&NodePredicate::ancestor(NodePredicate::id_eq(
            "provider::parent"
        ))),
        Err(QueryError::NotFoundPredicate(_))
    ));
}

fn provider_parent_repair_marks_ambiguous_native_parents_without_namespacing(
    harness: AppKitHarness,
) {
    let host = harness.create_window(320.0, 180.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 180.0));
    let first = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    let second = make_view(harness.main_thread_marker(), NSSize::new(120.0, 60.0));
    root.addSubview(&first);
    root.addSubview(&second);
    host.set_content_view(&root);
    for native in [&first, &second] {
        host.register_view(
            native,
            InstrumentedView {
                id: Some("battlefield".into()),
                role: Some(Role::Container),
                label: Some("Native Battlefield".into()),
            },
        );
    }
    host.set_semantic_provider(Box::new(UniqueProviderWithAmbiguousNativeParent));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_child = scene
        .find(&NodePredicate::id_eq("provider-card/label"))
        .unwrap();

    assert_eq!(scene.node(provider_child).unwrap().parent_id, None);
    assert_eq!(
        scene
            .node(provider_child)
            .unwrap()
            .properties
            .get("glasscheck:ambiguous_parent_id"),
        Some(&PropertyValue::string("battlefield"))
    );
}

fn semantic_click_uses_matched_handle_when_native_ids_duplicate(harness: AppKitHarness) {
    let host = harness.create_window(260.0, 160.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(260.0, 160.0));
    let first = ButtonActionTarget::new(harness.main_thread_marker());
    let first_button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("First"),
            Some(first.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    first_button.setFrame(NSRect::new(
        NSPoint::new(20.0, 20.0),
        NSSize::new(100.0, 32.0),
    ));
    let second = ButtonActionTarget::new(harness.main_thread_marker());
    let second_button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Second"),
            Some(second.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    second_button.setFrame(NSRect::new(
        NSPoint::new(140.0, 20.0),
        NSSize::new(100.0, 32.0),
    ));
    root.addSubview(&first_button);
    root.addSubview(&second_button);
    host.set_content_view(&root);
    host.register_view(
        &first_button,
        InstrumentedView {
            id: Some("duplicate-id".into()),
            role: Some(Role::Button),
            label: Some("First".into()),
        },
    );
    host.register_view(
        &second_button,
        InstrumentedView {
            id: Some("duplicate-id".into()),
            role: Some(Role::Button),
            label: Some("Second".into()),
        },
    );
    harness.settle(2);

    host.click_node(&NodePredicate::label(glasscheck_core::TextMatch::exact(
        "Second",
    )))
    .unwrap();
    harness.settle(2);

    assert_eq!(first.ivars().actions.get(), 0);
    assert_eq!(second.ivars().actions.get(), 1);
}

fn semantic_click_targets_registered_node(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let view = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.register_view(
        &view,
        InstrumentedView {
            id: Some("click-target".into()),
            role: Some(Role::Button),
            label: Some("Click Target".into()),
        },
    );
    harness.settle(2);

    host.click_node(&NodePredicate::id_eq("click-target"))
        .unwrap();
    harness.settle(2);

    assert_eq!(view.ivars().mouse_downs.get(), 1);
}

fn provider_click_after_content_swap_does_not_dispatch_to_stale_registered_view(
    harness: AppKitHarness,
) {
    let host = harness.create_window(220.0, 140.0);
    let initial_root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    let target = ButtonActionTarget::new(harness.main_thread_marker());
    let stale_button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Stale"),
            Some(target.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    stale_button.setFrame(NSRect::new(
        NSPoint::new(20.0, 20.0),
        NSSize::new(120.0, 32.0),
    ));
    initial_root.addSubview(&stale_button);
    host.set_content_view(&initial_root);
    harness.settle(2);

    let attached = AppKitWindowHost::from_window(host.window());
    attached.register_view(
        &stale_button,
        InstrumentedView {
            id: Some("stale-button".into()),
            role: Some(Role::Button),
            label: Some("Stale".into()),
        },
    );
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));
    assert!(attached
        .snapshot_scene()
        .find(&NodePredicate::id_eq("stale-button"))
        .is_ok());

    let replacement_root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    host.set_content_view(&replacement_root);
    harness.settle(2);

    attached
        .click_node(&NodePredicate::id_eq("provider-node"))
        .unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().actions.get(), 0);
}

fn semantic_click_on_registered_ancestor_routes_to_descendant_hit_view(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let parent = ClickTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, 140.0)),
    );
    let child = ClickTrackingChildView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(50.0, 40.0), NSSize::new(120.0, 60.0)),
    );
    parent.addSubview(&child);
    host.set_content_view(&parent);
    host.register_view(
        &parent,
        InstrumentedView {
            id: Some("container".into()),
            role: Some(Role::Button),
            label: Some("Container".into()),
        },
    );
    harness.settle(2);

    host.click_node(&NodePredicate::id_eq("container")).unwrap();
    harness.settle(2);

    assert_eq!(child.ivars().mouse_downs.get(), 1);
    assert_eq!(parent.ivars().mouse_downs.get(), 0);
}

fn semantic_click_reports_missing_node(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let view = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    host.set_content_view(&view);
    let error = host
        .click_node(&NodePredicate::id_eq("missing"))
        .unwrap_err();
    assert!(matches!(error, RegionResolveError::NotFound(_)));
}

fn semantic_click_uses_visible_hit_testable_point(harness: AppKitHarness) {
    let host = harness.create_window(120.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(120.0, 120.0));
    let clipping_parent = make_view(harness.main_thread_marker(), NSSize::new(40.0, 40.0));
    clipping_parent.setFrameOrigin(NSPoint::new(10.0, 10.0));
    let target = PointTrackingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(30.0, 30.0), NSSize::new(30.0, 30.0)),
    );
    clipping_parent.addSubview(&target);
    root.addSubview(&clipping_parent);
    host.set_content_view(&root);
    host.register_view(
        &target,
        InstrumentedView {
            id: Some("clipped-target".into()),
            role: Some(Role::Button),
            label: Some("Clipped Target".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(scene.find(&NodePredicate::id_eq("clipped-target")).unwrap())
        .unwrap();
    assert_eq!(
        node.visible_rect,
        Some(Rect::new(Point::new(40.0, 40.0), Size::new(10.0, 10.0)))
    );
    assert!(node.hit_testable);

    host.click_node(&NodePredicate::id_eq("clipped-target"))
        .unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().mouse_downs.get(), 1);
    assert_eq!(target.ivars().last_click_x.get(), 45.0);
    assert_eq!(target.ivars().last_click_y.get(), 45.0);
}

fn semantic_click_reports_unhittable_registered_node(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let target = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    let occluder = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    root.addSubview(&target);
    root.addSubview(&occluder);
    host.set_content_view(&root);
    host.register_view(
        &target,
        InstrumentedView {
            id: Some("covered-target".into()),
            role: Some(Role::Button),
            label: Some("Covered Target".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(scene.find(&NodePredicate::id_eq("covered-target")).unwrap())
        .unwrap();
    assert!(!node.hit_testable);

    let error = host
        .click_node(&NodePredicate::id_eq("covered-target"))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::InputUnavailable));
    assert_eq!(target.ivars().mouse_downs.get(), 0);
}

fn semantic_click_reports_unhittable_registered_root_node(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = NullHitTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&root);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("unhittable-root".into()),
            role: Some(Role::Button),
            label: Some("Unhittable Root".into()),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(
            scene
                .find(&NodePredicate::id_eq("unhittable-root"))
                .unwrap(),
        )
        .unwrap();
    assert!(!node.hit_testable);

    let error = host
        .click_node(&NodePredicate::id_eq("unhittable-root"))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::InputUnavailable));
    assert_eq!(root.ivars().mouse_downs.get(), 0);
}

fn semantic_click_rejects_registered_node_when_hit_test_returns_ancestor(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = AncestorHitTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    let target = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(80.0, 40.0)),
    );
    root.addSubview(&target);
    host.set_content_view(&root);
    host.register_view(
        &target,
        InstrumentedView {
            id: Some("ancestor-hit-target".into()),
            role: Some(Role::Button),
            label: Some("Ancestor Hit Target".into()),
        },
    );
    harness.settle(2);

    let error = host
        .click_node(&NodePredicate::id_eq("ancestor-hit-target"))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::InputUnavailable));
    assert_eq!(target.ivars().mouse_downs.get(), 0);
    assert_eq!(root.ivars().mouse_downs.get(), 0);
}

fn semantic_click_rejects_registered_node_when_hit_test_is_unknown(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = NullHitTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    let target = CountingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(80.0, 40.0)),
    );
    root.addSubview(&target);
    host.set_content_view(&root);
    host.register_view(
        &target,
        InstrumentedView {
            id: Some("unknown-hit-target".into()),
            role: Some(Role::Button),
            label: Some("Unknown Hit Target".into()),
        },
    );
    harness.settle(2);

    let error = host
        .click_node(&NodePredicate::id_eq("unknown-hit-target"))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::InputUnavailable));
    assert_eq!(target.ivars().mouse_downs.get(), 0);
    assert_eq!(root.ivars().mouse_downs.get(), 0);
}

fn attached_window_prunes_stale_registered_views_after_content_swap(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let releases = Rc::new(Cell::new(0));
    let initial_root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let initial_child = ReleaseTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(80.0, 24.0)),
        Rc::clone(&releases),
    );
    initial_root.addSubview(&initial_child);
    host.set_content_view(&initial_root);
    harness.settle(2);

    let attached = AppKitWindowHost::from_window(host.window());
    attached.register_view(
        &initial_child,
        InstrumentedView {
            id: Some("stale-dealloc-child".into()),
            role: Some(Role::Container),
            label: Some("Stale".into()),
        },
    );
    assert!(attached
        .snapshot_scene()
        .find(&NodePredicate::id_eq("stale-dealloc-child"))
        .is_ok());

    let replacement_root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    host.set_content_view(&replacement_root);
    drop(initial_child);
    drop(initial_root);
    let _ = attached.capture();
    let releases_before_prune = releases.get();
    harness.settle(2);

    let error = attached
        .snapshot_scene()
        .find(&NodePredicate::id_eq("stale-dealloc-child"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFoundPredicate(_)));
    assert!(
        releases.get() > releases_before_prune,
        "releases_before_prune={} releases_after_prune={}",
        releases_before_prune,
        releases.get()
    );
}

fn provider_only_semantic_click_reports_unavailable_input(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    host.window().setContentView(None);
    let attached = AppKitWindowHost::from_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let error = attached
        .click_node(&NodePredicate::id_eq("provider-node"))
        .unwrap_err();

    assert!(matches!(error, RegionResolveError::InputUnavailable));
}

fn semantic_click_stress_does_not_duplicate_mouse_downs(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let target = ButtonActionTarget::new(harness.main_thread_marker());
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Click Target"),
            Some(target.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    button.setFrame(NSRect::new(
        NSPoint::new(12.0, 10.0),
        NSSize::new(156.0, 96.0),
    ));
    root.addSubview(&button);
    host.set_content_view(&root);
    host.register_view(
        &button,
        InstrumentedView {
            id: Some("click-target".into()),
            role: Some(Role::Button),
            label: Some("Click Target".into()),
        },
    );
    harness.settle(2);

    for _ in 0..25 {
        host.click_node(&NodePredicate::id_eq("click-target"))
            .unwrap();
        harness.settle(1);
    }

    assert_eq!(target.ivars().actions.get(), 25);
}

fn make_view(mtm: MainThreadMarker, size: NSSize) -> Retained<NSView> {
    NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), size),
    )
}

fn make_text_view(
    mtm: MainThreadMarker,
    size: NSSize,
    content: &str,
) -> Retained<objc2_app_kit::NSTextView> {
    let view = objc2_app_kit::NSTextView::initWithFrame(
        objc2_app_kit::NSTextView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), size),
    );
    view.setEditable(false);
    view.setSelectable(false);
    view.setDrawsBackground(false);
    view.setString(&NSString::from_str(content));
    let font = NSFont::systemFontOfSize(13.0);
    view.setFont(Some(&font));
    view
}

#[derive(Clone, Default)]
struct RoutingTrackingIvars {
    mouse_moves: Cell<usize>,
    key_downs: Cell<usize>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RoutingTrackingIvars]
    struct RoutingTrackingView;

    impl RoutingTrackingView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, _event: &NSEvent) {
            let next = self.ivars().mouse_moves.get() + 1;
            self.ivars().mouse_moves.set(next);
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, _event: &NSEvent) {
            let next = self.ivars().key_downs.get() + 1;
            self.ivars().key_downs.set(next);
        }
    }
);

impl RoutingTrackingView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RoutingTrackingIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

#[derive(Default)]
struct CardSceneProvider;

impl SemanticProvider for CardSceneProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "battlefield",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(600.0, 360.0)),
            ),
            SemanticNode::new(
                "battlefield/card:session-3",
                Role::custom("card"),
                Rect::new(Point::new(24.0, 24.0), Size::new(240.0, 180.0)),
            )
            .with_parent("battlefield", 0)
            .with_property("session_id", PropertyValue::Integer(3)),
            SemanticNode::new(
                "battlefield/card:session-3/title",
                Role::Label,
                Rect::new(Point::new(36.0, 36.0), Size::new(120.0, 20.0)),
            )
            .with_parent("battlefield/card:session-3", 0)
            .with_label("Session 3"),
            SemanticNode::new(
                "battlefield/card:session-3/status-chip",
                Role::custom("status-chip"),
                Rect::new(Point::new(36.0, 64.0), Size::new(100.0, 22.0)),
            )
            .with_parent("battlefield/card:session-3", 1)
            .with_state("selected", PropertyValue::Bool(true)),
        ]
    }
}

struct BrokenCardSceneProvider;

impl SemanticProvider for BrokenCardSceneProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "battlefield/card:session-3",
                Role::custom("card"),
                Rect::new(Point::new(24.0, 24.0), Size::new(240.0, 180.0)),
            )
            .with_property("session_id", PropertyValue::Integer(3)),
            SemanticNode::new(
                "battlefield/card:session-3/title",
                Role::Label,
                Rect::new(Point::new(36.0, 70.0), Size::new(120.0, 20.0)),
            )
            .with_label("Session 3"),
            SemanticNode::new(
                "battlefield/card:session-3/status-chip",
                Role::custom("status-chip"),
                Rect::new(Point::new(36.0, 48.0), Size::new(100.0, 22.0)),
            ),
        ]
    }
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
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars().mouse_downs.set(self.ivars().mouse_downs.get() + 1);
        }
    }
);

impl CountingClickView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CountingClickIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

#[derive(Default)]
struct PointTrackingClickIvars {
    mouse_downs: Cell<usize>,
    last_click_x: Cell<f64>,
    last_click_y: Cell<f64>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PointTrackingClickIvars]
    struct PointTrackingClickView;

    impl PointTrackingClickView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.ivars().mouse_downs.set(self.ivars().mouse_downs.get() + 1);
            let point = event.locationInWindow();
            self.ivars().last_click_x.set(point.x);
            self.ivars().last_click_y.set(point.y);
        }
    }
);

impl PointTrackingClickView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PointTrackingClickIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

#[derive(Default)]
struct ClickTrackingIvars {
    mouse_downs: Cell<usize>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClickTrackingIvars]
    struct AncestorHitTrackingContainerView;

    impl AncestorHitTrackingContainerView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            if self.mouse_inRect(point, self.bounds()) {
                self as *const Self as *mut NSView
            } else {
                std::ptr::null_mut()
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_downs
                .set(self.ivars().mouse_downs.get() + 1);
        }
    }
);

impl AncestorHitTrackingContainerView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickTrackingIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClickTrackingIvars]
    struct NullHitTrackingContainerView;

    impl NullHitTrackingContainerView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_downs
                .set(self.ivars().mouse_downs.get() + 1);
        }
    }
);

impl NullHitTrackingContainerView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickTrackingIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClickTrackingIvars]
    struct ClickTrackingContainerView;

    impl ClickTrackingContainerView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let Some(child) = self.subviews().to_vec().into_iter().next() else {
                return std::ptr::null_mut();
            };
            if self.mouse_inRect(point, child.frame()) {
                Retained::<NSView>::as_ptr(&child) as *mut NSView
            } else {
                std::ptr::null_mut()
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_downs
                .set(self.ivars().mouse_downs.get() + 1);
        }
    }
);

impl ClickTrackingContainerView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickTrackingIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class!(
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClickTrackingIvars]
    struct ClickTrackingChildView;

    impl ClickTrackingChildView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            false
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_downs
                .set(self.ivars().mouse_downs.get() + 1);
        }
    }
);

impl ClickTrackingChildView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickTrackingIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

#[derive(Clone)]
struct ReleaseTrackingIvars {
    releases: Rc<Cell<usize>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ReleaseTrackingIvars]
    struct ReleaseTrackingView;

    impl ReleaseTrackingView {
        #[unsafe(method(release))]
        fn release(&self) {
            self.ivars()
                .releases
                .set(self.ivars().releases.get().saturating_add(1));
            unsafe { msg_send![super(self), release] }
        }
    }
);

impl ReleaseTrackingView {
    fn new(mtm: MainThreadMarker, frame: NSRect, releases: Rc<Cell<usize>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ReleaseTrackingIvars { releases });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

struct ProviderOnlySceneProvider;

impl SemanticProvider for ProviderOnlySceneProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![SemanticNode::new(
            "provider-node",
            Role::Container,
            Rect::new(Point::new(12.0, 16.0), Size::new(64.0, 32.0)),
        )
        .with_label("Provider Node")
        .with_property("provider", PropertyValue::Bool(true))]
    }
}

struct CollidingProvider;

impl SemanticProvider for CollidingProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "battlefield",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(320.0, 180.0)),
            ),
            SemanticNode::new(
                "battlefield/card:session-3",
                Role::custom("card"),
                Rect::new(Point::new(24.0, 24.0), Size::new(160.0, 80.0)),
            )
            .with_parent("battlefield", 0),
        ]
    }
}

struct CollidingProviderWithSourceId;

impl SemanticProvider for CollidingProviderWithSourceId {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![SemanticNode::new(
            "battlefield",
            Role::Container,
            Rect::new(Point::new(0.0, 0.0), Size::new(320.0, 180.0)),
        )
        .with_property("source_id", PropertyValue::string("provider-owned"))]
    }
}

struct CollidingProviderWithNativeChild;

impl SemanticProvider for CollidingProviderWithNativeChild {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "provider-duplicate",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(80.0, 48.0)),
            ),
            SemanticNode::new(
                "provider-duplicate",
                Role::Container,
                Rect::new(Point::new(96.0, 0.0), Size::new(80.0, 48.0)),
            )
            .with_property("variant", PropertyValue::string("duplicate")),
            SemanticNode::new(
                "battlefield/native-child",
                Role::Label,
                Rect::new(Point::new(36.0, 48.0), Size::new(60.0, 20.0)),
            )
            .with_parent("battlefield", 0),
        ]
    }
}

struct DuplicateParentProvider;

impl SemanticProvider for DuplicateParentProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "parent",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(120.0, 60.0)),
            ),
            SemanticNode::new(
                "parent",
                Role::Container,
                Rect::new(Point::new(140.0, 0.0), Size::new(120.0, 60.0)),
            ),
            SemanticNode::new(
                "parent/child",
                Role::Label,
                Rect::new(Point::new(12.0, 12.0), Size::new(60.0, 20.0)),
            )
            .with_parent("parent", 0),
        ]
    }
}

struct UniqueProviderWithAmbiguousNativeParent;

impl SemanticProvider for UniqueProviderWithAmbiguousNativeParent {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![
            SemanticNode::new(
                "provider-card",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(120.0, 60.0)),
            ),
            SemanticNode::new(
                "provider-card/label",
                Role::Label,
                Rect::new(Point::new(12.0, 12.0), Size::new(60.0, 20.0)),
            )
            .with_parent("battlefield", 0),
        ]
    }
}

#[derive(Default)]
struct ButtonActionIvars {
    actions: Cell<usize>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ButtonActionIvars]
    struct ButtonActionTarget;

    impl ButtonActionTarget {
        #[unsafe(method(buttonPressed:))]
        fn button_pressed(&self, _sender: Option<&NSButton>) {
            self.ivars().actions.set(self.ivars().actions.get() + 1);
        }
    }
);

impl ButtonActionTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ButtonActionIvars::default());
        unsafe { msg_send![super(this), init] }
    }
}
