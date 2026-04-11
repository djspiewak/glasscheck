use crate::{
    query::{predicate_is_metadata_supported, predicate_matches_metadata},
    NodeHandle, NodeMetadata, NodePredicate, Point, QueryError, QueryRoot, Rect, SceneSnapshot,
    Size,
};

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
    /// Anchor to a stable handle resolved earlier in the same scene snapshot.
    Handle(NodeHandle),
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

    /// Creates a region covering the full bounds of a stable node handle.
    #[must_use]
    pub fn handle(handle: NodeHandle) -> Self {
        Self::new(Anchor::Handle(handle), RelativeBounds::full())
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
    /// The region predicate matched no nodes.
    NotFound(NodePredicate),
    /// The region predicate matched more than one node.
    MultipleMatches {
        predicate: NodePredicate,
        count: usize,
    },
    /// The host is detached from a real window and cannot synthesize input.
    DetachedRootView,
    /// The host cannot synthesize input for the resolved node.
    InputUnavailable,
    /// The host cannot capture pixels for the resolved region.
    CaptureUnavailable,
    /// The stable node handle was invalid for the scene snapshot.
    InvalidHandle(NodeHandle),
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
            Self::DetachedRootView => write!(f, "detached root-view hosts cannot synthesize input"),
            Self::InputUnavailable => write!(f, "input synthesis is unavailable for this host"),
            Self::CaptureUnavailable => write!(f, "region capture is unavailable for this host"),
            Self::InvalidHandle(handle) => write!(f, "invalid node handle {}", handle.index()),
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
        if self.is_compatibility_root() {
            if !predicate_is_metadata_supported(predicate) {
                return Err(RegionResolveError::NotFound(predicate.clone()));
            }
            let matches = compatibility_find_all(self.all(), predicate);
            return match matches.as_slice() {
                [] => Err(RegionResolveError::NotFound(predicate.clone())),
                [node] => Ok(node),
                _ => Err(RegionResolveError::MultipleMatches {
                    predicate: predicate.clone(),
                    count: matches.len(),
                }),
            };
        }

        let handle = self
            .backing_scene()
            .find(predicate)
            .map_err(map_query_error)?;
        Ok(self
            .all()
            .get(handle.index())
            .expect("scene and compatibility metadata stay aligned"))
    }

    /// Returns all nodes matching `predicate`.
    #[must_use]
    pub fn find_all_by_predicate(&self, predicate: &NodePredicate) -> Vec<&NodeMetadata> {
        if self.is_compatibility_root() {
            if !predicate_is_metadata_supported(predicate) {
                return Vec::new();
            }
            return compatibility_find_all(self.all(), predicate);
        }
        self.backing_scene()
            .find_all(predicate)
            .into_iter()
            .filter_map(|handle| self.all().get(handle.index()))
            .collect()
    }

    /// Resolves `region` to an absolute rectangle inside `root_bounds`.
    pub fn resolve_region(
        &self,
        root_bounds: Rect,
        region: &RegionSpec,
    ) -> Result<Rect, RegionResolveError> {
        if self.is_compatibility_root() {
            return resolve_region_from_metadata(self.all(), root_bounds, region);
        }
        self.backing_scene().resolve_region(root_bounds, region)
    }
}

impl SceneSnapshot {
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
    scene: &SceneSnapshot,
    root_bounds: Rect,
    region: &RegionSpec,
) -> Result<Rect, RegionResolveError> {
    validate_relative_bounds(region.bounds)?;
    let anchor_rect = match &region.anchor {
        Anchor::Root => root_bounds,
        Anchor::Node(predicate) => {
            let handle = scene.find(predicate).map_err(map_query_error)?;
            scene
                .node(handle)
                .map(|node| node.rect)
                .ok_or(RegionResolveError::InvalidHandle(handle))?
        }
        Anchor::Handle(handle) => scene
            .node(*handle)
            .map(|node| node.rect)
            .ok_or(RegionResolveError::InvalidHandle(*handle))?,
        Anchor::Region(parent) => resolve_region(scene, root_bounds, parent)?,
    };

    Ok(Rect::new(
        Point::new(
            anchor_rect.origin.x + anchor_rect.size.width * region.bounds.x,
            anchor_rect.origin.y + anchor_rect.size.height * region.bounds.y,
        ),
        Size::new(
            anchor_rect.size.width * region.bounds.width,
            anchor_rect.size.height * region.bounds.height,
        ),
    ))
}

fn map_query_error(error: QueryError) -> RegionResolveError {
    match error {
        QueryError::NotFoundPredicate(predicate) => RegionResolveError::NotFound(predicate),
        QueryError::MultiplePredicateMatches { predicate, count } => {
            RegionResolveError::MultipleMatches { predicate, count }
        }
        QueryError::NotFound(selector) => {
            RegionResolveError::NotFound(NodePredicate::id_eq(selector.id.unwrap_or_default()))
        }
        QueryError::MultipleMatches { selector, count } => RegionResolveError::MultipleMatches {
            predicate: NodePredicate::id_eq(selector.id.unwrap_or_default()),
            count,
        },
    }
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

fn compatibility_find_all<'a>(
    nodes: &'a [NodeMetadata],
    predicate: &NodePredicate,
) -> Vec<&'a NodeMetadata> {
    nodes
        .iter()
        .filter(|node| predicate_matches_metadata(predicate, node))
        .collect()
}

fn resolve_region_from_metadata(
    nodes: &[NodeMetadata],
    root_bounds: Rect,
    region: &RegionSpec,
) -> Result<Rect, RegionResolveError> {
    validate_relative_bounds(region.bounds)?;
    let anchor_rect = match &region.anchor {
        Anchor::Root => root_bounds,
        Anchor::Node(predicate) => {
            if !predicate_is_metadata_supported(predicate) {
                return Err(RegionResolveError::NotFound(predicate.clone()));
            }
            let matches = compatibility_find_all(nodes, predicate);
            match matches.as_slice() {
                [] => return Err(RegionResolveError::NotFound(predicate.clone())),
                [node] => node.rect,
                _ => {
                    return Err(RegionResolveError::MultipleMatches {
                        predicate: predicate.clone(),
                        count: matches.len(),
                    })
                }
            }
        }
        Anchor::Handle(handle) => nodes
            .get(handle.index())
            .map(|node| node.rect)
            .ok_or(RegionResolveError::InvalidHandle(*handle))?,
        Anchor::Region(parent) => resolve_region_from_metadata(nodes, root_bounds, parent)?,
    };

    Ok(Rect::new(
        Point::new(
            anchor_rect.origin.x + anchor_rect.size.width * region.bounds.x,
            anchor_rect.origin.y + anchor_rect.size.height * region.bounds.y,
        ),
        Size::new(
            anchor_rect.size.width * region.bounds.width,
            anchor_rect.size.height * region.bounds.height,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scene::Role, Point, PropertyValue, Rect, SceneSnapshot, SemanticNode, Size, TextMatch,
    };

    fn rect() -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))
    }

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

    #[test]
    fn handle_anchor_resolves_scene_node() {
        let scene = SceneSnapshot::new(vec![crate::SemanticNode::new(
            "card",
            Role::Container,
            Rect::new(Point::new(25.0, 40.0), Size::new(80.0, 30.0)),
        )
        .with_property("session_id", PropertyValue::Integer(7))]);
        let handle = scene
            .find(&NodePredicate::property_eq(
                "session_id",
                PropertyValue::Integer(7),
            ))
            .unwrap();
        let rect = scene
            .resolve_region(
                Rect::new(Point::new(0.0, 0.0), Size::new(200.0, 200.0)),
                &RegionSpec::handle(handle),
            )
            .unwrap();
        assert_eq!(
            rect,
            Rect::new(Point::new(25.0, 40.0), Size::new(80.0, 30.0))
        );
    }

    #[test]
    fn rich_predicates_are_preserved_for_scene_backed_roots() {
        let mut label = SemanticNode::new("label", Role::Label, rect())
            .with_parent("panel", 0)
            .with_label("Inspector")
            .with_state("expanded", PropertyValue::Bool(false));
        label.value = Some("Inspector Value".into());
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("root", Role::Container, rect())
                .with_class("workspace")
                .with_tag("app")
                .with_property("theme", PropertyValue::string("dark")),
            SemanticNode::new("panel", Role::Container, rect())
                .with_parent("root", 0)
                .with_class("panel")
                .with_tag("primary")
                .with_property("session_id", PropertyValue::Integer(7))
                .with_state("selected", PropertyValue::Bool(true)),
            label,
        ]);
        let root = QueryRoot::from_scene(scene);

        assert_eq!(
            root.find_by_predicate(&NodePredicate::property_eq(
                "session_id",
                PropertyValue::Integer(7)
            ))
            .unwrap()
            .id
            .as_deref(),
            Some("panel")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::ClassEq("workspace".into()))
                .unwrap()
                .id
                .as_deref(),
            Some("root")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::TagEq("primary".into()))
                .unwrap()
                .id
                .as_deref(),
            Some("panel")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::Value(TextMatch::exact("Inspector Value")))
                .unwrap()
                .id
                .as_deref(),
            Some("label")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::StateEq(
                "expanded".into(),
                PropertyValue::Bool(false)
            ))
            .unwrap()
            .id
            .as_deref(),
            Some("label")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::parent(NodePredicate::id_eq("panel")))
                .unwrap()
                .id
                .as_deref(),
            Some("label")
        );
        assert_eq!(
            root.find_by_predicate(&NodePredicate::ancestor(NodePredicate::id_eq("panel")))
                .unwrap()
                .id
                .as_deref(),
            Some("label")
        );

        let descendants =
            root.find_all_by_predicate(&NodePredicate::ancestor(NodePredicate::id_eq("root")));
        assert_eq!(descendants.len(), 2);
        assert_eq!(descendants[0].id.as_deref(), Some("panel"));
        assert_eq!(descendants[1].id.as_deref(), Some("label"));
    }

    #[test]
    fn rich_predicates_fail_when_not_present_in_scene_backed_roots() {
        let scene = SceneSnapshot::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("panel", Role::Container, rect()).with_parent("root", 0),
        ]);
        let root = QueryRoot::from_scene(scene);

        assert!(matches!(
            root.find_by_predicate(&NodePredicate::property_eq(
                "session_id",
                PropertyValue::Integer(7)
            )),
            Err(RegionResolveError::NotFound(_))
        ));
        assert!(matches!(
            root.find_by_predicate(&NodePredicate::Value(TextMatch::contains("missing"))),
            Err(RegionResolveError::NotFound(_))
        ));
        assert!(root
            .find_all_by_predicate(&NodePredicate::ancestor(NodePredicate::id_eq("missing")))
            .is_empty());
    }
}
