use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{Interactability, NodePredicate, Point, QueryError, QueryMatch, Rect};

const PAINT_ORDER_PATH_PROPERTY: &str = "glasscheck:paint_order_path";

/// A semantic role attached to a UI node under test.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// A top-level window.
    Window,
    /// A generic container.
    Container,
    /// A clickable button-like control.
    Button,
    /// A text-editing control.
    TextInput,
    /// A static text label.
    Label,
    /// A list or repeated collection container.
    List,
    /// A single item within a list or collection.
    ListItem,
    /// A visual image or image placeholder.
    Image,
    /// A text run or text fragment.
    TextRun,
    /// A decorative or semantic marker.
    Marker,
    /// A placeholder or empty-state surface.
    Placeholder,
    /// A divider or separator primitive.
    Divider,
    /// A caller-defined semantic role.
    Custom(String),
}

impl Role {
    /// Creates a custom semantic role.
    #[must_use]
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom(name.into())
    }
}

/// A structured property value attached to a semantic node.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropertyValue {
    /// Boolean property.
    Bool(bool),
    /// Signed integer property.
    Integer(i64),
    /// String property.
    String(String),
}

impl PropertyValue {
    /// Creates a string property value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }
}

/// Immutable handle to a node within a specific scene snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeHandle {
    index: usize,
}

impl NodeHandle {
    /// Returns the zero-based node index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// Rich semantic metadata captured for a UI node under test.
///
/// Use this as the primary testing model for new code. Compared with the older
/// flat `NodeMetadata` model, it supports hierarchy, selectors, properties,
/// state, visibility, and hit-testing semantics in one snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// Semantic identifier for this snapshot.
    ///
    /// Callers may use this for exact matching within the current snapshot, but should not
    /// assume it remains unchanged across future snapshots when duplicate native IDs or provider
    /// ID collisions require disambiguation.
    pub id: String,
    /// Stable test-facing selectors or aliases.
    ///
    /// Prefer selectors for cross-snapshot tests. Unlike `id`, selectors are
    /// caller-defined and are not rewritten to resolve snapshot-local
    /// collisions.
    pub selectors: BTreeSet<String>,
    /// Semantic role for the node.
    pub role: Role,
    /// Bounds of the node in root coordinates.
    pub rect: Rect,
    /// Optional user-visible label.
    pub label: Option<String>,
    /// Optional textual value.
    pub value: Option<String>,
    /// Parent node identifier when this node is part of a hierarchy.
    ///
    /// This references the parent snapshot ID, including any snapshot-local disambiguation.
    pub parent_id: Option<String>,
    /// Stable child ordering within the parent.
    pub child_index: usize,
    /// Stable z-order within the parent.
    pub z_index: i32,
    /// Whether the node is logically visible.
    pub visible: bool,
    /// Visible rect after clipping, when known.
    pub visible_rect: Option<Rect>,
    /// Whether the node participates in hit testing.
    pub hit_testable: bool,
    /// Effective opacity in the range `[0.0, 1.0]`.
    pub opacity: f64,
    /// Semantic classes for grouping or matching.
    pub classes: Vec<String>,
    /// Semantic tags for grouping or matching.
    pub tags: Vec<String>,
    /// Structured state payload.
    pub state: BTreeMap<String, PropertyValue>,
    /// Structured property payload.
    pub properties: BTreeMap<String, PropertyValue>,
}

impl SemanticNode {
    /// Creates a node with the required fields and sensible defaults.
    #[must_use]
    pub fn new(id: impl Into<String>, role: Role, rect: Rect) -> Self {
        Self {
            id: id.into(),
            selectors: BTreeSet::new(),
            role,
            rect,
            label: None,
            value: None,
            parent_id: None,
            child_index: 0,
            z_index: 0,
            visible: true,
            visible_rect: None,
            hit_testable: true,
            opacity: 1.0,
            classes: Vec::new(),
            tags: Vec::new(),
            state: BTreeMap::new(),
            properties: BTreeMap::new(),
        }
    }

    /// Sets the user-visible label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a stable test-facing selector.
    #[must_use]
    pub fn with_selector(mut self, selector: impl Into<String>) -> Self {
        self.selectors.insert(selector.into());
        self
    }

    /// Adds multiple stable test-facing selectors.
    #[must_use]
    pub fn with_selectors<I, S>(mut self, selectors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selectors.extend(selectors.into_iter().map(Into::into));
        self
    }

    /// Sets the parent node identifier and child ordering.
    #[must_use]
    pub fn with_parent(mut self, parent_id: impl Into<String>, child_index: usize) -> Self {
        self.parent_id = Some(parent_id.into());
        self.child_index = child_index;
        self
    }

    /// Adds a property entry.
    #[must_use]
    pub fn with_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Adds a state entry.
    #[must_use]
    pub fn with_state(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.state.insert(key.into(), value);
        self
    }

    /// Adds a semantic class.
    #[must_use]
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// Adds a semantic tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// Serializable snapshot of the semantic scene under test.
///
/// A snapshot is the main query surface for semantic assertions, waits, and
/// scene diffs. Build one per assertion step when the UI can change over time.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSnapshot {
    nodes: Vec<SemanticNode>,
    id_index: BTreeMap<String, Vec<usize>>,
    selector_index: BTreeMap<String, Vec<usize>>,
    children_index: BTreeMap<String, Vec<usize>>,
}

impl Default for SceneSnapshot {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl SceneSnapshot {
    /// Creates a scene snapshot from collected semantic nodes.
    ///
    /// The constructor builds indexes for exact ID lookup, selector lookup, and
    /// parent-child traversal.
    #[must_use]
    pub fn new(nodes: Vec<SemanticNode>) -> Self {
        let mut id_index = BTreeMap::<String, Vec<usize>>::new();
        let mut selector_index = BTreeMap::<String, Vec<usize>>::new();
        let mut children_index = BTreeMap::<String, Vec<usize>>::new();

        for (index, node) in nodes.iter().enumerate() {
            id_index.entry(node.id.clone()).or_default().push(index);
            for selector in &node.selectors {
                selector_index
                    .entry(selector.clone())
                    .or_default()
                    .push(index);
            }
            if let Some(parent_id) = node.parent_id.as_ref() {
                children_index
                    .entry(parent_id.clone())
                    .or_default()
                    .push(index);
            }
        }

        for indices in children_index.values_mut() {
            indices.sort_by_key(|index| {
                let node = &nodes[*index];
                (node.child_index, node.z_index)
            });
        }

        Self {
            nodes,
            id_index,
            selector_index,
            children_index,
        }
    }

    /// Returns all nodes in the scene.
    #[must_use]
    pub fn all(&self) -> &[SemanticNode] {
        &self.nodes
    }

    /// Returns the node referenced by `handle`.
    #[must_use]
    pub fn node(&self, handle: NodeHandle) -> Option<&SemanticNode> {
        self.nodes.get(handle.index)
    }

    /// Finds all handles matching `predicate`.
    #[must_use]
    pub fn find_all(&self, predicate: &crate::NodePredicate) -> Vec<NodeHandle> {
        match predicate {
            crate::NodePredicate::IdEq(id) => {
                return self
                    .id_index
                    .get(id)
                    .into_iter()
                    .flat_map(|indices| indices.iter().copied())
                    .map(|index| NodeHandle { index })
                    .collect();
            }
            crate::NodePredicate::SelectorEq(selector) => {
                return self
                    .selector_index
                    .get(selector)
                    .into_iter()
                    .flat_map(|indices| indices.iter().copied())
                    .map(|index| NodeHandle { index })
                    .collect();
            }
            _ => {}
        }
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let context = scene_context(self, index);
                predicate
                    .matches(&context, node)
                    .then_some(NodeHandle { index })
            })
            .collect()
    }

    /// Finds exactly one handle matching `predicate`.
    pub fn find(&self, predicate: &crate::NodePredicate) -> Result<NodeHandle, crate::QueryError> {
        let matches = self.find_all(predicate);
        match matches.as_slice() {
            [] => Err(crate::QueryError::NotFoundPredicate(predicate.clone())),
            [handle] => Ok(*handle),
            _ => Err(crate::QueryError::MultiplePredicateMatches {
                predicate: predicate.clone(),
                count: matches.len(),
            }),
        }
    }

    /// Returns the immediate children of `parent`, ordered by child index then z-index.
    #[must_use]
    pub fn children_of(&self, parent: NodeHandle) -> Vec<NodeHandle> {
        let Some(parent_node) = self.node(parent) else {
            return Vec::new();
        };
        if self
            .id_index
            .get(parent_node.id.as_str())
            .map_or(0, Vec::len)
            != 1
        {
            return Vec::new();
        }
        self.children_index
            .get(parent_node.id.as_str())
            .into_iter()
            .flat_map(|indices| indices.iter().copied())
            .map(|index| NodeHandle { index })
            .collect()
    }

    /// Returns the parent of `node`, when present and resolvable.
    #[must_use]
    pub fn parent_of(&self, node: NodeHandle) -> Option<NodeHandle> {
        let node = self.node(node)?;
        let parent_id = node.parent_id.as_deref()?;
        self.id_index
            .get(parent_id)
            .and_then(|indices| (indices.len() == 1).then_some(indices[0]))
            .map(|index| NodeHandle { index })
    }

    /// Resolves exactly one semantic match with rich metadata.
    pub fn resolve(&self, predicate: &NodePredicate) -> Result<QueryMatch<'_>, QueryError> {
        let handle = self.find(predicate)?;
        self.resolve_handle(handle)
            .ok_or(QueryError::NotFoundPredicate(predicate.clone()))
    }

    /// Resolves all semantic matches with rich metadata.
    #[must_use]
    pub fn resolve_all(&self, predicate: &NodePredicate) -> Vec<QueryMatch<'_>> {
        self.find_all(predicate)
            .into_iter()
            .filter_map(|handle| self.resolve_handle(handle))
            .collect()
    }

    /// Returns whether the predicate matches any nodes.
    #[must_use]
    pub fn exists(&self, predicate: &NodePredicate) -> bool {
        !self.find_all(predicate).is_empty()
    }

    /// Returns the number of nodes matching the predicate.
    #[must_use]
    pub fn count(&self, predicate: &NodePredicate) -> usize {
        self.find_all(predicate).len()
    }

    /// Returns the raw bounds of the unique match.
    pub fn bounds(&self, predicate: &NodePredicate) -> Result<Rect, QueryError> {
        Ok(self.resolve(predicate)?.bounds)
    }

    /// Returns the visible bounds of the unique match.
    pub fn visible_bounds(&self, predicate: &NodePredicate) -> Result<Rect, QueryError> {
        self.resolve(predicate)?
            .visible_bounds
            .ok_or(QueryError::NotFoundPredicate(predicate.clone()))
    }

    /// Returns the center point of the unique match's raw bounds.
    pub fn center(&self, predicate: &NodePredicate) -> Result<Point, QueryError> {
        Ok(rect_center(self.bounds(predicate)?))
    }

    /// Returns the center point of the unique match's visible bounds.
    pub fn visible_center(&self, predicate: &NodePredicate) -> Result<Point, QueryError> {
        Ok(rect_center(self.visible_bounds(predicate)?))
    }

    /// Returns all raw bounds for all matches.
    #[must_use]
    pub fn all_bounds(&self, predicate: &NodePredicate) -> Vec<Rect> {
        self.resolve_all(predicate)
            .into_iter()
            .map(|resolved| resolved.bounds)
            .collect()
    }

    /// Returns the interactability classification for the unique match.
    pub fn interactability(
        &self,
        predicate: &NodePredicate,
    ) -> Result<Interactability, QueryError> {
        Ok(self.resolve(predicate)?.interactability)
    }

    /// Returns the preferred hit point for the unique match.
    pub fn preferred_hit_point(&self, predicate: &NodePredicate) -> Result<Point, QueryError> {
        self.interactability(predicate)?
            .preferred_hit_point()
            .ok_or(QueryError::NotFoundPredicate(predicate.clone()))
    }

    /// Returns the topmost node at `point`, when any semantic hit exists.
    #[must_use]
    pub fn topmost_at(&self, point: Point) -> Option<NodeHandle> {
        self.hit_path_at(point).into_iter().next()
    }

    /// Returns the semantic hit path at `point`, ordered from topmost to backmost.
    #[must_use]
    pub fn hit_path_at(&self, point: Point) -> Vec<NodeHandle> {
        let mut hits = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let rect = node.visible_rect.unwrap_or(node.rect);
                (node.visible
                    && node.hit_testable
                    && rect.size.width > 0.0
                    && rect.size.height > 0.0
                    && rect.contains(point))
                .then_some((index, node))
            })
            .collect::<Vec<_>>();

        hits.sort_by(|(left_index, left), (right_index, right)| {
            compare_paint_order(
                self,
                (*left_index, left.child_index, left.z_index),
                (*right_index, right.child_index, right.z_index),
            )
        });

        hits.into_iter()
            .map(|(index, _)| NodeHandle { index })
            .collect()
    }

    fn resolve_handle(&self, handle: NodeHandle) -> Option<QueryMatch<'_>> {
        let node = self.node(handle)?;
        let interactability = classify_interactability(self, handle, node);
        let preferred_hit_point = interactability.preferred_hit_point();
        Some(QueryMatch {
            handle,
            node,
            bounds: node.rect,
            visible_bounds: visible_bounds_for_node(node),
            interactability,
            preferred_hit_point,
        })
    }
}

/// Application-supplied virtual semantic provider.
pub trait SemanticProvider {
    /// Produces the current semantic nodes for the live scene.
    fn snapshot_nodes(&self) -> Vec<SemanticNode>;
}

fn classify_interactability(
    scene: &SceneSnapshot,
    handle: NodeHandle,
    node: &SemanticNode,
) -> Interactability {
    if !node.visible {
        return Interactability::Hidden;
    }
    if node.rect.size.width <= 0.0 || node.rect.size.height <= 0.0 {
        return Interactability::ZeroSized;
    }
    let visible_bounds = node.visible_rect.unwrap_or(node.rect);
    if visible_bounds.size.width <= 0.0 || visible_bounds.size.height <= 0.0 {
        return Interactability::FullyClipped;
    }
    if node.state.get("disabled") == Some(&PropertyValue::Bool(true))
        || node.properties.get("disabled") == Some(&PropertyValue::Bool(true))
    {
        return Interactability::Disabled;
    }
    if !node.hit_testable {
        return Interactability::NotHitTestable;
    }
    let mut occluded = None;
    for hit_point in interactability_probe_points(visible_bounds) {
        match scene.topmost_at(hit_point) {
            Some(topmost) if topmost == handle => {
                return Interactability::Interactable { hit_point };
            }
            Some(topmost) => {
                occluded.get_or_insert((hit_point, topmost));
            }
            None => {}
        }
    }
    if let Some((hit_point, topmost)) = occluded {
        Interactability::Occluded { hit_point, topmost }
    } else {
        Interactability::NotHitTestable
    }
}

fn rect_center(rect: Rect) -> Point {
    Point::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

fn visible_bounds_for_node(node: &SemanticNode) -> Option<Rect> {
    node.visible
        .then_some(node.visible_rect.unwrap_or(node.rect))
}

fn interactability_probe_points(rect: Rect) -> Vec<Point> {
    let x0 = rect.origin.x;
    let x1 = rect.origin.x + rect.size.width / 2.0;
    let x2 = rect.origin.x + rect.size.width;
    let y0 = rect.origin.y;
    let y1 = rect.origin.y + rect.size.height / 2.0;
    let y2 = rect.origin.y + rect.size.height;

    let mut points = vec![
        Point::new(x1, y1),
        Point::new(x0, y0),
        Point::new(x2, y0),
        Point::new(x0, y2),
        Point::new(x2, y2),
    ];

    for row in 0..3 {
        for column in 0..3 {
            let point = Point::new(
                rect.origin.x + rect.size.width * ((column as f64 + 0.5) / 3.0),
                rect.origin.y + rect.size.height * ((row as f64 + 0.5) / 3.0),
            );
            if !points.contains(&point) {
                points.push(point);
            }
        }
    }

    points
}

fn compare_paint_order(
    scene: &SceneSnapshot,
    left: (usize, usize, i32),
    right: (usize, usize, i32),
) -> std::cmp::Ordering {
    if is_ancestor(scene, left.0, right.0) {
        return std::cmp::Ordering::Greater;
    }
    if is_ancestor(scene, right.0, left.0) {
        return std::cmp::Ordering::Less;
    }

    if let Some(ordering) = compare_explicit_paint_paths(scene, left.0, right.0) {
        return ordering;
    }

    let left_path = paint_order_path(scene, left.0);
    let right_path = paint_order_path(scene, right.0);

    for (left_segment, right_segment) in left_path.iter().zip(right_path.iter()) {
        if left_segment != right_segment {
            return right_segment.cmp(left_segment);
        }
    }

    right_path
        .len()
        .cmp(&left_path.len())
        .then_with(|| right.2.cmp(&left.2))
        .then_with(|| right.1.cmp(&left.1))
        .then_with(|| right.0.cmp(&left.0))
}

fn compare_explicit_paint_paths(
    scene: &SceneSnapshot,
    left_index: usize,
    right_index: usize,
) -> Option<std::cmp::Ordering> {
    let left = explicit_paint_path(scene.node(NodeHandle { index: left_index })?)?;
    let right = explicit_paint_path(scene.node(NodeHandle { index: right_index })?)?;

    for (left_segment, right_segment) in left.iter().zip(right.iter()) {
        if left_segment != right_segment {
            return Some(right_segment.cmp(left_segment));
        }
    }

    Some(
        right
            .len()
            .cmp(&left.len())
            .then_with(|| right_index.cmp(&left_index)),
    )
}

fn explicit_paint_path(node: &SemanticNode) -> Option<Vec<usize>> {
    let PropertyValue::String(path) = node.properties.get(PAINT_ORDER_PATH_PROPERTY)? else {
        return None;
    };
    path.split('/')
        .map(str::parse::<usize>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn paint_order_path(scene: &SceneSnapshot, index: usize) -> Vec<(i32, usize, usize)> {
    let mut path = Vec::new();
    let mut current = Some(NodeHandle { index });

    while let Some(handle) = current {
        let Some(node) = scene.node(handle) else {
            break;
        };
        path.push((node.z_index, node.child_index, handle.index()));
        current = scene.parent_of(handle);
    }

    path.reverse();
    path
}

fn is_ancestor(scene: &SceneSnapshot, ancestor_index: usize, descendant_index: usize) -> bool {
    let mut current = scene.parent_of(NodeHandle {
        index: descendant_index,
    });
    while let Some(parent) = current {
        if parent.index() == ancestor_index {
            return true;
        }
        current = scene.parent_of(parent);
    }
    false
}

/// Context exposed to predicates for relationship-aware matching.
pub(crate) trait PredicateContext {
    fn parent_of(&self, node: &SemanticNode) -> Option<&SemanticNode>;
    fn has_ancestor_matching(&self, node: &SemanticNode, predicate: &crate::NodePredicate) -> bool;
}

struct ScenePredicateContext<'a> {
    scene: &'a SceneSnapshot,
}

impl PredicateContext for ScenePredicateContext<'_> {
    fn parent_of(&self, node: &SemanticNode) -> Option<&SemanticNode> {
        let parent_id = node.parent_id.as_deref()?;
        self.scene
            .id_index
            .get(parent_id)
            .and_then(|indices| (indices.len() == 1).then_some(indices[0]))
            .and_then(|index| self.scene.all().get(index))
    }

    fn has_ancestor_matching(&self, node: &SemanticNode, predicate: &crate::NodePredicate) -> bool {
        let mut parent = self.parent_of(node);
        let mut visited = HashSet::new();
        while let Some(current) = parent {
            if predicate.matches(self, current) {
                return true;
            }
            if !visited.insert(current.id.clone()) {
                return false;
            }
            parent = self.parent_of(current);
        }
        false
    }
}

pub(crate) fn scene_context(scene: &SceneSnapshot, index: usize) -> impl PredicateContext + '_ {
    let _ = index;
    ScenePredicateContext { scene }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodePredicate, Point, QueryError, Size};
    use std::sync::mpsc;
    use std::time::Duration;

    fn rect() -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))
    }

    #[test]
    fn ancestor_predicates_follow_parent_chains() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("branch", Role::Container, rect()).with_parent("root", 0),
            SemanticNode::new("leaf", Role::Label, rect()).with_parent("branch", 0),
        ]);

        let handle = scene
            .find(&NodePredicate::ancestor(NodePredicate::id_eq("branch")))
            .unwrap();
        assert_eq!(scene.node(handle).unwrap().id, "leaf");
    }

    #[test]
    fn ancestor_predicates_stop_on_parent_cycles() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("a", Role::Container, rect()).with_parent("b", 0),
            SemanticNode::new("b", Role::Container, rect()).with_parent("a", 0),
        ]);
        let predicate = NodePredicate::ancestor(NodePredicate::id_eq("missing"));
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = scene.find(&predicate);
            tx.send(result).unwrap();
        });

        let result = rx
            .recv_timeout(Duration::from_millis(200))
            .expect("ancestor lookup should terminate even on cycles");
        assert!(matches!(result, Err(QueryError::NotFoundPredicate(_))));
    }

    #[test]
    fn children_and_parent_queries_use_indexes() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("first", Role::Label, rect()).with_parent("root", 0),
            SemanticNode::new("second", Role::Label, rect()).with_parent("root", 1),
        ]);

        let root = scene.find(&NodePredicate::id_eq("root")).unwrap();
        let children = scene.children_of(root);

        assert_eq!(children.len(), 2);
        assert_eq!(scene.node(children[0]).unwrap().id, "first");
        assert_eq!(scene.node(children[1]).unwrap().id, "second");
        assert_eq!(scene.parent_of(children[0]), Some(root));
    }

    #[test]
    fn duplicate_parent_ids_do_not_merge_children() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("child", Role::Label, rect()).with_parent("root", 0),
        ]);

        let duplicates = scene.find_all(&NodePredicate::id_eq("root"));

        assert_eq!(duplicates.len(), 2);
        assert!(scene.children_of(duplicates[0]).is_empty());
        assert_eq!(
            scene.parent_of(scene.find(&NodePredicate::id_eq("child")).unwrap()),
            None
        );
    }

    #[test]
    fn selector_indexes_support_exact_and_fuzzy_lookup() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("run", Role::Button, rect())
                .with_selector("toolbar.run")
                .with_selector("run"),
            SemanticNode::new("stop", Role::Button, rect()).with_selector("toolbar.stop"),
        ]);

        assert_eq!(scene.count(&NodePredicate::selector_eq("toolbar.run")), 1);
        assert_eq!(
            scene
                .find(&NodePredicate::any_selector(crate::TextMatch::contains(
                    "stop"
                )))
                .unwrap()
                .index(),
            1
        );
        assert!(!scene.exists(&NodePredicate::selector_eq("missing")));
    }

    #[test]
    fn resolve_helpers_expose_bounds_centers_and_counts() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("run", Role::Button, rect()).with_selector("toolbar.run"),
            SemanticNode::new(
                "secondary",
                Role::Button,
                Rect::new(Point::new(20.0, 0.0), Size::new(10.0, 10.0)),
            ),
        ]);

        assert_eq!(scene.count(&NodePredicate::role_eq(Role::Button)), 2);
        assert_eq!(
            scene
                .bounds(&NodePredicate::selector_eq("toolbar.run"))
                .unwrap(),
            rect()
        );
        assert_eq!(
            scene
                .center(&NodePredicate::selector_eq("toolbar.run"))
                .unwrap(),
            Point::new(5.0, 5.0)
        );
        assert_eq!(
            scene
                .all_bounds(&NodePredicate::role_eq(Role::Button))
                .len(),
            2
        );
    }

    #[test]
    fn visible_bounds_fall_back_to_raw_bounds_when_visible_rect_is_unknown() {
        let visible =
            SceneSnapshot::new(vec![SemanticNode::new("provider", Role::Button, rect())
                .with_selector("provider.button")]);

        assert_eq!(
            visible
                .visible_bounds(&NodePredicate::selector_eq("provider.button"))
                .unwrap(),
            rect()
        );
        assert_eq!(
            visible
                .resolve(&NodePredicate::selector_eq("provider.button"))
                .unwrap()
                .visible_bounds,
            Some(rect())
        );

        let hidden = SceneSnapshot::new(vec![SemanticNode {
            visible: false,
            ..SemanticNode::new("provider", Role::Button, rect()).with_selector("provider.button")
        }]);
        assert!(hidden
            .visible_bounds(&NodePredicate::selector_eq("provider.button"))
            .is_err());
        assert_eq!(
            hidden
                .resolve(&NodePredicate::selector_eq("provider.button"))
                .unwrap()
                .visible_bounds,
            None
        );
    }

    #[test]
    fn hit_path_orders_descendants_above_their_ancestors_within_one_subtree() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new(
                "parent",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 20.0)),
            ),
            SemanticNode::new(
                "child",
                Role::Button,
                Rect::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
            )
            .with_parent("parent", 0),
        ]);

        let hit_path = scene.hit_path_at(Point::new(10.0, 10.0));
        assert_eq!(scene.node(hit_path[0]).unwrap().id, "child");
        assert_eq!(scene.topmost_at(Point::new(10.0, 10.0)), Some(hit_path[0]));
    }

    #[test]
    fn hit_path_prefers_later_painted_sibling_subtrees_over_deeper_descendants() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new(
                "root",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 20.0)),
            ),
            SemanticNode::new(
                "branch",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 20.0)),
            )
            .with_parent("root", 0),
            SemanticNode::new(
                "leaf",
                Role::Button,
                Rect::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
            )
            .with_parent("branch", 0),
            SemanticNode::new(
                "overlay",
                Role::Button,
                Rect::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
            )
            .with_parent("root", 1),
        ]);

        let hit_path = scene.hit_path_at(Point::new(10.0, 10.0));
        let hit_ids = hit_path
            .into_iter()
            .map(|handle| scene.node(handle).unwrap().id.clone())
            .collect::<Vec<_>>();

        assert_eq!(hit_ids[0], "overlay");
        assert_eq!(hit_ids[1], "leaf");
        assert_eq!(
            scene
                .node(scene.topmost_at(Point::new(10.0, 10.0)).unwrap())
                .unwrap()
                .id,
            "overlay"
        );
    }

    #[test]
    fn hit_path_prefers_explicit_native_paint_paths_when_registered_parent_chain_is_reduced() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new(
                "root",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 20.0)),
            )
            .with_property(PAINT_ORDER_PATH_PROPERTY, PropertyValue::string("0")),
            SemanticNode::new(
                "left-leaf",
                Role::Button,
                Rect::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
            )
            .with_parent("root", 0)
            .with_property(PAINT_ORDER_PATH_PROPERTY, PropertyValue::string("0/0/0")),
            SemanticNode::new(
                "right-overlay",
                Role::Button,
                Rect::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
            )
            .with_parent("root", 0)
            .with_property(PAINT_ORDER_PATH_PROPERTY, PropertyValue::string("0/1/0")),
        ]);

        let hit_path = scene.hit_path_at(Point::new(10.0, 10.0));
        let hit_ids = hit_path
            .into_iter()
            .map(|handle| scene.node(handle).unwrap().id.clone())
            .collect::<Vec<_>>();

        assert_eq!(hit_ids[0], "right-overlay");
        assert_eq!(hit_ids[1], "left-leaf");
        assert!(matches!(
            scene.interactability(&NodePredicate::id_eq("left-leaf")),
            Ok(Interactability::Occluded { topmost, .. })
                if scene.node(topmost).unwrap().id == "right-overlay"
        ));
    }

    #[test]
    fn interactability_distinguishes_interactable_and_occluded_nodes() {
        let target = SemanticNode::new(
            "target",
            Role::Button,
            Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0)),
        );
        let interactable_scene = SceneSnapshot::new(vec![target.clone()]);
        assert!(matches!(
            interactable_scene.interactability(&NodePredicate::id_eq("target")),
            Ok(Interactability::Interactable { .. })
        ));

        let occluded_scene = SceneSnapshot::new(vec![
            SemanticNode::new(
                "root",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 20.0)),
            ),
            SemanticNode::new(
                "branch",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 20.0)),
            )
            .with_parent("root", 0),
            target.with_parent("branch", 0),
            SemanticNode::new(
                "overlay",
                Role::Button,
                Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0)),
            )
            .with_parent("root", 1),
        ]);

        assert!(matches!(
            occluded_scene.interactability(&NodePredicate::id_eq("target")),
            Ok(Interactability::Occluded { topmost, .. })
                if occluded_scene.node(topmost).unwrap().id == "overlay"
        ));
    }

    #[test]
    fn interactability_uses_fallback_probe_points_when_center_is_occluded() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new(
                "target",
                Role::Button,
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 12.0)),
            ),
            SemanticNode::new(
                "overlay",
                Role::Button,
                Rect::new(Point::new(4.0, 4.0), Size::new(4.0, 4.0)),
            )
            .with_property(PAINT_ORDER_PATH_PROPERTY, PropertyValue::string("1")),
        ]);

        assert!(matches!(
            scene.interactability(&NodePredicate::id_eq("target")),
            Ok(Interactability::Interactable { hit_point })
                if hit_point != Point::new(6.0, 6.0)
        ));
    }
}
