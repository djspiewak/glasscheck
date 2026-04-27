#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use block2::RcBlock;
use glasscheck_appkit::{
    AppKitDialogError, AppKitDialogKind, AppKitDialogQuery, AppKitHarness,
    AppKitMenuCaptureOptions, AppKitMenuError, AppKitMenuTarget, AppKitSceneSource,
    AppKitSnapshotContext, HitPointSearch, HitPointStrategy, InstrumentedView,
};
use glasscheck_core::{
    assert_above, assert_vertical_alignment, compare_images, CompareConfig, LayoutTolerance,
    NodeProvenanceKind, NodeRecipe, PixelMatch, PixelProbe, Point, PollOptions, PropertyValue,
    QueryError, Rect, RegionResolveError, RelativeBounds, Role, Selector, SemanticNode,
    SemanticProvider, SemanticSnapshot, Size, SurfaceId, SurfaceQuery, TextMatch, TextRange,
    TransientSurfaceSpec,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn,
    NSApplication, NSApplicationDelegate, NSBackingStoreType, NSBezierPath, NSButton, NSColor,
    NSControlStateValueMixed, NSControlStateValueOn, NSEvent, NSEventMask, NSEventModifierFlags,
    NSEventType, NSFont, NSMenu, NSMenuItem, NSOpenPanel, NSPanel, NSSavePanel, NSTextField,
    NSTextInputClient, NSTextView, NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindow,
    NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};

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
    run("contextual_scene_source_uses_host_geometry_helpers", || {
        contextual_scene_source_uses_host_geometry_helpers(harness)
    });
    run("selected_text_range_reports_scalar_offsets", || {
        selected_text_range_reports_scalar_offsets(harness)
    });
    run(
        "provider_only_text_click_reaches_local_mouse_up_monitor_without_duplicate_mouse_up",
        || {
            provider_only_text_click_reaches_local_mouse_up_monitor_without_duplicate_mouse_up(
                harness,
            )
        },
    );
    run(
        "provider_only_background_click_reaches_local_mouse_up_monitor_without_dropping_mouse_up",
        || {
            provider_only_background_click_reaches_local_mouse_up_monitor_without_dropping_mouse_up(
                harness,
            )
        },
    );
    run("click_text_position_moves_real_nstextview_caret", || {
        click_text_position_moves_real_nstextview_caret(harness)
    });
    run(
        "click_text_position_synthesizes_text_view_mouse_down",
        || click_text_position_synthesizes_text_view_mouse_down(harness),
    );
    run(
        "click_text_position_completes_before_follow_up_input",
        || click_text_position_completes_before_follow_up_input(harness),
    );
    run("session_discovers_window_by_title", || {
        session_discovers_window_by_title(harness)
    });
    run("session_opens_owned_transient_window_and_evicts_it", || {
        session_opens_owned_transient_window_and_evicts_it(harness)
    });
    run("session_waits_for_alert_and_clicks_button", || {
        session_waits_for_alert_and_clicks_button(harness)
    });
    run("three_button_alert_click_uses_appkit_order", || {
        three_button_alert_click_uses_appkit_order(harness)
    });
    run("three_button_alert_cancel_uses_appkit_order", || {
        three_button_alert_cancel_uses_appkit_order(harness)
    });
    run(
        "alert_accessory_text_field_can_be_inspected_and_set",
        || alert_accessory_text_field_can_be_inspected_and_set(harness),
    );
    run("dialog_text_rejects_labels_and_buttons", || {
        dialog_text_rejects_labels_and_buttons(harness)
    });
    run("dialog_query_miss_times_out_cleanly", || {
        dialog_query_miss_times_out_cleanly(harness)
    });
    run("dialog_methods_report_missing_surface", || {
        dialog_methods_report_missing_surface(harness)
    });
    run("alert_dialog_actions_report_missing_selectors", || {
        alert_dialog_actions_report_missing_selectors(harness)
    });
    run("save_panel_path_selection_is_deterministic", || {
        save_panel_path_selection_is_deterministic(harness)
    });
    run("save_panel_rejects_wrong_dialog_kind", || {
        save_panel_rejects_wrong_dialog_kind(harness)
    });
    run("custom_panel_with_button_stays_panel_kind", || {
        custom_panel_with_button_stays_panel_kind(harness)
    });
    run("ordinary_window_is_not_discovered_as_dialog_panel", || {
        ordinary_window_is_not_discovered_as_dialog_panel(harness)
    });
    run("open_panel_reports_missing_paths_and_cancels", || {
        open_panel_reports_missing_paths_and_cancels(harness)
    });
    run("open_panel_live_file_selection_is_explicit", || {
        open_panel_live_file_selection_is_explicit(harness)
    });
    run(
        "transient_surface_hover_updates_active_always_mouse_moved_tracking_state",
        || transient_surface_hover_updates_active_always_mouse_moved_tracking_state(harness),
    );
    run(
        "transient_surface_hover_delivers_single_mouse_moved_callback_per_step",
        || transient_surface_hover_delivers_single_mouse_moved_callback_per_step(harness),
    );
    run(
        "click_targets_attached_child_window_even_when_parent_window_is_present",
        || click_targets_attached_child_window_even_when_parent_window_is_present(harness),
    );
    run(
        "semantic_click_targets_attached_child_window_even_when_parent_window_is_present",
        || semantic_click_targets_attached_child_window_even_when_parent_window_is_present(harness),
    );
    run(
        "provider_only_semantic_click_targets_attached_child_window_even_when_parent_window_is_present",
        || {
            provider_only_semantic_click_targets_attached_child_window_even_when_parent_window_is_present(harness)
        },
    );
    run(
        "provider_only_semantic_click_uses_standalone_path_after_attached_child_detach",
        || provider_only_semantic_click_uses_standalone_path_after_attached_child_detach(harness),
    );
    run(
        "click_text_position_moves_attached_child_nstextview_caret",
        || click_text_position_moves_attached_child_nstextview_caret(harness),
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
    run(
        "semantic_only_snapshot_does_not_resize_small_windows",
        || semantic_only_snapshot_does_not_resize_small_windows(harness),
    );
    run(
        "geometry_only_region_resolution_does_not_resize_small_windows",
        || geometry_only_region_resolution_does_not_resize_small_windows(harness),
    );
    run(
        "visual_recipe_snapshot_rebinds_provider_state_after_capture_resize",
        || visual_recipe_snapshot_rebinds_provider_state_after_capture_resize(harness),
    );
    run(
        "visual_recipe_root_region_resolution_uses_post_capture_root_bounds",
        || visual_recipe_root_region_resolution_uses_post_capture_root_bounds(harness),
    );
    run("registered_native_selectors_are_queryable", || {
        registered_native_selectors_are_queryable(harness)
    });
    run("provider_unique_ids_record_source_id_provenance", || {
        provider_unique_ids_record_source_id_provenance(harness)
    });
    run(
        "native_snapshot_marks_public_properties_with_native_provenance",
        || native_snapshot_marks_public_properties_with_native_provenance(harness),
    );
    run(
        "resolve_hit_point_supports_selector_lookup_and_missing_errors",
        || resolve_hit_point_supports_selector_lookup_and_missing_errors(harness),
    );
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
    run("unique_native_ids_do_not_expose_source_id_aliases", || {
        unique_native_ids_do_not_expose_source_id_aliases(harness)
    });
    run(
        "provider_namespacing_preserves_existing_source_id_property",
        || provider_namespacing_preserves_existing_source_id_property(harness),
    );
    run(
        "capture_region_uses_same_provider_snapshot_as_pixels",
        || capture_region_uses_same_provider_snapshot_as_pixels(harness),
    );
    run(
        "provider_namespacing_marks_unresolved_native_parent_reference_as_ambiguous",
        || provider_namespacing_marks_unresolved_native_parent_reference_as_ambiguous(harness),
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
        "provider_only_semantic_click_invokes_unregistered_control",
        || provider_only_semantic_click_invokes_unregistered_control(harness),
    );
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
    run("semantic_click_falls_back_when_center_is_occluded", || {
        semantic_click_falls_back_when_center_is_occluded(harness)
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
        "context_click_node_opens_native_menu_scene_and_activates_item",
        || context_click_node_opens_native_menu_scene_and_activates_item(harness),
    );
    run(
        "context_click_node_retries_registered_owner_when_descendant_has_no_menu",
        || context_click_node_retries_registered_owner_when_descendant_has_no_menu(harness),
    );
    run("context_click_node_uses_control_primary_fallback", || {
        context_click_node_uses_control_primary_fallback(harness)
    });
    run("session_context_click_node_uses_named_surface", || {
        session_context_click_node_uses_named_surface(harness)
    });
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
        "synthesized_input_keeps_background_test_windows_hidden",
        || synthesized_input_keeps_background_test_windows_hidden(harness),
    );
    run(
        "key_press_targets_attached_window_even_when_another_window_is_key",
        || key_press_targets_attached_window_even_when_another_window_is_key(harness),
    );
    run(
        "making_peer_window_key_does_not_surface_background_test_windows",
        || making_peer_window_key_does_not_surface_background_test_windows(harness),
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
    run("scene_source_recipes_support_external_offsets", || {
        scene_source_recipes_support_external_offsets(harness)
    });
    run(
        "scene_source_recipe_hit_target_uses_window_coordinates",
        || scene_source_recipe_hit_target_uses_window_coordinates(harness),
    );
    run(
        "scene_source_recipe_hit_target_respects_search_strategy",
        || scene_source_recipe_hit_target_respects_search_strategy(harness),
    );
    run(
        "scene_source_recipe_clicks_with_explicit_hit_target_even_when_locator_rect_is_empty",
        || {
            scene_source_recipe_clicks_with_explicit_hit_target_even_when_locator_rect_is_empty(
                harness,
            )
        },
    );
    run(
        "scene_source_recipe_is_omitted_when_anchor_is_missing",
        || scene_source_recipe_is_omitted_when_anchor_is_missing(harness),
    );
    run(
        "background_window_hover_updates_active_in_active_app_tracking_state",
        || background_window_hover_updates_active_in_active_app_tracking_state(harness),
    );
    run(
        "background_window_click_reaches_local_mouse_up_monitor",
        || background_window_click_reaches_local_mouse_up_monitor(harness),
    );
    run("key_press_queued_reaches_local_key_down_monitor", || {
        key_press_queued_reaches_local_key_down_monitor(harness)
    });
    run("key_press_queued_reaches_local_key_up_monitor", || {
        key_press_queued_reaches_local_key_up_monitor(harness)
    });
    run(
        "key_press_queued_delivers_to_first_responder_after_monitor",
        || key_press_queued_delivers_to_first_responder_after_monitor(harness),
    );
    run("key_press_direct_bypasses_local_key_monitor", || {
        key_press_direct_bypasses_local_key_monitor(harness)
    });
    run("key_press_queued_carries_modifier_flags", || {
        key_press_queued_carries_modifier_flags(harness)
    });
    run("main_menu_snapshot_reports_native_hierarchy", || {
        main_menu_snapshot_reports_native_hierarchy(harness)
    });
    run("main_menu_open_targets_cover_success_and_errors", || {
        main_menu_open_targets_cover_success_and_errors(harness)
    });
    run("opened_menu_capture_renders_offscreen_pixels", || {
        opened_menu_capture_renders_offscreen_pixels(harness)
    });
    run("opened_menu_capture_rects_use_image_coordinates", || {
        opened_menu_capture_rects_use_image_coordinates(harness)
    });
    run("opened_menu_highlighted_capture_changes_pixels", || {
        opened_menu_highlighted_capture_changes_pixels(harness)
    });
    run(
        "opened_menu_activation_invokes_action_and_rejects_invalid_items",
        || opened_menu_activation_invokes_action_and_rejects_invalid_items(harness),
    );
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

fn main_menu_snapshot_reports_native_hierarchy(harness: AppKitHarness) {
    let fixture = MenuFixture::install(harness.main_thread_marker());
    let scene = harness
        .menu_bar()
        .snapshot()
        .expect("menu bar snapshot should read NSApplication.mainMenu");

    let bar = scene
        .resolve(&Selector::selector_eq("menu.bar"))
        .expect("menu bar root should be queryable");
    assert_eq!(bar.node.role, Role::MenuBar);
    assert!(bar.node.visible);
    assert_eq!(bar.node.visible_rect, Some(bar.node.rect));
    assert!(bar.node.hit_testable);

    let file = scene
        .resolve(&Selector::selector_eq("menu.title.file"))
        .expect("File menu should be queryable");
    assert_eq!(file.node.role, Role::Menu);
    assert_eq!(file.node.label.as_deref(), Some("File"));
    assert_eq!(
        file.node.state.get("enabled"),
        Some(&PropertyValue::Bool(true))
    );
    assert_eq!(
        file.node.state.get("hidden"),
        Some(&PropertyValue::Bool(false))
    );
    assert!(file.node.visible);
    assert_eq!(file.node.visible_rect, Some(file.node.rect));
    assert!(file.node.hit_testable);

    let leaf_top = scene
        .resolve(&Selector::selector_eq("menu.title.leaf-top"))
        .expect("top-level item without submenu should be queryable");
    assert!(leaf_top.node.visible);
    assert_eq!(leaf_top.node.visible_rect, Some(leaf_top.node.rect));
    assert!(leaf_top.node.hit_testable);

    let disabled_top = scene
        .resolve(&Selector::selector_eq("menu.title.disabled-top"))
        .expect("disabled top-level menu should be queryable");
    assert_eq!(
        disabled_top.node.state.get("enabled"),
        Some(&PropertyValue::Bool(false))
    );
    assert!(disabled_top.node.visible);
    assert_eq!(disabled_top.node.visible_rect, Some(disabled_top.node.rect));
    assert!(!disabled_top.node.hit_testable);

    let hidden_top = scene
        .resolve(&Selector::selector_eq("menu.title.hidden-top"))
        .expect("hidden top-level menu should be queryable");
    assert_eq!(
        hidden_top.node.state.get("hidden"),
        Some(&PropertyValue::Bool(true))
    );
    assert!(!hidden_top.node.visible);
    assert_eq!(hidden_top.node.visible_rect, None);
    assert!(!hidden_top.node.hit_testable);

    let new_item = scene
        .resolve(&Selector::selector_eq("menu.item.path.file.new"))
        .expect("New menu item should be queryable");
    assert_eq!(new_item.node.role, Role::MenuItem);
    assert_eq!(
        new_item.node.properties.get("key_equivalent"),
        Some(&PropertyValue::string("n"))
    );
    assert_eq!(
        new_item.node.properties.get("key_modifiers"),
        Some(&PropertyValue::string("cmd"))
    );
    assert!(!new_item.node.visible);
    assert_eq!(new_item.node.visible_rect, None);
    assert!(!new_item.node.hit_testable);

    let disabled = scene
        .resolve(&Selector::selector_eq("menu.item.path.file.open"))
        .expect("disabled menu item should be queryable");
    assert_eq!(
        disabled.node.state.get("enabled"),
        Some(&PropertyValue::Bool(false))
    );
    assert!(!disabled.node.hit_testable);

    let hidden = scene
        .resolve(&Selector::selector_eq("menu.item.path.file.secret"))
        .expect("hidden menu item should be queryable");
    assert_eq!(
        hidden.node.state.get("hidden"),
        Some(&PropertyValue::Bool(true))
    );
    assert!(!hidden.node.visible);

    let checked = scene
        .resolve(&Selector::selector_eq("menu.item.path.file.autosave"))
        .expect("checked menu item should be queryable");
    assert_eq!(
        checked.node.state.get("checked"),
        Some(&PropertyValue::Bool(true))
    );

    let mixed = scene
        .resolve(&Selector::selector_eq("menu.item.path.file.mixed-state"))
        .expect("mixed menu item should be queryable");
    assert_eq!(
        mixed.node.state.get("mixed"),
        Some(&PropertyValue::Bool(true))
    );

    let submenu = scene
        .resolve(&Selector::selector_eq("menu.item.path.file.export"))
        .expect("submenu item should be queryable");
    assert_eq!(
        submenu.node.properties.get("has_submenu"),
        Some(&PropertyValue::Bool(true))
    );
    assert!(!submenu.node.visible);
    assert_eq!(submenu.node.visible_rect, None);
    assert!(!submenu.node.hit_testable);

    let export_container = scene
        .resolve(&Selector::selector_eq("menu.title.export"))
        .expect("submenu container should remain semantically queryable");
    assert_eq!(export_container.node.role, Role::Menu);
    assert!(!export_container.node.visible);
    assert_eq!(export_container.node.visible_rect, None);
    assert!(!export_container.node.hit_testable);

    assert_eq!(fixture.activations.get(), 0);
}

fn main_menu_open_targets_cover_success_and_errors(harness: AppKitHarness) {
    let fixture = MenuFixture::install(harness.main_thread_marker());
    let menu_bar = harness.menu_bar();

    assert!(menu_bar
        .open(AppKitMenuTarget::Title("File".into()))
        .expect("File menu should open by title")
        .snapshot()
        .expect("opened File menu should snapshot")
        .exists(&Selector::selector_eq("menu.item.path.file.new")));
    assert!(menu_bar
        .open(AppKitMenuTarget::Index(0))
        .expect("File menu should open by index")
        .snapshot()
        .expect("opened File menu should snapshot")
        .exists(&Selector::selector_eq("menu.item.path.file.new")));
    assert!(menu_bar
        .open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.title.file"
        )))
        .expect("File menu should open by selector")
        .snapshot()
        .expect("opened File menu should snapshot")
        .exists(&Selector::selector_eq("menu.item.path.file.new")));
    assert!(menu_bar
        .open(AppKitMenuTarget::Title("Edit".into()))
        .expect("Edit menu should open by title")
        .snapshot()
        .expect("opened Edit menu should snapshot")
        .exists(&Selector::selector_eq("menu.title.edit")));
    assert!(menu_bar
        .open(AppKitMenuTarget::Index(1))
        .expect("Edit menu should open by index")
        .snapshot()
        .expect("opened Edit menu should snapshot")
        .exists(&Selector::selector_eq("menu.title.edit")));
    assert!(menu_bar
        .open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.title.edit"
        )))
        .expect("Edit menu should open by selector")
        .snapshot()
        .expect("opened Edit menu should snapshot")
        .exists(&Selector::selector_eq("menu.title.edit")));

    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Title("Missing".into())),
        Err(AppKitMenuError::MenuNotFound(AppKitMenuTarget::Title(title))) if title == "Missing"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Index(99)),
        Err(AppKitMenuError::MenuNotFound(AppKitMenuTarget::Index(99)))
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.item.path.file.new"
        ))),
        Err(AppKitMenuError::InvalidMenuTarget(
            AppKitMenuTarget::Selector(_)
        ))
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.title.missing"
        ))),
        Err(AppKitMenuError::MenuNotFound(AppKitMenuTarget::Selector(_)))
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Selector(Selector::role_eq(Role::Menu))),
        Err(AppKitMenuError::AmbiguousMenuTarget { count, .. }) if count > 1
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Title("Leaf Top".into())),
        Err(AppKitMenuError::MissingSubmenu(title)) if title == "Leaf Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Index(2)),
        Err(AppKitMenuError::MissingSubmenu(title)) if title == "Leaf Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.title.leaf-top"
        ))),
        Err(AppKitMenuError::MissingSubmenu(title)) if title == "Leaf Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Title("Disabled Top".into())),
        Err(AppKitMenuError::ItemDisabled(title)) if title == "Disabled Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Index(3)),
        Err(AppKitMenuError::ItemDisabled(title)) if title == "Disabled Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.title.disabled-top"
        ))),
        Err(AppKitMenuError::ItemDisabled(title)) if title == "Disabled Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Title("Hidden Top".into())),
        Err(AppKitMenuError::ItemHidden(title)) if title == "Hidden Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Index(4)),
        Err(AppKitMenuError::ItemHidden(title)) if title == "Hidden Top"
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Selector(Selector::selector_eq(
            "menu.title.hidden-top"
        ))),
        Err(AppKitMenuError::ItemHidden(title)) if title == "Hidden Top"
    ));

    drop(fixture);
    let _cleared = ClearedMenuFixture::install(harness.main_thread_marker());
    assert!(matches!(
        menu_bar.snapshot(),
        Err(AppKitMenuError::MissingMainMenu)
    ));
    assert!(matches!(
        menu_bar.open(AppKitMenuTarget::Title("File".into())),
        Err(AppKitMenuError::MissingMainMenu)
    ));
}

fn opened_menu_capture_renders_offscreen_pixels(harness: AppKitHarness) {
    let _fixture = MenuFixture::install(harness.main_thread_marker());
    let opened = harness
        .menu_bar()
        .open(AppKitMenuTarget::Title("File".into()))
        .expect("File menu should open");

    let capture = opened
        .capture(&AppKitMenuCaptureOptions::default())
        .expect("default capture should render offscreen");

    assert!(capture.image.width >= 160);
    assert!(capture.image.height >= 80);
    assert!(
        capture
            .image
            .data
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && (pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0)),
        "offscreen capture should contain visible menu pixels"
    );
    assert!(capture
        .scene
        .exists(&Selector::selector_eq("menu.item.path.file.new")));
    let pdf = capture
        .scene
        .resolve(&Selector::selector_eq("menu.item.path.file.export.pdf"))
        .expect("submenu descendant should remain semantically queryable");
    assert!(!pdf.node.visible);
    assert_eq!(pdf.node.visible_rect, None);
    assert!(!pdf.node.hit_testable);
    let export_submenu = capture
        .scene
        .resolve(&Selector::selector_eq("menu.title.export"))
        .expect("submenu container should remain semantically queryable");
    assert_eq!(export_submenu.node.role, Role::Menu);
    assert!(!export_submenu.node.visible);
    assert_eq!(export_submenu.node.visible_rect, None);
    assert!(!export_submenu.node.hit_testable);
}

fn opened_menu_capture_rects_use_image_coordinates(harness: AppKitHarness) {
    let _fixture = MenuFixture::install(harness.main_thread_marker());
    let opened = harness
        .menu_bar()
        .open(AppKitMenuTarget::Title("File".into()))
        .expect("File menu should open");

    let capture = opened
        .capture(&AppKitMenuCaptureOptions::default())
        .expect("default capture should render offscreen");
    let new_item = capture
        .scene
        .resolve(&Selector::selector_eq("menu.item.path.file.new"))
        .expect("New item should remain queryable");
    let open_item = capture
        .scene
        .resolve(&Selector::selector_eq("menu.item.path.file.open"))
        .expect("Open item should remain queryable");

    assert!(
        new_item.node.rect.origin.y < open_item.node.rect.origin.y,
        "image-space menu rows should increase downward: New={:?}, Open={:?}",
        new_item.node.rect,
        open_item.node.rect
    );
    assert_eq!(new_item.node.visible_rect, Some(new_item.node.rect));
    assert_eq!(open_item.node.visible_rect, Some(open_item.node.rect));
    assert!(new_item.node.visible);
    assert!(new_item.node.hit_testable);
    assert!(new_item.node.rect.origin.y >= 0.0);
    assert!(open_item.node.rect.origin.y >= 0.0);
    assert!(
        new_item.node.rect.origin.y + new_item.node.rect.size.height <= capture.image.height as f64
    );
    assert!(
        open_item.node.rect.origin.y + open_item.node.rect.size.height
            <= capture.image.height as f64
    );
}

fn opened_menu_highlighted_capture_changes_pixels(harness: AppKitHarness) {
    let _fixture = MenuFixture::install(harness.main_thread_marker());
    let opened = harness
        .menu_bar()
        .open(AppKitMenuTarget::Title("File".into()))
        .expect("File menu should open");

    let plain = opened
        .capture(&AppKitMenuCaptureOptions::default())
        .expect("plain capture should render offscreen");
    let highlighted = opened
        .capture(&AppKitMenuCaptureOptions {
            highlighted_item: Some(Selector::selector_eq("menu.item.path.file.new")),
            allow_visible_fallback: false,
        })
        .expect("highlighted capture should render offscreen");

    assert_ne!(plain.image, highlighted.image);
    let node = highlighted
        .scene
        .resolve(&Selector::selector_eq("menu.item.path.file.new"))
        .expect("highlighted item should remain queryable");
    assert_eq!(
        node.node.state.get("highlighted"),
        Some(&PropertyValue::Bool(true))
    );
    let highlighted_rect = node
        .node
        .visible_rect
        .expect("highlighted item should have an image-space visible rect");
    let plain_average = plain.image.average_rgba(highlighted_rect);
    let highlighted_average = highlighted.image.average_rgba(highlighted_rect);
    let average_delta = plain_average
        .iter()
        .zip(highlighted_average.iter())
        .map(|(plain, highlighted)| (plain - highlighted).abs())
        .sum::<f64>();
    assert!(
        average_delta > 10.0,
        "highlighted visible rect should sample changed pixels: plain={plain_average:?}, highlighted={highlighted_average:?}"
    );

    let submenu_child = Selector::selector_eq("menu.item.path.file.export.pdf");
    assert!(matches!(
        opened.capture(&AppKitMenuCaptureOptions {
            highlighted_item: Some(submenu_child.clone()),
            allow_visible_fallback: false,
        }),
        Err(AppKitMenuError::HighlightTargetNotRendered(selector)) if selector == submenu_child
    ));

    let hidden_item = Selector::selector_eq("menu.item.path.file.secret");
    assert!(matches!(
        opened.capture(&AppKitMenuCaptureOptions {
            highlighted_item: Some(hidden_item.clone()),
            allow_visible_fallback: false,
        }),
        Err(AppKitMenuError::HighlightTargetNotRendered(selector)) if selector == hidden_item
    ));
}

fn opened_menu_activation_invokes_action_and_rejects_invalid_items(harness: AppKitHarness) {
    let fixture = MenuFixture::install(harness.main_thread_marker());
    let delegate_fixture = MenuDelegateFixture::install(harness.main_thread_marker());
    harness.settle(2);

    let opened = harness
        .menu_bar()
        .open(AppKitMenuTarget::Title("File".into()))
        .expect("File menu should open");

    opened
        .activate(&Selector::selector_eq("menu.item.path.file.new"))
        .expect("enabled visible item should activate");
    assert_eq!(fixture.activations.get(), 1);

    opened
        .activate(&Selector::selector_eq(
            "menu.item.path.file.responder-action",
        ))
        .expect("targetless responder-chain item should activate");
    assert_eq!(delegate_fixture.activations.get(), 1);
    assert_eq!(fixture.activations.get(), 1);

    assert!(matches!(
        opened.activate(&Selector::selector_eq("menu.item.path.file.open")),
        Err(AppKitMenuError::ItemDisabled(title)) if title == "Open..."
    ));
    assert!(matches!(
        opened.activate(&Selector::selector_eq("menu.item.path.file.secret")),
        Err(AppKitMenuError::ItemHidden(title)) if title == "Secret"
    ));
    assert!(matches!(
        opened.activate(&Selector::selector_eq("menu.item.path.file.separator-4")),
        Err(AppKitMenuError::SeparatorActivation(_))
    ));
    assert!(matches!(
        opened.activate(&Selector::selector_eq("menu.item.path.file.export")),
        Err(AppKitMenuError::NonActionableItem(title)) if title == "Export"
    ));
    let submenu_child = Selector::selector_eq("menu.item.path.file.export.pdf");
    assert!(matches!(
        opened.activate(&submenu_child),
        Err(AppKitMenuError::ActivationTargetNotRendered(selector)) if selector == submenu_child
    ));
    assert!(matches!(
        opened.activate(&Selector::label(glasscheck_core::TextMatch::exact(
            "Duplicate"
        ))),
        Err(AppKitMenuError::AmbiguousItemTarget { count: 2, .. })
    ));
    assert!(matches!(
        opened.activate(&Selector::selector_eq(
            "menu.item.path.file.missing-target"
        )),
        Err(AppKitMenuError::ActionTargetNotFound(title)) if title == "Missing Target"
    ));
    assert!(matches!(
        opened.activate(&Selector::selector_eq("menu.item.path.file.missing")),
        Err(AppKitMenuError::ItemNotFound(_))
    ));
    assert_eq!(fixture.activations.get(), 1);
}

fn temp_test_dir(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("glasscheck-{label}-{stamp}"));
    fs::create_dir_all(&path).expect("temporary test directory should be creatable");
    path
}

fn hide_native_dialog_window(window: &NSWindow) {
    window.setExcludedFromWindowsMenu(true);
    window.setCanHide(false);
    window.setHidesOnDeactivate(true);
    window.setAlphaValue(0.0);
}

fn prepare_alert_for_background_test(alert: &NSAlert) {
    let window = alert.window();
    hide_native_dialog_window(&window);
}

fn native_file_panel_contracts_enabled() -> bool {
    std::env::var_os("GLASSCHECK_RUN_NATIVE_FILE_PANEL_TESTS").is_some()
}

fn short_poll_options() -> PollOptions {
    PollOptions {
        timeout: Duration::from_millis(80),
        interval: Duration::from_millis(8),
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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &label,
        InstrumentedView {
            id: Some("sidebar-label".into()),
            role: Some(Role::Label),
            label: Some("Draft".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let attached = harness.attach_window(host.window());
    attached.register_view(
        &row,
        InstrumentedView {
            id: Some("sidebar-row".into()),
            role: Some(Role::ListItem),
            label: Some("Draft".into()),
            selectors: Vec::new(),
        },
    );
    attached.register_view(
        &label,
        InstrumentedView {
            id: Some("sidebar-label".into()),
            role: Some(Role::Label),
            label: Some("Draft".into()),
            selectors: Vec::new(),
        },
    );

    let scene = attached.snapshot_scene();
    let row = scene.find(&Selector::id_eq("sidebar-row")).unwrap();
    let label = scene.find(&Selector::id_eq("sidebar-label")).unwrap();
    assert_eq!(scene.node(row).unwrap().label.as_deref(), Some("Draft"));
    assert_eq!(scene.node(label).unwrap().role, Role::Label);
}

fn attached_window_reports_missing_node(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    host.set_content_view(&root);
    let attached = harness.attach_window(host.window());
    let error = attached
        .snapshot_scene()
        .find(&Selector::id_eq("missing"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFound(_)));
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
            selectors: Vec::new(),
        },
    );
    host.set_semantic_provider(Box::new(ProviderOnlySceneProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider = scene
        .node(
            scene
                .find(&Selector::property_eq(
                    "provider",
                    PropertyValue::Bool(true),
                ))
                .expect("scene should include provider nodes"),
        )
        .unwrap();
    let provider_selector = scene
        .node(
            scene
                .find(&Selector::selector_eq("provider.node"))
                .expect("scene should preserve provider selectors"),
        )
        .unwrap();
    let native = scene
        .node(
            scene
                .find(&Selector::id_eq("native-child"))
                .expect("scene should include registered native nodes"),
        )
        .unwrap();

    assert_eq!(provider.id.as_str(), "provider-node");
    assert_eq!(provider_selector.id.as_str(), "provider-node");
    assert_eq!(native.id.as_str(), "native-child");
    assert!(scene
        .find(&Selector::parent(Selector::id_eq("native-child")))
        .is_err());
}

fn semantic_only_snapshot_does_not_resize_small_windows(harness: AppKitHarness) {
    let host = harness.create_window(24.0, 24.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(24.0, 24.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(ProviderOnlySceneProvider));
    harness.settle(2);

    let before = host.window().frame().size;
    let scene = host.snapshot_scene();
    let after_snapshot = host.window().frame().size;
    let after_query = host.window().frame().size;

    assert!(scene.find(&Selector::id_eq("provider-node")).is_ok());
    assert_eq!(before.width, after_snapshot.width);
    assert_eq!(before.height, after_snapshot.height);
    assert_eq!(before.width, after_query.width);
    assert_eq!(before.height, after_query.height);
    assert!(scene.find(&Selector::id_eq("provider-node")).is_ok());
}

fn geometry_only_region_resolution_does_not_resize_small_windows(harness: AppKitHarness) {
    let host = harness.create_window(24.0, 24.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(24.0, 24.0));
    host.set_content_view(&root);
    host.set_scene_source(Box::new(OffsetRecipeProvider));
    harness.settle(2);

    let before = host.window().frame().size;
    let resolved = host
        .resolve_region(&glasscheck_core::RegionSpec::rect(Rect::new(
            Point::new(2.0, 3.0),
            Size::new(5.0, 6.0),
        )))
        .expect("geometry-only region should resolve without capture side effects");
    let after = host.window().frame().size;

    assert_eq!(
        resolved,
        Rect::new(Point::new(2.0, 3.0), Size::new(5.0, 6.0))
    );
    assert_eq!(before.width, after.width);
    assert_eq!(before.height, after.height);
}

fn visual_recipe_snapshot_rebinds_provider_state_after_capture_resize(harness: AppKitHarness) {
    let host = harness.create_window(24.0, 24.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(24.0, 24.0));
    let marker = SolidFillView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(12.0, 12.0)),
    );
    root.addSubview(&marker);
    host.set_content_view(&root);
    host.set_scene_source(Box::new(ResizeAwareVisualRecipeProvider {
        root: unsafe {
            Retained::retain(&*root as *const NSView as *mut NSView)
                .expect("root view should retain for provider")
        },
    }));
    harness.settle(2);

    let before = host.window().frame().size;
    let scene = host.snapshot_scene();
    let after = host.window().frame().size;
    let handle = scene
        .find(&Selector::selector_eq("provider.visual"))
        .expect("visual recipe should resolve after capture-time resize");
    let node = scene
        .node(handle)
        .expect("resolved recipe should be present");

    assert!(after.width > before.width);
    assert!(node.rect.origin.x > before.width);
    assert!(scene.recipe_errors().is_empty());
}

fn visual_recipe_root_region_resolution_uses_post_capture_root_bounds(harness: AppKitHarness) {
    let host = harness.create_window(24.0, 24.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(24.0, 24.0));
    let marker = SolidFillView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(12.0, 12.0)),
    );
    root.addSubview(&marker);
    host.set_content_view(&root);
    host.set_scene_source(Box::new(ResizeAwareVisualRecipeProvider {
        root: unsafe {
            Retained::retain(&*root as *const NSView as *mut NSView)
                .expect("root view should retain for provider")
        },
    }));
    harness.settle(2);

    let before = host.window().frame().size;
    let resolved = host
        .resolve_region(&glasscheck_core::RegionSpec::root())
        .expect("root region should resolve after capture-time resize");
    let after = host.window().frame().size;
    let root_bounds = root.bounds();

    assert!(after.width > before.width);
    assert_eq!(resolved.size.width, root_bounds.size.width);
    assert_eq!(resolved.size.height, root_bounds.size.height);
    assert!(resolved.size.width > before.width);
}

fn registered_native_selectors_are_queryable(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    let child = make_view(harness.main_thread_marker(), NSSize::new(40.0, 20.0));
    child.setFrameOrigin(NSPoint::new(20.0, 10.0));
    root.addSubview(&child);
    host.set_content_view(&root);
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("native-child".into()),
            role: Some(Role::ListItem),
            label: Some("Loose".into()),
            selectors: vec!["sidebar.row".into(), "draft".into()],
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let exact = scene
        .find(&Selector::selector_eq("sidebar.row"))
        .expect("exact selector should match the registered node");
    let fuzzy = scene
        .find(&Selector::any_selector(
            glasscheck_core::TextMatch::contains("draft"),
        ))
        .expect("fuzzy selector match should find the registered node");

    assert_eq!(scene.node(exact).unwrap().id, "native-child");
    assert_eq!(scene.node(fuzzy).unwrap().id, "native-child");
    assert!(scene
        .find(&Selector::selector_eq("missing.selector"))
        .is_err());
}

fn native_snapshot_marks_public_properties_with_native_provenance(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    let child = make_view(harness.main_thread_marker(), NSSize::new(40.0, 20.0));
    root.addSubview(&child);
    host.set_content_view(&root);
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("native-child".into()),
            role: Some(Role::ListItem),
            label: Some("Loose".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(scene.find(&Selector::id_eq("native-child")).unwrap())
        .unwrap();

    assert_eq!(node.provenance, NodeProvenanceKind::Native);
    assert_eq!(
        node.property_provenance.get("glasscheck:paint_order_path"),
        Some(&NodeProvenanceKind::Native)
    );
}

fn resolve_hit_point_supports_selector_lookup_and_missing_errors(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    let child = make_view(harness.main_thread_marker(), NSSize::new(40.0, 20.0));
    child.setFrameOrigin(NSPoint::new(20.0, 10.0));
    root.addSubview(&child);
    host.set_content_view(&root);
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("native-child".into()),
            role: Some(Role::ListItem),
            label: Some("Loose".into()),
            selectors: vec!["sidebar.row".into()],
        },
    );
    harness.settle(2);

    let hit_point = host
        .resolve_hit_point(
            &Selector::selector_eq("sidebar.row"),
            &HitPointSearch {
                strategy: HitPointStrategy::VisibleCenterFirst,
                sample_count: 1,
            },
        )
        .expect("selector-based hit point resolution should succeed");
    assert_eq!(hit_point, Point::new(40.0, 20.0));

    let error = host
        .resolve_hit_point(
            &Selector::selector_eq("missing.selector"),
            &HitPointSearch {
                strategy: HitPointStrategy::Grid,
                sample_count: 9,
            },
        )
        .unwrap_err();
    assert!(matches!(error, RegionResolveError::NotFound(_)));
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);
    assert!(host
        .snapshot_scene()
        .find(&Selector::id_eq("detached-child"))
        .is_ok());

    child.removeFromSuperview();
    harness.settle(2);

    let error = host
        .snapshot_scene()
        .find(&Selector::id_eq("detached-child"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFound(_)));
}

fn provider_unique_ids_record_source_id_provenance(harness: AppKitHarness) {
    let host = harness.create_window(160.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(160.0, 120.0));
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(ProviderOnlySceneProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let handle = scene.find(&Selector::id_eq("provider-node")).unwrap();
    let node = scene.node(handle).unwrap();

    assert_eq!(
        node.properties.get("glasscheck:source_id"),
        Some(&PropertyValue::string("provider-node"))
    );
    assert_eq!(
        node.property_provenance.get("glasscheck:source_id"),
        Some(&NodeProvenanceKind::Declared)
    );
}

fn attached_window_registry_drops_nodes_after_content_swap(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let initial_root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let initial_child = make_view(harness.main_thread_marker(), NSSize::new(80.0, 24.0));
    initial_root.addSubview(&initial_child);
    host.set_content_view(&initial_root);
    harness.settle(2);

    let attached = harness.attach_window(host.window());
    attached.register_view(
        &initial_child,
        InstrumentedView {
            id: Some("stale-child".into()),
            role: Some(Role::Container),
            label: Some("Stale".into()),
            selectors: Vec::new(),
        },
    );
    assert!(attached
        .snapshot_scene()
        .find(&Selector::id_eq("stale-child"))
        .is_ok());

    let replacement_root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    host.set_content_view(&replacement_root);
    harness.settle(2);

    let error = attached
        .snapshot_scene()
        .find(&Selector::id_eq("stale-child"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFound(_)));
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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &clipped_child,
        InstrumentedView {
            id: Some("clipped-child".into()),
            role: Some(Role::Container),
            label: Some("Clipped".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let hidden = scene
        .node(scene.find(&Selector::id_eq("hidden-child")).unwrap())
        .unwrap();
    let clipped = scene
        .node(scene.find(&Selector::id_eq("clipped-child")).unwrap())
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
    let attached = harness.attach_window(host.window());

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
    let attached = harness.attach_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let scene = attached.snapshot_scene();
    let node = scene
        .find(&Selector::id_eq("provider-node"))
        .expect("provider node should be queryable without a content view");

    assert_eq!(
        scene.node(node).unwrap().label.as_deref(),
        Some("Provider Node")
    );
}

fn provider_only_region_capture_fails_cleanly(harness: AppKitHarness) {
    let host = harness.create_window(240.0, 160.0);
    host.window().setContentView(None);
    let attached = harness.attach_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let error = attached
        .capture_region(&glasscheck_core::RegionSpec::node(Selector::id_eq(
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
    let host = harness.attach_root_view(&root, None);
    host.register_view(
        &root,
        InstrumentedView {
            id: Some("root".into()),
            role: Some(Role::Button),
            label: Some("Root".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    assert!(host.window().contentView().is_some());
    let _ = host.input();
    let error = host.click_node(&Selector::id_eq("root")).unwrap_err();
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

    let attached = harness.attach_root_view(&pinned, Some(host.window()));
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let attached = harness.attach_root_view(&pinned, Some(host.window()));
    attached.register_view(
        &pinned,
        InstrumentedView {
            id: Some("pinned-click".into()),
            role: Some(Role::Button),
            label: Some("Pinned".into()),
            selectors: vec!["pinned.click".into()],
        },
    );

    let hit_point = attached
        .resolve_hit_point(
            &Selector::selector_eq("pinned.click"),
            &HitPointSearch::default(),
        )
        .unwrap();
    assert_eq!(hit_point, Point::new(100.0, 60.0));

    attached
        .click_node(&Selector::id_eq("pinned-click"))
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
        .text_range_rect(&view, TextRange::new(0, 10))
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

fn contextual_scene_source_uses_host_geometry_helpers(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(180.0, 80.0),
        "Context provider",
    );
    view.setFrameOrigin(NSPoint::new(30.0, 36.0));
    root.addSubview(&view);
    host.set_content_view(&root);
    host.set_contextual_scene_source(Box::new(ContextualTextSceneSource {
        view: unsafe {
            Retained::retain(&*view as *const NSTextView as *mut NSTextView)
                .expect("text view should retain")
        },
    }));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let text = scene
        .resolve(&Selector::selector_eq("context.text"))
        .expect("context-backed text node should resolve");
    let caret = scene
        .resolve(&Selector::selector_eq("context.caret"))
        .expect("context-backed caret node should resolve");

    assert!(text.bounds.size.width > 0.0);
    assert!(caret.bounds.size.height > 0.0);
}

fn selected_text_range_reports_scalar_offsets(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = make_text_view(
        harness.main_thread_marker(),
        NSSize::new(180.0, 80.0),
        "Caret movement",
    );
    view.setSelectable(true);
    view.setFrameOrigin(NSPoint::new(30.0, 36.0));
    root.addSubview(&view);
    host.set_content_view(&root);
    harness.settle(2);

    host.input().set_selection(&view, TextRange::new(5, 0));
    harness.settle(2);
    assert_eq!(host.selected_text_range(&view), TextRange::new(5, 0));
    assert!(host.insertion_caret_rect(&view, 1).is_some());
}

fn click_text_position_moves_real_nstextview_caret(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(180.0, 80.0), "ab");
    view.setEditable(true);
    view.setSelectable(true);
    view.setFrameOrigin(NSPoint::new(30.0, 36.0));
    root.addSubview(&view);
    host.set_content_view(&root);
    host.window().makeFirstResponder(Some(&view));
    host.window().makeKeyWindow();
    harness.settle(2);

    host.input().set_selection(&view, TextRange::new(2, 0));
    harness.settle(2);

    host.click_text_position(&view, 1).unwrap();
    assert_eq!(host.selected_text_range(&view), TextRange::new(1, 0));

    host.window().orderOut(None);
    host.window().close();
}

fn click_text_position_synthesizes_text_view_mouse_down(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = MouseDownTrackingTextView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 80.0)),
    );
    view.setEditable(true);
    view.setSelectable(true);
    view.setDrawsBackground(false);
    view.setString(&NSString::from_str("ab"));
    view.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    view.setFrameOrigin(NSPoint::new(30.0, 36.0));
    root.addSubview(&view);
    host.set_content_view(&root);
    host.window().makeFirstResponder(Some(&view));
    host.window().makeKeyWindow();
    harness.settle(2);

    host.click_text_position(&view, 1).unwrap();
    assert_eq!(view.ivars().mouse_downs.get(), 1);

    host.window().orderOut(None);
    host.window().close();
}

fn click_text_position_completes_before_follow_up_input(harness: AppKitHarness) {
    let host = harness.create_window(320.0, 200.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(320.0, 200.0));
    let view = make_text_view(harness.main_thread_marker(), NSSize::new(180.0, 80.0), "ab");
    view.setEditable(true);
    view.setSelectable(true);
    view.setFrameOrigin(NSPoint::new(30.0, 36.0));
    root.addSubview(&view);
    host.set_content_view(&root);
    host.window().makeFirstResponder(Some(&view));
    host.window().makeKeyWindow();
    harness.settle(2);

    host.input().set_selection(&view, TextRange::new(2, 0));
    harness.settle(2);

    host.click_text_position(&view, 1).unwrap();
    host.input().type_text_direct(&view, "X");

    assert_eq!(view.string().to_string(), "aXb");
    assert_eq!(host.selected_text_range(&view), TextRange::new(2, 0));

    host.window().orderOut(None);
    host.window().close();
}

fn click_text_position_moves_attached_child_nstextview_caret(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let parent = harness.create_window(240.0, 140.0);
    let parent_root = make_view(mtm, NSSize::new(240.0, 140.0));
    parent.set_content_view(&parent_root);

    let child = harness.create_window(220.0, 120.0);
    let child_root = make_view(mtm, NSSize::new(220.0, 120.0));
    let view = MouseDownTrackingTextView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 80.0)),
    );
    view.setEditable(true);
    view.setSelectable(true);
    view.setDrawsBackground(false);
    view.setString(&NSString::from_str("ab"));
    view.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    view.setFrameOrigin(NSPoint::new(20.0, 20.0));
    child_root.addSubview(&view);
    child.set_content_view(&child_root);

    unsafe {
        parent
            .window()
            .addChildWindow_ordered(child.window(), NSWindowOrderingMode::Above);
    }
    child.window().orderFrontRegardless();
    harness.settle(2);

    let attached = harness.attach_window(child.window());
    attached.input().set_selection(&view, TextRange::new(2, 0));
    harness.settle(2);

    attached.click_text_position(&view, 1).unwrap();

    assert_eq!(attached.selected_text_range(&view), TextRange::new(1, 0));
    assert!(!child.window().isKeyWindow());

    parent.window().removeChildWindow(child.window());
    child.window().orderOut(None);
    child.window().close();
    parent.window().orderOut(None);
    parent.window().close();
}

fn session_discovers_window_by_title(harness: AppKitHarness) {
    let main = harness.create_window(240.0, 120.0);
    let chooser = harness.create_window(180.0, 100.0);
    main.set_title("Main Editor");
    chooser.set_title("Table Picker");
    harness.settle(2);

    let session = harness.session();
    session.attach_host("main", main);
    assert!(
        session.discover_window("picker", &SurfaceQuery::title_contains("Picker")),
        "session should discover title-matched windows"
    );

    session
        .snapshot_scene(&SurfaceId::new("picker"))
        .expect("discovered surface should be attached and snapshotable");
}

fn session_opens_owned_transient_window_and_evicts_it(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let inserted = Rc::new(Cell::new(false));
    let host = harness.create_window(320.0, 220.0);
    let picker = harness.create_window(180.0, 64.0);
    let root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 220.0)),
    );
    let opener_target = ChildWindowOpenTarget::new(mtm, picker.window());
    let insert_target = ChildWindowInsertTarget::new(mtm, inserted.clone(), picker.window());
    let insert_button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Insert Table"),
            Some(&*insert_target),
            Some(sel!(buttonPressed:)),
            mtm,
        )
    };
    insert_button.setFrame(NSRect::new(
        NSPoint::new(16.0, 16.0),
        NSSize::new(140.0, 32.0),
    ));
    let picker_root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 64.0)),
    );
    picker_root.addSubview(&insert_button);
    picker.set_content_view(&picker_root);

    let opener = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Open Picker"),
            Some(&*opener_target),
            Some(sel!(openChildWindow:)),
            mtm,
        )
    };
    opener.setFrame(NSRect::new(
        NSPoint::new(28.0, 140.0),
        NSSize::new(120.0, 32.0),
    ));
    root.addSubview(&opener);
    host.set_content_view(&root);
    host.register_node(
        &opener,
        InstrumentedView {
            id: Some("open-picker".into()),
            role: Some(Role::Button),
            label: Some("Open Picker".into()),
            ..Default::default()
        },
    );
    host.set_scene_source(Box::new(InsertedTableScene {
        inserted: inserted.clone(),
    }));
    harness.settle(2);

    let session = harness.session();
    session.attach_host("main", host);
    session
        .open_transient_with_click(
            "picker",
            &TransientSurfaceSpec::new("main", Selector::id_eq("open-picker")),
            PollOptions::default(),
        )
        .expect("session should attach the newly opened child window");

    session.with_surface(&SurfaceId::new("picker"), |picker| {
        picker.register_node(
            &insert_button,
            InstrumentedView {
                id: Some("insert-table".into()),
                role: Some(Role::Button),
                label: Some("Insert Table".into()),
                ..Default::default()
            },
        );
    });
    harness.settle(2);

    session
        .click_node(&SurfaceId::new("picker"), &Selector::id_eq("insert-table"))
        .expect("picker surface should be attached")
        .expect("insert action should succeed");
    session
        .wait_for_surface_closed(&SurfaceId::new("picker"), PollOptions::default())
        .expect("picker surface should be evicted after the transient closes");
    assert!(!session.surface_is_open(&SurfaceId::new("picker")));

    let main_scene = session
        .snapshot_scene(&SurfaceId::new("main"))
        .expect("main surface should remain attached");
    assert!(
        main_scene.find(&Selector::id_eq("inserted-table")).is_ok(),
        "main scene should reflect the table insertion after the popover action"
    );
}

fn session_waits_for_alert_and_clicks_button(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Discard Changes?"));
    alert.setInformativeText(&NSString::from_str("Unsaved edits will be lost."));
    alert.addButtonWithTitle(&NSString::from_str("Discard"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);

    session
        .wait_for_dialog(
            "alert",
            &AppKitDialogQuery::alert().title_contains("Discard"),
            PollOptions::default(),
        )
        .expect("alert sheet should be discovered");
    session.with_surface(&SurfaceId::new("alert"), |host| {
        assert!(
            host.window().alphaValue() == 0.0,
            "discovered alert sheet should stay transparent under the background test policy"
        );
    });
    assert_eq!(
        session
            .dialog_kind(&SurfaceId::new("alert"))
            .expect("alert kind should be available"),
        AppKitDialogKind::Alert
    );
    let scene = session
        .snapshot_dialog_scene(&SurfaceId::new("alert"))
        .expect("alert scene should snapshot");
    assert!(
        scene
            .find_all(&Selector::label(glasscheck_core::TextMatch::contains(
                "Discard"
            )))
            .len()
            >= 1,
        "alert scene should expose visible alert text"
    );
    assert!(
        scene
            .find(&Selector::property_eq(
                "appkit:button_role",
                PropertyValue::string("confirm")
            ))
            .is_ok(),
        "alert scene should expose a confirm button"
    );

    session
        .click_dialog_button(
            &SurfaceId::new("alert"),
            &Selector::property_eq("appkit:button_role", PropertyValue::string("confirm")),
        )
        .expect("confirm button should be clickable");
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("alert completion should run after button click");
    assert_eq!(result.get(), Some(NSAlertFirstButtonReturn));
    session
        .wait_for_surface_closed(&SurfaceId::new("alert"), PollOptions::default())
        .expect("completed alert surface should close");
}

fn three_button_alert_click_uses_appkit_order(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Three Button Order"));
    alert.addButtonWithTitle(&NSString::from_str("Keep"));
    alert.addButtonWithTitle(&NSString::from_str("Review"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);
    session
        .wait_for_dialog(
            "three-button-alert",
            &AppKitDialogQuery::alert().title_contains("Three Button"),
            PollOptions::default(),
        )
        .expect("three-button alert should be discovered");

    session
        .click_dialog_button(
            &SurfaceId::new("three-button-alert"),
            &Selector::label(glasscheck_core::TextMatch::exact("Review")),
        )
        .expect("middle alert button should be clicked through the real AppKit button");
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("three-button alert completion should run");
    assert_eq!(result.get(), Some(NSAlertSecondButtonReturn));
    session
        .wait_for_surface_closed(
            &SurfaceId::new("three-button-alert"),
            PollOptions::default(),
        )
        .expect("completed three-button alert surface should close");
}

fn three_button_alert_cancel_uses_appkit_order(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Three Button Cancel Order"));
    alert.addButtonWithTitle(&NSString::from_str("Keep"));
    alert.addButtonWithTitle(&NSString::from_str("Review"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);
    session
        .wait_for_dialog(
            "three-button-cancel-alert",
            &AppKitDialogQuery::alert().title_contains("Three Button Cancel"),
            PollOptions::default(),
        )
        .expect("three-button cancel alert should be discovered");

    let scene = session
        .snapshot_dialog_scene(&SurfaceId::new("three-button-cancel-alert"))
        .expect("three-button cancel alert should snapshot");
    let keep = scene
        .node(
            scene
                .find(&Selector::label(glasscheck_core::TextMatch::exact("Keep")))
                .expect("keep button should be present"),
        )
        .unwrap();
    let review = scene
        .node(
            scene
                .find(&Selector::label(glasscheck_core::TextMatch::exact(
                    "Review",
                )))
                .expect("review button should be present"),
        )
        .unwrap();
    let cancel = scene
        .node(
            scene
                .find(&Selector::label(glasscheck_core::TextMatch::exact(
                    "Cancel",
                )))
                .expect("cancel button should be present"),
        )
        .unwrap();
    assert_eq!(keep.parent_id.as_deref(), review.parent_id.as_deref());
    assert_eq!(review.parent_id.as_deref(), cancel.parent_id.as_deref());
    assert_eq!(keep.child_index, 0);
    assert_eq!(review.child_index, 1);
    assert_eq!(cancel.child_index, 2);

    session
        .cancel_dialog(
            &SurfaceId::new("three-button-cancel-alert"),
            PollOptions::default(),
        )
        .expect("three-button alert should cancel through the real AppKit button");
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("three-button cancel completion should run");
    assert_eq!(result.get(), Some(NSAlertThirdButtonReturn));
    session
        .wait_for_surface_closed(
            &SurfaceId::new("three-button-cancel-alert"),
            PollOptions::default(),
        )
        .expect("completed three-button cancel alert surface should close");
}

fn alert_accessory_text_field_can_be_inspected_and_set(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Name Export"));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let field = NSTextField::textFieldWithString(
        &NSString::from_str("draft-name"),
        harness.main_thread_marker(),
    );
    field.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(220.0, 24.0),
    ));
    alert.setAccessoryView(Some(&field));
    alert.layout();
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);
    session
        .wait_for_dialog(
            "alert-text",
            &AppKitDialogQuery::alert().title_contains("Name Export"),
            PollOptions::default(),
        )
        .expect("alert with accessory should be discovered");

    let scene = session
        .snapshot_dialog_scene(&SurfaceId::new("alert-text"))
        .expect("alert scene should snapshot");
    assert!(
        scene.find(&Selector::role_eq(Role::TextInput)).is_ok(),
        "editable accessory field should be semantic text input"
    );
    session
        .set_dialog_text(
            &SurfaceId::new("alert-text"),
            &Selector::role_eq(Role::TextInput),
            "final-name",
        )
        .expect("accessory field text should be settable");
    assert_eq!(field.stringValue().to_string(), "final-name");

    session
        .click_dialog_button(
            &SurfaceId::new("alert-text"),
            &Selector::property_eq("appkit:button_role", PropertyValue::string("confirm")),
        )
        .expect("confirm button should be clickable");
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("alert completion should run");
    assert_eq!(result.get(), Some(NSAlertFirstButtonReturn));
    session
        .wait_for_surface_closed(&SurfaceId::new("alert-text"), PollOptions::default())
        .expect("completed alert surface should close");
}

fn dialog_text_rejects_labels_and_buttons(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Text Mutation Guard"));
    let ok_button = alert.addButtonWithTitle(&NSString::from_str("OK"));
    let label = NSTextField::labelWithString(
        &NSString::from_str("Read Only Label"),
        harness.main_thread_marker(),
    );
    label.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(220.0, 24.0),
    ));
    alert.setAccessoryView(Some(&label));
    alert.layout();
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);
    session
        .wait_for_dialog(
            "text-guard",
            &AppKitDialogQuery::alert().title_contains("Text Mutation"),
            PollOptions::default(),
        )
        .expect("text guard alert should be discovered");

    let label_error = session
        .set_dialog_text(
            &SurfaceId::new("text-guard"),
            &Selector::label(glasscheck_core::TextMatch::exact("Read Only Label")),
            "mutated label",
        )
        .unwrap_err();
    assert!(matches!(label_error, AppKitDialogError::InputUnavailable));
    assert_eq!(label.stringValue().to_string(), "Read Only Label");

    let button_error = session
        .set_dialog_text(
            &SurfaceId::new("text-guard"),
            &Selector::label(glasscheck_core::TextMatch::exact("OK")),
            "mutated button",
        )
        .unwrap_err();
    assert!(matches!(button_error, AppKitDialogError::InputUnavailable));
    assert_eq!(ok_button.title().to_string(), "OK");

    session.with_surface(&SurfaceId::new("main"), |host| {
        host.window()
            .endSheet_returnCode(&alert.window(), NSAlertSecondButtonReturn);
    });
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("text guard alert should clean up");
}

fn dialog_query_miss_times_out_cleanly(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Actual Dialog Title"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);

    let error = session
        .wait_for_dialog(
            "missing-dialog",
            &AppKitDialogQuery::alert().title_eq("Different Dialog Title"),
            short_poll_options(),
        )
        .unwrap_err();
    assert!(
        matches!(error, glasscheck_core::PollError::Timeout { .. }),
        "non-matching dialog query should time out, got {error:?}"
    );
    assert!(matches!(
        session.snapshot_dialog_scene(&SurfaceId::new("missing-dialog")),
        Err(AppKitDialogError::MissingSurface)
    ));

    session.with_surface(&SurfaceId::new("main"), |host| {
        host.window()
            .endSheet_returnCode(&alert.window(), NSAlertSecondButtonReturn);
    });
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("negative query fixture should be cleaned up");
}

fn dialog_methods_report_missing_surface(harness: AppKitHarness) {
    let session = harness.session();
    let missing = SurfaceId::new("missing-dialog");
    let save_path = temp_test_dir("missing-save-surface").join("export.txt");
    let open_path = temp_test_dir("missing-open-surface").join("fixture.txt");
    fs::write(&open_path, "fixture").expect("open fixture should be writable");

    assert!(matches!(
        session.dialog_kind(&missing),
        Err(AppKitDialogError::MissingSurface)
    ));
    assert!(matches!(
        session.snapshot_dialog_scene(&missing),
        Err(AppKitDialogError::MissingSurface)
    ));
    assert!(matches!(
        session.click_dialog_button(&missing, &Selector::role_eq(Role::Button)),
        Err(AppKitDialogError::MissingSurface)
    ));
    assert!(matches!(
        session.set_dialog_text(&missing, &Selector::role_eq(Role::TextInput), "ignored"),
        Err(AppKitDialogError::MissingSurface)
    ));
    assert!(matches!(
        session.choose_save_panel_path(&missing, &save_path, short_poll_options()),
        Err(AppKitDialogError::MissingSurface)
    ));
    assert!(matches!(
        session.choose_open_panel_paths(&missing, &[open_path], short_poll_options()),
        Err(AppKitDialogError::MissingSurface)
    ));
    assert!(matches!(
        session.cancel_dialog(&missing, short_poll_options()),
        Err(AppKitDialogError::MissingSurface)
    ));
}

fn alert_dialog_actions_report_missing_selectors(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Selector Negative Coverage"));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let field = NSTextField::textFieldWithString(
        &NSString::from_str("draft"),
        harness.main_thread_marker(),
    );
    alert.setAccessoryView(Some(&field));
    alert.layout();
    prepare_alert_for_background_test(&alert);

    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);
    session
        .wait_for_dialog(
            "selector-negative",
            &AppKitDialogQuery::alert().title_contains("Selector Negative"),
            PollOptions::default(),
        )
        .expect("alert should be discovered");

    let missing_text = session
        .set_dialog_text(
            &SurfaceId::new("selector-negative"),
            &Selector::label(glasscheck_core::TextMatch::exact("missing text field")),
            "ignored",
        )
        .unwrap_err();
    assert!(matches!(
        missing_text,
        AppKitDialogError::Resolve(RegionResolveError::NotFound(_))
    ));
    assert_eq!(field.stringValue().to_string(), "draft");

    let missing_button = session
        .click_dialog_button(
            &SurfaceId::new("selector-negative"),
            &Selector::label(glasscheck_core::TextMatch::exact("Not A Button")),
        )
        .unwrap_err();
    assert!(matches!(
        missing_button,
        AppKitDialogError::Resolve(RegionResolveError::NotFound(_))
    ));
    assert!(result.get().is_none());

    session
        .cancel_dialog(&SurfaceId::new("selector-negative"), PollOptions::default())
        .expect("negative selector alert should still be cancellable");
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("selector negative fixture should clean up");
}

fn save_panel_path_selection_is_deterministic(harness: AppKitHarness) {
    if !native_file_panel_contracts_enabled() {
        println!(
            "skipping live NSSavePanel contract; set GLASSCHECK_RUN_NATIVE_FILE_PANEL_TESTS=1 to run"
        );
        return;
    }
    let session = harness.session();
    let temp = temp_test_dir("save-panel");
    let save_path = temp.join("exported.txt");
    let panel = NSSavePanel::savePanel(harness.main_thread_marker());
    panel.setTitle(Some(&NSString::from_str("Export Document")));
    panel.setMessage(Some(&NSString::from_str("Choose a destination.")));
    panel.setPrompt(Some(&NSString::from_str("Export")));
    panel.setNameFieldStringValue(&NSString::from_str("draft.txt"));
    hide_native_dialog_window(&panel);
    session.attach_window("save-panel", &panel);

    let scene = session
        .snapshot_dialog_scene(&SurfaceId::new("save-panel"))
        .expect("save panel scene should snapshot");
    let root = scene
        .node(scene.find(&Selector::id_eq("appkit.dialog")).unwrap())
        .unwrap();
    assert_eq!(
        root.properties.get("appkit:dialog_kind"),
        Some(&PropertyValue::string("save_panel"))
    );
    assert_eq!(
        root.properties.get("appkit:prompt"),
        Some(&PropertyValue::string("Export"))
    );
    assert_eq!(
        root.properties.get("appkit:name_field"),
        Some(&PropertyValue::string("draft.txt"))
    );

    session
        .choose_save_panel_path(
            &SurfaceId::new("save-panel"),
            &save_path,
            PollOptions::default(),
        )
        .expect("save panel should accept deterministic path");
    assert_eq!(
        panel
            .URL()
            .and_then(|url| url.path())
            .map(|path| PathBuf::from(path.to_string())),
        Some(save_path)
    );
}

fn save_panel_rejects_wrong_dialog_kind(harness: AppKitHarness) {
    let owner = harness.create_window(320.0, 180.0);
    let session = harness.session();
    session.attach_host("main", owner);
    let result = Rc::new(Cell::new(None));
    let result_for_block = result.clone();
    let block = RcBlock::new(move |response: isize| {
        result_for_block.set(Some(response));
    });
    let alert = NSAlert::new(harness.main_thread_marker());
    alert.setMessageText(&NSString::from_str("Wrong Kind"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    prepare_alert_for_background_test(&alert);
    session.with_surface(&SurfaceId::new("main"), |host| {
        alert.beginSheetModalForWindow_completionHandler(host.window(), Some(&block));
    });
    harness.settle(2);
    session
        .wait_for_dialog(
            "wrong-kind",
            &AppKitDialogQuery::alert().title_contains("Wrong Kind"),
            PollOptions::default(),
        )
        .expect("alert should be discovered");

    let error = session
        .choose_save_panel_path(
            &SurfaceId::new("wrong-kind"),
            &temp_test_dir("wrong-kind").join("file.txt"),
            PollOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        AppKitDialogError::KindMismatch {
            expected: AppKitDialogKind::SavePanel,
            actual: AppKitDialogKind::Alert
        }
    ));
    session
        .cancel_dialog(&SurfaceId::new("wrong-kind"), PollOptions::default())
        .expect("wrong-kind alert should remain cancellable");
    harness
        .wait_until(PollOptions::default(), || result.get().is_some())
        .expect("wrong-kind alert completion should run");
}

fn custom_panel_with_button_stays_panel_kind(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let session = harness.session();
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 120.0)),
        NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
        NSBackingStoreType::Buffered,
        false,
    );
    unsafe {
        panel.setReleasedWhenClosed(false);
    }
    panel.setTitle(&NSString::from_str("Custom Panel"));
    hide_native_dialog_window(&panel);
    let root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 120.0)),
    );
    let label = NSTextField::labelWithString(
        &NSString::from_str("Read Only Panel Label"),
        harness.main_thread_marker(),
    );
    label.setFrame(NSRect::new(
        NSPoint::new(16.0, 76.0),
        NSSize::new(180.0, 20.0),
    ));
    root.addSubview(&label);
    let field = NSTextField::textFieldWithString(
        &NSString::from_str("draft"),
        harness.main_thread_marker(),
    );
    field.setFrame(NSRect::new(
        NSPoint::new(16.0, 48.0),
        NSSize::new(160.0, 24.0),
    ));
    root.addSubview(&field);
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str("Apply"), None, None, mtm)
    };
    button.setFrame(NSRect::new(
        NSPoint::new(16.0, 16.0),
        NSSize::new(96.0, 32.0),
    ));
    root.addSubview(&button);
    panel.setContentView(Some(&root));

    session.attach_window("custom-panel", &panel);

    assert_eq!(
        session
            .dialog_kind(&SurfaceId::new("custom-panel"))
            .expect("custom panel kind should be available"),
        AppKitDialogKind::Panel
    );
    let scene = session
        .snapshot_dialog_scene(&SurfaceId::new("custom-panel"))
        .expect("custom panel should snapshot");
    let root = scene
        .node(scene.find(&Selector::id_eq("appkit.dialog")).unwrap())
        .unwrap();
    assert_eq!(
        root.properties.get("appkit:dialog_kind"),
        Some(&PropertyValue::string("panel"))
    );
    let content_root = scene
        .node(
            scene
                .find(&Selector::id_eq("appkit.dialog.view.0"))
                .expect("content root should be present"),
        )
        .unwrap();
    assert_eq!(content_root.parent_id.as_deref(), Some("appkit.dialog"));
    assert_eq!(content_root.child_index, 0);
    let label_node = scene
        .node(
            scene
                .find(&Selector::label(glasscheck_core::TextMatch::exact(
                    "Read Only Panel Label",
                )))
                .expect("custom panel label should be present"),
        )
        .unwrap();
    assert_eq!(
        label_node.parent_id.as_deref(),
        Some("appkit.dialog.view.0")
    );
    assert_eq!(label_node.child_index, 0);
    let field_node = scene
        .node(
            scene
                .find(&Selector::role_eq(Role::TextInput))
                .expect("custom panel text field should be present"),
        )
        .unwrap();
    assert_eq!(
        field_node.parent_id.as_deref(),
        Some("appkit.dialog.view.0")
    );
    assert_eq!(field_node.child_index, 1);
    let button_node = scene
        .node(
            scene
                .find(&Selector::label(glasscheck_core::TextMatch::exact("Apply")))
                .expect("custom panel button should be present"),
        )
        .unwrap();
    assert_eq!(
        button_node.parent_id.as_deref(),
        Some("appkit.dialog.view.0")
    );
    assert_eq!(button_node.child_index, 2);
    assert!(
        scene
            .find(&Selector::label(glasscheck_core::TextMatch::exact("Apply")))
            .is_ok(),
        "custom panel button should remain visible in the dialog scene"
    );
    let label_error = session
        .click_dialog_button(
            &SurfaceId::new("custom-panel"),
            &Selector::label(glasscheck_core::TextMatch::exact("Read Only Panel Label")),
        )
        .unwrap_err();
    assert!(matches!(label_error, AppKitDialogError::InputUnavailable));
    let text_field_error = session
        .click_dialog_button(
            &SurfaceId::new("custom-panel"),
            &Selector::role_eq(Role::TextInput),
        )
        .unwrap_err();
    assert!(matches!(
        text_field_error,
        AppKitDialogError::InputUnavailable
    ));
}

fn ordinary_window_is_not_discovered_as_dialog_panel(harness: AppKitHarness) {
    let session = harness.session();
    let owner = harness.create_window(240.0, 120.0);
    owner.set_title("Panel Negative Owner");
    session.attach_host("main", owner);

    let ordinary = harness.create_window(240.0, 120.0);
    ordinary.set_title("Ordinary Window Panel Query Negative");
    ordinary.window().setAlphaValue(0.42);
    harness.settle(2);

    let error = session
        .wait_for_dialog(
            "ordinary-dialog",
            &AppKitDialogQuery::kind(AppKitDialogKind::Panel).title_contains("Ordinary Window"),
            short_poll_options(),
        )
        .unwrap_err();
    assert!(
        matches!(error, glasscheck_core::PollError::Timeout { .. }),
        "ordinary NSWindow should not satisfy Panel dialog discovery, got {error:?}"
    );
    assert!(
        !session.surface_is_open(&SurfaceId::new("ordinary-dialog")),
        "ordinary NSWindow should not be attached as a dialog surface"
    );
    assert_eq!(
        ordinary.window().alphaValue(),
        0.42,
        "dialog discovery must not apply dialog background transparency to ordinary NSWindow surfaces"
    );

    assert!(
        session.discover_window(
            "ordinary-window",
            &SurfaceQuery::title_contains("Ordinary Window"),
        ),
        "fixture should remain discoverable through ordinary window discovery"
    );
    assert!(matches!(
        session.dialog_kind(&SurfaceId::new("ordinary-window")),
        Err(AppKitDialogError::NotDialog)
    ));
}

fn open_panel_reports_missing_paths_and_cancels(harness: AppKitHarness) {
    if !native_file_panel_contracts_enabled() {
        println!(
            "skipping live NSOpenPanel contract; set GLASSCHECK_RUN_NATIVE_FILE_PANEL_TESTS=1 to run"
        );
        return;
    }
    let session = harness.session();
    let panel = NSOpenPanel::openPanel(harness.main_thread_marker());
    panel.setTitle(Some(&NSString::from_str("Open Fixture")));
    panel.setMessage(Some(&NSString::from_str("Choose a fixture.")));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    hide_native_dialog_window(&panel);
    session.attach_window("open-panel", &panel);
    assert_eq!(
        session.dialog_kind(&SurfaceId::new("open-panel")).unwrap(),
        AppKitDialogKind::OpenPanel
    );

    let missing = temp_test_dir("open-panel-missing").join("missing.txt");
    let error = session
        .choose_open_panel_paths(
            &SurfaceId::new("open-panel"),
            &[missing.clone()],
            PollOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(error, AppKitDialogError::MissingRequestedPath(path) if path == missing));
    session
        .cancel_dialog(&SurfaceId::new("open-panel"), PollOptions::default())
        .expect("open panel should cancel");
}

fn open_panel_live_file_selection_is_explicit(harness: AppKitHarness) {
    if !native_file_panel_contracts_enabled() {
        println!(
            "skipping live NSOpenPanel selection contract; set GLASSCHECK_RUN_NATIVE_FILE_PANEL_TESTS=1 to run"
        );
        return;
    }
    let session = harness.session();
    let temp = temp_test_dir("open-panel-select");
    let file = temp.join("fixture.txt");
    fs::write(&file, "fixture").expect("fixture file should be writable");
    let panel = NSOpenPanel::openPanel(harness.main_thread_marker());
    panel.setTitle(Some(&NSString::from_str("Open Fixture")));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    hide_native_dialog_window(&panel);
    session.attach_window("open-panel-select", &panel);

    match session.choose_open_panel_paths(
        &SurfaceId::new("open-panel-select"),
        &[file.clone()],
        PollOptions::default(),
    ) {
        Ok(_) => {
            assert_eq!(
                panel
                    .URLs()
                    .iter()
                    .filter_map(|url| url.path())
                    .map(|path| PathBuf::from(path.to_string()))
                    .collect::<Vec<_>>(),
                vec![file]
            );
        }
        Err(AppKitDialogError::UnsupportedLiveSelection(_)) => {
            session
                .cancel_dialog(&SurfaceId::new("open-panel-select"), PollOptions::default())
                .expect("unsupported live selection should still leave panel cancellable");
        }
        Err(error) => panic!("unexpected open panel selection error: {error}"),
    }
}

fn transient_surface_hover_updates_active_always_mouse_moved_tracking_state(
    harness: AppKitHarness,
) {
    let fixture = open_picker_contract_fixture(harness);

    let initial_scene = fixture
        .session
        .snapshot_scene(&SurfaceId::new("picker"))
        .expect("picker surface should be attached");
    assert_highlighted_picker_cell(&initial_scene, 1, 1);
    assert_picker_label(&initial_scene, "1 × 1");

    fixture
        .session
        .hover_node(
            &SurfaceId::new("picker"),
            &Selector::id_eq("table.picker.cell.3.4"),
            &HitPointSearch::default(),
        )
        .expect("picker surface should be attached")
        .expect("hover should succeed");
    harness.settle(2);

    let picker_scene = fixture
        .session
        .snapshot_scene(&SurfaceId::new("picker"))
        .expect("picker surface should remain attached");
    assert_highlighted_picker_cell(&picker_scene, 3, 4);
    assert_picker_label(&picker_scene, "3 × 4");
}

fn transient_surface_hover_delivers_single_mouse_moved_callback_per_step(harness: AppKitHarness) {
    let fixture = open_picker_contract_fixture(harness);

    assert_eq!(fixture.tracking_mouse_moved_count(), 0);
    assert_eq!(fixture.tracking_mouse_exited_count(), 0);

    fixture
        .session
        .hover_node(
            &SurfaceId::new("picker"),
            &Selector::id_eq("table.picker.cell.3.4"),
            &HitPointSearch::default(),
        )
        .expect("picker surface should be attached")
        .expect("hover should succeed");
    harness.settle(2);

    assert_eq!(fixture.tracking_mouse_moved_count(), 1);
    assert_eq!(fixture.tracking_mouse_exited_count(), 0);

    fixture
        .session
        .hover_node(
            &SurfaceId::new("picker"),
            &Selector::id_eq("table.picker.cell.4.2"),
            &HitPointSearch::default(),
        )
        .expect("picker surface should be attached")
        .expect("second hover should succeed");
    harness.settle(2);

    assert_eq!(fixture.tracking_mouse_moved_count(), 2);
    assert_eq!(fixture.tracking_mouse_exited_count(), 0);

    let picker_scene = fixture
        .session
        .snapshot_scene(&SurfaceId::new("picker"))
        .expect("picker surface should remain attached");
    assert_highlighted_picker_cell(&picker_scene, 4, 2);
    assert_picker_label(&picker_scene, "4 × 2");
}

fn background_window_hover_updates_active_in_active_app_tracking_state(harness: AppKitHarness) {
    let fixture = open_background_picker_fixture(harness);
    assert!(
        !fixture.window().isVisible(),
        "background picker window should start hidden before hover"
    );
    assert!(
        !fixture.window().isKeyWindow(),
        "background picker window should start non-key before hover"
    );

    fixture
        .host()
        .hover_node(
            &Selector::id_eq("table.picker.cell.3.4"),
            &HitPointSearch::default(),
        )
        .expect("background picker host should resolve hover targets");
    harness.settle(2);

    assert_eq!(fixture.tracking_mouse_moved_count(), 1);

    let picker_scene = fixture.host().snapshot_scene();
    assert_highlighted_picker_cell(&picker_scene, 3, 4);
    assert_picker_label(&picker_scene, "3 × 4");
    assert!(
        !fixture.window().isVisible(),
        "background picker window should remain hidden after hover"
    );
    assert!(
        !fixture.window().isKeyWindow(),
        "background picker window should remain non-key after hover"
    );
}

fn background_window_click_reaches_local_mouse_up_monitor(harness: AppKitHarness) {
    let fixture = open_background_picker_fixture(harness);
    assert!(
        !fixture.window().isVisible(),
        "background picker window should start hidden before click"
    );
    assert!(
        !fixture.window().isKeyWindow(),
        "background picker window should start non-key before click"
    );

    fixture
        .host()
        .click_node(&Selector::id_eq("table.picker.cell.2.3"))
        .expect("background picker host should resolve click targets");
    harness.settle(2);

    assert_eq!(fixture.inserted_dims(), Some((2, 3)));
    assert!(
        !fixture.window().isVisible(),
        "background picker window should remain hidden after click"
    );
    assert!(
        !fixture.window().isKeyWindow(),
        "background picker window should remain non-key after click"
    );
}

fn provider_only_text_click_reaches_local_mouse_up_monitor_without_duplicate_mouse_up(
    harness: AppKitHarness,
) {
    struct MouseUpMonitorGuard(Option<Retained<AnyObject>>);

    impl Drop for MouseUpMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let mtm = harness.main_thread_marker();
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(mtm, NSSize::new(220.0, 140.0));
    let view = ClickTrackingChildView::new(
        mtm,
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(180.0, 80.0)),
    );
    view.setEditable(true);
    view.setSelectable(true);
    view.setDrawsBackground(false);
    view.setString(&NSString::from_str("abcdefg"));
    view.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    root.addSubview(&view);
    host.set_content_view(&root);
    let text_view: &NSTextView = &view;
    host.set_contextual_scene_source(Box::new(ContextualTextSceneSource {
        view: unsafe {
            Retained::retain(text_view as *const NSTextView as *mut NSTextView)
                .expect("context text view should retain")
        },
    }));

    let monitor_calls = Rc::new(Cell::new(0));
    let monitor_calls_for_block = monitor_calls.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let _event_ref = unsafe { event.as_ref() };
        monitor_calls_for_block.set(monitor_calls_for_block.get() + 1);
        event.as_ptr()
    });
    let _monitor = MouseUpMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseUp, &block)
    });

    harness.settle(2);
    assert!(
        !host.window().isVisible(),
        "provider-only text click should keep the background test window hidden"
    );
    assert!(!host.window().isKeyWindow());

    host.click_node(&Selector::selector_eq("context.text"))
        .unwrap();
    harness.settle(2);

    assert_eq!(
        (
            monitor_calls.get(),
            view.ivars().mouse_downs.get(),
            view.ivars().mouse_ups.get(),
        ),
        (1, 1, 1)
    );
    assert!(
        !host.window().isVisible(),
        "provider-only text click should not surface the background test window"
    );
    assert!(!host.window().isKeyWindow());
}

fn provider_only_background_click_reaches_local_mouse_up_monitor_without_dropping_mouse_up(
    harness: AppKitHarness,
) {
    struct MouseUpMonitorGuard(Option<Retained<AnyObject>>);

    impl Drop for MouseUpMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let host = harness.create_window(180.0, 120.0);
    let root = ClickTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let window_number = host.window().windowNumber();
    let monitor_calls = Rc::new(Cell::new(0));
    let monitor_calls_for_block = monitor_calls.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        if event_ref.windowNumber() == window_number {
            monitor_calls_for_block.set(monitor_calls_for_block.get() + 1);
        }
        event.as_ptr()
    });
    let _monitor = MouseUpMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseUp, &block)
    });

    harness.settle(2);
    assert!(
        !host.window().isVisible(),
        "provider-only background click should keep the background test window hidden"
    );
    assert!(!host.window().isKeyWindow());

    host.click_node(&Selector::id_eq("provider-node")).unwrap();
    harness.settle(2);

    assert_eq!(
        (
            monitor_calls.get(),
            root.ivars().mouse_downs.get(),
            root.ivars().mouse_ups.get(),
        ),
        (1, 1, 1)
    );
    assert!(
        !host.window().isVisible(),
        "provider-only background click should not surface the background test window"
    );
    assert!(!host.window().isKeyWindow());
}

fn click_targets_attached_child_window_even_when_parent_window_is_present(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let parent = harness.create_window(180.0, 120.0);
    let parent_view = RoutingTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    parent.set_content_view(&parent_view);
    parent.window().makeFirstResponder(Some(&parent_view));

    let child = harness.create_window(180.0, 120.0);
    let child_view = ClickTrackingChildView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    child.set_content_view(&child_view);

    unsafe {
        parent
            .window()
            .addChildWindow_ordered(child.window(), NSWindowOrderingMode::Above);
    }

    let other = harness.create_window(180.0, 120.0);
    let other_view = RoutingTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    other.set_content_view(&other_view);
    other.window().makeFirstResponder(Some(&other_view));
    other.window().makeKeyWindow();
    harness.settle(2);

    assert!(child.window().parentWindow().is_some());
    assert!(!child.window().isKeyWindow());

    let attached = harness.attach_window(child.window());
    attached.input().click(Point::new(40.0, 40.0)).unwrap();

    assert_eq!(child_view.ivars().mouse_downs.get(), 1);
    assert_eq!(child_view.ivars().mouse_ups.get(), 1);
    assert_eq!(parent_view.ivars().mouse_downs.get(), 0);
    assert_eq!(other_view.ivars().mouse_downs.get(), 0);
    assert!(!child.window().isKeyWindow());
}

fn semantic_click_targets_attached_child_window_even_when_parent_window_is_present(
    harness: AppKitHarness,
) {
    let mtm = harness.main_thread_marker();
    let parent = harness.create_window(180.0, 120.0);
    let parent_view = RoutingTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    parent.set_content_view(&parent_view);
    parent.window().makeFirstResponder(Some(&parent_view));

    let child = harness.create_window(180.0, 120.0);
    let child_view = ClickTrackingChildView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    child.set_content_view(&child_view);

    unsafe {
        parent
            .window()
            .addChildWindow_ordered(child.window(), NSWindowOrderingMode::Above);
    }

    let other = harness.create_window(180.0, 120.0);
    let other_view = RoutingTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    other.set_content_view(&other_view);
    other.window().makeFirstResponder(Some(&other_view));
    other.window().makeKeyWindow();
    harness.settle(2);

    let attached = harness.attach_window(child.window());
    attached.register_node(
        &child_view,
        InstrumentedView {
            id: Some("attached-child-click-target".into()),
            role: Some(Role::Button),
            label: Some("Attached Child Click Target".into()),
            ..Default::default()
        },
    );
    harness.settle(2);

    attached
        .click_node(&Selector::id_eq("attached-child-click-target"))
        .unwrap();

    assert_eq!(child_view.ivars().mouse_downs.get(), 1);
    assert_eq!(child_view.ivars().mouse_ups.get(), 1);
    assert_eq!(parent_view.ivars().mouse_downs.get(), 0);
    assert_eq!(other_view.ivars().mouse_downs.get(), 0);
    assert!(!child.window().isKeyWindow());
}

fn provider_only_semantic_click_targets_attached_child_window_even_when_parent_window_is_present(
    harness: AppKitHarness,
) {
    let mtm = harness.main_thread_marker();
    let parent = harness.create_window(180.0, 120.0);
    let parent_view = RoutingTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    parent.set_content_view(&parent_view);
    parent.window().makeFirstResponder(Some(&parent_view));

    let child = harness.create_window(220.0, 120.0);
    let child_root = make_view(mtm, NSSize::new(220.0, 120.0));
    let child_view = make_text_view(mtm, NSSize::new(180.0, 80.0), "ab");
    child_view.setEditable(true);
    child_view.setSelectable(true);
    child_view.setDrawsBackground(false);
    child_view.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    child_view.setFrameOrigin(NSPoint::new(20.0, 20.0));
    child_root.addSubview(&child_view);
    child.set_content_view(&child_root);

    unsafe {
        parent
            .window()
            .addChildWindow_ordered(child.window(), NSWindowOrderingMode::Above);
    }

    let other = harness.create_window(180.0, 120.0);
    let other_view = RoutingTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    other.set_content_view(&other_view);
    other.window().makeFirstResponder(Some(&other_view));
    other.window().makeKeyWindow();
    harness.settle(2);
    let other_was_key = other.window().isKeyWindow();

    assert!(child.window().parentWindow().is_some());
    assert!(!child.window().isKeyWindow());

    let attached = harness.attach_window(child.window());
    attached
        .input()
        .set_selection(&child_view, TextRange::new(2, 0));
    attached.set_contextual_scene_source(Box::new(AttachedChildCaretSceneSource {
        view: unsafe {
            Retained::retain(&*child_view as *const NSTextView as *mut NSTextView)
                .expect("attached child text view should retain")
        },
        location: 1,
    }));
    attached
        .input()
        .set_selection(&child_view, TextRange::new(2, 0));
    attached
        .click_node(&Selector::selector_eq("provider.caret"))
        .unwrap();
    harness.settle(2);

    // The caret scene source targets location 1 (between 'a' and 'b' in "ab").
    assert_eq!(
        attached.selected_text_range(&child_view),
        TextRange::new(1, 0)
    );
    assert_eq!(other_view.ivars().mouse_downs.get(), 0);
    assert_eq!(other_view.ivars().key_downs.get(), 0);
    assert!(!child.window().isKeyWindow());
    assert_eq!(other.window().isKeyWindow(), other_was_key);
}

fn provider_only_semantic_click_uses_standalone_path_after_attached_child_detach(
    harness: AppKitHarness,
) {
    struct MouseUpMonitorGuard(Option<Retained<AnyObject>>);

    impl Drop for MouseUpMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let mtm = harness.main_thread_marker();
    let parent = harness.create_window(180.0, 120.0);
    let child = harness.create_window(220.0, 120.0);
    let child_root = make_view(mtm, NSSize::new(220.0, 120.0));
    let child_view = make_text_view(mtm, NSSize::new(180.0, 80.0), "ab");
    child_view.setEditable(true);
    child_view.setSelectable(true);
    child_view.setDrawsBackground(false);
    child_view.setFont(Some(&NSFont::systemFontOfSize(13.0)));
    child_view.setFrameOrigin(NSPoint::new(20.0, 20.0));
    child_root.addSubview(&child_view);
    child.set_content_view(&child_root);

    unsafe {
        parent
            .window()
            .addChildWindow_ordered(child.window(), NSWindowOrderingMode::Above);
    }
    assert!(child.window().parentWindow().is_some());

    let attached = harness.attach_window(child.window());
    attached.set_contextual_scene_source(Box::new(AttachedChildCaretSceneSource {
        view: unsafe {
            Retained::retain(&*child_view as *const NSTextView as *mut NSTextView)
                .expect("attached child text view should retain")
        },
        location: 1,
    }));

    parent.window().removeChildWindow(child.window());
    harness.settle(2);
    assert!(child.window().parentWindow().is_none());
    assert!(!child.window().isVisible());
    assert!(!child.window().isKeyWindow());

    let window_number = child.window().windowNumber();
    let monitor_calls = Rc::new(Cell::new(0));
    let monitor_calls_for_block = monitor_calls.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        if event_ref.windowNumber() == window_number {
            monitor_calls_for_block.set(monitor_calls_for_block.get() + 1);
        }
        event.as_ptr()
    });
    let _monitor = MouseUpMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseUp, &block)
    });

    attached
        .input()
        .set_selection(&child_view, TextRange::new(2, 0));
    attached
        .click_node(&Selector::selector_eq("provider.caret"))
        .unwrap();
    harness.settle(2);

    assert_eq!(monitor_calls.get(), 1);
    assert_ne!(
        attached.selected_text_range(&child_view),
        TextRange::new(2, 0)
    );
    assert!(!child.window().isVisible());
    assert!(!child.window().isKeyWindow());
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

    let attached = harness.attach_window(host.window());
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
    other.window().makeKeyWindow();
    harness.settle(2);
    let other_was_key = other.window().isKeyWindow();

    target.input().move_mouse(Point::new(40.0, 40.0)).unwrap();
    harness.settle(2);

    assert_eq!(target_view.ivars().mouse_moves.get(), 1);
    assert_eq!(other_view.ivars().mouse_moves.get(), 0);
    assert!(!target.window().isKeyWindow());
    assert_eq!(other.window().isKeyWindow(), other_was_key);
}

fn synthesized_input_keeps_background_test_windows_hidden(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.window().setAcceptsMouseMovedEvents(true);
    host.window().makeFirstResponder(Some(&view));
    harness.settle(2);

    assert!(
        !host.window().isVisible(),
        "background test window should start hidden"
    );
    assert!(!host.window().isKeyWindow());

    host.input().move_mouse(Point::new(24.0, 24.0)).unwrap();
    host.input()
        .key_press("a", glasscheck_core::KeyModifiers::default())
        .unwrap();
    harness.settle(2);

    assert!(
        !host.window().isVisible(),
        "synthetic input should not surface the background test window"
    );
    assert!(!host.window().isKeyWindow());
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
    other.window().makeKeyWindow();
    harness.settle(2);
    let other_was_key = other.window().isKeyWindow();

    target
        .input()
        .key_press("a", glasscheck_core::KeyModifiers::default())
        .unwrap();
    harness.settle(2);

    assert_eq!(target_view.ivars().key_downs.get(), 1);
    assert_eq!(other_view.ivars().key_downs.get(), 0);
    assert!(!target.window().isKeyWindow());
    assert_eq!(other.window().isKeyWindow(), other_was_key);
}

fn making_peer_window_key_does_not_surface_background_test_windows(harness: AppKitHarness) {
    let target = harness.create_window(180.0, 120.0);
    let target_view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    target.set_content_view(&target_view);

    let other = harness.create_window(180.0, 120.0);
    let other_view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    other.set_content_view(&other_view);
    harness.settle(2);

    assert!(!target.window().isVisible());
    assert!(!other.window().isVisible());

    other.window().makeKeyWindow();
    harness.settle(2);

    assert!(
        !target.window().isVisible(),
        "background target window should remain hidden when a peer becomes key"
    );
    assert!(
        !other.window().isVisible(),
        "background peer window should not surface when made key"
    );
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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &left,
        InstrumentedView {
            id: Some("left".into()),
            role: Some(Role::Container),
            label: Some("Left".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &right,
        InstrumentedView {
            id: Some("right".into()),
            role: Some(Role::Container),
            label: Some("Right".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &label,
        InstrumentedView {
            id: Some("label".into()),
            role: Some(Role::Label),
            label: Some("Nested".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let left_handle = scene.find(&Selector::id_eq("left")).unwrap();
    let right_handle = scene.find(&Selector::id_eq("right")).unwrap();
    let label_handle = scene.find(&Selector::id_eq("label")).unwrap();

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

    let root_children = scene.find_all(&Selector::parent(Selector::id_eq("root")));
    assert_eq!(root_children.len(), 2);
    assert_eq!(scene.node(root_children[0]).unwrap().id, "left");
    assert_eq!(scene.node(root_children[1]).unwrap().id, "right");
    assert_eq!(
        scene
            .find(&Selector::parent(Selector::id_eq("right")))
            .unwrap()
            .index(),
        label_handle.index()
    );
    assert_eq!(
        scene
            .find(&Selector::ancestor(Selector::id_eq("right")))
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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &child,
        InstrumentedView {
            id: Some("child".into()),
            role: Some(Role::Container),
            label: Some("Child".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let error = host
        .snapshot_scene()
        .find(&Selector::ancestor(Selector::id_eq("missing")))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFound(_)));
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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &label,
        InstrumentedView {
            id: Some("label".into()),
            role: Some(Role::Label),
            label: Some("Nested".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let label_handle = scene.find(&Selector::id_eq("label")).unwrap();
    assert_eq!(
        scene.node(label_handle).unwrap().parent_id.as_deref(),
        Some("root")
    );
    let children = scene.find_all(&Selector::parent(Selector::id_eq("root")));
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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &left,
        InstrumentedView {
            id: Some("duplicate-parent".into()),
            role: Some(Role::Container),
            label: Some("Left Parent".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &right,
        InstrumentedView {
            id: Some("duplicate-parent".into()),
            role: Some(Role::Container),
            label: Some("Right Parent".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &left_label,
        InstrumentedView {
            id: Some("left-label".into()),
            role: Some(Role::Label),
            label: Some("Left".into()),
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &right_label,
        InstrumentedView {
            id: Some("right-label".into()),
            role: Some(Role::Label),
            label: Some("Right".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let parents = scene.find_all(&Selector::id_eq("duplicate-parent"));
    assert_eq!(parents.len(), 0);

    let disambiguated_parents = scene.find_all(&Selector::id_eq("native::duplicate-parent"));
    assert_eq!(disambiguated_parents.len(), 1);
    assert_eq!(
        scene
            .find_all(&Selector::id_eq("native::duplicate-parent#1"))
            .len(),
        1
    );

    let left_label = scene.find(&Selector::id_eq("left-label")).unwrap();
    let right_label = scene.find(&Selector::id_eq("right-label")).unwrap();
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
            .find(&Selector::ancestor(Selector::id_eq(
                "native::duplicate-parent"
            )))
            .unwrap()
            .index(),
        left_label.index()
    );
    assert_eq!(
        scene
            .find(&Selector::ancestor(Selector::id_eq(
                "native::duplicate-parent#1"
            )))
            .unwrap()
            .index(),
        right_label.index()
    );
    let original_id_matches = scene.find_all(&Selector::property_eq(
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

    host.input().click(Point::new(40.0, 32.0)).unwrap();
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    host.click_node(&Selector::id_eq("run-button")).unwrap();
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

    host.input().click(Point::new(190.0, 110.0)).unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().actions.get(), 0);
}

fn scene_source_recipes_support_external_offsets(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    host.set_content_view(&root);
    host.set_scene_source(Box::new(OffsetRecipeProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let handle = scene
        .find(&Selector::selector_eq("provider.adjacent"))
        .expect("offset recipe node should resolve");
    let node = scene.node(handle).unwrap();
    assert_eq!(
        node.rect,
        Rect::new(Point::new(110.0, 20.0), Size::new(40.0, 20.0))
    );
}

fn scene_source_recipe_hit_target_uses_window_coordinates(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = PointTrackingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, 140.0)),
    );
    let pinned = NullHitTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(60.0, 20.0), NSSize::new(100.0, 60.0)),
    );
    root.addSubview(&pinned);
    host.set_content_view(&root);
    harness.settle(2);

    let attached = harness.attach_root_view(&pinned, Some(host.window()));
    attached.set_scene_source(Box::new(ExplicitHitTargetRecipeProvider));
    harness.settle(2);

    let hit_point = attached
        .resolve_hit_point(
            &Selector::selector_eq("provider.hit-target"),
            &HitPointSearch::default(),
        )
        .expect("recipe hit target should resolve in window coordinates");
    assert_eq!(hit_point, Point::new(81.0, 33.0));

    attached
        .click_node(&Selector::selector_eq("provider.hit-target"))
        .expect("recipe hit target should be clickable");
    harness.settle(2);

    assert_eq!(root.ivars().mouse_downs.get(), 1);
    assert_eq!(root.ivars().last_click_x.get(), 81.0);
    assert_eq!(root.ivars().last_click_y.get(), 33.0);
}

fn scene_source_recipe_hit_target_respects_search_strategy(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = PointTrackingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, 140.0)),
    );
    let pinned = NullHitTrackingContainerView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(60.0, 20.0), NSSize::new(100.0, 60.0)),
    );
    root.addSubview(&pinned);
    host.set_content_view(&root);
    harness.settle(2);

    let attached = harness.attach_root_view(&pinned, Some(host.window()));
    attached.set_scene_source(Box::new(ExplicitHitTargetRecipeProvider));
    harness.settle(2);

    let search = HitPointSearch {
        strategy: HitPointStrategy::CornersAndCenter,
        sample_count: 5,
    };
    let hit_point = attached
        .resolve_hit_point(&Selector::selector_eq("provider.hit-target"), &search)
        .expect("recipe hit target should respect the requested search strategy");
    assert_eq!(hit_point, Point::new(80.0, 32.0));

    attached
        .click_node_with_search(&Selector::selector_eq("provider.hit-target"), &search)
        .expect("recipe hit target click should use the searched point");
    harness.settle(2);

    assert_eq!(root.ivars().mouse_downs.get(), 1);
    assert_eq!(root.ivars().last_click_x.get(), 80.0);
    assert_eq!(root.ivars().last_click_y.get(), 32.0);
}

fn scene_source_recipe_clicks_with_explicit_hit_target_even_when_locator_rect_is_empty(
    harness: AppKitHarness,
) {
    let host = harness.create_window(220.0, 140.0);
    let root = PointTrackingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(220.0, 140.0)),
    );
    host.set_content_view(&root);
    host.set_scene_source(Box::new(ExplicitHitTargetOnlyRecipeProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(
            scene
                .find(&Selector::selector_eq("provider.hit-target-only"))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(node.rect.size, Size::new(0.0, 0.0));

    let hit_point = host
        .resolve_hit_point(
            &Selector::selector_eq("provider.hit-target-only"),
            &HitPointSearch::default(),
        )
        .expect("explicit hit target should resolve even when the main rect is empty");
    assert_eq!(hit_point, Point::new(21.0, 13.0));

    host.click_node(&Selector::selector_eq("provider.hit-target-only"))
        .expect("explicit hit target should drive clicks even when the main rect is empty");
    harness.settle(2);

    assert_eq!(root.ivars().mouse_downs.get(), 1);
    assert_eq!(root.ivars().last_click_x.get(), 21.0);
    assert_eq!(root.ivars().last_click_y.get(), 13.0);
}

fn scene_source_recipe_is_omitted_when_anchor_is_missing(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    host.set_content_view(&root);
    host.set_scene_source(Box::new(MissingAnchorRecipeProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    assert_eq!(scene.recipe_errors().len(), 1);
    assert_eq!(scene.recipe_errors()[0].recipe_id, "adjacent");
    assert!(matches!(
        scene.recipe_errors()[0].error,
        RegionResolveError::NotFound(_)
    ));
    assert!(scene
        .find(&Selector::selector_eq("provider.adjacent"))
        .is_err());
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
        .find(&Selector::property_eq(
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
        .find(&Selector::property_eq(
            "session_id",
            PropertyValue::Integer(99),
        ))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFound(_)));
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
                .find(&Selector::id_eq("battlefield/card:session-3/title"))
                .unwrap(),
        )
        .unwrap();
    let chip = scene
        .node(
            scene
                .find(&Selector::id_eq("battlefield/card:session-3/status-chip"))
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
                .find(&Selector::id_eq("battlefield/card:session-3/title"))
                .unwrap(),
        )
        .unwrap();
    let chip = scene
        .node(
            scene
                .find(&Selector::id_eq("battlefield/card:session-3/status-chip"))
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
            selectors: Vec::new(),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_root = scene
        .find(&Selector::property_eq(
            "glasscheck:source_id",
            PropertyValue::string("battlefield"),
        ))
        .unwrap();
    let provider_child = scene
        .find(&Selector::property_eq(
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
    assert_eq!(
        scene
            .find_all(&Selector::property_eq(
                "glasscheck:source_id",
                PropertyValue::string("battlefield"),
            ))
            .len(),
        1
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
            selectors: Vec::new(),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProvider));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let native_handle = scene.find(&Selector::id_eq("battlefield")).unwrap();

    assert_eq!(
        scene.node(native_handle).unwrap().label.as_deref(),
        Some("Native Battlefield")
    );
    assert!(scene
        .find(&Selector::id_eq("provider::battlefield/card:session-3",))
        .is_ok());
}

fn unique_native_ids_do_not_expose_source_id_aliases(harness: AppKitHarness) {
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let native = scene.find(&Selector::id_eq("battlefield")).unwrap();

    assert_eq!(
        scene
            .node(native)
            .unwrap()
            .properties
            .get("glasscheck:source_id"),
        None
    );
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
            selectors: Vec::new(),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProviderWithSourceId));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider = scene
        .find(&Selector::property_eq(
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

fn capture_region_uses_same_provider_snapshot_as_pixels(harness: AppKitHarness) {
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Move Me"),
            None,
            None,
            harness.main_thread_marker(),
        )
    };
    button.setFrame(NSRect::new(
        NSPoint::new(260.0, 24.0),
        NSSize::new(120.0, 32.0),
    ));
    root.addSubview(&button);
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(MovingRecipeProvider { button }));
    harness.settle(2);

    let image = host
        .capture_region(&glasscheck_core::RegionSpec::node(Selector::selector_eq(
            "provider.target",
        )))
        .expect("capture should resolve the provider recipe");

    assert_eq!(image.width, 120);
    assert_eq!(image.height, 32);
    assert!(
        image.bright_pixel_fraction(0.95) < 1.0,
        "captured region should contain the moved button rather than a blank background"
    );
}

fn provider_namespacing_marks_unresolved_native_parent_reference_as_ambiguous(
    harness: AppKitHarness,
) {
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
            selectors: Vec::new(),
        },
    );
    host.set_semantic_provider(Box::new(CollidingProviderWithNativeChild));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_child = scene
        .find(&Selector::property_eq(
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
                selectors: Vec::new(),
            },
        );
    }
    host.set_semantic_provider(Box::new(CollidingProviderWithNativeChild));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_child = scene
        .find(&Selector::property_eq(
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
        .find(&Selector::id_eq("provider::parent/child"))
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
        scene.find(&Selector::ancestor(Selector::id_eq("provider::parent"))),
        Err(QueryError::NotFound(_))
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
                selectors: Vec::new(),
            },
        );
    }
    host.set_semantic_provider(Box::new(UniqueProviderWithAmbiguousNativeParent));
    harness.settle(2);

    let scene = host.snapshot_scene();
    let provider_child = scene.find(&Selector::id_eq("provider-card/label")).unwrap();

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
            selectors: Vec::new(),
        },
    );
    host.register_view(
        &second_button,
        InstrumentedView {
            id: Some("duplicate-id".into()),
            role: Some(Role::Button),
            label: Some("Second".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    host.click_node(&Selector::label(glasscheck_core::TextMatch::exact(
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    host.click_node(&Selector::id_eq("click-target")).unwrap();
    harness.settle(2);

    assert_eq!(view.ivars().mouse_downs.get(), 1);
}

fn provider_only_semantic_click_invokes_unregistered_control(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    let target = ButtonActionTarget::new(harness.main_thread_marker());
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Provider Button"),
            Some(target.as_ref()),
            Some(sel!(buttonPressed:)),
            harness.main_thread_marker(),
        )
    };
    button.setFrame(NSRect::new(
        NSPoint::new(24.0, 28.0),
        NSSize::new(120.0, 32.0),
    ));
    root.addSubview(&button);
    host.set_content_view(&root);
    host.set_semantic_provider(Box::new(ProviderControlOverlay));
    harness.settle(2);

    host.click_node(&Selector::id_eq("provider-control"))
        .unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().actions.get(), 1);
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

    let attached = harness.attach_window(host.window());
    attached.register_view(
        &stale_button,
        InstrumentedView {
            id: Some("stale-button".into()),
            role: Some(Role::Button),
            label: Some("Stale".into()),
            selectors: Vec::new(),
        },
    );
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));
    assert!(attached
        .snapshot_scene()
        .find(&Selector::id_eq("stale-button"))
        .is_ok());

    let replacement_root = make_view(harness.main_thread_marker(), NSSize::new(220.0, 140.0));
    host.set_content_view(&replacement_root);
    harness.settle(2);

    attached
        .click_node(&Selector::id_eq("provider-node"))
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    host.click_node(&Selector::id_eq("container")).unwrap();
    harness.settle(2);

    assert_eq!(child.ivars().mouse_downs.get(), 1);
    assert_eq!(parent.ivars().mouse_downs.get(), 0);
}

fn semantic_click_reports_missing_node(harness: AppKitHarness) {
    let host = harness.create_window(180.0, 120.0);
    let view = make_view(harness.main_thread_marker(), NSSize::new(180.0, 120.0));
    host.set_content_view(&view);
    let error = host.click_node(&Selector::id_eq("missing")).unwrap_err();
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(scene.find(&Selector::id_eq("clipped-target")).unwrap())
        .unwrap();
    assert_eq!(
        node.visible_rect,
        Some(Rect::new(Point::new(40.0, 40.0), Size::new(10.0, 10.0)))
    );
    assert!(node.hit_testable);

    host.click_node(&Selector::id_eq("clipped-target")).unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().mouse_downs.get(), 1);
    assert_eq!(target.ivars().last_click_x.get(), 45.0);
    assert_eq!(target.ivars().last_click_y.get(), 45.0);
}

fn semantic_click_falls_back_when_center_is_occluded(harness: AppKitHarness) {
    let host = harness.create_window(120.0, 120.0);
    let root = make_view(harness.main_thread_marker(), NSSize::new(120.0, 120.0));
    let target = PointTrackingClickView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(30.0, 30.0)),
    );
    let occluder = make_view(harness.main_thread_marker(), NSSize::new(8.0, 8.0));
    occluder.setFrameOrigin(NSPoint::new(31.0, 31.0));
    root.addSubview(&target);
    root.addSubview(&occluder);
    host.set_content_view(&root);
    host.register_view(
        &target,
        InstrumentedView {
            id: Some("partially-covered-target".into()),
            role: Some(Role::Button),
            label: Some("Partially Covered".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(
            scene
                .find(&Selector::id_eq("partially-covered-target"))
                .unwrap(),
        )
        .unwrap();
    assert!(node.hit_testable);

    host.click_node(&Selector::id_eq("partially-covered-target"))
        .unwrap();
    harness.settle(2);

    assert_eq!(target.ivars().mouse_downs.get(), 1);
    assert_ne!(target.ivars().last_click_x.get(), 35.0);
    assert_ne!(target.ivars().last_click_y.get(), 35.0);
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(scene.find(&Selector::id_eq("covered-target")).unwrap())
        .unwrap();
    assert!(!node.hit_testable);

    let error = host
        .click_node(&Selector::id_eq("covered-target"))
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let scene = host.snapshot_scene();
    let node = scene
        .node(scene.find(&Selector::id_eq("unhittable-root")).unwrap())
        .unwrap();
    assert!(!node.hit_testable);

    let error = host
        .click_node(&Selector::id_eq("unhittable-root"))
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let error = host
        .click_node(&Selector::id_eq("ancestor-hit-target"))
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let error = host
        .click_node(&Selector::id_eq("unknown-hit-target"))
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

    let attached = harness.attach_window(host.window());
    attached.register_view(
        &initial_child,
        InstrumentedView {
            id: Some("stale-dealloc-child".into()),
            role: Some(Role::Container),
            label: Some("Stale".into()),
            selectors: Vec::new(),
        },
    );
    assert!(attached
        .snapshot_scene()
        .find(&Selector::id_eq("stale-dealloc-child"))
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
        .find(&Selector::id_eq("stale-dealloc-child"))
        .unwrap_err();
    assert!(matches!(error, QueryError::NotFound(_)));
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
    let attached = harness.attach_window(host.window());
    attached.set_semantic_provider(Box::new(ProviderOnlySceneProvider));

    let error = attached
        .click_node(&Selector::id_eq("provider-node"))
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
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    for _ in 0..25 {
        host.click_node(&Selector::id_eq("click-target")).unwrap();
        harness.settle(1);
    }

    assert_eq!(target.ivars().actions.get(), 25);
}

fn context_click_node_opens_native_menu_scene_and_activates_item(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let host = harness.create_window(260.0, 180.0);
    let root = make_view(mtm, NSSize::new(260.0, 180.0));
    let inserted = Rc::new(Cell::new(false));
    let (menu, menu_target) = make_context_menu(mtm, inserted.clone());
    let editor = ContextMenuTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(160.0, 90.0)),
        menu,
        false,
    );
    root.addSubview(&editor);
    host.set_content_view(&root);
    host.register_view(
        &editor,
        InstrumentedView {
            id: Some("editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
            selectors: vec!["editor.surface".into()],
        },
    );
    host.set_scene_source(Box::new(InsertedTableScene {
        inserted: inserted.clone(),
    }));
    harness.settle(2);

    let context_menu = host
        .context_click_node(&Selector::selector_eq("editor.surface"))
        .expect("context-click should expose a native NSMenu");
    let scene = context_menu.snapshot_scene();

    let root_node = scene
        .node(scene.find(&Selector::selector_eq("context-menu")).unwrap())
        .unwrap();
    assert_eq!(root_node.role, Role::Menu);
    assert_eq!(root_node.label.as_deref(), Some("Editor Menu"));

    let insert = scene
        .node(
            scene
                .find(&Selector::label(TextMatch::exact("Insert Table")))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(insert.role, Role::MenuItem);
    assert_eq!(
        insert.state.get("enabled"),
        Some(&PropertyValue::Bool(true))
    );
    assert_eq!(
        insert.state.get("checked"),
        Some(&PropertyValue::Bool(true))
    );
    assert_eq!(
        insert.properties.get("glasscheck:key_equivalent"),
        Some(&PropertyValue::String("t".into()))
    );
    assert_eq!(
        insert.properties.get("glasscheck:key_equivalent_modifiers"),
        Some(&PropertyValue::String("command".into()))
    );

    let separator = scene
        .node(
            scene
                .find(&Selector::selector_eq("context-menu.separator[1]"))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(separator.role, Role::Divider);

    let disabled_error = context_menu
        .activate_item(&Selector::label(TextMatch::exact("Disabled Command")))
        .unwrap_err();
    assert!(matches!(
        disabled_error,
        glasscheck_appkit::AppKitContextMenuError::DisabledMenuItem
    ));
    assert!(!inserted.get());

    context_menu
        .activate_item(&Selector::label(TextMatch::exact("Insert Table")))
        .expect("enabled menu item should activate");
    harness.settle(2);

    assert!(inserted.get());
    assert_eq!(menu_target.ivars().actions.get(), 1);
    assert_eq!(editor.ivars().right_click_events.get(), 1);
    assert_eq!(editor.ivars().control_click_events.get(), 0);
    assert_eq!(editor.ivars().will_open_events.get(), 1);
    assert!(host
        .snapshot_scene()
        .find(&Selector::id_eq("inserted-table"))
        .is_ok());
}

fn context_click_node_retries_registered_owner_when_descendant_has_no_menu(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let host = harness.create_window(260.0, 180.0);
    let root = make_view(mtm, NSSize::new(260.0, 180.0));
    let (menu, _menu_target) = make_context_menu(mtm, Rc::new(Cell::new(false)));
    let editor = ContextMenuTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(20.0, 20.0), NSSize::new(160.0, 90.0)),
        menu,
        false,
    );
    let child = ContextMenuRejectingChildView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(160.0, 90.0)),
    );
    editor.addSubview(&child);
    root.addSubview(&editor);
    host.set_content_view(&root);
    host.register_view(
        &editor,
        InstrumentedView {
            id: Some("owner-editor".into()),
            role: Some(Role::TextInput),
            label: Some("Owner Editor".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let context_menu = host
        .context_click_node(&Selector::id_eq("owner-editor"))
        .expect("context click should retry the registered owner after a child returns no menu");

    assert!(context_menu
        .snapshot_scene()
        .find(&Selector::label(TextMatch::exact("Insert Table")))
        .is_ok());
    assert_eq!(child.ivars().menu_for_events.get(), 2);
    assert_eq!(editor.ivars().right_click_events.get(), 1);
    assert_eq!(editor.ivars().control_click_events.get(), 0);
    assert_eq!(editor.ivars().will_open_events.get(), 1);
}

fn context_click_node_uses_control_primary_fallback(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(mtm, NSSize::new(220.0, 140.0));
    let (menu, _menu_target) = make_context_menu(mtm, Rc::new(Cell::new(false)));
    let editor = ContextMenuTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(16.0, 16.0), NSSize::new(150.0, 80.0)),
        menu,
        true,
    );
    root.addSubview(&editor);
    host.set_content_view(&root);
    host.register_view(
        &editor,
        InstrumentedView {
            id: Some("control-editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let context_menu = host
        .context_click_node(&Selector::id_eq("control-editor"))
        .expect("context click should fall back to Control-primary semantics");

    assert!(context_menu
        .snapshot_scene()
        .find(&Selector::label(TextMatch::exact("Insert Table")))
        .is_ok());
    assert_eq!(editor.ivars().right_click_events.get(), 1);
    assert_eq!(editor.ivars().control_click_events.get(), 1);
    assert_eq!(editor.ivars().will_open_events.get(), 1);
}

fn session_context_click_node_uses_named_surface(harness: AppKitHarness) {
    let mtm = harness.main_thread_marker();
    let host = harness.create_window(220.0, 140.0);
    let root = make_view(mtm, NSSize::new(220.0, 140.0));
    let (menu, _menu_target) = make_context_menu(mtm, Rc::new(Cell::new(false)));
    let editor = ContextMenuTrackingView::new(
        mtm,
        NSRect::new(NSPoint::new(16.0, 16.0), NSSize::new(150.0, 80.0)),
        menu,
        false,
    );
    root.addSubview(&editor);
    host.set_content_view(&root);
    host.register_view(
        &editor,
        InstrumentedView {
            id: Some("session-editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
            selectors: Vec::new(),
        },
    );
    harness.settle(2);

    let point_menu = host
        .context_click_root_point(Point::new(24.0, 24.0))
        .expect("root-point context click should open the menu");
    assert!(point_menu
        .snapshot_scene()
        .find(&Selector::label(TextMatch::exact("Insert Table")))
        .is_ok());

    let session = harness.session();
    session.attach_host("editor-surface", host);
    let menu = session
        .context_click_node(
            &SurfaceId::new("editor-surface"),
            &Selector::id_eq("session-editor"),
        )
        .expect("surface should be registered")
        .expect("session context click should open the menu");

    assert!(menu
        .snapshot_scene()
        .find(&Selector::label(TextMatch::exact("Insert Table")))
        .is_ok());
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
    mouse_downs: Cell<usize>,
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

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            let next = self.ivars().mouse_downs.get() + 1;
            self.ivars().mouse_downs.set(next);
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

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    struct SolidFillView;

    impl SolidFillView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.0, 0.0, 1.0).setFill();
            NSBezierPath::fillRect(self.bounds());
        }
    }
);

impl SolidFillView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
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
    mouse_ups: Cell<usize>,
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
        fn mouse_down(&self, event: &NSEvent) {
            self.ivars()
                .mouse_downs
                .set(self.ivars().mouse_downs.get() + 1);
            unsafe {
                let () = msg_send![super(self), mouseDown: event];
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.ivars()
                .mouse_ups
                .set(self.ivars().mouse_ups.get() + 1);
            let _ = event;
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

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_ups
                .set(self.ivars().mouse_ups.get() + 1);
        }
    }
);

impl ClickTrackingChildView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickTrackingIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class!(
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClickTrackingIvars]
    struct MouseDownTrackingTextView;

    impl MouseDownTrackingTextView {
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
            self.ivars()
                .mouse_downs
                .set(self.ivars().mouse_downs.get() + 1);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_ups
                .set(self.ivars().mouse_ups.get() + 1);
        }
    }
);

impl MouseDownTrackingTextView {
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

struct ProviderControlOverlay;

struct ContextualTextSceneSource {
    view: Retained<NSTextView>,
}

struct AttachedChildCaretSceneSource {
    view: Retained<NSTextView>,
    location: usize,
}

impl SemanticProvider for ProviderOnlySceneProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![SemanticNode::new(
            "provider-node",
            Role::Container,
            Rect::new(Point::new(12.0, 16.0), Size::new(64.0, 32.0)),
        )
        .with_label("Provider Node")
        .with_selector("provider.node")
        .with_property("provider", PropertyValue::Bool(true))]
    }
}

impl SemanticProvider for ProviderControlOverlay {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![SemanticNode::new(
            "provider-control",
            Role::Button,
            Rect::new(Point::new(24.0, 28.0), Size::new(120.0, 32.0)),
        )
        .with_selector("provider.control")]
    }
}

impl AppKitSceneSource for ContextualTextSceneSource {
    fn snapshot(&self, context: &AppKitSnapshotContext<'_>) -> SemanticSnapshot {
        let text_rect = context
            .text_range_rect(&self.view, TextRange::new(0, 7))
            .expect("text geometry should resolve");
        let caret_rect = context
            .insertion_caret_rect(&self.view, 3)
            .expect("caret geometry should resolve");
        SemanticSnapshot::new(
            vec![
                SemanticNode::new("context-text", Role::TextRun, text_rect)
                    .with_selector("context.text"),
                SemanticNode::new("context-caret", Role::Marker, caret_rect)
                    .with_selector("context.caret"),
            ],
            Vec::new(),
        )
    }
}

impl AppKitSceneSource for AttachedChildCaretSceneSource {
    fn snapshot(&self, context: &AppKitSnapshotContext<'_>) -> SemanticSnapshot {
        let caret_rect = context
            .insertion_caret_rect(&self.view, self.location)
            .expect("attached child caret geometry should resolve");
        let hit_rect = Rect::new(
            Point::new(caret_rect.origin.x - 2.0, caret_rect.origin.y),
            Size::new(4.0, caret_rect.size.height.max(4.0)),
        );
        SemanticSnapshot::new(
            vec![SemanticNode::new("provider-caret", Role::Marker, hit_rect)
                .with_selector("provider.caret")],
            Vec::new(),
        )
    }
}

struct OffsetRecipeProvider;

impl SemanticProvider for OffsetRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        vec![SemanticNode::new(
            "anchor",
            Role::Container,
            Rect::new(Point::new(20.0, 20.0), Size::new(40.0, 20.0)),
        )
        .with_selector("provider.anchor")]
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "adjacent",
            Role::Button,
            glasscheck_core::RegionSpec::node(Selector::selector_eq("provider.anchor"))
                .right_of(50.0, 40.0),
        )
        .with_selector("provider.adjacent")]
    }
}

struct MovingRecipeProvider {
    button: Retained<NSButton>,
}

impl SemanticProvider for MovingRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        self.button.setFrame(NSRect::new(
            NSPoint::new(24.0, 24.0),
            NSSize::new(120.0, 32.0),
        ));
        vec![SemanticNode::new(
            "anchor",
            Role::Button,
            Rect::new(Point::new(24.0, 24.0), Size::new(120.0, 32.0)),
        )
        .with_selector("provider.anchor")]
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "captured-button",
            Role::Button,
            glasscheck_core::RegionSpec::node(Selector::selector_eq("provider.anchor")),
        )
        .with_selector("provider.target")]
    }
}

struct ResizeAwareVisualRecipeProvider {
    root: Retained<NSView>,
}

impl SemanticProvider for ResizeAwareVisualRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        let bounds = self.root.bounds();
        vec![SemanticNode::new(
            "anchor",
            Role::Image,
            Rect::new(
                Point::new((bounds.size.width - 12.0).max(0.0), 2.0),
                Size::new(12.0, 12.0),
            ),
        )
        .with_selector("provider.anchor")]
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "visual-anchor",
            Role::Image,
            glasscheck_core::RegionSpec::node(Selector::selector_eq("provider.anchor")),
        )
        .with_hit_target(
            glasscheck_core::RegionSpec::rect(Rect::new(
                Point::new(0.0, 0.0),
                Size::new(12.0, 12.0),
            ))
            .pixel_probe(PixelProbe::new(PixelMatch::new([255, 0, 0, 255], 0, 1), 1)),
        )
        .with_selector("provider.visual")]
    }
}

struct MissingAnchorRecipeProvider;

impl SemanticProvider for MissingAnchorRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        Vec::new()
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "adjacent",
            Role::Button,
            glasscheck_core::RegionSpec::node(Selector::selector_eq("provider.anchor"))
                .right_of(50.0, 40.0),
        )
        .with_selector("provider.adjacent")]
    }
}

struct ExplicitHitTargetRecipeProvider;

impl SemanticProvider for ExplicitHitTargetRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        Vec::new()
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "provider.hit-target",
            Role::Button,
            glasscheck_core::RegionSpec::rect(Rect::new(
                Point::new(10.0, 10.0),
                Size::new(60.0, 30.0),
            )),
        )
        .with_selector("provider.hit-target")
        .with_hit_target(glasscheck_core::RegionSpec::rect(Rect::new(
            Point::new(20.0, 12.0),
            Size::new(2.0, 2.0),
        )))]
    }
}

struct ExplicitHitTargetOnlyRecipeProvider;

impl SemanticProvider for ExplicitHitTargetOnlyRecipeProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        Vec::new()
    }

    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        vec![NodeRecipe::new(
            "provider.hit-target-only",
            Role::Button,
            glasscheck_core::RegionSpec::rect(Rect::new(
                Point::new(10.0, 10.0),
                Size::new(0.0, 0.0),
            )),
        )
        .with_selector("provider.hit-target-only")
        .with_hit_target(glasscheck_core::RegionSpec::rect(Rect::new(
            Point::new(20.0, 12.0),
            Size::new(2.0, 2.0),
        )))]
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

struct ContextMenuViewIvars {
    menu: Retained<NSMenu>,
    require_control: bool,
    right_click_events: Cell<usize>,
    control_click_events: Cell<usize>,
    will_open_events: Cell<usize>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ContextMenuViewIvars]
    struct ContextMenuTrackingView;

    impl ContextMenuTrackingView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(menuForEvent:))]
        fn menu_for_event(&self, event: &NSEvent) -> *mut NSMenu {
            if event.r#type() == NSEventType::RightMouseDown {
                self.ivars()
                    .right_click_events
                    .set(self.ivars().right_click_events.get() + 1);
            }
            let control_click = event.modifierFlags().contains(NSEventModifierFlags::Control);
            if control_click {
                self.ivars()
                    .control_click_events
                    .set(self.ivars().control_click_events.get() + 1);
            }
            if self.ivars().require_control && !control_click {
                return std::ptr::null_mut();
            }
            &*self.ivars().menu as *const NSMenu as *mut NSMenu
        }

        #[unsafe(method(willOpenMenu:withEvent:))]
        fn will_open_menu(&self, _menu: &NSMenu, _event: &NSEvent) {
            self.ivars()
                .will_open_events
                .set(self.ivars().will_open_events.get() + 1);
        }
    }
);

impl ContextMenuTrackingView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        menu: Retained<NSMenu>,
        require_control: bool,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ContextMenuViewIvars {
            menu,
            require_control,
            right_click_events: Cell::new(0),
            control_click_events: Cell::new(0),
            will_open_events: Cell::new(0),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

#[derive(Default)]
struct ContextMenuRejectingChildIvars {
    menu_for_events: Cell<usize>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ContextMenuRejectingChildIvars]
    struct ContextMenuRejectingChildView;

    impl ContextMenuRejectingChildView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(menuForEvent:))]
        fn menu_for_event(&self, _event: &NSEvent) -> *mut NSMenu {
            self.ivars()
                .menu_for_events
                .set(self.ivars().menu_for_events.get() + 1);
            std::ptr::null_mut()
        }
    }
);

impl ContextMenuRejectingChildView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ContextMenuRejectingChildIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

struct MenuActionIvars {
    activations: Rc<Cell<usize>>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuActionIvars]
    struct MenuActionTarget;

    impl MenuActionTarget {
        #[unsafe(method(performMenuAction:))]
        fn perform_menu_action(&self, _sender: Option<&AnyObject>) {
            self.ivars()
                .activations
                .set(self.ivars().activations.get() + 1);
        }
    }
);

impl MenuActionTarget {
    fn new(mtm: MainThreadMarker, activations: Rc<Cell<usize>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MenuActionIvars { activations });
        unsafe { msg_send![super(this), init] }
    }
}

struct MenuActionDelegateIvars {
    activations: Rc<Cell<usize>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuActionDelegateIvars]
    struct MenuActionDelegate;

    unsafe impl NSObjectProtocol for MenuActionDelegate {}

    unsafe impl NSApplicationDelegate for MenuActionDelegate {}

    impl MenuActionDelegate {
        #[unsafe(method(performMenuAction:))]
        fn perform_menu_action(&self, _sender: Option<&AnyObject>) {
            self.ivars()
                .activations
                .set(self.ivars().activations.get() + 1);
        }
    }
);

impl MenuActionDelegate {
    fn new(mtm: MainThreadMarker, activations: Rc<Cell<usize>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MenuActionDelegateIvars { activations });
        unsafe { msg_send![super(this), init] }
    }
}

struct MenuDelegateFixture {
    mtm: MainThreadMarker,
    previous_delegate: Option<Retained<ProtocolObject<dyn NSApplicationDelegate>>>,
    activations: Rc<Cell<usize>>,
    #[allow(dead_code)]
    delegate: Retained<MenuActionDelegate>,
}

impl MenuDelegateFixture {
    fn install(mtm: MainThreadMarker) -> Self {
        let app = NSApplication::sharedApplication(mtm);
        let previous_delegate = app.delegate();
        let activations = Rc::new(Cell::new(0));
        let delegate = MenuActionDelegate::new(mtm, activations.clone());
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        Self {
            mtm,
            previous_delegate,
            activations,
            delegate,
        }
    }
}

impl Drop for MenuDelegateFixture {
    fn drop(&mut self) {
        let app = NSApplication::sharedApplication(self.mtm);
        app.setDelegate(self.previous_delegate.as_deref());
    }
}

struct MenuFixture {
    mtm: MainThreadMarker,
    previous_main_menu: Option<Retained<NSMenu>>,
    activations: Rc<Cell<usize>>,
    #[allow(dead_code)]
    target: Retained<MenuActionTarget>,
    #[allow(dead_code)]
    main_menu: Retained<NSMenu>,
}

struct ClearedMenuFixture {
    mtm: MainThreadMarker,
    previous_main_menu: Option<Retained<NSMenu>>,
}

impl ClearedMenuFixture {
    fn install(mtm: MainThreadMarker) -> Self {
        let app = NSApplication::sharedApplication(mtm);
        let previous_main_menu = app.mainMenu();
        app.setMainMenu(None);
        Self {
            mtm,
            previous_main_menu,
        }
    }
}

impl Drop for ClearedMenuFixture {
    fn drop(&mut self) {
        let app = NSApplication::sharedApplication(self.mtm);
        app.setMainMenu(self.previous_main_menu.as_deref());
    }
}

impl MenuFixture {
    fn install(mtm: MainThreadMarker) -> Self {
        let app = NSApplication::sharedApplication(mtm);
        let previous_main_menu = app.mainMenu();
        let activations = Rc::new(Cell::new(0));
        let target = MenuActionTarget::new(mtm, activations.clone());
        let main_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Main"));
        main_menu.setAutoenablesItems(false);

        let file_item = make_menu_item(mtm, "File", None, "", None);
        let file_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("File"));
        file_menu.setAutoenablesItems(false);
        main_menu.addItem(&file_item);
        main_menu.setSubmenu_forItem(Some(&file_menu), &file_item);

        let new_item = make_menu_item(
            mtm,
            "New",
            Some(target.as_ref()),
            "n",
            Some(sel!(performMenuAction:)),
        );
        file_menu.addItem(&new_item);

        let responder_item = make_menu_item(
            mtm,
            "Responder Action",
            None,
            "",
            Some(sel!(performMenuAction:)),
        );
        file_menu.addItem(&responder_item);

        let open_item = make_menu_item(
            mtm,
            "Open...",
            Some(target.as_ref()),
            "o",
            Some(sel!(performMenuAction:)),
        );
        open_item.setEnabled(false);
        file_menu.addItem(&open_item);

        let missing_target_item = make_menu_item(
            mtm,
            "Missing Target",
            None,
            "",
            Some(sel!(missingMenuAction:)),
        );
        file_menu.addItem(&missing_target_item);

        file_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let autosave_item = make_menu_item(
            mtm,
            "Autosave",
            Some(target.as_ref()),
            "",
            Some(sel!(performMenuAction:)),
        );
        autosave_item.setState(NSControlStateValueOn);
        file_menu.addItem(&autosave_item);

        let mixed_item = make_menu_item(
            mtm,
            "Mixed State",
            Some(target.as_ref()),
            "",
            Some(sel!(performMenuAction:)),
        );
        mixed_item.setState(NSControlStateValueMixed);
        file_menu.addItem(&mixed_item);

        let secret_item = make_menu_item(
            mtm,
            "Secret",
            Some(target.as_ref()),
            "",
            Some(sel!(performMenuAction:)),
        );
        secret_item.setHidden(true);
        file_menu.addItem(&secret_item);

        let export_item = make_menu_item(mtm, "Export", None, "", None);
        let export_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Export"));
        export_menu.setAutoenablesItems(false);
        let pdf_item = make_menu_item(
            mtm,
            "PDF",
            Some(target.as_ref()),
            "",
            Some(sel!(performMenuAction:)),
        );
        export_menu.addItem(&pdf_item);
        file_menu.addItem(&export_item);
        file_menu.setSubmenu_forItem(Some(&export_menu), &export_item);

        let duplicate_a = make_menu_item(
            mtm,
            "Duplicate",
            Some(target.as_ref()),
            "",
            Some(sel!(performMenuAction:)),
        );
        let duplicate_b = make_menu_item(
            mtm,
            "Duplicate",
            Some(target.as_ref()),
            "",
            Some(sel!(performMenuAction:)),
        );
        file_menu.addItem(&duplicate_a);
        file_menu.addItem(&duplicate_b);

        let edit_item = make_menu_item(mtm, "Edit", None, "", None);
        let edit_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Edit"));
        main_menu.addItem(&edit_item);
        main_menu.setSubmenu_forItem(Some(&edit_menu), &edit_item);

        let leaf_item = make_menu_item(mtm, "Leaf Top", None, "", None);
        main_menu.addItem(&leaf_item);

        let disabled_item = make_menu_item(mtm, "Disabled Top", None, "", None);
        let disabled_menu =
            NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Disabled Top"));
        disabled_item.setEnabled(false);
        main_menu.addItem(&disabled_item);
        main_menu.setSubmenu_forItem(Some(&disabled_menu), &disabled_item);

        let hidden_item = make_menu_item(mtm, "Hidden Top", None, "", None);
        let hidden_menu =
            NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Hidden Top"));
        hidden_item.setHidden(true);
        main_menu.addItem(&hidden_item);
        main_menu.setSubmenu_forItem(Some(&hidden_menu), &hidden_item);

        app.setMainMenu(Some(&main_menu));

        Self {
            mtm,
            previous_main_menu,
            activations,
            target,
            main_menu,
        }
    }
}

impl Drop for MenuFixture {
    fn drop(&mut self) {
        let app = NSApplication::sharedApplication(self.mtm);
        app.setMainMenu(self.previous_main_menu.as_deref());
    }
}

fn make_menu_item(
    mtm: MainThreadMarker,
    title: &str,
    target: Option<&MenuActionTarget>,
    key_equivalent: &str,
    action: Option<objc2::runtime::Sel>,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key_equivalent),
        )
    };
    if let Some(target) = target {
        unsafe {
            item.setTarget(Some(target));
        }
    }
    if !key_equivalent.is_empty() {
        item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
    }
    item
}

struct ContextMenuActionIvars {
    inserted: Rc<Cell<bool>>,
    actions: Cell<usize>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ContextMenuActionIvars]
    struct ContextMenuActionTarget;

    impl ContextMenuActionTarget {
        #[unsafe(method(insertTable:))]
        fn insert_table(&self, _sender: Option<&AnyObject>) {
            self.ivars().inserted.set(true);
            self.ivars().actions.set(self.ivars().actions.get() + 1);
        }
    }
);

impl ContextMenuActionTarget {
    fn new(mtm: MainThreadMarker, inserted: Rc<Cell<bool>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ContextMenuActionIvars {
            inserted,
            actions: Cell::new(0),
        });
        unsafe { msg_send![super(this), init] }
    }
}

fn make_context_menu(
    mtm: MainThreadMarker,
    inserted: Rc<Cell<bool>>,
) -> (Retained<NSMenu>, Retained<ContextMenuActionTarget>) {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Editor Menu"));
    menu.setAutoenablesItems(false);
    let target = ContextMenuActionTarget::new(mtm, inserted);
    let insert = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Insert Table"),
            Some(sel!(insertTable:)),
            &NSString::from_str("t"),
        )
    };
    unsafe { insert.setTarget(Some(target.as_ref())) };
    insert.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
    insert.setState(NSControlStateValueOn);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let disabled = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Disabled Command"),
            Some(sel!(insertTable:)),
            &NSString::from_str(""),
        )
    };
    unsafe { disabled.setTarget(Some(target.as_ref())) };
    disabled.setEnabled(false);

    (menu, target)
}

struct InsertedTableScene {
    inserted: Rc<Cell<bool>>,
}

impl SemanticProvider for InsertedTableScene {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        self.inserted
            .get()
            .then(|| {
                vec![SemanticNode::new(
                    "inserted-table",
                    Role::Container,
                    Rect::new(Point::new(24.0, 40.0), Size::new(220.0, 120.0)),
                )]
            })
            .unwrap_or_default()
    }
}

struct ChildWindowOpenIvars {
    child_window: Retained<objc2_app_kit::NSWindow>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ChildWindowOpenIvars]
    struct ChildWindowOpenTarget;

    impl ChildWindowOpenTarget {
        #[unsafe(method(openChildWindow:))]
        fn open_child_window(&self, sender: Option<&NSButton>) {
            let sender = sender.expect("popover opener should be a button");
            let parent = sender.window().expect("opener button should belong to a window");
            unsafe {
                parent.addChildWindow_ordered(
                    &self.ivars().child_window,
                    NSWindowOrderingMode::Above,
                );
            }
            self.ivars().child_window.orderFrontRegardless();
        }
    }
);

impl ChildWindowOpenTarget {
    fn new(mtm: MainThreadMarker, child_window: &objc2_app_kit::NSWindow) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ChildWindowOpenIvars {
            child_window: unsafe {
                Retained::retain(child_window as *const _ as *mut _)
                    .expect("child window opener should retain successfully")
            },
        });
        unsafe { msg_send![super(this), init] }
    }
}

struct ChildWindowInsertIvars {
    child_window: Retained<objc2_app_kit::NSWindow>,
    inserted: Rc<Cell<bool>>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ChildWindowInsertIvars]
    struct ChildWindowInsertTarget;

    impl ChildWindowInsertTarget {
        #[unsafe(method(buttonPressed:))]
        fn button_pressed(&self, _sender: Option<&NSButton>) {
            self.ivars().inserted.set(true);
            if let Some(parent) = self.ivars().child_window.parentWindow() {
                parent.removeChildWindow(&self.ivars().child_window);
            }
            self.ivars().child_window.orderOut(None);
            self.ivars().child_window.close();
        }
    }
);

impl ChildWindowInsertTarget {
    fn new(
        mtm: MainThreadMarker,
        inserted: Rc<Cell<bool>>,
        child_window: &objc2_app_kit::NSWindow,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ChildWindowInsertIvars {
            child_window: unsafe {
                Retained::retain(child_window as *const _ as *mut _)
                    .expect("child window insert target should retain successfully")
            },
            inserted,
        });
        unsafe { msg_send![super(this), init] }
    }
}

const PICKER_ROWS: usize = 4;
const PICKER_COLS: usize = 4;
const PICKER_CELL_SIZE: f64 = 24.0;
const PICKER_GRID_ORIGIN_X: f64 = 16.0;
const PICKER_GRID_ORIGIN_Y: f64 = 40.0;
const PICKER_LABEL_ORIGIN_X: f64 = 16.0;
const PICKER_LABEL_ORIGIN_Y: f64 = 12.0;
const PICKER_LABEL_WIDTH: f64 = 72.0;
const PICKER_LABEL_HEIGHT: f64 = 20.0;
const PICKER_WINDOW_WIDTH: f64 = 160.0;
const PICKER_WINDOW_HEIGHT: f64 = 160.0;

#[derive(Clone, Copy)]
struct PickerState {
    hovered_row: usize,
    hovered_col: usize,
}

impl Default for PickerState {
    fn default() -> Self {
        Self {
            hovered_row: 1,
            hovered_col: 1,
        }
    }
}

impl PickerState {
    fn label(self) -> String {
        format!("{} × {}", self.hovered_row, self.hovered_col)
    }
}

struct PickerSceneProvider {
    state: Rc<RefCell<PickerState>>,
}

impl SemanticProvider for PickerSceneProvider {
    fn snapshot_nodes(&self) -> Vec<SemanticNode> {
        let state = *self.state.borrow();
        let mut nodes = vec![
            SemanticNode::new(
                "table.picker",
                Role::Container,
                Rect::new(
                    Point::new(0.0, 0.0),
                    Size::new(PICKER_WINDOW_WIDTH, PICKER_WINDOW_HEIGHT),
                ),
            )
            .with_selector("table.picker"),
            SemanticNode::new(
                "table.picker.label",
                Role::Label,
                Rect::new(
                    Point::new(PICKER_LABEL_ORIGIN_X, PICKER_LABEL_ORIGIN_Y),
                    Size::new(PICKER_LABEL_WIDTH, PICKER_LABEL_HEIGHT),
                ),
            )
            .with_selector("table.picker.label")
            .with_property("text", PropertyValue::string(state.label())),
        ];

        for row in 1..=PICKER_ROWS {
            for col in 1..=PICKER_COLS {
                let rect = Rect::new(
                    Point::new(
                        PICKER_GRID_ORIGIN_X + ((col - 1) as f64 * PICKER_CELL_SIZE),
                        PICKER_GRID_ORIGIN_Y + ((row - 1) as f64 * PICKER_CELL_SIZE),
                    ),
                    Size::new(PICKER_CELL_SIZE, PICKER_CELL_SIZE),
                );
                nodes.push(
                    SemanticNode::new(format!("table.picker.cell.{row}.{col}"), Role::Button, rect)
                        .with_selector("table.picker.cell")
                        .with_property("rows", PropertyValue::Integer(row as i64))
                        .with_property("cols", PropertyValue::Integer(col as i64))
                        .with_state(
                            "highlighted",
                            PropertyValue::Bool(
                                state.hovered_row == row && state.hovered_col == col,
                            ),
                        ),
                );
            }
        }

        nodes
    }
}

struct PickerTrackingOwnerIvars {
    container: Retained<NSView>,
    state: Rc<RefCell<PickerState>>,
    mouse_moved_calls: Rc<Cell<usize>>,
    mouse_exited_calls: Rc<Cell<usize>>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PickerTrackingOwnerIvars]
    struct PickerTrackingOwner;

    impl PickerTrackingOwner {
        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.ivars()
                .mouse_moved_calls
                .set(self.ivars().mouse_moved_calls.get() + 1);
            let window_point = event.locationInWindow();
            let local_point = self
                .ivars()
                .container
                .convertPoint_fromView(window_point, None);
            if let Some((row, col)) = picker_cell_at_point(local_point) {
                *self.ivars().state.borrow_mut() = PickerState {
                    hovered_row: row,
                    hovered_col: col,
                };
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars()
                .mouse_exited_calls
                .set(self.ivars().mouse_exited_calls.get() + 1);
            *self.ivars().state.borrow_mut() = PickerState::default();
        }
    }
);

impl PickerTrackingOwner {
    fn new(
        mtm: MainThreadMarker,
        container: &NSView,
        state: Rc<RefCell<PickerState>>,
        mouse_moved_calls: Rc<Cell<usize>>,
        mouse_exited_calls: Rc<Cell<usize>>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PickerTrackingOwnerIvars {
            container: unsafe {
                Retained::retain(container as *const _ as *mut _)
                    .expect("tracking owner should retain the picker container")
            },
            state,
            mouse_moved_calls,
            mouse_exited_calls,
        });
        unsafe { msg_send![super(this), init] }
    }
}

struct PickerContractFixture {
    session: glasscheck_appkit::AppKitSession,
    tracking_mouse_moved_calls: Rc<Cell<usize>>,
    tracking_mouse_exited_calls: Rc<Cell<usize>>,
    _picker_window: Retained<NSWindow>,
    _picker_container: Retained<NSView>,
    _tracking_area: Retained<NSTrackingArea>,
    _tracking_owner: Retained<PickerTrackingOwner>,
    _opener_target: Retained<ChildWindowOpenTarget>,
}

impl PickerContractFixture {
    fn tracking_mouse_moved_count(&self) -> usize {
        self.tracking_mouse_moved_calls.get()
    }

    fn tracking_mouse_exited_count(&self) -> usize {
        self.tracking_mouse_exited_calls.get()
    }
}

struct BackgroundPickerFixture {
    host: glasscheck_appkit::AppKitWindowHost,
    inserted_dims: Rc<RefCell<Option<(usize, usize)>>>,
    tracking_mouse_moved_calls: Rc<Cell<usize>>,
    monitor: Option<Retained<AnyObject>>,
    _tracking_area: Retained<NSTrackingArea>,
    _tracking_owner: Retained<PickerTrackingOwner>,
}

impl BackgroundPickerFixture {
    fn host(&self) -> &glasscheck_appkit::AppKitWindowHost {
        &self.host
    }

    fn window(&self) -> &NSWindow {
        self.host.window()
    }

    fn inserted_dims(&self) -> Option<(usize, usize)> {
        *self.inserted_dims.borrow()
    }

    fn tracking_mouse_moved_count(&self) -> usize {
        self.tracking_mouse_moved_calls.get()
    }
}

impl Drop for BackgroundPickerFixture {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }
}

fn open_picker_contract_fixture(harness: AppKitHarness) -> PickerContractFixture {
    let mtm = harness.main_thread_marker();
    let picker_state = Rc::new(RefCell::new(PickerState::default()));
    let tracking_mouse_moved_calls = Rc::new(Cell::new(0));
    let tracking_mouse_exited_calls = Rc::new(Cell::new(0));
    let host = harness.create_window(320.0, 220.0);
    let picker = harness.create_window(PICKER_WINDOW_WIDTH, PICKER_WINDOW_HEIGHT);
    let root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 220.0)),
    );
    let picker_root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(PICKER_WINDOW_WIDTH, PICKER_WINDOW_HEIGHT),
        ),
    );
    picker.set_content_view(&picker_root);

    let opener_target = ChildWindowOpenTarget::new(mtm, picker.window());
    let opener = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Open Picker"),
            Some(&*opener_target),
            Some(sel!(openChildWindow:)),
            mtm,
        )
    };
    opener.setFrame(NSRect::new(
        NSPoint::new(28.0, 140.0),
        NSSize::new(120.0, 32.0),
    ));
    root.addSubview(&opener);
    host.set_content_view(&root);
    host.register_node(
        &opener,
        InstrumentedView {
            id: Some("open-picker".into()),
            role: Some(Role::Button),
            label: Some("Open Picker".into()),
            ..Default::default()
        },
    );
    harness.settle(2);

    let session = harness.session();
    session.attach_host("main", host);
    session
        .click_node(&SurfaceId::new("main"), &Selector::id_eq("open-picker"))
        .expect("main surface should be attached")
        .expect("picker opener click should succeed");
    picker.window().makeKeyAndOrderFront(None);
    harness.settle(2);
    assert!(
        picker.window().parentWindow().is_some(),
        "picker opener should parent the child window before attachment"
    );
    session.attach_window("picker", picker.window());
    let tracking_owner = PickerTrackingOwner::new(
        mtm,
        &picker_root,
        picker_state.clone(),
        tracking_mouse_moved_calls.clone(),
        tracking_mouse_exited_calls.clone(),
    );
    let tracking_options = NSTrackingAreaOptions::MouseMoved | NSTrackingAreaOptions::ActiveAlways;
    let owner_object: &AnyObject = unsafe { &*(std::ptr::from_ref(&*tracking_owner).cast()) };
    let tracking_area = unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            NSTrackingArea::alloc(),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(PICKER_WINDOW_WIDTH, PICKER_WINDOW_HEIGHT),
            ),
            tracking_options,
            Some(owner_object),
            None,
        )
    };
    picker_root.addTrackingArea(&tracking_area);
    session.with_surface(&SurfaceId::new("picker"), |picker_host| {
        picker_host.set_scene_source(Box::new(PickerSceneProvider {
            state: picker_state.clone(),
        }));
        picker_host.window().setAcceptsMouseMovedEvents(true);
    });
    harness.settle(2);

    PickerContractFixture {
        session,
        tracking_mouse_moved_calls,
        tracking_mouse_exited_calls,
        _picker_window: unsafe {
            Retained::retain(picker.window() as *const _ as *mut _)
                .expect("picker fixture should retain the picker window")
        },
        _picker_container: unsafe {
            Retained::retain(&*picker_root as *const _ as *mut _)
                .expect("picker fixture should retain the picker container")
        },
        _tracking_area: tracking_area,
        _tracking_owner: tracking_owner,
        _opener_target: opener_target,
    }
}

fn open_background_picker_fixture(harness: AppKitHarness) -> BackgroundPickerFixture {
    let mtm = harness.main_thread_marker();
    let picker_state = Rc::new(RefCell::new(PickerState::default()));
    let inserted_dims = Rc::new(RefCell::new(None));
    let tracking_mouse_moved_calls = Rc::new(Cell::new(0));
    let tracking_mouse_exited_calls = Rc::new(Cell::new(0));

    let host = harness.create_window(PICKER_WINDOW_WIDTH, PICKER_WINDOW_HEIGHT);
    let root = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(PICKER_WINDOW_WIDTH, PICKER_WINDOW_HEIGHT),
        ),
    );
    host.set_content_view(&root);
    host.set_scene_source(Box::new(PickerSceneProvider {
        state: picker_state.clone(),
    }));

    let tracking_owner = PickerTrackingOwner::new(
        mtm,
        &root,
        picker_state,
        tracking_mouse_moved_calls.clone(),
        tracking_mouse_exited_calls,
    );
    let tracking_options = NSTrackingAreaOptions::MouseMoved
        | NSTrackingAreaOptions::MouseEnteredAndExited
        | NSTrackingAreaOptions::ActiveInActiveApp
        | NSTrackingAreaOptions::InVisibleRect;
    let owner_object: &AnyObject = unsafe { &*(std::ptr::from_ref(&*tracking_owner).cast()) };
    let tracking_area = unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            NSTrackingArea::alloc(),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            tracking_options,
            Some(owner_object),
            None,
        )
    };
    root.addTrackingArea(&tracking_area);

    let window_number = host.window().windowNumber();
    let inserted_dims_for_monitor = inserted_dims.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        if event_ref.windowNumber() != window_number {
            return event.as_ptr();
        }
        let Some((row, col)) = picker_cell_at_point(event_ref.locationInWindow()) else {
            return event.as_ptr();
        };
        *inserted_dims_for_monitor.borrow_mut() = Some((row, col));
        std::ptr::null_mut()
    });
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseUp, &block)
    };

    harness.settle(2);

    BackgroundPickerFixture {
        host,
        inserted_dims,
        tracking_mouse_moved_calls,
        monitor,
        _tracking_area: tracking_area,
        _tracking_owner: tracking_owner,
    }
}

fn picker_cell_at_point(point: NSPoint) -> Option<(usize, usize)> {
    let relative_x = point.x - PICKER_GRID_ORIGIN_X;
    let relative_y = point.y - PICKER_GRID_ORIGIN_Y;
    if relative_x < 0.0 || relative_y < 0.0 {
        return None;
    }

    let col = (relative_x / PICKER_CELL_SIZE).floor() as usize + 1;
    let row = (relative_y / PICKER_CELL_SIZE).floor() as usize + 1;
    (row <= PICKER_ROWS && col <= PICKER_COLS).then_some((row, col))
}

fn assert_highlighted_picker_cell(scene: &glasscheck_core::Scene, row: usize, col: usize) {
    let handle = scene
        .find(&Selector::id_eq(&format!("table.picker.cell.{row}.{col}")))
        .expect("picker cell should resolve");
    let node = scene.node(handle).expect("picker cell should exist");
    assert_eq!(
        node.state.get("highlighted"),
        Some(&PropertyValue::Bool(true)),
        "expected table.picker.cell.{row}.{col} to be highlighted"
    );
}

fn assert_picker_label(scene: &glasscheck_core::Scene, text: &str) {
    let handle = scene
        .find(&Selector::id_eq("table.picker.label"))
        .expect("picker label should resolve");
    let node = scene.node(handle).expect("picker label should exist");
    assert_eq!(
        node.properties.get("text"),
        Some(&PropertyValue::string(text)),
        "expected picker label text to match"
    );
}

fn key_press_queued_reaches_local_key_down_monitor(harness: AppKitHarness) {
    struct KeyMonitorGuard(Option<Retained<AnyObject>>);
    impl Drop for KeyMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let host = harness.create_window(180.0, 120.0);
    let view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.window().makeFirstResponder(Some(&*view));
    harness.settle(2);

    let monitor_count = Rc::new(Cell::new(0usize));
    let count_ref = monitor_count.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        count_ref.set(count_ref.get() + 1);
        event.as_ptr()
    });
    let _monitor = KeyMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
    });

    host.input()
        .key_press_queued("a", glasscheck_core::KeyModifiers::default())
        .expect("queued key press should succeed");
    harness.settle(2);

    assert_eq!(
        monitor_count.get(),
        1,
        "local key-down monitor should see the queued event exactly once"
    );
}

fn key_press_queued_reaches_local_key_up_monitor(harness: AppKitHarness) {
    struct KeyMonitorGuard(Option<Retained<AnyObject>>);
    impl Drop for KeyMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let host = harness.create_window(180.0, 120.0);
    let view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.window().makeFirstResponder(Some(&*view));
    harness.settle(2);

    let monitor_count = Rc::new(Cell::new(0usize));
    let count_ref = monitor_count.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        count_ref.set(count_ref.get() + 1);
        event.as_ptr()
    });
    let _monitor = KeyMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyUp, &block)
    });

    host.input()
        .key_press_queued("a", glasscheck_core::KeyModifiers::default())
        .expect("queued key press should succeed");
    harness.settle(2);

    assert_eq!(
        monitor_count.get(),
        1,
        "local key-up monitor should see the paired queued up event"
    );
}

fn key_press_queued_delivers_to_first_responder_after_monitor(harness: AppKitHarness) {
    struct KeyMonitorGuard(Option<Retained<AnyObject>>);
    impl Drop for KeyMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let host = harness.create_window(180.0, 120.0);
    let view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.window().makeFirstResponder(Some(&*view));
    harness.settle(2);

    let monitor_count = Rc::new(Cell::new(0usize));
    let count_ref = monitor_count.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        count_ref.set(count_ref.get() + 1);
        event.as_ptr()
    });
    let _monitor = KeyMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
    });

    host.input()
        .key_press_queued("a", glasscheck_core::KeyModifiers::default())
        .expect("queued key press should succeed");
    harness.settle(2);

    assert_eq!(
        monitor_count.get(),
        1,
        "local monitor should observe the queued key-down event"
    );
    assert_eq!(
        view.ivars().key_downs.get(),
        1,
        "first responder should receive keyDown: after the monitor"
    );
}

fn key_press_direct_bypasses_local_key_monitor(harness: AppKitHarness) {
    struct KeyMonitorGuard(Option<Retained<AnyObject>>);
    impl Drop for KeyMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let host = harness.create_window(180.0, 120.0);
    let view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.window().makeFirstResponder(Some(&*view));
    harness.settle(2);

    let monitor_count = Rc::new(Cell::new(0usize));
    let count_ref = monitor_count.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        count_ref.set(count_ref.get() + 1);
        event.as_ptr()
    });
    let _monitor = KeyMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
    });

    host.input()
        .key_press("a", glasscheck_core::KeyModifiers::default())
        .unwrap();
    harness.settle(2);

    assert_eq!(
        monitor_count.get(),
        0,
        "direct key_press should bypass local monitors (regression guard)"
    );
}

fn key_press_queued_carries_modifier_flags(harness: AppKitHarness) {
    struct KeyMonitorGuard(Option<Retained<AnyObject>>);
    impl Drop for KeyMonitorGuard {
        fn drop(&mut self) {
            if let Some(monitor) = self.0.take() {
                unsafe { NSEvent::removeMonitor(&monitor) };
            }
        }
    }

    let host = harness.create_window(180.0, 120.0);
    let view = RoutingTrackingView::new(
        harness.main_thread_marker(),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(180.0, 120.0)),
    );
    host.set_content_view(&view);
    host.window().makeFirstResponder(Some(&*view));
    harness.settle(2);

    let saw_control = Rc::new(Cell::new(false));
    let saw_ref = saw_control.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let flags = unsafe { event.as_ref() }.modifierFlags();
        if flags.contains(NSEventModifierFlags::Control) {
            saw_ref.set(true);
        }
        event.as_ptr()
    });
    let _monitor = KeyMonitorGuard(unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
    });

    host.input()
        .key_press_queued(
            "a",
            glasscheck_core::KeyModifiers {
                control: true,
                ..glasscheck_core::KeyModifiers::default()
            },
        )
        .expect("queued key press with modifier should succeed");
    harness.settle(2);

    assert!(
        saw_control.get(),
        "queued key event should carry the Control modifier flag"
    );
}
