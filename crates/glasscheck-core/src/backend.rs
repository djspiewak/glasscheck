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

    /// Performs the backend's best-effort click interaction at `point`.
    ///
    /// Backends may synthesize native pointer events, invoke an activation path,
    /// or fall back to focus routing when full event synthesis is unavailable.
    fn click(&self, point: Point);

    /// Synthesizes a pointer click at the center of `rect`.
    fn click_rect_center(&self, rect: Rect) {
        self.click(Point::new(
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        ));
    }

    /// Performs the backend's best-effort pointer-move interaction at `point`.
    fn move_mouse(&self, point: Point);

    /// Performs the backend's best-effort key interaction.
    ///
    /// Some backends may ignore unsupported modifier combinations or route text
    /// input through native text APIs instead of low-level key events.
    fn key_press(&self, key: &str, modifiers: KeyModifiers);

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
    use super::normalize_provider_nodes;
    use crate::{Point, Rect, Role, SemanticNode, Size};
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
}
