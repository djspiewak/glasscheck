//! Backend-neutral primitives for functional testing of graphical native Rust UIs.
//!
//! Use this crate when tests can expose scene nodes, capture pixels, or both.
//! Nodes may come from native widget/view registration, declarative recipes,
//! coarse geometry, pixel probing, fuzzy image matching, or custom refinement.
//! This crate targets native graphical UIs rather than browser-based UIs. Most
//! users should depend on `glasscheck` instead so they get the supported native
//! backend integration as well.

mod anchor;
mod assertions;
mod backend;
mod color_match;
mod geometry;
mod image;
mod layout;
mod query;
mod scene;
mod semantic;
mod text;
mod wait;

pub use anchor::RegionSpec as RegionLocator;
pub use anchor::{
    AbsoluteBounds, Anchor, ImageMatch, PixelMatch, PixelProbe, RegionProbe, RegionRefiner,
    RegionResolveError, RegionSpec, RelativeBounds,
};
pub use assertions::{
    assert_snapshot_matches, compare_images, load_png, save_png, CompareConfig, CompareResult,
    SnapshotArtifacts, SnapshotConfig, SnapshotError,
};
pub use backend::{
    assert_anchored_text_renders, crop_image_bottom_left, normalize_provider_nodes,
    registered_node_id, AnchoredTextAssertionError, AnchoredTextHarness, Harness, HitPointSearch,
    HitPointStrategy, InputDriver, InputSynthesisError, InstrumentedNode, KeyModifiers, TextRange,
};
pub use geometry::{Point, Rect, Size};
pub use image::Image;
pub use layout::{
    assert_above, assert_adjacent_horizontally, assert_adjacent_vertically,
    assert_contained_within, assert_contains_point, assert_horizontal_alignment, assert_left_of,
    assert_non_overlapping, assert_same_height, assert_same_width, assert_vertical_alignment,
    LayoutError, LayoutTolerance,
};
pub use query::{PropertyValue, QueryError, Selector, TextMatch};
pub use scene::{
    resolve_node_recipes, NodeHandle, NodeProvenanceKind, NodeRecipe, NodeRecipeResolutionError,
    ResolvedNodeRecipes, Role, Scene, SemanticNode, SemanticProvider,
};
pub use scene::{SemanticNode as Node, SemanticProvider as SceneSource};
pub use semantic::SemanticAssertionError as NodeAssertionError;
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
