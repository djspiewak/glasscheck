pub use crate::scene::PropertyValue;

use crate::{scene::PredicateContext, scene::Role, Rect, SceneSnapshot, SemanticNode};

/// Compatibility metadata captured for a UI node under test.
///
/// Prefer `SemanticNode` for new tests. This type exists for flat metadata
/// cases and for migrating older query-based tests.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeMetadata {
    /// Semantic identifier captured for a specific snapshot.
    ///
    /// This is suitable for exact matching within the current snapshot, but it may change across
    /// snapshots when IDs are disambiguated or namespaced during snapshot construction.
    pub id: Option<String>,
    /// Semantic role for the node.
    pub role: Option<Role>,
    /// Human-readable label for the node.
    pub label: Option<String>,
    /// Bounds of the node in root coordinates.
    pub rect: Rect,
}

impl From<&SemanticNode> for NodeMetadata {
    fn from(value: &SemanticNode) -> Self {
        Self {
            id: Some(value.id.clone()),
            role: Some(value.role.clone()),
            label: value.label.clone(),
            rect: value.rect,
        }
    }
}

/// A semantic selector used to find instrumented nodes.
///
/// `Selector` only supports exact ID, role, and label matching. Use
/// `NodePredicate` when tests need selectors, hierarchy, properties, state, or
/// text matching rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selector {
    /// Required node identifier.
    pub id: Option<String>,
    /// Required node role.
    pub role: Option<Role>,
    /// Required node label.
    pub label: Option<String>,
}

impl Selector {
    /// Creates a selector that matches a node by the snapshot-local semantic ID.
    #[must_use]
    pub fn by_id(id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            role: None,
            label: None,
        }
    }

    /// Creates a selector that matches a node by semantic role.
    #[must_use]
    pub fn by_role(role: Role) -> Self {
        Self {
            id: None,
            role: Some(role),
            label: None,
        }
    }

    /// Creates a selector that matches a node by label.
    #[must_use]
    pub fn by_label(label: impl Into<String>) -> Self {
        Self {
            id: None,
            role: None,
            label: Some(label.into()),
        }
    }

    /// Returns `true` when all populated selector fields match `node`.
    #[must_use]
    pub fn matches(&self, node: &NodeMetadata) -> bool {
        self.id
            .as_ref()
            .is_none_or(|id| node.id.as_ref() == Some(id))
            && self
                .role
                .as_ref()
                .is_none_or(|role| node.role.as_ref() == Some(role))
            && self
                .label
                .as_ref()
                .is_none_or(|label| node.label.as_ref() == Some(label))
    }
}

/// Errors returned when resolving selectors or predicates against a scene.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryError {
    /// No nodes matched the selector.
    NotFound(Selector),
    /// More than one node matched a selector that expected a single result.
    MultipleMatches { selector: Selector, count: usize },
    /// No nodes matched the predicate.
    NotFoundPredicate(crate::NodePredicate),
    /// More than one node matched a predicate that expected a single result.
    MultiplePredicateMatches {
        predicate: crate::NodePredicate,
        count: usize,
    },
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(_) => write!(f, "no nodes matched selector"),
            Self::MultipleMatches { count, .. } => write!(f, "selector matched {} nodes", count),
            Self::NotFoundPredicate(_) => write!(f, "no nodes matched predicate"),
            Self::MultiplePredicateMatches { count, .. } => {
                write!(f, "predicate matched {} nodes", count)
            }
        }
    }
}

impl std::error::Error for QueryError {}

/// A compatibility wrapper over the richer scene snapshot model.
///
/// This is useful when an existing test suite already thinks in terms of
/// flat metadata lookup. The tradeoff is that richer scene relationships are
/// only available when the root is backed by a `SceneSnapshot`.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryRoot {
    scene: Option<SceneSnapshot>,
    nodes: Vec<NodeMetadata>,
    compatibility: bool,
}

impl Default for QueryRoot {
    fn default() -> Self {
        Self {
            scene: None,
            nodes: Vec::new(),
            compatibility: true,
        }
    }
}

impl QueryRoot {
    /// Creates a query root from collected node metadata.
    #[must_use]
    pub fn new(nodes: Vec<NodeMetadata>) -> Self {
        Self {
            scene: None,
            nodes,
            compatibility: true,
        }
    }

    /// Creates a query root from a richer scene snapshot.
    ///
    /// Use this when callers still want `Selector`-style queries but already
    /// have a full scene snapshot available.
    #[must_use]
    pub fn from_scene(scene: SceneSnapshot) -> Self {
        let nodes = scene.all().iter().map(NodeMetadata::from).collect();
        Self {
            scene: Some(scene),
            nodes,
            compatibility: false,
        }
    }

    /// Returns the underlying scene snapshot when this root was built from one.
    #[must_use]
    pub fn scene(&self) -> Option<&SceneSnapshot> {
        self.scene.as_ref()
    }

    /// Returns all nodes in the query root.
    #[must_use]
    pub fn all(&self) -> &[NodeMetadata] {
        &self.nodes
    }

    /// Returns `true` when the root was built from compatibility metadata.
    #[must_use]
    pub(crate) fn is_compatibility_root(&self) -> bool {
        self.compatibility
    }

    #[must_use]
    pub(crate) fn backing_scene(&self) -> &SceneSnapshot {
        self.scene
            .as_ref()
            .expect("scene-backed query roots must carry a scene snapshot")
    }

    /// Finds exactly one node matching `selector`.
    pub fn find(&self, selector: &Selector) -> Result<&NodeMetadata, QueryError> {
        let matches: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| selector.matches(node))
            .collect();
        match matches.as_slice() {
            [] => Err(QueryError::NotFound(selector.clone())),
            [node] => Ok(node),
            _ => Err(QueryError::MultipleMatches {
                selector: selector.clone(),
                count: matches.len(),
            }),
        }
    }

    /// Returns all nodes matching `selector`.
    #[must_use]
    pub fn find_all(&self, selector: &Selector) -> Vec<&NodeMetadata> {
        self.nodes
            .iter()
            .filter(|node| selector.matches(node))
            .collect()
    }
}

/// Returns `true` when `predicate` can be faithfully evaluated against compatibility metadata.
#[must_use]
pub(crate) fn predicate_is_metadata_supported(predicate: &NodePredicate) -> bool {
    match predicate {
        NodePredicate::IdEq(_) | NodePredicate::RoleEq(_) | NodePredicate::Label(_) => true,
        NodePredicate::Value(_)
        | NodePredicate::SelectorEq(_)
        | NodePredicate::AnySelector(_)
        | NodePredicate::ClassEq(_)
        | NodePredicate::TagEq(_)
        | NodePredicate::PropertyEq(_, _)
        | NodePredicate::StateEq(_, _)
        | NodePredicate::Parent(_)
        | NodePredicate::Ancestor(_) => false,
        NodePredicate::Not(predicate) => predicate_is_metadata_supported(predicate),
        NodePredicate::And(predicates) | NodePredicate::Or(predicates) => {
            predicates.iter().all(predicate_is_metadata_supported)
        }
    }
}

/// Returns `true` when `predicate` matches `node` under compatibility metadata semantics.
#[must_use]
pub(crate) fn predicate_matches_metadata(predicate: &NodePredicate, node: &NodeMetadata) -> bool {
    match predicate {
        NodePredicate::IdEq(id) => node.id.as_deref() == Some(id.as_str()),
        NodePredicate::RoleEq(role) => node.role.as_ref() == Some(role),
        NodePredicate::Label(matcher) => node
            .label
            .as_deref()
            .is_some_and(|label| matcher.matches(label)),
        NodePredicate::Value(_)
        | NodePredicate::SelectorEq(_)
        | NodePredicate::AnySelector(_)
        | NodePredicate::ClassEq(_)
        | NodePredicate::TagEq(_)
        | NodePredicate::PropertyEq(_, _)
        | NodePredicate::StateEq(_, _)
        | NodePredicate::Parent(_)
        | NodePredicate::Ancestor(_) => false,
        NodePredicate::Not(predicate) => !predicate_matches_metadata(predicate, node),
        NodePredicate::And(predicates) => predicates
            .iter()
            .all(|predicate| predicate_matches_metadata(predicate, node)),
        NodePredicate::Or(predicates) => predicates
            .iter()
            .any(|predicate| predicate_matches_metadata(predicate, node)),
    }
}

/// Text matching semantics for semantic node predicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextMatch {
    /// Match a string exactly.
    Exact(String),
    /// Match when the candidate contains the substring.
    Contains(String),
    /// Match when the candidate starts with the prefix.
    StartsWith(String),
    /// Match when the candidate ends with the suffix.
    EndsWith(String),
    /// Match using lowercase normalization on both matcher and candidate.
    CaseInsensitive(Box<TextMatch>),
    /// Match after trimming and collapsing repeated whitespace.
    NormalizedWhitespace(Box<TextMatch>),
    /// Match when any alternative matches.
    OneOf(Vec<TextMatch>),
}

impl TextMatch {
    /// Creates an exact text matcher.
    #[must_use]
    pub fn exact(value: impl Into<String>) -> Self {
        Self::Exact(value.into())
    }

    /// Creates a substring matcher.
    #[must_use]
    pub fn contains(value: impl Into<String>) -> Self {
        Self::Contains(value.into())
    }

    /// Creates a prefix matcher.
    #[must_use]
    pub fn starts_with(value: impl Into<String>) -> Self {
        Self::StartsWith(value.into())
    }

    /// Creates a suffix matcher.
    #[must_use]
    pub fn ends_with(value: impl Into<String>) -> Self {
        Self::EndsWith(value.into())
    }

    /// Creates a case-insensitive matcher.
    #[must_use]
    pub fn case_insensitive(matcher: TextMatch) -> Self {
        Self::CaseInsensitive(Box::new(matcher))
    }

    /// Creates a matcher that normalizes whitespace before matching.
    #[must_use]
    pub fn normalized_whitespace(matcher: TextMatch) -> Self {
        Self::NormalizedWhitespace(Box::new(matcher))
    }

    /// Creates a matcher that succeeds when any alternative matches.
    #[must_use]
    pub fn one_of(matchers: Vec<TextMatch>) -> Self {
        Self::OneOf(matchers)
    }

    /// Returns `true` when `candidate` satisfies this matcher.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact(expected) => candidate == expected,
            Self::Contains(expected) => candidate.contains(expected),
            Self::StartsWith(expected) => candidate.starts_with(expected),
            Self::EndsWith(expected) => candidate.ends_with(expected),
            Self::CaseInsensitive(matcher) => matcher_matches_case_insensitive(matcher, candidate),
            Self::NormalizedWhitespace(matcher) => {
                matcher.matches(&normalize_whitespace(candidate))
            }
            Self::OneOf(matchers) => matchers.iter().any(|matcher| matcher.matches(candidate)),
        }
    }
}

/// Composable predicate for semantic node lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodePredicate {
    /// Match an exact semantic identifier.
    IdEq(String),
    /// Match an exact selector alias.
    SelectorEq(String),
    /// Match any selector using the supplied text semantics.
    AnySelector(TextMatch),
    /// Match an exact semantic role.
    RoleEq(Role),
    /// Match a label with the supplied text semantics.
    Label(TextMatch),
    /// Match a value with the supplied text semantics.
    Value(TextMatch),
    /// Match any semantic class exactly.
    ClassEq(String),
    /// Match any semantic tag exactly.
    TagEq(String),
    /// Match a structured property exactly.
    PropertyEq(String, PropertyValue),
    /// Match a structured state value exactly.
    StateEq(String, PropertyValue),
    /// Match when the parent satisfies the predicate.
    Parent(Box<NodePredicate>),
    /// Match when any ancestor satisfies the predicate.
    Ancestor(Box<NodePredicate>),
    /// Negate a predicate.
    Not(Box<NodePredicate>),
    /// Conjunction of multiple predicates.
    And(Vec<NodePredicate>),
    /// Disjunction of multiple predicates.
    Or(Vec<NodePredicate>),
}

impl NodePredicate {
    /// Creates an exact semantic-ID predicate.
    #[must_use]
    pub fn id_eq(id: impl Into<String>) -> Self {
        Self::IdEq(id.into())
    }

    /// Creates an exact selector predicate.
    #[must_use]
    pub fn selector_eq(selector: impl Into<String>) -> Self {
        Self::SelectorEq(selector.into())
    }

    /// Creates an any-selector predicate.
    #[must_use]
    pub fn any_selector(matcher: TextMatch) -> Self {
        Self::AnySelector(matcher)
    }

    /// Creates an exact semantic-role predicate.
    #[must_use]
    pub fn role_eq(role: Role) -> Self {
        Self::RoleEq(role)
    }

    /// Creates a label predicate.
    #[must_use]
    pub fn label(matcher: TextMatch) -> Self {
        Self::Label(matcher)
    }

    /// Creates a property predicate.
    #[must_use]
    pub fn property_eq(key: impl Into<String>, value: PropertyValue) -> Self {
        Self::PropertyEq(key.into(), value)
    }

    /// Creates a state predicate.
    #[must_use]
    pub fn state_eq(key: impl Into<String>, value: PropertyValue) -> Self {
        Self::StateEq(key.into(), value)
    }

    /// Creates a parent-constrained predicate.
    #[must_use]
    pub fn parent(predicate: NodePredicate) -> Self {
        Self::Parent(Box::new(predicate))
    }

    /// Creates an ancestor-constrained predicate.
    #[must_use]
    pub fn ancestor(predicate: NodePredicate) -> Self {
        Self::Ancestor(Box::new(predicate))
    }

    /// Negates a predicate.
    #[must_use]
    pub fn not(predicate: NodePredicate) -> Self {
        Self::Not(Box::new(predicate))
    }

    /// Combines predicates with conjunction semantics.
    #[must_use]
    pub fn and(predicates: Vec<NodePredicate>) -> Self {
        Self::And(predicates)
    }

    /// Combines predicates with disjunction semantics.
    #[must_use]
    pub fn or(predicates: Vec<NodePredicate>) -> Self {
        Self::Or(predicates)
    }

    /// Returns `true` when `node` satisfies the predicate.
    #[must_use]
    pub(crate) fn matches<C: PredicateContext>(&self, context: &C, node: &SemanticNode) -> bool {
        match self {
            Self::IdEq(id) => node.id == *id,
            Self::SelectorEq(selector) => node.selectors.contains(selector),
            Self::AnySelector(matcher) => node
                .selectors
                .iter()
                .any(|selector| matcher.matches(selector)),
            Self::RoleEq(role) => node.role == *role,
            Self::Label(matcher) => node
                .label
                .as_deref()
                .is_some_and(|label| matcher.matches(label)),
            Self::Value(matcher) => node
                .value
                .as_deref()
                .is_some_and(|value| matcher.matches(value)),
            Self::ClassEq(class) => node.classes.iter().any(|candidate| candidate == class),
            Self::TagEq(tag) => node.tags.iter().any(|candidate| candidate == tag),
            Self::PropertyEq(key, value) => node.properties.get(key) == Some(value),
            Self::StateEq(key, value) => node.state.get(key) == Some(value),
            Self::Parent(predicate) => context
                .parent_of(node)
                .is_some_and(|parent| predicate.matches(context, parent)),
            Self::Ancestor(predicate) => context.has_ancestor_matching(node, predicate),
            Self::Not(predicate) => !predicate.matches(context, node),
            Self::And(predicates) => predicates
                .iter()
                .all(|predicate| predicate.matches(context, node)),
            Self::Or(predicates) => predicates
                .iter()
                .any(|predicate| predicate.matches(context, node)),
        }
    }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn matcher_matches_case_insensitive(matcher: &TextMatch, candidate: &str) -> bool {
    let lower_candidate = candidate.to_lowercase();
    match matcher {
        TextMatch::Exact(expected) => lower_candidate == expected.to_lowercase(),
        TextMatch::Contains(expected) => lower_candidate.contains(&expected.to_lowercase()),
        TextMatch::StartsWith(expected) => lower_candidate.starts_with(&expected.to_lowercase()),
        TextMatch::EndsWith(expected) => lower_candidate.ends_with(&expected.to_lowercase()),
        TextMatch::CaseInsensitive(inner) => matcher_matches_case_insensitive(inner, candidate),
        TextMatch::NormalizedWhitespace(inner) => {
            matcher_matches_case_insensitive(inner, &normalize_whitespace(candidate))
        }
        TextMatch::OneOf(matchers) => matchers
            .iter()
            .any(|matcher| matcher_matches_case_insensitive(matcher, candidate)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodePredicate, Point, PropertyValue, RegionResolveError, Role, Size, TextMatch};

    fn rect() -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))
    }

    #[test]
    fn find_by_id_returns_match() {
        let root = QueryRoot::new(vec![NodeMetadata {
            id: Some("editor".into()),
            role: Some(Role::TextInput),
            label: Some("Editor".into()),
            rect: rect(),
        }]);

        let node = root.find(&Selector::by_id("editor")).unwrap();
        assert_eq!(node.label.as_deref(), Some("Editor"));
    }

    #[test]
    fn multiple_matches_fail() {
        let root = QueryRoot::new(vec![
            NodeMetadata {
                id: Some("a".into()),
                role: Some(Role::Button),
                label: Some("Run".into()),
                rect: rect(),
            },
            NodeMetadata {
                id: Some("b".into()),
                role: Some(Role::Button),
                label: Some("Run".into()),
                rect: rect(),
            },
        ]);

        let error = root.find(&Selector::by_label("Run")).unwrap_err();
        assert!(matches!(error, QueryError::MultipleMatches { .. }));
    }

    #[test]
    fn compatibility_roots_do_not_default_missing_rich_fields() {
        let root = QueryRoot::new(vec![NodeMetadata {
            id: Some("panel".into()),
            role: None,
            label: Some("Inspector".into()),
            rect: rect(),
        }]);

        assert_eq!(
            root.find_by_predicate(&NodePredicate::label(TextMatch::exact("Inspector")))
                .unwrap()
                .id
                .as_deref(),
            Some("panel")
        );
        assert!(matches!(
            root.find_by_predicate(&NodePredicate::role_eq(Role::Container)),
            Err(RegionResolveError::NotFound(_))
        ));
        assert!(matches!(
            root.find_by_predicate(&NodePredicate::property_eq(
                "session_id",
                PropertyValue::Integer(7)
            )),
            Err(RegionResolveError::NotFound(_))
        ));
        assert!(root
            .find_all_by_predicate(&NodePredicate::ancestor(NodePredicate::id_eq("panel")))
            .is_empty());
        assert!(root.scene().is_none());
    }

    #[test]
    fn scene_backed_roots_expose_original_scene() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("panel", Role::Container, rect()).with_label("Inspector")
        ]);
        let root = QueryRoot::from_scene(scene.clone());

        assert_eq!(root.scene(), Some(&scene));
    }

    #[test]
    fn default_query_root_is_an_empty_compatibility_root() {
        let root = QueryRoot::default();

        assert!(root.scene().is_none());
        assert!(root
            .find_all_by_predicate(&NodePredicate::id_eq("missing"))
            .is_empty());
        assert!(matches!(
            root.find_by_predicate(&NodePredicate::id_eq("missing")),
            Err(RegionResolveError::NotFound(_))
        ));
    }

    #[test]
    fn selector_predicates_match_scene_backed_selectors() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("primary", Role::Button, rect())
                .with_selector("toolbar.run")
                .with_selector("run"),
            SemanticNode::new("secondary", Role::Button, rect()).with_selector("toolbar.stop"),
        ]);

        let root = QueryRoot::from_scene(scene);
        assert_eq!(
            root.find_by_predicate(&NodePredicate::selector_eq("toolbar.run"))
                .unwrap()
                .id
                .as_deref(),
            Some("primary")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::any_selector(TextMatch::contains("stop")))
                .unwrap()
                .id
                .as_deref(),
            Some("secondary")
        );
    }

    #[test]
    fn selector_predicates_do_not_match_compatibility_roots() {
        let root = QueryRoot::new(vec![NodeMetadata {
            id: Some("button".into()),
            role: Some(Role::Button),
            label: Some("Run".into()),
            rect: rect(),
        }]);

        assert!(matches!(
            root.find_by_predicate(&NodePredicate::selector_eq("toolbar.run")),
            Err(RegionResolveError::NotFound(_))
        ));
        assert!(root
            .find_all_by_predicate(&NodePredicate::any_selector(TextMatch::contains("run")))
            .is_empty());
    }

    #[test]
    fn text_match_extensions_cover_positive_and_negative_cases() {
        assert!(TextMatch::ends_with("Panel").matches("Editor Panel"));
        assert!(!TextMatch::ends_with("Panel").matches("Panel Editor"));

        assert!(TextMatch::case_insensitive(TextMatch::exact("run")).matches("RUN"));
        assert!(!TextMatch::case_insensitive(TextMatch::exact("run")).matches("stop"));

        assert!(
            TextMatch::normalized_whitespace(TextMatch::exact("Run Tests"))
                .matches("  Run   Tests ")
        );
        assert!(
            !TextMatch::normalized_whitespace(TextMatch::exact("Run Tests")).matches("Run Faster")
        );

        assert!(
            TextMatch::one_of(vec![TextMatch::exact("Run"), TextMatch::exact("Build")])
                .matches("Build")
        );
        assert!(
            !TextMatch::one_of(vec![TextMatch::exact("Run"), TextMatch::exact("Build")])
                .matches("Test")
        );
    }
}
