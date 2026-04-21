use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use crate::{
    wait_for_condition, AnchoredTextExpectation, Image, Point, PollError, PollOptions,
    PropertyValue, Rect, RegionResolveError, RegionSpec, Role, SemanticNode, TextAssertionConfig,
    TextAssertionError, TextRenderer,
};

/// Shared semantic metadata registered for a native node exposed to queries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstrumentedNode {
    /// Stable semantic identifier.
    pub id: Option<String>,
    /// Semantic role.
    pub role: Option<Role>,
    /// Human-readable label.
    pub label: Option<String>,
    /// Stable test-facing selectors or aliases.
    pub selectors: Vec<String>,
    /// Structured state exposed on the native node.
    pub state: BTreeMap<String, PropertyValue>,
    /// Structured properties exposed on the native node.
    pub properties: BTreeMap<String, PropertyValue>,
}

/// Unified provider output captured for one semantic snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticSnapshot {
    /// Materialized semantic nodes.
    pub nodes: Vec<SemanticNode>,
    /// Declarative recipes resolved against the same snapshot.
    pub recipes: Vec<crate::NodeRecipe>,
}

impl SemanticSnapshot {
    /// Creates a snapshot from nodes and recipes.
    #[must_use]
    pub fn new(nodes: Vec<SemanticNode>, recipes: Vec<crate::NodeRecipe>) -> Self {
        Self { nodes, recipes }
    }

    /// Creates a snapshot containing only concrete nodes.
    #[must_use]
    pub fn from_nodes(nodes: Vec<SemanticNode>) -> Self {
        Self {
            nodes,
            recipes: Vec::new(),
        }
    }
}

/// Stable identifier for one attached surface inside a logical session.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceId(String);

impl SurfaceId {
    /// Creates a surface identifier from a stable string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        debug_assert!(!id.is_empty(), "SurfaceId must not be empty");
        Self(id)
    }

    /// Borrows the raw identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SurfaceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SurfaceId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Query used by session helpers to discover transient surfaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceQuery {
    /// Matches a surface title exactly.
    TitleEquals(String),
    /// Matches a surface title by substring.
    TitleContains(String),
}

impl SurfaceQuery {
    /// Builds an exact-title query.
    #[must_use]
    pub fn title_eq(title: impl Into<String>) -> Self {
        Self::TitleEquals(title.into())
    }

    /// Builds a contains-title query.
    #[must_use]
    pub fn title_contains(title: impl Into<String>) -> Self {
        let title = title.into();
        debug_assert!(!title.is_empty(), "SurfaceQuery::title_contains pattern must not be empty");
        Self::TitleContains(title)
    }

    /// Returns whether `title` satisfies this query.
    #[must_use]
    pub fn matches_title(&self, title: &str) -> bool {
        match self {
            Self::TitleEquals(expected) => title == expected,
            Self::TitleContains(expected) => title.contains(expected),
        }
    }
}

/// Owner-aware specification for opening and attaching a transient surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransientSurfaceSpec {
    /// Surface that owns the transient.
    pub owner: SurfaceId,
    /// Semantic selector for the control that opens the transient.
    pub opener: crate::Selector,
}

impl TransientSurfaceSpec {
    /// Creates a transient-surface spec from an owner surface and opener selector.
    #[must_use]
    pub fn new(owner: impl Into<SurfaceId>, opener: crate::Selector) -> Self {
        Self {
            owner: owner.into(),
            opener,
        }
    }
}

/// Backend-neutral key modifier state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// Shift modifier.
    pub shift: bool,
    /// Control modifier.
    pub control: bool,
    /// Alt/Option modifier.
    pub alt: bool,
    /// Command/Super/Meta modifier.
    pub meta: bool,
}

/// Backend-neutral text range using Unicode scalar indices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextRange {
    /// Start offset.
    pub start: usize,
    /// Length in Unicode scalar values.
    pub len: usize,
}

impl TextRange {
    /// Creates a range from `start` and `len`.
    #[must_use]
    pub const fn new(start: usize, len: usize) -> Self {
        Self { start, len }
    }
}

/// Search strategy for semantic hit-point resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitPointSearch {
    /// Strategy for generating candidate hit points within the visible region.
    pub strategy: HitPointStrategy,
    /// Maximum number of candidate points to sample; used by `Grid` and `VisibleCenterFirst`.
    pub sample_count: usize,
}

impl Default for HitPointSearch {
    fn default() -> Self {
        Self {
            strategy: HitPointStrategy::VisibleCenterFirst,
            sample_count: 9,
        }
    }
}

/// Candidate generation strategy for semantic hit-point resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitPointStrategy {
    /// Try the visible center before sampling other points.
    VisibleCenterFirst,
    /// Sample a regular grid inside the visible region.
    Grid,
    /// Sample the corners and center of the visible region.
    CornersAndCenter,
}

/// Errors returned by backend input-dispatch APIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSynthesisError {
    /// The active backend cannot provide this input API on the current platform.
    UnsupportedBackend,
    /// No attached native window is available for input dispatch.
    MissingWindow,
    /// The native surface required for input dispatch is unavailable.
    MissingSurface,
    /// The native backend-specific window identifier is unavailable.
    MissingWindowId,
    /// The backend could not obtain a target widget or responder for input dispatch.
    MissingTarget,
    /// The requested key could not be mapped to a native key code.
    UnsupportedKey(String),
    /// The backend does not have a focused target for keyboard input.
    MissingFocusTarget,
    /// The native transport reported an operation failure.
    TransportFailure(&'static str),
}

impl std::fmt::Display for InputSynthesisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedBackend => write!(f, "input dispatch is unsupported"),
            Self::MissingWindow => write!(f, "input dispatch requires an attached window"),
            Self::MissingSurface => write!(f, "input dispatch requires a native surface"),
            Self::MissingWindowId => write!(f, "input dispatch requires a native window id"),
            Self::MissingTarget => write!(f, "input dispatch could not resolve a target"),
            Self::UnsupportedKey(key) => {
                write!(f, "input dispatch does not support key {key:?}")
            }
            Self::MissingFocusTarget => {
                write!(f, "keyboard input dispatch requires a focused target")
            }
            Self::TransportFailure(operation) => {
                write!(f, "native transport failed during {operation}")
            }
        }
    }
}

impl std::error::Error for InputSynthesisError {}

/// Shared error type for anchored text assertions.
#[derive(Debug)]
pub enum AnchoredTextAssertionError<E> {
    /// Region resolution failed before rendering or capture.
    Resolve(RegionResolveError),
    /// The underlying text assertion failed.
    Assert(TextAssertionError<E>),
}

impl<E> std::fmt::Display for AnchoredTextAssertionError<E>
where
    E: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolve(error) => write!(f, "{error}"),
            Self::Assert(error) => write!(f, "{error}"),
        }
    }
}

impl<E> std::error::Error for AnchoredTextAssertionError<E> where E: std::error::Error {}

/// Common harness behavior shared by platform backends.
pub trait Harness {
    /// Backend-specific window host type.
    type WindowHost;

    /// Flushes the native event loop once.
    fn flush(&self);

    /// Flushes the event loop at least `frames` times.
    fn settle(&self, frames: usize) {
        for _ in 0..frames.max(1) {
            self.flush();
        }
    }

    /// Polls `predicate`, flushing the event loop between attempts.
    fn wait_until<F>(&self, options: PollOptions, mut predicate: F) -> Result<usize, PollError>
    where
        F: FnMut() -> bool,
    {
        wait_for_condition(options, || {
            self.flush();
            predicate()
        })
    }

    /// Creates a test window with the requested content size.
    fn create_window(&self, width: f64, height: f64) -> Self::WindowHost;

    /// Runs the backend event loop for the given duration.
    fn wait_for_duration(&self, duration: Duration);
}

/// Shared input-driver behavior.
pub trait InputDriver {
    /// Native text widget type used by the backend.
    type NativeText;

    /// Dispatches a backend click interaction at `point`.
    ///
    /// Backends may route this through platform control activation instead of
    /// raw pointer-event injection when that matches native behavior.
    fn click(&self, point: Point) -> Result<(), InputSynthesisError>;

    /// Synthesizes a pointer click at the center of `rect`.
    fn click_rect_center(&self, rect: Rect) -> Result<(), InputSynthesisError> {
        self.click(Point::new(
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        ))
    }

    /// Dispatches a backend pointer-move interaction at `point`.
    fn move_mouse(&self, point: Point) -> Result<(), InputSynthesisError>;

    /// Dispatches a backend key interaction.
    ///
    /// Backends may use responder, controller, or text-insertion APIs instead
    /// of raw native key-event synthesis.
    fn key_press(&self, key: &str, modifiers: KeyModifiers) -> Result<(), InputSynthesisError>;

    /// Inserts text directly into a native text widget.
    fn type_text_direct(&self, view: &Self::NativeText, text: &str);

    /// Replaces the entire text contents of a native text widget.
    fn replace_text(&self, view: &Self::NativeText, text: &str);

    /// Sets the selected range in a native text widget.
    fn set_selection(&self, view: &Self::NativeText, range: TextRange);
}

/// Common text-harness behavior for anchored text assertions.
pub trait AnchoredTextHarness: TextRenderer {
    /// Resolves and asserts an anchored text expectation.
    fn assert_text_renders_anchored(
        &self,
        expectation: &AnchoredTextExpectation,
        artifact_dir: &Path,
        config: &TextAssertionConfig,
    ) -> Result<(), AnchoredTextAssertionError<Self::Error>>;
}

/// Resolves an anchored text expectation and delegates to `renderer`.
pub fn assert_anchored_text_renders<R>(
    renderer: &R,
    resolve_region: impl Fn(&RegionSpec) -> Result<Rect, RegionResolveError>,
    expectation: &AnchoredTextExpectation,
    artifact_dir: &Path,
    config: &TextAssertionConfig,
) -> Result<(), AnchoredTextAssertionError<R::Error>>
where
    R: TextRenderer,
    R::Error: std::fmt::Display,
{
    let rect = resolve_region(&expectation.region).map_err(AnchoredTextAssertionError::Resolve)?;
    let expectation = expectation.resolve(rect);
    crate::assert_text_renders(renderer, &expectation, artifact_dir, config)
        .map_err(AnchoredTextAssertionError::Assert)
}

/// Returns the stable registered ID for a backend-native node.
#[must_use]
pub fn registered_node_id(index: usize, id: Option<&str>, prefix: &str) -> String {
    id.map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{prefix}-{index}"))
}

/// Namespaces provider nodes when they collide with native IDs or each other.
#[must_use]
pub fn normalize_provider_nodes(
    mut nodes: Vec<SemanticNode>,
    native_ids: &BTreeSet<String>,
) -> Vec<SemanticNode> {
    let provider_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let needs_namespace =
        provider_ids.iter().any(|id| native_ids.contains(id)) || has_duplicates(&provider_ids);
    if !needs_namespace {
        for node in &mut nodes {
            node.properties
                .entry("glasscheck:source_id".into())
                .or_insert_with(|| PropertyValue::String(node.id.clone()));
            node.property_provenance
                .entry("glasscheck:source_id".into())
                .or_insert(node.provenance);
        }
        return nodes;
    }

    let mut original_counts = BTreeMap::<String, usize>::new();
    for id in &provider_ids {
        *original_counts.entry(id.clone()).or_default() += 1;
    }

    let mut original_to_unique = BTreeMap::<String, String>::new();
    let mut assigned_counts = BTreeMap::<String, usize>::new();

    for node in &mut nodes {
        let original_id = node.id.clone();
        let base_id = format!("provider::{original_id}");
        let occurrence = assigned_counts.entry(base_id.clone()).or_default();
        let unique_id = if *occurrence == 0 {
            base_id.clone()
        } else {
            format!("{base_id}#{occurrence}")
        };
        *occurrence += 1;
        node.id = unique_id.clone();
        node.properties.insert(
            "glasscheck:source_id".into(),
            PropertyValue::String(original_id),
        );
        node.property_provenance
            .insert("glasscheck:source_id".into(), node.provenance);
    }

    for node in &nodes {
        let Some(PropertyValue::String(original_id)) = node.properties.get("glasscheck:source_id")
        else {
            continue;
        };
        if original_counts.get(original_id).copied().unwrap_or(0) == 1 {
            original_to_unique.insert(original_id.clone(), node.id.clone());
        }
    }

    for node in &mut nodes {
        if let Some(parent_id) = node.parent_id.as_ref() {
            if original_counts.get(parent_id).copied().unwrap_or(0) == 1 {
                node.parent_id = original_to_unique.get(parent_id).cloned();
            } else {
                node.properties.insert(
                    "glasscheck:ambiguous_parent_id".into(),
                    PropertyValue::String(parent_id.clone()),
                );
                node.property_provenance
                    .insert("glasscheck:ambiguous_parent_id".into(), node.provenance);
                node.parent_id = None;
            }
        }
    }

    nodes
}

fn has_duplicates(ids: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    ids.iter().any(|id| !seen.insert(id.clone()))
}

/// Crops a rectangle expressed in bottom-left-origin view coordinates.
#[must_use]
pub fn crop_image_bottom_left(image: &Image, rect: Rect) -> Image {
    crate::image::crop_image_view_coordinates(image, rect)
}

#[cfg(test)]
mod tests {
    use super::{normalize_provider_nodes, SurfaceQuery, TransientSurfaceSpec};
    use crate::{
        NodeProvenanceKind, Point, PropertyValue, Rect, Role, Selector, SemanticNode, Size,
        SurfaceId,
    };
    use std::collections::BTreeSet;

    fn rect() -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))
    }

    #[test]
    fn provider_ids_are_namespaced_when_they_collide() {
        let native_ids = BTreeSet::from(["battlefield".to_string()]);
        let nodes = normalize_provider_nodes(
            vec![
                SemanticNode::new("battlefield", Role::Container, rect()),
                SemanticNode::new("battlefield/card", Role::Container, rect())
                    .with_parent("battlefield", 0),
            ],
            &native_ids,
        );

        assert_eq!(nodes[0].id, "provider::battlefield");
        assert_eq!(nodes[1].parent_id.as_deref(), Some("provider::battlefield"));
    }

    #[test]
    fn provider_normalization_preserves_provenance_for_added_properties() {
        let native_ids = BTreeSet::from(["battlefield".to_string()]);
        let nodes = normalize_provider_nodes(
            vec![
                SemanticNode::new("battlefield", Role::Container, rect())
                    .with_provenance(NodeProvenanceKind::Matched),
                SemanticNode::new("battlefield/card", Role::Container, rect())
                    .with_parent("battlefield", 0)
                    .with_provenance(NodeProvenanceKind::Matched),
            ],
            &native_ids,
        );

        assert_eq!(
            nodes[0].properties.get("glasscheck:source_id"),
            Some(&PropertyValue::string("battlefield"))
        );
        assert_eq!(
            nodes[0].property_provenance.get("glasscheck:source_id"),
            Some(&NodeProvenanceKind::Matched)
        );
    }

    #[test]
    fn provider_normalization_records_source_id_without_namespacing() {
        let native_ids = BTreeSet::new();
        let nodes = normalize_provider_nodes(
            vec![SemanticNode::new("provider-node", Role::Container, rect())
                .with_provenance(NodeProvenanceKind::Declared)],
            &native_ids,
        );

        assert_eq!(
            nodes[0].properties.get("glasscheck:source_id"),
            Some(&PropertyValue::string("provider-node"))
        );
        assert_eq!(
            nodes[0].property_provenance.get("glasscheck:source_id"),
            Some(&NodeProvenanceKind::Declared)
        );
    }

    #[test]
    fn transient_surface_spec_captures_owner_and_opener() {
        let spec = TransientSurfaceSpec::new("editor", Selector::id_eq("open-table-picker"));

        assert_eq!(spec.owner, SurfaceId::new("editor"));
        assert_eq!(spec.opener, Selector::id_eq("open-table-picker"));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SurfaceId must not be empty")]
    fn surface_id_rejects_empty_string() {
        let _ = SurfaceId::new("");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SurfaceQuery::title_contains pattern must not be empty")]
    fn surface_query_title_contains_rejects_empty_pattern() {
        let _ = SurfaceQuery::title_contains("");
    }
}
