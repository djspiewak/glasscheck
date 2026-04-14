pub use crate::scene::PropertyValue;

use crate::{scene::PredicateContext, scene::Role, SemanticNode};

/// Errors returned when resolving selectors against a scene.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryError {
    /// No nodes matched the selector.
    NotFound(Selector),
    /// More than one node matched a selector that expected a single result.
    MultipleMatches { selector: Selector, count: usize },
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(_) => write!(f, "no nodes matched selector"),
            Self::MultipleMatches { count, .. } => write!(f, "selector matched {} nodes", count),
        }
    }
}

impl std::error::Error for QueryError {}

/// Text matching semantics for semantic selectors.
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

/// Composable selector for semantic node lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
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
    /// Match when the parent satisfies the selector.
    Parent(Box<Selector>),
    /// Match when any ancestor satisfies the selector.
    Ancestor(Box<Selector>),
    /// Negate a selector.
    Not(Box<Selector>),
    /// Conjunction of multiple selectors.
    And(Vec<Selector>),
    /// Disjunction of multiple selectors.
    Or(Vec<Selector>),
}

impl Selector {
    /// Creates an exact semantic-ID selector.
    #[must_use]
    pub fn id_eq(id: impl Into<String>) -> Self {
        Self::IdEq(id.into())
    }

    /// Creates an exact selector-alias selector.
    #[must_use]
    pub fn selector_eq(selector: impl Into<String>) -> Self {
        Self::SelectorEq(selector.into())
    }

    /// Creates an any-selector selector.
    #[must_use]
    pub fn any_selector(matcher: TextMatch) -> Self {
        Self::AnySelector(matcher)
    }

    /// Creates an exact semantic-role selector.
    #[must_use]
    pub fn role_eq(role: Role) -> Self {
        Self::RoleEq(role)
    }

    /// Creates a label selector.
    #[must_use]
    pub fn label(matcher: TextMatch) -> Self {
        Self::Label(matcher)
    }

    /// Creates a property selector.
    #[must_use]
    pub fn property_eq(key: impl Into<String>, value: PropertyValue) -> Self {
        Self::PropertyEq(key.into(), value)
    }

    /// Creates a state selector.
    #[must_use]
    pub fn state_eq(key: impl Into<String>, value: PropertyValue) -> Self {
        Self::StateEq(key.into(), value)
    }

    /// Creates a parent-constrained selector.
    #[must_use]
    pub fn parent(selector: Selector) -> Self {
        Self::Parent(Box::new(selector))
    }

    /// Creates an ancestor-constrained selector.
    #[must_use]
    pub fn ancestor(selector: Selector) -> Self {
        Self::Ancestor(Box::new(selector))
    }

    /// Negates a selector.
    #[must_use]
    pub fn not(selector: Selector) -> Self {
        Self::Not(Box::new(selector))
    }

    /// Combines selectors with conjunction semantics.
    #[must_use]
    pub fn and(selectors: Vec<Selector>) -> Self {
        Self::And(selectors)
    }

    /// Combines selectors with disjunction semantics.
    #[must_use]
    pub fn or(selectors: Vec<Selector>) -> Self {
        Self::Or(selectors)
    }

    /// Returns `true` when `node` satisfies the selector.
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
    use crate::{Point, QueryError, Rect, Role, Scene, SemanticNode, Size, TextMatch};

    fn rect() -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))
    }

    #[test]
    fn scene_find_by_id_returns_match() {
        let scene = Scene::new(vec![
            SemanticNode::new("editor", Role::TextInput, rect()).with_label("Editor")
        ]);

        let handle = scene.find(&Selector::id_eq("editor")).unwrap();
        assert_eq!(scene.node(handle).unwrap().label.as_deref(), Some("Editor"));
    }

    #[test]
    fn multiple_matches_fail() {
        let scene = Scene::new(vec![
            SemanticNode::new("a", Role::Button, rect()).with_label("Run"),
            SemanticNode::new("b", Role::Button, rect()).with_label("Run"),
        ]);

        let error = scene
            .find(&Selector::label(TextMatch::exact("Run")))
            .unwrap_err();
        assert!(matches!(error, QueryError::MultipleMatches { .. }));
    }

    #[test]
    fn selector_supports_scene_only_queries() {
        let scene = Scene::new(vec![
            SemanticNode::new("primary", Role::Button, rect())
                .with_selector("toolbar.run")
                .with_selector("run"),
            SemanticNode::new("secondary", Role::Button, rect()).with_selector("toolbar.stop"),
        ]);

        assert_eq!(
            scene
                .node(scene.find(&Selector::selector_eq("toolbar.run")).unwrap())
                .unwrap()
                .id
                .as_str(),
            "primary"
        );
        assert_eq!(
            scene
                .node(
                    scene
                        .find(&Selector::any_selector(TextMatch::contains("stop")))
                        .unwrap(),
                )
                .unwrap()
                .id
                .as_str(),
            "secondary"
        );
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
