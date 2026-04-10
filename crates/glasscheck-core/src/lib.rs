//! Core primitives for stable functional testing of native Rust UIs.
//!
//! This crate provides portable geometry, image comparison, semantic query,
//! text assertion, and polling utilities. Platform-specific crates can build
//! on these APIs to offer in-process UI testing without external automation.

mod anchor;
mod assertions;
mod geometry;
mod image;
mod query;
mod text;
mod wait;

pub use anchor::{
    Anchor, NodePredicate, RegionResolveError, RegionSpec, RelativeBounds, TextMatch,
};
pub use assertions::{
    assert_snapshot_matches, compare_images, load_png, save_png, CompareConfig, CompareResult,
    SnapshotArtifacts, SnapshotConfig, SnapshotError,
};
pub use geometry::{Point, Rect, Size};
pub use image::Image;
pub use query::{NodeMetadata, QueryError, QueryRoot, Role, Selector};
pub use text::{
    assert_text_renders, compare_rendered_text, AnchoredTextExpectation, RgbaColor,
    TextAssertionArtifacts, TextAssertionConfig, TextAssertionError, TextExpectation, TextRenderer,
};
pub use wait::{wait_for_condition, wait_for_image_stability, PollError, PollOptions};
