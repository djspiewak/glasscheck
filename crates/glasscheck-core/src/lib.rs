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
    assert_above, assert_contained_within, assert_left_of, assert_non_overlapping,
    assert_vertical_alignment, LayoutError, LayoutTolerance,
};
pub use query::{
    NodeMetadata, NodePredicate, PropertyValue, QueryError, QueryRoot, Selector, TextMatch,
};
pub use scene::{NodeHandle, Role, SceneSnapshot, SemanticNode, SemanticProvider};
pub use text::{
    assert_text_renders, compare_rendered_text, font_expectation_has_conflict,
    AnchoredTextExpectation, RgbaColor, TextAssertionArtifacts, TextAssertionConfig,
    TextAssertionError, TextExpectation, TextRenderer,
};
pub use wait::{
    wait_for_condition, wait_for_image_stability, wait_for_scene_stability, PollError, PollOptions,
};
