use crate::Rect;

/// A semantic role attached to an instrumented UI node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// A top-level window.
    Window,
    /// A generic container view.
    Container,
    /// A clickable button-like control.
    Button,
    /// A text-editing control.
    TextInput,
    /// A static text label.
    Label,
    /// A caller-defined semantic role.
    Custom(&'static str),
}

/// Semantic metadata captured for a UI node under test.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeMetadata {
    /// Stable semantic identifier for the node.
    pub id: Option<String>,
    /// Semantic role for the node.
    pub role: Option<Role>,
    /// Human-readable label for the node.
    pub label: Option<String>,
    /// Bounds of the node in root coordinates.
    pub rect: Rect,
}

/// A semantic selector used to find instrumented nodes.
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
    /// Creates a selector that matches a node by semantic ID.
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

/// Errors returned when resolving selectors against a query tree.
#[derive(Debug)]
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
            Self::MultipleMatches { count, .. } => {
                write!(f, "selector matched {} nodes", count)
            }
        }
    }
}

impl std::error::Error for QueryError {}

/// A flat semantic view of the UI nodes available for querying.
#[derive(Clone, Debug, Default)]
pub struct QueryRoot {
    nodes: Vec<NodeMetadata>,
}

impl QueryRoot {
    /// Creates a query root from collected node metadata.
    #[must_use]
    pub fn new(nodes: Vec<NodeMetadata>) -> Self {
        Self { nodes }
    }

    /// Returns all nodes in the query root.
    #[must_use]
    pub fn all(&self) -> &[NodeMetadata] {
        &self.nodes
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Size};

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
}
