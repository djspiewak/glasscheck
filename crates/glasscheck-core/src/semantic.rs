use std::collections::BTreeMap;

use crate::{
    assert_above, assert_adjacent_horizontally, assert_adjacent_vertically,
    assert_contained_within, assert_contains_point, assert_horizontal_alignment, assert_left_of,
    assert_same_height, assert_same_width, assert_vertical_alignment, LayoutError, LayoutTolerance,
    NodeHandle, Point, PropertyValue, QueryError, Rect, Scene, Selector, SemanticNode,
};

#[derive(Clone, Debug, PartialEq)]
pub struct QueryMatch<'a> {
    pub handle: NodeHandle,
    pub node: &'a SemanticNode,
    pub bounds: Rect,
    pub visible_bounds: Option<Rect>,
    pub interactability: Interactability,
    pub preferred_hit_point: Option<Point>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedNode {
    pub handle: NodeHandle,
    pub node: SemanticNode,
    pub bounds: Rect,
    pub visible_bounds: Option<Rect>,
    pub interactability: Interactability,
    pub preferred_hit_point: Option<Point>,
}

impl<'a> From<QueryMatch<'a>> for ResolvedNode {
    fn from(value: QueryMatch<'a>) -> Self {
        Self {
            handle: value.handle,
            node: value.node.clone(),
            bounds: value.bounds,
            visible_bounds: value.visible_bounds,
            interactability: value.interactability,
            preferred_hit_point: value.preferred_hit_point,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interactability {
    Interactable {
        hit_point: Point,
    },
    Missing,
    Hidden,
    FullyClipped,
    ZeroSized,
    Disabled,
    NotHitTestable,
    Occluded {
        hit_point: Point,
        topmost: NodeHandle,
    },
}

impl Interactability {
    #[must_use]
    pub fn preferred_hit_point(self) -> Option<Point> {
        match self {
            Self::Interactable { hit_point } | Self::Occluded { hit_point, .. } => Some(hit_point),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_hit_testable(self) -> bool {
        !matches!(
            self,
            Self::Missing
                | Self::Hidden
                | Self::FullyClipped
                | Self::ZeroSized
                | Self::NotHitTestable
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SemanticAssertionError {
    Query(QueryError),
    Layout(LayoutError),
    MissingVisibleBounds(Selector),
    UnexpectedCount {
        predicate: Selector,
        expected: usize,
        actual: usize,
    },
    UnexpectedInteractability {
        predicate: Selector,
        actual: Interactability,
    },
    PropertyMismatch {
        predicate: Selector,
        key: String,
        expected: PropertyValue,
        actual: Option<PropertyValue>,
    },
    StateMismatch {
        predicate: Selector,
        key: String,
        expected: PropertyValue,
        actual: Option<PropertyValue>,
    },
}

impl From<QueryError> for SemanticAssertionError {
    fn from(value: QueryError) -> Self {
        Self::Query(value)
    }
}

impl From<LayoutError> for SemanticAssertionError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl std::fmt::Display for SemanticAssertionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Query(error) => write!(f, "{error}"),
            Self::Layout(error) => write!(f, "{error}"),
            Self::MissingVisibleBounds(_) => write!(f, "matched node has no visible bounds"),
            Self::UnexpectedCount {
                expected, actual, ..
            } => {
                write!(f, "expected {expected} matches, found {actual}")
            }
            Self::UnexpectedInteractability { actual, .. } => {
                write!(f, "unexpected interactability: {actual:?}")
            }
            Self::PropertyMismatch { key, .. } => write!(f, "property mismatch for {key}"),
            Self::StateMismatch { key, .. } => write!(f, "state mismatch for {key}"),
        }
    }
}

impl std::error::Error for SemanticAssertionError {}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneDelta {
    pub added: Vec<SemanticNode>,
    pub removed: Vec<SemanticNode>,
    pub changed: Vec<NodeChange>,
    pub reordered: Vec<ReorderChange>,
    pub ambiguous_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeChange {
    pub id: String,
    pub rect_before: Rect,
    pub rect_after: Rect,
    pub visibility_before: bool,
    pub visibility_after: bool,
    pub hit_testable_before: bool,
    pub hit_testable_after: bool,
    pub state_changes: BTreeMap<String, (Option<PropertyValue>, Option<PropertyValue>)>,
    pub property_changes: BTreeMap<String, (Option<PropertyValue>, Option<PropertyValue>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReorderChange {
    pub id: String,
    pub child_index_before: usize,
    pub child_index_after: usize,
    pub z_index_before: i32,
    pub z_index_after: i32,
}

pub fn assert_exists(scene: &Scene, predicate: &Selector) -> Result<(), SemanticAssertionError> {
    let actual = scene.count(predicate);
    if actual > 0 {
        Ok(())
    } else {
        Err(SemanticAssertionError::UnexpectedCount {
            predicate: predicate.clone(),
            expected: 1,
            actual,
        })
    }
}

pub fn assert_not_exists(
    scene: &Scene,
    predicate: &Selector,
) -> Result<(), SemanticAssertionError> {
    let actual = scene.count(predicate);
    if actual == 0 {
        Ok(())
    } else {
        Err(SemanticAssertionError::UnexpectedCount {
            predicate: predicate.clone(),
            expected: 0,
            actual,
        })
    }
}

pub fn assert_unique(
    scene: &Scene,
    predicate: &Selector,
) -> Result<NodeHandle, SemanticAssertionError> {
    Ok(scene.find(predicate)?)
}

pub fn assert_count(
    scene: &Scene,
    predicate: &Selector,
    expected: usize,
) -> Result<(), SemanticAssertionError> {
    let actual = scene.count(predicate);
    if actual == expected {
        Ok(())
    } else {
        Err(SemanticAssertionError::UnexpectedCount {
            predicate: predicate.clone(),
            expected,
            actual,
        })
    }
}

pub fn assert_visible(scene: &Scene, predicate: &Selector) -> Result<(), SemanticAssertionError> {
    let resolved = scene.resolve(predicate)?;
    resolved
        .visible_bounds
        .is_some()
        .then_some(())
        .ok_or_else(|| SemanticAssertionError::MissingVisibleBounds(predicate.clone()))
}

pub fn assert_hit_testable(
    scene: &Scene,
    predicate: &Selector,
) -> Result<(), SemanticAssertionError> {
    let interactability = scene.interactability(predicate)?;
    if interactability.is_hit_testable() {
        Ok(())
    } else {
        Err(SemanticAssertionError::UnexpectedInteractability {
            predicate: predicate.clone(),
            actual: interactability,
        })
    }
}

pub fn assert_interactable(
    scene: &Scene,
    predicate: &Selector,
) -> Result<Point, SemanticAssertionError> {
    match scene.interactability(predicate)? {
        Interactability::Interactable { hit_point } => Ok(hit_point),
        actual => Err(SemanticAssertionError::UnexpectedInteractability {
            predicate: predicate.clone(),
            actual,
        }),
    }
}

pub fn assert_property(
    scene: &Scene,
    predicate: &Selector,
    key: &str,
    expected: &PropertyValue,
) -> Result<(), SemanticAssertionError> {
    let resolved = scene.resolve(predicate)?;
    let actual = resolved.node.properties.get(key).cloned();
    if actual.as_ref() == Some(expected) {
        Ok(())
    } else {
        Err(SemanticAssertionError::PropertyMismatch {
            predicate: predicate.clone(),
            key: key.into(),
            expected: expected.clone(),
            actual,
        })
    }
}

pub fn assert_state(
    scene: &Scene,
    predicate: &Selector,
    key: &str,
    expected: &PropertyValue,
) -> Result<(), SemanticAssertionError> {
    let resolved = scene.resolve(predicate)?;
    let actual = resolved.node.state.get(key).cloned();
    if actual.as_ref() == Some(expected) {
        Ok(())
    } else {
        Err(SemanticAssertionError::StateMismatch {
            predicate: predicate.clone(),
            key: key.into(),
            expected: expected.clone(),
            actual,
        })
    }
}

pub fn assert_above_node(
    scene: &Scene,
    upper: &Selector,
    lower: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_above(
        scene.bounds(upper)?,
        scene.bounds(lower)?,
        tolerance,
    )?)
}

pub fn assert_left_of_node(
    scene: &Scene,
    left: &Selector,
    right: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_left_of(
        scene.bounds(left)?,
        scene.bounds(right)?,
        tolerance,
    )?)
}

pub fn assert_contained_within_node(
    scene: &Scene,
    inner: &Selector,
    outer: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_contained_within(
        scene.bounds(inner)?,
        scene.bounds(outer)?,
        tolerance,
    )?)
}

pub fn assert_same_width_node(
    scene: &Scene,
    left: &Selector,
    right: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_same_width(
        scene.bounds(left)?,
        scene.bounds(right)?,
        tolerance,
    )?)
}

pub fn assert_same_height_node(
    scene: &Scene,
    left: &Selector,
    right: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_same_height(
        scene.bounds(left)?,
        scene.bounds(right)?,
        tolerance,
    )?)
}

pub fn assert_horizontally_aligned_node(
    scene: &Scene,
    left: &Selector,
    right: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_horizontal_alignment(
        scene.bounds(left)?,
        scene.bounds(right)?,
        tolerance,
    )?)
}

pub fn assert_vertically_aligned_node(
    scene: &Scene,
    left: &Selector,
    right: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_vertical_alignment(
        scene.bounds(left)?,
        scene.bounds(right)?,
        tolerance,
    )?)
}

pub fn assert_contains_point_node(
    scene: &Scene,
    predicate: &Selector,
    point: Point,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_contains_point(
        scene.bounds(predicate)?,
        point,
        tolerance,
    )?)
}

pub fn assert_adjacent_horizontally_node(
    scene: &Scene,
    left: &Selector,
    right: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_adjacent_horizontally(
        scene.bounds(left)?,
        scene.bounds(right)?,
        tolerance,
    )?)
}

pub fn assert_adjacent_vertically_node(
    scene: &Scene,
    upper: &Selector,
    lower: &Selector,
    tolerance: LayoutTolerance,
) -> Result<(), SemanticAssertionError> {
    Ok(assert_adjacent_vertically(
        scene.bounds(upper)?,
        scene.bounds(lower)?,
        tolerance,
    )?)
}

/// Computes a scene delta by matching nodes through a stable source identity when available.
///
/// This prefers the `glasscheck:source_id` property when it is uniquely present in each snapshot,
/// and otherwise falls back to the unique snapshot-local `id`. Ambiguous identities are excluded
/// from cross-snapshot matching and surfaced in `SceneDelta::ambiguous_ids`.
pub fn diff_scenes(before: &Scene, after: &Scene) -> SceneDelta {
    let (before_nodes, mut ambiguous_ids) = index_unique_nodes(before);
    let (after_nodes, after_ambiguous) = index_unique_nodes(after);
    for id in after_ambiguous {
        if !ambiguous_ids.contains(&id) {
            ambiguous_ids.push(id);
        }
    }

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut reordered = Vec::new();

    for (id, before_node) in &before_nodes {
        if !after_nodes.contains_key(id) {
            removed.push((*before_node).clone());
        }
    }
    for (id, after_node) in &after_nodes {
        if !before_nodes.contains_key(id) {
            added.push((*after_node).clone());
        }
    }
    for (id, before_node) in &before_nodes {
        let Some(after_node) = after_nodes.get(id) else {
            continue;
        };
        let state_changes = property_changes(&before_node.state, &after_node.state);
        let property_changes = property_changes(&before_node.properties, &after_node.properties);
        if before_node.rect != after_node.rect
            || before_node.visible != after_node.visible
            || before_node.hit_testable != after_node.hit_testable
            || !state_changes.is_empty()
            || !property_changes.is_empty()
        {
            changed.push(NodeChange {
                id: diff_identity(before_node).display.to_string(),
                rect_before: before_node.rect,
                rect_after: after_node.rect,
                visibility_before: before_node.visible,
                visibility_after: after_node.visible,
                hit_testable_before: before_node.hit_testable,
                hit_testable_after: after_node.hit_testable,
                state_changes,
                property_changes,
            });
        }
        if before_node.child_index != after_node.child_index
            || before_node.z_index != after_node.z_index
        {
            reordered.push(ReorderChange {
                id: diff_identity(before_node).display.to_string(),
                child_index_before: before_node.child_index,
                child_index_after: after_node.child_index,
                z_index_before: before_node.z_index,
                z_index_after: after_node.z_index,
            });
        }
    }

    SceneDelta {
        added,
        removed,
        changed,
        reordered,
        ambiguous_ids,
    }
}

pub fn assert_node_appeared(
    before: &Scene,
    after: &Scene,
    predicate: &Selector,
) -> Result<(), SemanticAssertionError> {
    if before.count(predicate) == 0 && after.count(predicate) > 0 {
        Ok(())
    } else {
        Err(SemanticAssertionError::UnexpectedCount {
            predicate: predicate.clone(),
            expected: 1,
            actual: after.count(predicate),
        })
    }
}

pub fn assert_node_disappeared(
    before: &Scene,
    after: &Scene,
    predicate: &Selector,
) -> Result<(), SemanticAssertionError> {
    if before.count(predicate) > 0 && after.count(predicate) == 0 {
        Ok(())
    } else {
        Err(SemanticAssertionError::UnexpectedCount {
            predicate: predicate.clone(),
            expected: 0,
            actual: after.count(predicate),
        })
    }
}

pub fn assert_node_moved(
    before: &Scene,
    after: &Scene,
    predicate: &Selector,
) -> Result<(Rect, Rect), SemanticAssertionError> {
    let before_rect = before.bounds(predicate)?;
    let after_rect = after.bounds(predicate)?;
    if before_rect != after_rect {
        Ok((before_rect, after_rect))
    } else {
        Err(SemanticAssertionError::Layout(LayoutError::Relationship {
            expected: "node moved",
            left: before_rect,
            right: after_rect,
            tolerance: LayoutTolerance::default(),
        }))
    }
}

pub fn assert_state_changed(
    before: &Scene,
    after: &Scene,
    predicate: &Selector,
    key: &str,
    from: &PropertyValue,
    to: &PropertyValue,
) -> Result<(), SemanticAssertionError> {
    assert_state(before, predicate, key, from)?;
    assert_state(after, predicate, key, to)?;
    Ok(())
}

fn index_unique_nodes<'a>(scene: &'a Scene) -> (BTreeMap<String, &'a SemanticNode>, Vec<String>) {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut labels = BTreeMap::<String, String>::new();
    for node in scene.all() {
        let identity = diff_identity(node);
        *counts.entry(identity.key.clone()).or_default() += 1;
        labels
            .entry(identity.key)
            .or_insert(identity.display.to_string());
    }

    let mut unique = BTreeMap::new();
    let mut ambiguous = Vec::new();
    for node in scene.all() {
        let identity = diff_identity(node);
        if counts.get(&identity.key) == Some(&1) {
            unique.insert(identity.key, node);
        } else if let Some(label) = labels.get(&identity.key) {
            if !ambiguous.contains(label) {
                ambiguous.push(label.clone());
            }
        }
    }
    (unique, ambiguous)
}

struct DiffIdentity<'a> {
    key: String,
    display: &'a str,
}

fn diff_identity(node: &SemanticNode) -> DiffIdentity<'_> {
    match node.properties.get("glasscheck:source_id") {
        Some(PropertyValue::String(source_id)) => DiffIdentity {
            key: format!("source:{source_id}"),
            display: source_id,
        },
        _ => DiffIdentity {
            key: format!("id:{}", node.id),
            display: &node.id,
        },
    }
}

fn property_changes(
    before: &BTreeMap<String, PropertyValue>,
    after: &BTreeMap<String, PropertyValue>,
) -> BTreeMap<String, (Option<PropertyValue>, Option<PropertyValue>)> {
    let mut keys = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|key| {
            let before_value = before.get(&key).cloned();
            let after_value = after.get(&key).cloned();
            (before_value != after_value).then_some((key, (before_value, after_value)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Role, Size, TextMatch};

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    fn scene() -> Scene {
        Scene::new(vec![
            SemanticNode::new("toolbar", Role::Container, rect(0.0, 0.0, 100.0, 20.0)),
            SemanticNode::new("run", Role::Button, rect(0.0, 0.0, 20.0, 20.0))
                .with_parent("toolbar", 0)
                .with_selector("toolbar.run")
                .with_property("enabled", PropertyValue::Bool(true))
                .with_state("armed", PropertyValue::Bool(false)),
            SemanticNode::new("stop", Role::Button, rect(20.0, 0.0, 20.0, 20.0))
                .with_parent("toolbar", 1)
                .with_selector("toolbar.stop"),
        ])
    }

    #[test]
    fn existence_assertions_cover_pass_and_fail_cases() {
        let scene = scene();
        assert!(assert_exists(&scene, &Selector::selector_eq("toolbar.run")).is_ok());
        assert!(assert_exists(&scene, &Selector::role_eq(Role::Button)).is_ok());
        assert!(assert_not_exists(&scene, &Selector::selector_eq("missing")).is_ok());
        assert!(assert_exists(&scene, &Selector::selector_eq("missing")).is_err());
        assert!(assert_not_exists(&scene, &Selector::selector_eq("toolbar.run")).is_err());
        assert!(assert_unique(&scene, &Selector::role_eq(Role::Button)).is_err());
    }

    #[test]
    fn count_property_and_state_assertions_cover_pass_and_fail_cases() {
        let scene = scene();
        assert!(assert_count(&scene, &Selector::role_eq(Role::Button), 2).is_ok());
        assert!(assert_count(&scene, &Selector::role_eq(Role::Button), 1).is_err());

        assert!(assert_property(
            &scene,
            &Selector::selector_eq("toolbar.run"),
            "enabled",
            &PropertyValue::Bool(true)
        )
        .is_ok());
        assert!(assert_property(
            &scene,
            &Selector::selector_eq("toolbar.run"),
            "enabled",
            &PropertyValue::Bool(false)
        )
        .is_err());

        assert!(assert_state(
            &scene,
            &Selector::selector_eq("toolbar.run"),
            "armed",
            &PropertyValue::Bool(false)
        )
        .is_ok());
        assert!(assert_state(
            &scene,
            &Selector::selector_eq("toolbar.run"),
            "armed",
            &PropertyValue::Bool(true)
        )
        .is_err());
    }

    #[test]
    fn layout_wrappers_cover_positive_and_negative_cases() {
        let scene = scene();
        assert!(assert_left_of_node(
            &scene,
            &Selector::selector_eq("toolbar.run"),
            &Selector::selector_eq("toolbar.stop"),
            LayoutTolerance::default(),
        )
        .is_ok());
        assert!(assert_left_of_node(
            &scene,
            &Selector::selector_eq("toolbar.stop"),
            &Selector::selector_eq("toolbar.run"),
            LayoutTolerance::default(),
        )
        .is_err());
        assert!(assert_same_height_node(
            &scene,
            &Selector::selector_eq("toolbar.run"),
            &Selector::selector_eq("toolbar.stop"),
            LayoutTolerance::default(),
        )
        .is_ok());

        let non_adjacent = Scene::new(vec![
            SemanticNode::new("left", Role::Button, rect(0.0, 0.0, 10.0, 10.0))
                .with_selector("left"),
            SemanticNode::new("right", Role::Button, rect(10.5, 25.0, 10.0, 10.0))
                .with_selector("right"),
            SemanticNode::new("upper", Role::Button, rect(0.0, 0.0, 10.0, 10.0))
                .with_selector("upper"),
            SemanticNode::new("lower", Role::Button, rect(25.0, 11.0, 10.0, 10.0))
                .with_selector("lower"),
        ]);
        assert!(assert_adjacent_horizontally_node(
            &non_adjacent,
            &Selector::selector_eq("left"),
            &Selector::selector_eq("right"),
            LayoutTolerance::default(),
        )
        .is_err());
        assert!(assert_adjacent_vertically_node(
            &non_adjacent,
            &Selector::selector_eq("upper"),
            &Selector::selector_eq("lower"),
            LayoutTolerance::default(),
        )
        .is_err());
    }

    #[test]
    fn scene_diff_reports_changes_and_ambiguous_ids() {
        let before = Scene::new(vec![
            SemanticNode::new("run", Role::Button, rect(0.0, 0.0, 20.0, 20.0))
                .with_state("armed", PropertyValue::Bool(false)),
            SemanticNode::new("dup", Role::Label, rect(0.0, 0.0, 10.0, 10.0)),
            SemanticNode::new("dup", Role::Label, rect(10.0, 0.0, 10.0, 10.0)),
        ]);
        let after = Scene::new(vec![
            SemanticNode::new("run", Role::Button, rect(5.0, 0.0, 20.0, 20.0))
                .with_state("armed", PropertyValue::Bool(true)),
            SemanticNode::new("new", Role::Button, rect(0.0, 20.0, 20.0, 20.0)),
            SemanticNode::new("dup", Role::Label, rect(0.0, 0.0, 10.0, 10.0)),
            SemanticNode::new("dup", Role::Label, rect(10.0, 0.0, 10.0, 10.0)),
        ]);

        let delta = diff_scenes(&before, &after);
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.removed.len(), 0);
        assert_eq!(delta.changed.len(), 1);
        assert!(delta.ambiguous_ids.contains(&"dup".to_string()));
        assert!(assert_node_appeared(&before, &after, &Selector::id_eq("new")).is_ok());
        assert!(assert_node_disappeared(&before, &after, &Selector::id_eq("new")).is_err());
        assert!(assert_node_moved(&before, &after, &Selector::id_eq("run")).is_ok());
        assert!(assert_state_changed(
            &before,
            &after,
            &Selector::id_eq("run"),
            "armed",
            &PropertyValue::Bool(false),
            &PropertyValue::Bool(true)
        )
        .is_ok());
    }

    #[test]
    fn scene_diff_prefers_stable_source_ids_over_snapshot_local_ids() {
        let before = Scene::new(vec![SemanticNode::new(
            "provider::battlefield",
            Role::Container,
            rect(0.0, 0.0, 20.0, 20.0),
        )
        .with_property("glasscheck:source_id", PropertyValue::string("battlefield"))]);
        let after = Scene::new(vec![SemanticNode::new(
            "battlefield",
            Role::Container,
            rect(5.0, 0.0, 20.0, 20.0),
        )
        .with_property("glasscheck:source_id", PropertyValue::string("battlefield"))]);

        let delta = diff_scenes(&before, &after);

        assert!(delta.added.is_empty());
        assert!(delta.removed.is_empty());
        assert_eq!(delta.changed.len(), 1);
        assert_eq!(delta.changed[0].id, "battlefield");
    }

    #[test]
    fn scene_diff_marks_duplicate_source_ids_as_ambiguous_and_does_not_match_them() {
        let before = Scene::new(vec![
            SemanticNode::new("first", Role::Container, rect(0.0, 0.0, 10.0, 10.0))
                .with_property("glasscheck:source_id", PropertyValue::string("shared")),
            SemanticNode::new("second", Role::Container, rect(10.0, 0.0, 10.0, 10.0))
                .with_property("glasscheck:source_id", PropertyValue::string("shared")),
        ]);
        let after = Scene::new(vec![
            SemanticNode::new("renamed", Role::Container, rect(0.0, 0.0, 10.0, 10.0))
                .with_property("glasscheck:source_id", PropertyValue::string("shared")),
            SemanticNode::new("renamed#1", Role::Container, rect(10.0, 0.0, 10.0, 10.0))
                .with_property("glasscheck:source_id", PropertyValue::string("shared")),
        ]);

        let delta = diff_scenes(&before, &after);

        assert!(delta.changed.is_empty());
        assert!(delta.reordered.is_empty());
        assert!(delta.ambiguous_ids.contains(&"shared".to_string()));
    }

    #[test]
    fn interactability_assertions_cover_positive_and_negative_cases() {
        let scene = scene();
        assert!(assert_interactable(&scene, &Selector::selector_eq("toolbar.run")).is_ok());
        assert!(assert_hit_testable(&scene, &Selector::selector_eq("toolbar.run")).is_ok());

        let hidden = Scene::new(vec![SemanticNode::new(
            "run",
            Role::Button,
            rect(0.0, 0.0, 20.0, 20.0),
        )
        .with_selector("toolbar.run")
        .with_state("disabled", PropertyValue::Bool(true))]);
        assert!(assert_interactable(&hidden, &Selector::selector_eq("toolbar.run")).is_err());
        assert!(matches!(
            hidden.interactability(&Selector::selector_eq("toolbar.run")),
            Ok(Interactability::Disabled)
        ));
        assert!(!hidden.exists(&Selector::any_selector(TextMatch::contains("missing"))));
    }

    #[test]
    fn hit_testable_assertions_accept_occluded_nodes_but_reject_non_hit_testable_ones() {
        let occluded = Scene::new(vec![
            SemanticNode::new("root", Role::Container, rect(0.0, 0.0, 20.0, 20.0)),
            SemanticNode::new("target", Role::Button, rect(0.0, 0.0, 10.0, 10.0))
                .with_parent("root", 0)
                .with_selector("target"),
            SemanticNode::new("overlay", Role::Button, rect(0.0, 0.0, 10.0, 10.0))
                .with_parent("root", 1),
        ]);
        assert!(assert_hit_testable(&occluded, &Selector::selector_eq("target")).is_ok());
        assert!(assert_interactable(&occluded, &Selector::selector_eq("target")).is_err());

        let non_hit_testable = Scene::new(vec![SemanticNode {
            hit_testable: false,
            ..SemanticNode::new("target", Role::Button, rect(0.0, 0.0, 10.0, 10.0))
                .with_selector("target")
        }]);
        assert!(matches!(
            assert_hit_testable(&non_hit_testable, &Selector::selector_eq("target")),
            Err(SemanticAssertionError::UnexpectedInteractability {
                actual: Interactability::NotHitTestable,
                ..
            })
        ));
    }

    #[test]
    fn visible_assertions_treat_missing_visible_rect_as_visible_when_node_is_visible() {
        let visible = Scene::new(vec![SemanticNode::new(
            "provider",
            Role::Button,
            rect(0.0, 0.0, 20.0, 20.0),
        )
        .with_selector("provider.button")]);
        assert!(assert_visible(&visible, &Selector::selector_eq("provider.button")).is_ok());

        let hidden = Scene::new(vec![SemanticNode {
            visible: false,
            ..SemanticNode::new("provider", Role::Button, rect(0.0, 0.0, 20.0, 20.0))
                .with_selector("provider.button")
        }]);
        assert!(matches!(
            assert_visible(&hidden, &Selector::selector_eq("provider.button")),
            Err(SemanticAssertionError::MissingVisibleBounds(_))
        ));
    }
}
