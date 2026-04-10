use crate::{NodeMetadata, QueryRoot, Rect};

/// Text matching semantics for semantic node predicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextMatch {
    /// Match a string exactly.
    Exact(String),
    /// Match when the candidate contains the substring.
    Contains(String),
    /// Match when the candidate starts with the prefix.
    StartsWith(String),
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

    /// Returns `true` when `candidate` satisfies this matcher.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        match self {
            Self::Exact(expected) => candidate == expected,
            Self::Contains(expected) => candidate.contains(expected),
            Self::StartsWith(expected) => candidate.starts_with(expected),
        }
    }
}

/// Composable predicate for semantic node lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodePredicate {
    /// Match an exact semantic identifier.
    IdEq(String),
    /// Match an exact semantic role.
    RoleEq(crate::Role),
    /// Match a label with the supplied text semantics.
    Label(TextMatch),
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

    /// Creates an exact semantic-role predicate.
    #[must_use]
    pub fn role_eq(role: crate::Role) -> Self {
        Self::RoleEq(role)
    }

    /// Creates a label predicate.
    #[must_use]
    pub fn label(matcher: TextMatch) -> Self {
        Self::Label(matcher)
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
    pub fn matches(&self, node: &NodeMetadata) -> bool {
        match self {
            Self::IdEq(id) => node.id.as_ref() == Some(id),
            Self::RoleEq(role) => node.role.as_ref() == Some(role),
            Self::Label(matcher) => node
                .label
                .as_deref()
                .is_some_and(|label| matcher.matches(label)),
            Self::And(predicates) => predicates.iter().all(|predicate| predicate.matches(node)),
            Self::Or(predicates) => predicates.iter().any(|predicate| predicate.matches(node)),
        }
    }
}

/// Bounds expressed relative to an anchor's resolved rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeBounds {
    /// Relative horizontal origin in the range `[0.0, 1.0]`.
    pub x: f64,
    /// Relative vertical origin in the range `[0.0, 1.0]`.
    pub y: f64,
    /// Relative width in the range `[0.0, 1.0]`.
    pub width: f64,
    /// Relative height in the range `[0.0, 1.0]`.
    pub height: f64,
}

impl RelativeBounds {
    /// Creates relative bounds from fractional components.
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns bounds covering the full anchor region.
    #[must_use]
    pub const fn full() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }

    /// Returns relative bounds inset by fractional amounts on each edge.
    #[must_use]
    pub fn inset(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self::new(left, top, 1.0 - left - right, 1.0 - top - bottom)
    }
}

/// A semantic or derived anchor for resolving a region.
#[derive(Clone, Debug, PartialEq)]
pub enum Anchor {
    /// Anchor to the full window/root region.
    Root,
    /// Anchor to the unique node matching the predicate.
    Node(NodePredicate),
    /// Anchor to a previously described region.
    Region(Box<RegionSpec>),
}

/// Declarative specification of a region derived from an anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionSpec {
    /// The anchor to resolve first.
    pub anchor: Anchor,
    /// Bounds relative to the resolved anchor.
    pub bounds: RelativeBounds,
}

impl RegionSpec {
    /// Creates a new region derived from `anchor`.
    #[must_use]
    pub const fn new(anchor: Anchor, bounds: RelativeBounds) -> Self {
        Self { anchor, bounds }
    }

    /// Creates a region covering the full root bounds.
    #[must_use]
    pub const fn root() -> Self {
        Self::new(Anchor::Root, RelativeBounds::full())
    }

    /// Creates a region covering the full bounds of the node matching `predicate`.
    #[must_use]
    pub fn node(predicate: NodePredicate) -> Self {
        Self::new(Anchor::Node(predicate), RelativeBounds::full())
    }

    /// Creates a subregion relative to this region.
    #[must_use]
    pub fn subregion(self, bounds: RelativeBounds) -> Self {
        Self::new(Anchor::Region(Box::new(self)), bounds)
    }
}

/// Errors returned when resolving semantic regions.
#[derive(Clone, Debug, PartialEq)]
pub enum RegionResolveError {
    /// The node anchor matched no nodes.
    NotFound(NodePredicate),
    /// The node anchor matched more than one node.
    MultipleMatches {
        predicate: NodePredicate,
        count: usize,
    },
    /// Relative bounds are not finite or fall outside the normalized range.
    InvalidBounds(RelativeBounds),
}

impl std::fmt::Display for RegionResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(_) => write!(f, "no nodes matched region predicate"),
            Self::MultipleMatches { count, .. } => {
                write!(f, "region predicate matched {} nodes", count)
            }
            Self::InvalidBounds(bounds) => write!(
                f,
                "invalid relative bounds ({}, {}, {}, {})",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
        }
    }
}

impl std::error::Error for RegionResolveError {}

impl QueryRoot {
    /// Finds exactly one node matching `predicate`.
    pub fn find_by_predicate(
        &self,
        predicate: &NodePredicate,
    ) -> Result<&NodeMetadata, RegionResolveError> {
        let matches: Vec<_> = self
            .all()
            .iter()
            .filter(|node| predicate.matches(node))
            .collect();
        match matches.as_slice() {
            [] => Err(RegionResolveError::NotFound(predicate.clone())),
            [node] => Ok(node),
            _ => Err(RegionResolveError::MultipleMatches {
                predicate: predicate.clone(),
                count: matches.len(),
            }),
        }
    }

    /// Returns all nodes matching `predicate`.
    #[must_use]
    pub fn find_all_by_predicate(&self, predicate: &NodePredicate) -> Vec<&NodeMetadata> {
        self.all()
            .iter()
            .filter(|node| predicate.matches(node))
            .collect()
    }

    /// Resolves `region` to an absolute rectangle inside `root_bounds`.
    pub fn resolve_region(
        &self,
        root_bounds: Rect,
        region: &RegionSpec,
    ) -> Result<Rect, RegionResolveError> {
        resolve_region(self, root_bounds, region)
    }
}

fn resolve_region(
    root: &QueryRoot,
    root_bounds: Rect,
    region: &RegionSpec,
) -> Result<Rect, RegionResolveError> {
    validate_relative_bounds(region.bounds)?;
    let anchor_rect = match &region.anchor {
        Anchor::Root => root_bounds,
        Anchor::Node(predicate) => root.find_by_predicate(predicate)?.rect,
        Anchor::Region(parent) => resolve_region(root, root_bounds, parent)?,
    };

    Ok(Rect::new(
        crate::Point::new(
            anchor_rect.origin.x + anchor_rect.size.width * region.bounds.x,
            anchor_rect.origin.y + anchor_rect.size.height * region.bounds.y,
        ),
        crate::Size::new(
            anchor_rect.size.width * region.bounds.width,
            anchor_rect.size.height * region.bounds.height,
        ),
    ))
}

fn validate_relative_bounds(bounds: RelativeBounds) -> Result<(), RegionResolveError> {
    let values = [bounds.x, bounds.y, bounds.width, bounds.height];
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0 || *value > 1.0)
    {
        return Err(RegionResolveError::InvalidBounds(bounds));
    }
    if bounds.x + bounds.width > 1.0 || bounds.y + bounds.height > 1.0 {
        return Err(RegionResolveError::InvalidBounds(bounds));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Role, Size};

    fn root() -> QueryRoot {
        QueryRoot::new(vec![
            NodeMetadata {
                id: Some("editor".into()),
                role: Some(Role::TextInput),
                label: Some("Editor".into()),
                rect: Rect::new(Point::new(10.0, 20.0), Size::new(100.0, 50.0)),
            },
            NodeMetadata {
                id: Some("run".into()),
                role: Some(Role::Button),
                label: Some("Run Tests".into()),
                rect: Rect::new(Point::new(130.0, 20.0), Size::new(80.0, 30.0)),
            },
        ])
    }

    #[test]
    fn text_match_variants_work() {
        assert!(TextMatch::exact("Run").matches("Run"));
        assert!(TextMatch::contains("Tes").matches("Run Tests"));
        assert!(TextMatch::starts_with("Run").matches("Run Tests"));
    }

    #[test]
    fn predicate_composition_matches_expected_node() {
        let predicate = NodePredicate::and(vec![
            NodePredicate::role_eq(Role::Button),
            NodePredicate::label(TextMatch::contains("Test")),
        ]);
        let root = root();
        let node = root.find_by_predicate(&predicate).unwrap();
        assert_eq!(node.id.as_deref(), Some("run"));
    }

    #[test]
    fn region_resolution_supports_nested_subregions() {
        let rect = root()
            .resolve_region(
                Rect::new(Point::new(0.0, 0.0), Size::new(300.0, 200.0)),
                &RegionSpec::node(NodePredicate::id_eq("editor"))
                    .subregion(RelativeBounds::new(0.25, 0.5, 0.5, 0.25)),
            )
            .unwrap();

        assert_eq!(
            rect,
            Rect::new(Point::new(35.0, 45.0), Size::new(50.0, 12.5))
        );
    }

    #[test]
    fn region_resolution_rejects_out_of_range_bounds() {
        let error = root()
            .resolve_region(
                Rect::new(Point::new(0.0, 0.0), Size::new(300.0, 200.0)),
                &RegionSpec::new(Anchor::Root, RelativeBounds::new(0.8, 0.0, 0.3, 1.0)),
            )
            .unwrap_err();

        assert!(matches!(error, RegionResolveError::InvalidBounds(_)));
    }
}
