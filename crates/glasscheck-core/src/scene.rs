use std::collections::{BTreeMap, HashSet};

use crate::Rect;

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
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// Stable semantic identifier for the node.
    pub id: String,
    /// Semantic role for the node.
    pub role: Role,
    /// Bounds of the node in root coordinates.
    pub rect: Rect,
    /// Optional user-visible label.
    pub label: Option<String>,
    /// Optional textual value.
    pub value: Option<String>,
    /// Parent node identifier when this node is part of a hierarchy.
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
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSnapshot {
    nodes: Vec<SemanticNode>,
    id_index: BTreeMap<String, Vec<usize>>,
    children_index: BTreeMap<String, Vec<usize>>,
}

impl Default for SceneSnapshot {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl SceneSnapshot {
    /// Creates a scene snapshot from collected semantic nodes.
    #[must_use]
    pub fn new(nodes: Vec<SemanticNode>) -> Self {
        let mut id_index = BTreeMap::<String, Vec<usize>>::new();
        let mut children_index = BTreeMap::<String, Vec<usize>>::new();

        for (index, node) in nodes.iter().enumerate() {
            id_index.entry(node.id.clone()).or_default().push(index);
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
        if let crate::NodePredicate::IdEq(id) = predicate {
            return self
                .id_index
                .get(id)
                .into_iter()
                .flat_map(|indices| indices.iter().copied())
                .map(|index| NodeHandle { index })
                .collect();
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
}

/// Application-supplied virtual semantic provider.
pub trait SemanticProvider {
    /// Produces the current semantic nodes for the live scene.
    fn snapshot_nodes(&self) -> Vec<SemanticNode>;
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
}
