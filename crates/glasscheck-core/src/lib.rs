//! Core primitives for stable functional testing of native Rust UIs.
//!
//! This crate provides portable geometry, image comparison, semantic query,
//! text assertion, and polling utilities. Platform-specific crates can build
//! on these APIs to offer in-process UI testing without external automation.

mod anchor;
mod assertions;
mod backend;
mod geometry;
mod image;
mod layout;
mod query;
mod scene;
mod semantic;
mod text;
mod wait;

pub use anchor::{Anchor, RegionResolveError, RegionSpec, RelativeBounds};
pub use assertions::{
    assert_snapshot_matches, compare_images, load_png, save_png, CompareConfig, CompareResult,
    SnapshotArtifacts, SnapshotConfig, SnapshotError,
};
pub use backend::{
    assert_anchored_text_renders, crop_image_bottom_left, normalize_provider_nodes,
    registered_node_id, AnchoredTextAssertionError, AnchoredTextHarness, Harness, InputDriver,
    InstrumentedNode, KeyModifiers, TextRange,
};
pub use geometry::{Point, Rect, Size};
pub use image::Image;
pub use layout::{
    assert_above, assert_adjacent_horizontally, assert_adjacent_vertically,
    assert_contained_within, assert_contains_point, assert_horizontal_alignment, assert_left_of,
    assert_non_overlapping, assert_same_height, assert_same_width, assert_vertical_alignment,
    LayoutError, LayoutTolerance,
};
pub use query::{
    NodeMetadata, NodePredicate, PropertyValue, QueryError, QueryRoot, Selector, TextMatch,
};
pub use scene::{NodeHandle, Role, SceneSnapshot, SemanticNode, SemanticProvider};
pub use semantic::{
    assert_above_node, assert_adjacent_horizontally_node, assert_adjacent_vertically_node,
    assert_contained_within_node, assert_contains_point_node, assert_count, assert_exists,
    assert_hit_testable, assert_horizontally_aligned_node, assert_interactable,
    assert_left_of_node, assert_node_appeared, assert_node_disappeared, assert_node_moved,
    assert_not_exists, assert_property, assert_same_height_node, assert_same_width_node,
    assert_state, assert_state_changed, assert_unique, assert_vertically_aligned_node,
    assert_visible, diff_scenes, Interactability, NodeChange, QueryMatch, ReorderChange,
    ResolvedNode, SceneDelta, SemanticAssertionError,
};
pub use text::{
    assert_text_renders, compare_rendered_text, font_expectation_has_conflict,
    AnchoredTextExpectation, RgbaColor, TextAssertionArtifacts, TextAssertionConfig,
    TextAssertionError, TextExpectation, TextRenderer,
};
pub use wait::{
    wait_for_absent, wait_for_bounds, wait_for_condition, wait_for_count, wait_for_exists,
    wait_for_hit_testable, wait_for_image_stability, wait_for_interactable, wait_for_property,
    wait_for_scene_stability, wait_for_state, wait_for_visible, PollError, PollOptions, WaitError,
};
