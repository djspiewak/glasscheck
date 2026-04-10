use crate::Rect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Window,
    Container,
    Button,
    TextInput,
    Label,
    Custom(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeMetadata {
    pub id: Option<String>,
    pub role: Option<Role>,
    pub label: Option<String>,
    pub rect: Rect,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selector {
    pub id: Option<String>,
    pub role: Option<Role>,
    pub label: Option<String>,
}

impl Selector {
    #[must_use]
    pub fn by_id(id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            role: None,
            label: None,
        }
    }

    #[must_use]
    pub fn by_role(role: Role) -> Self {
        Self {
            id: None,
            role: Some(role),
            label: None,
        }
    }

    #[must_use]
    pub fn by_label(label: impl Into<String>) -> Self {
        Self {
            id: None,
            role: None,
            label: Some(label.into()),
        }
    }

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

#[derive(Debug)]
pub enum QueryError {
    NotFound(Selector),
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

#[derive(Clone, Debug, Default)]
pub struct QueryRoot {
    nodes: Vec<NodeMetadata>,
}

impl QueryRoot {
    #[must_use]
    pub fn new(nodes: Vec<NodeMetadata>) -> Self {
        Self { nodes }
    }

    #[must_use]
    pub fn all(&self) -> &[NodeMetadata] {
        &self.nodes
    }

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
