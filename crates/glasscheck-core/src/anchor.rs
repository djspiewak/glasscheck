use std::fmt;
use std::sync::Arc;

use crate::{
    assertions::CompareConfig,
    color_match::{
        transparent_reference_pixel_constrains_match, transparent_reference_pixel_matches,
    },
    image::Image,
    query::{predicate_is_metadata_supported, predicate_matches_metadata},
    NodeHandle, NodeMetadata, NodePredicate, Point, QueryError, QueryRoot, Rect, SceneSnapshot,
    Size,
};

/// Bounds expressed relative to an anchor's resolved rectangle.
///
/// Unlike the original normalized-only model, these values may extend outside
/// the anchor. This allows callers to define regions adjacent to another node
/// and then refine visually inside that space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeBounds {
    /// Relative horizontal origin.
    pub x: f64,
    /// Relative vertical origin.
    pub y: f64,
    /// Relative width.
    pub width: f64,
    /// Relative height.
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

/// Absolute pixel adjustments applied after relative placement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AbsoluteBounds {
    /// Pixel delta from the computed relative origin.
    pub x: f64,
    /// Pixel delta from the computed relative origin.
    pub y: f64,
    /// Pixel delta added to the computed relative width.
    pub width: f64,
    /// Pixel delta added to the computed relative height.
    pub height: f64,
}

impl AbsoluteBounds {
    /// Creates absolute pixel adjustments.
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Exact or fuzzy pixel matcher used by visual refiners.
///
/// The expected RGBA value is treated as a foreground reference. Fully
/// transparent expected pixels are treated as unconstrained, and partially
/// transparent expected pixels are matched as that foreground composited over
/// an unknown background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelMatch {
    /// Expected foreground RGBA reference pixel.
    pub rgba: [u8; 4],
    /// Maximum absolute per-channel difference.
    pub tolerance: u8,
    /// Minimum alpha required in the live image before color matching is attempted.
    pub min_alpha: u8,
}

impl PixelMatch {
    /// Creates a pixel matcher.
    #[must_use]
    pub const fn new(rgba: [u8; 4], tolerance: u8, min_alpha: u8) -> Self {
        Self {
            rgba,
            tolerance,
            min_alpha,
        }
    }

    #[must_use]
    fn matches(self, pixel: [u8; 4]) -> bool {
        if pixel[3] < self.min_alpha {
            return false;
        }
        transparent_reference_pixel_matches(&pixel, &self.rgba, i16::from(self.tolerance))
    }
}

/// Refines a region by taking the bounding box of matching pixels.
///
/// Matching uses the same alpha-aware reference semantics as `PixelMatch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelProbe {
    /// Pixel matcher used during the scan.
    pub matcher: PixelMatch,
    /// Minimum number of matching pixels required for a successful refinement.
    pub min_pixels: usize,
}

impl PixelProbe {
    /// Creates a pixel probe.
    #[must_use]
    pub const fn new(matcher: PixelMatch, min_pixels: usize) -> Self {
        Self {
            matcher,
            min_pixels,
        }
    }
}

/// Refines a region by detecting a single connected component of matching pixels.
///
/// Matching uses the same alpha-aware reference semantics as `PixelMatch`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionProbe {
    /// Pixel matcher used during the scan.
    pub matcher: PixelMatch,
    /// Minimum component size required to accept a component.
    pub min_pixels: usize,
    /// Padding applied around the detected component.
    pub padding: f64,
}

impl RegionProbe {
    /// Creates a region probe.
    #[must_use]
    pub const fn new(matcher: PixelMatch, min_pixels: usize, padding: f64) -> Self {
        Self {
            matcher,
            min_pixels,
            padding,
        }
    }
}

/// Alpha-aware fuzzy image matcher for refining a region.
///
/// Fully transparent template pixels are ignored during scoring. Partially
/// transparent template pixels are matched as foreground references composited
/// over an unknown background, using the same semantics as `PixelMatch`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageMatch {
    /// Template image to search for.
    pub template: Image,
    /// Per-channel compare configuration.
    ///
    /// `match_threshold` is applied only to template pixels that constrain the
    /// match; fully transparent template pixels are excluded from the score.
    pub compare: CompareConfig,
}

impl ImageMatch {
    /// Creates an image matcher from a template and compare config.
    #[must_use]
    pub fn new(template: Image, compare: CompareConfig) -> Self {
        Self { template, compare }
    }
}

/// Custom region refiner hook.
pub trait RegionRefiner: Send + Sync {
    /// Produces a refined region inside or adjacent to `region`.
    fn refine(&self, image: &Image, region: Rect) -> Result<Rect, RegionResolveError>;
}

/// A semantic or derived anchor for resolving a region.
pub enum Anchor {
    /// Anchor to the full window/root region.
    Root,
    /// Anchor to the full bounds of a known absolute rectangle.
    Rect(Rect),
    /// Anchor to the unique node matching the predicate.
    Node(NodePredicate),
    /// Anchor to a stable handle resolved earlier in the same scene snapshot.
    Handle(NodeHandle),
    /// Anchor to a previously described region.
    Region(Box<RegionSpec>),
    /// Refine a parent region by probing matching pixels.
    PixelProbe {
        /// Region to search inside.
        region: Box<RegionSpec>,
        /// Probe configuration.
        probe: PixelProbe,
    },
    /// Refine a parent region by detecting a single matching component.
    RegionProbe {
        /// Region to search inside.
        region: Box<RegionSpec>,
        /// Probe configuration.
        probe: RegionProbe,
    },
    /// Refine a parent region by fuzzy template matching.
    ImageMatch {
        /// Region to search inside.
        region: Box<RegionSpec>,
        /// Matcher configuration.
        matcher: ImageMatch,
    },
    /// Refine a parent region with custom logic.
    Custom {
        /// Region to refine.
        region: Box<RegionSpec>,
        /// User-supplied refiner.
        refiner: Arc<dyn RegionRefiner>,
    },
}

impl Clone for Anchor {
    fn clone(&self) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Rect(rect) => Self::Rect(*rect),
            Self::Node(predicate) => Self::Node(predicate.clone()),
            Self::Handle(handle) => Self::Handle(*handle),
            Self::Region(region) => Self::Region(region.clone()),
            Self::PixelProbe { region, probe } => Self::PixelProbe {
                region: region.clone(),
                probe: *probe,
            },
            Self::RegionProbe { region, probe } => Self::RegionProbe {
                region: region.clone(),
                probe: *probe,
            },
            Self::ImageMatch { region, matcher } => Self::ImageMatch {
                region: region.clone(),
                matcher: matcher.clone(),
            },
            Self::Custom { region, refiner } => Self::Custom {
                region: region.clone(),
                refiner: Arc::clone(refiner),
            },
        }
    }
}

impl fmt::Debug for Anchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => f.write_str("Root"),
            Self::Rect(rect) => f.debug_tuple("Rect").field(rect).finish(),
            Self::Node(predicate) => f.debug_tuple("Node").field(predicate).finish(),
            Self::Handle(handle) => f.debug_tuple("Handle").field(handle).finish(),
            Self::Region(region) => f.debug_tuple("Region").field(region).finish(),
            Self::PixelProbe { region, probe } => f
                .debug_struct("PixelProbe")
                .field("region", region)
                .field("probe", probe)
                .finish(),
            Self::RegionProbe { region, probe } => f
                .debug_struct("RegionProbe")
                .field("region", region)
                .field("probe", probe)
                .finish(),
            Self::ImageMatch { region, matcher } => f
                .debug_struct("ImageMatch")
                .field("region", region)
                .field("matcher", matcher)
                .finish(),
            Self::Custom { region, .. } => f
                .debug_struct("Custom")
                .field("region", region)
                .field("refiner", &"<dyn RegionRefiner>")
                .finish(),
        }
    }
}

impl PartialEq for Anchor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Root, Self::Root) => true,
            (Self::Rect(left), Self::Rect(right)) => left == right,
            (Self::Node(left), Self::Node(right)) => left == right,
            (Self::Handle(left), Self::Handle(right)) => left == right,
            (Self::Region(left), Self::Region(right)) => left == right,
            (
                Self::PixelProbe {
                    region: left_region,
                    probe: left_probe,
                },
                Self::PixelProbe {
                    region: right_region,
                    probe: right_probe,
                },
            ) => left_region == right_region && left_probe == right_probe,
            (
                Self::RegionProbe {
                    region: left_region,
                    probe: left_probe,
                },
                Self::RegionProbe {
                    region: right_region,
                    probe: right_probe,
                },
            ) => left_region == right_region && left_probe == right_probe,
            (
                Self::ImageMatch {
                    region: left_region,
                    matcher: left_matcher,
                },
                Self::ImageMatch {
                    region: right_region,
                    matcher: right_matcher,
                },
            ) => left_region == right_region && left_matcher == right_matcher,
            (
                Self::Custom {
                    region: left_region,
                    refiner: left_refiner,
                },
                Self::Custom {
                    region: right_region,
                    refiner: right_refiner,
                },
            ) => left_region == right_region && Arc::ptr_eq(left_refiner, right_refiner),
            _ => false,
        }
    }
}

/// Declarative specification of a region derived from an anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionSpec {
    /// The anchor to resolve first.
    pub anchor: Anchor,
    /// Bounds relative to the resolved anchor.
    pub bounds: RelativeBounds,
    /// Absolute pixel offsets and size adjustments.
    pub absolute: AbsoluteBounds,
}

impl RegionSpec {
    /// Creates a new region derived from `anchor`.
    #[must_use]
    pub const fn new(anchor: Anchor, bounds: RelativeBounds) -> Self {
        Self {
            anchor,
            bounds,
            absolute: AbsoluteBounds::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Creates a new region derived from `anchor` with absolute adjustments.
    #[must_use]
    pub const fn new_with_absolute(
        anchor: Anchor,
        bounds: RelativeBounds,
        absolute: AbsoluteBounds,
    ) -> Self {
        Self {
            anchor,
            bounds,
            absolute,
        }
    }

    /// Creates a region covering the full root bounds.
    #[must_use]
    pub const fn root() -> Self {
        Self::new(Anchor::Root, RelativeBounds::full())
    }

    /// Creates a region covering `rect`.
    #[must_use]
    pub const fn rect(rect: Rect) -> Self {
        Self::new(Anchor::Rect(rect), RelativeBounds::full())
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

    /// Creates a region relative to this region using both fractional and absolute terms.
    #[must_use]
    pub fn offset_region(self, bounds: RelativeBounds, absolute: AbsoluteBounds) -> Self {
        Self::new_with_absolute(Anchor::Region(Box::new(self)), bounds, absolute)
    }

    /// Creates a region to the right of this region.
    #[must_use]
    pub fn right_of(self, gap: f64, width: f64) -> Self {
        self.offset_region(
            RelativeBounds::new(1.0, 0.0, 0.0, 1.0),
            AbsoluteBounds::new(gap, 0.0, width, 0.0),
        )
    }

    /// Creates a region to the left of this region.
    #[must_use]
    pub fn left_of(self, gap: f64, width: f64) -> Self {
        self.offset_region(
            RelativeBounds::new(0.0, 0.0, 0.0, 1.0),
            AbsoluteBounds::new(-(gap + width), 0.0, width, 0.0),
        )
    }

    /// Creates a region above this region.
    #[must_use]
    pub fn above(self, gap: f64, height: f64) -> Self {
        self.offset_region(
            RelativeBounds::new(0.0, 1.0, 1.0, 0.0),
            AbsoluteBounds::new(0.0, gap, 0.0, height),
        )
    }

    /// Creates a region below this region.
    #[must_use]
    pub fn below(self, gap: f64, height: f64) -> Self {
        self.offset_region(
            RelativeBounds::new(0.0, 0.0, 1.0, 0.0),
            AbsoluteBounds::new(0.0, -(gap + height), 0.0, height),
        )
    }

    /// Refines this region by taking the bounding box of matching pixels.
    #[must_use]
    pub fn pixel_probe(self, probe: PixelProbe) -> Self {
        Self::new(
            Anchor::PixelProbe {
                region: Box::new(self),
                probe,
            },
            RelativeBounds::full(),
        )
    }

    /// Refines this region by detecting a single connected component of matching pixels.
    #[must_use]
    pub fn region_probe(self, probe: RegionProbe) -> Self {
        Self::new(
            Anchor::RegionProbe {
                region: Box::new(self),
                probe,
            },
            RelativeBounds::full(),
        )
    }

    /// Refines this region by fuzzy template matching.
    #[must_use]
    pub fn image_match(self, matcher: ImageMatch) -> Self {
        Self::new(
            Anchor::ImageMatch {
                region: Box::new(self),
                matcher,
            },
            RelativeBounds::full(),
        )
    }

    /// Refines this region with custom user logic.
    #[must_use]
    pub fn custom_refine(self, refiner: Arc<dyn RegionRefiner>) -> Self {
        Self::new(
            Anchor::Custom {
                region: Box::new(self),
                refiner,
            },
            RelativeBounds::full(),
        )
    }

    /// Returns whether resolving this region requires live pixels.
    #[must_use]
    pub fn requires_image(&self) -> bool {
        match &self.anchor {
            Anchor::Root | Anchor::Rect(_) | Anchor::Node(_) | Anchor::Handle(_) => false,
            Anchor::Region(region) => region.requires_image(),
            Anchor::PixelProbe { .. }
            | Anchor::RegionProbe { .. }
            | Anchor::ImageMatch { .. }
            | Anchor::Custom { .. } => true,
        }
    }
}

/// Errors returned when resolving node regions.
#[derive(Clone, Debug, PartialEq)]
pub enum RegionResolveError {
    /// The region predicate matched no nodes.
    NotFound(NodePredicate),
    /// The region predicate matched more than one node.
    MultipleMatches {
        predicate: NodePredicate,
        count: usize,
    },
    /// A visual refiner found no matching target.
    VisualMatchMissing,
    /// A visual refiner found more than one candidate.
    VisualMatchAmbiguous { count: usize },
    /// A visual matcher found a candidate, but it did not pass the threshold.
    MatchBelowThreshold { score: f64, threshold: f64 },
    /// The host is detached from a real window and cannot synthesize input.
    DetachedRootView,
    /// The host cannot synthesize input for the resolved node.
    InputUnavailable,
    /// The host cannot capture pixels for the resolved region.
    CaptureUnavailable,
    /// The stable node handle was invalid for the scene snapshot.
    InvalidHandle(NodeHandle),
    /// Relative or absolute bounds were not finite.
    InvalidBounds(RelativeBounds),
    /// The final resolved region had invalid geometry.
    InvalidRegion(Rect),
}

impl fmt::Display for RegionResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(_) => write!(f, "no nodes matched region predicate"),
            Self::MultipleMatches { count, .. } => {
                write!(f, "region predicate matched {} nodes", count)
            }
            Self::VisualMatchMissing => write!(f, "visual refinement did not find a match"),
            Self::VisualMatchAmbiguous { count } => {
                write!(f, "visual refinement found {} candidate regions", count)
            }
            Self::MatchBelowThreshold { score, threshold } => write!(
                f,
                "visual match score {:.3} did not meet threshold {:.3}",
                score, threshold
            ),
            Self::DetachedRootView => write!(f, "detached root-view hosts cannot synthesize input"),
            Self::InputUnavailable => write!(f, "input synthesis is unavailable for this host"),
            Self::CaptureUnavailable => write!(f, "region capture is unavailable for this host"),
            Self::InvalidHandle(handle) => write!(f, "invalid node handle {}", handle.index()),
            Self::InvalidBounds(bounds) => write!(
                f,
                "invalid relative bounds ({}, {}, {}, {})",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
            Self::InvalidRegion(region) => write!(
                f,
                "invalid resolved region ({}, {}, {}, {})",
                region.origin.x, region.origin.y, region.size.width, region.size.height
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

        let Some(scene) = self.scene() else {
            return Err(RegionResolveError::NotFound(predicate.clone()));
        };
        let handle = scene.find(predicate).map_err(map_query_error)?;
        Ok(self
            .all()
            .get(handle.index())
            .ok_or(RegionResolveError::InvalidHandle(handle))?)
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
        self.scene()
            .into_iter()
            .flat_map(|scene| scene.find_all(predicate))
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
        self.scene().map_or_else(
            || resolve_region_from_metadata(self.all(), root_bounds, region),
            |scene| scene.resolve_region(root_bounds, region),
        )
    }
}

impl SceneSnapshot {
    /// Resolves `region` to an absolute rectangle inside `root_bounds`.
    pub fn resolve_region(
        &self,
        root_bounds: Rect,
        region: &RegionSpec,
    ) -> Result<Rect, RegionResolveError> {
        self.resolve_region_with_image(root_bounds, None, region)
    }

    /// Resolves `region` to an absolute rectangle inside `root_bounds`, using
    /// `image` when visual refiners are present.
    pub fn resolve_region_with_image(
        &self,
        root_bounds: Rect,
        image: Option<&Image>,
        region: &RegionSpec,
    ) -> Result<Rect, RegionResolveError> {
        resolve_region(self, root_bounds, image, region)
    }
}

fn resolve_region(
    scene: &SceneSnapshot,
    root_bounds: Rect,
    image: Option<&Image>,
    region: &RegionSpec,
) -> Result<Rect, RegionResolveError> {
    validate_relative_bounds(region.bounds)?;
    let anchor_rect = match &region.anchor {
        Anchor::Root => root_bounds,
        Anchor::Rect(rect) => *rect,
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
        Anchor::Region(parent) => resolve_region(scene, root_bounds, image, parent)?,
        Anchor::PixelProbe {
            region: parent,
            probe,
        } => {
            let parent_rect = resolve_region(scene, root_bounds, image, parent)?;
            let image = image.ok_or(RegionResolveError::CaptureUnavailable)?;
            resolve_pixel_probe(image, parent_rect, *probe)?
        }
        Anchor::RegionProbe {
            region: parent,
            probe,
        } => {
            let parent_rect = resolve_region(scene, root_bounds, image, parent)?;
            let image = image.ok_or(RegionResolveError::CaptureUnavailable)?;
            resolve_region_probe(image, parent_rect, *probe)?
        }
        Anchor::ImageMatch {
            region: parent,
            matcher,
        } => {
            let parent_rect = resolve_region(scene, root_bounds, image, parent)?;
            let image = image.ok_or(RegionResolveError::CaptureUnavailable)?;
            resolve_image_match(image, parent_rect, matcher)?
        }
        Anchor::Custom {
            region: parent,
            refiner,
        } => {
            let parent_rect = resolve_region(scene, root_bounds, image, parent)?;
            let image = image.ok_or(RegionResolveError::CaptureUnavailable)?;
            refiner.refine(image, parent_rect)?
        }
    };

    apply_region_terms(anchor_rect, region.bounds, region.absolute)
}

fn apply_region_terms(
    anchor_rect: Rect,
    bounds: RelativeBounds,
    absolute: AbsoluteBounds,
) -> Result<Rect, RegionResolveError> {
    let rect = Rect::new(
        Point::new(
            anchor_rect.origin.x + anchor_rect.size.width * bounds.x + absolute.x,
            anchor_rect.origin.y + anchor_rect.size.height * bounds.y + absolute.y,
        ),
        Size::new(
            anchor_rect.size.width * bounds.width + absolute.width,
            anchor_rect.size.height * bounds.height + absolute.height,
        ),
    );
    validate_rect(rect)?;
    Ok(rect)
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
    if values.iter().any(|value| !value.is_finite()) {
        return Err(RegionResolveError::InvalidBounds(bounds));
    }
    Ok(())
}

fn validate_rect(rect: Rect) -> Result<(), RegionResolveError> {
    let values = [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ];
    if values.iter().any(|value| !value.is_finite())
        || rect.size.width < 0.0
        || rect.size.height < 0.0
    {
        return Err(RegionResolveError::InvalidRegion(rect));
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
        Anchor::Rect(rect) => *rect,
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
        Anchor::PixelProbe { .. }
        | Anchor::RegionProbe { .. }
        | Anchor::ImageMatch { .. }
        | Anchor::Custom { .. } => return Err(RegionResolveError::CaptureUnavailable),
    };

    apply_region_terms(anchor_rect, region.bounds, region.absolute)
}

fn resolve_pixel_probe(
    image: &Image,
    region: Rect,
    probe: PixelProbe,
) -> Result<Rect, RegionResolveError> {
    let components = matching_components(image, region, probe.matcher);
    let matching_pixels = components
        .iter()
        .map(|component| component.pixel_count)
        .sum::<usize>();
    if matching_pixels < probe.min_pixels {
        return Err(RegionResolveError::VisualMatchMissing);
    }
    let bbox = components.iter().fold(None, |current, component| {
        Some(union_rect(current, component.rect))
    });
    bbox.ok_or(RegionResolveError::VisualMatchMissing)
}

fn resolve_region_probe(
    image: &Image,
    region: Rect,
    probe: RegionProbe,
) -> Result<Rect, RegionResolveError> {
    let mut components = matching_components(image, region, probe.matcher)
        .into_iter()
        .filter(|component| component.pixel_count >= probe.min_pixels)
        .collect::<Vec<_>>();
    match components.len() {
        0 => Err(RegionResolveError::VisualMatchMissing),
        1 => {
            let rect = inflate_rect(components.remove(0).rect, probe.padding);
            validate_rect(rect)?;
            Ok(rect)
        }
        count => Err(RegionResolveError::VisualMatchAmbiguous { count }),
    }
}

fn resolve_image_match(
    image: &Image,
    region: Rect,
    matcher: &ImageMatch,
) -> Result<Rect, RegionResolveError> {
    if matcher.template.width == 0 || matcher.template.height == 0 {
        return Err(RegionResolveError::VisualMatchMissing);
    }

    let crop_bounds = image.clamped_crop_rect(region);
    let base = image.crop(region);
    if base.width < matcher.template.width || base.height < matcher.template.height {
        return Err(RegionResolveError::VisualMatchMissing);
    }

    let mut best_score = -1.0f64;
    let mut best_rects = Vec::new();
    for y in 0..=(base.height - matcher.template.height) {
        for x in 0..=(base.width - matcher.template.width) {
            let candidate = base.crop(Rect::new(
                Point::new(f64::from(x), f64::from(y)),
                Size::new(
                    f64::from(matcher.template.width),
                    f64::from(matcher.template.height),
                ),
            ));
            let score = alpha_match_ratio(
                &candidate,
                &matcher.template,
                matcher.compare.channel_tolerance,
            );
            let rect = Rect::new(
                Point::new(
                    crop_bounds.origin.x + f64::from(x),
                    crop_bounds.origin.y + f64::from(y),
                ),
                Size::new(
                    f64::from(matcher.template.width),
                    f64::from(matcher.template.height),
                ),
            );
            if score > best_score {
                best_score = score;
                best_rects.clear();
                best_rects.push(rect);
            } else if (score - best_score).abs() <= f64::EPSILON {
                best_rects.push(rect);
            }
        }
    }

    if best_score < 0.0 {
        return Err(RegionResolveError::VisualMatchMissing);
    }
    if best_score < matcher.compare.match_threshold {
        return Err(RegionResolveError::MatchBelowThreshold {
            score: best_score,
            threshold: matcher.compare.match_threshold,
        });
    }
    if best_rects.len() > 1 {
        return Err(RegionResolveError::VisualMatchAmbiguous {
            count: best_rects.len(),
        });
    }

    best_rects
        .into_iter()
        .next()
        .ok_or(RegionResolveError::VisualMatchMissing)
}

fn alpha_match_ratio(actual: &Image, expected: &Image, tolerance: u8) -> f64 {
    if actual.width != expected.width || actual.height != expected.height {
        return 0.0;
    }

    let mut total = 0usize;
    let mut matched = 0usize;
    for (actual_pixel, expected_pixel) in actual
        .data
        .chunks_exact(4)
        .zip(expected.data.chunks_exact(4))
    {
        if !transparent_reference_pixel_constrains_match(expected_pixel) {
            continue;
        }
        total += 1;
        if transparent_reference_pixel_matches(actual_pixel, expected_pixel, i16::from(tolerance)) {
            matched += 1;
        }
    }

    if total == 0 {
        1.0
    } else {
        matched as f64 / total as f64
    }
}

#[derive(Clone, Copy, Debug)]
struct MatchComponent {
    rect: Rect,
    pixel_count: usize,
}

fn matching_components(image: &Image, region: Rect, matcher: PixelMatch) -> Vec<MatchComponent> {
    let crop_bounds = image.clamped_crop_rect(region);
    let cropped = image.crop(region);
    if cropped.width == 0 || cropped.height == 0 {
        return Vec::new();
    }

    let width = cropped.width as usize;
    let height = cropped.height as usize;
    let mut bitmap = vec![false; width * height];
    for y in 0..cropped.height {
        for x in 0..cropped.width {
            let pixel = cropped.pixel_at(x, y).unwrap_or([0, 0, 0, 0]);
            bitmap[y as usize * width + x as usize] = matcher.matches(pixel);
        }
    }

    let mut visited = vec![false; bitmap.len()];
    let mut components = Vec::new();
    for start_y in 0..height {
        for start_x in 0..width {
            let index = start_y * width + start_x;
            if visited[index] || !bitmap[index] {
                continue;
            }

            let mut stack = vec![(start_x, start_y)];
            visited[index] = true;
            let mut min_x = start_x;
            let mut max_x = start_x;
            let mut min_y = start_y;
            let mut max_y = start_y;
            let mut pixel_count = 0usize;

            while let Some((x, y)) = stack.pop() {
                pixel_count += 1;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);

                for (next_x, next_y) in neighbors(x, y, width, height) {
                    let next_index = next_y * width + next_x;
                    if visited[next_index] || !bitmap[next_index] {
                        continue;
                    }
                    visited[next_index] = true;
                    stack.push((next_x, next_y));
                }
            }

            components.push(MatchComponent {
                rect: Rect::new(
                    Point::new(
                        crop_bounds.origin.x + min_x as f64,
                        crop_bounds.origin.y + min_y as f64,
                    ),
                    Size::new((max_x - min_x + 1) as f64, (max_y - min_y + 1) as f64),
                ),
                pixel_count,
            });
        }
    }

    components
}

fn neighbors(x: usize, y: usize, width: usize, height: usize) -> [(usize, usize); 4] {
    [
        (x.saturating_sub(1), y),
        ((x + 1).min(width.saturating_sub(1)), y),
        (x, y.saturating_sub(1)),
        (x, (y + 1).min(height.saturating_sub(1))),
    ]
}

fn union_rect(current: Option<Rect>, next: Rect) -> Rect {
    let Some(current) = current else {
        return next;
    };
    let left = current.origin.x.min(next.origin.x);
    let bottom = current.origin.y.min(next.origin.y);
    let right = (current.origin.x + current.size.width).max(next.origin.x + next.size.width);
    let top = (current.origin.y + current.size.height).max(next.origin.y + next.size.height);
    Rect::new(
        Point::new(left, bottom),
        Size::new(right - left, top - bottom),
    )
}

fn inflate_rect(rect: Rect, padding: f64) -> Rect {
    Rect::new(
        Point::new(rect.origin.x - padding, rect.origin.y - padding),
        Size::new(
            rect.size.width + padding * 2.0,
            rect.size.height + padding * 2.0,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scene::Role, CompareConfig, Point, PropertyValue, Rect, SceneSnapshot, SemanticNode, Size,
        TextMatch,
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

    fn image_with_red_chip() -> Image {
        let mut data = vec![0u8; 12 * 10 * 4];
        for y in 2..6 {
            for x in 3..7 {
                let base = ((y * 12 + x) * 4) as usize;
                data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        Image::new(12, 10, data)
    }

    fn image_with_partial_alpha_reference_chip(pixel: [u8; 4]) -> Image {
        let mut data = vec![0u8; 12 * 10 * 4];
        for y in 2..6 {
            for x in 3..7 {
                let base = ((y * 12 + x) * 4) as usize;
                data[base..base + 4].copy_from_slice(&pixel);
            }
        }
        Image::new(12, 10, data)
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
    fn region_resolution_supports_external_offsets() {
        let rect = root()
            .resolve_region(
                Rect::new(Point::new(0.0, 0.0), Size::new(300.0, 200.0)),
                &RegionSpec::node(NodePredicate::id_eq("editor")).offset_region(
                    RelativeBounds::new(1.0, 0.0, 0.0, 1.0),
                    AbsoluteBounds::new(50.0, 0.0, 120.0, 0.0),
                ),
            )
            .unwrap();

        assert_eq!(
            rect,
            Rect::new(Point::new(160.0, 20.0), Size::new(120.0, 50.0))
        );
    }

    #[test]
    fn region_resolution_rejects_invalid_geometry() {
        let error = root()
            .resolve_region(
                Rect::new(Point::new(0.0, 0.0), Size::new(300.0, 200.0)),
                &RegionSpec::new_with_absolute(
                    Anchor::Root,
                    RelativeBounds::new(0.0, 0.0, 0.0, 0.0),
                    AbsoluteBounds::new(0.0, 0.0, -1.0, 20.0),
                ),
            )
            .unwrap_err();

        assert!(matches!(error, RegionResolveError::InvalidRegion(_)));
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
    fn pixel_probe_finds_matching_pixels() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_red_chip();
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().pixel_probe(PixelProbe::new(
                    PixelMatch::new([255, 0, 0, 255], 0, 255),
                    4,
                )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
    }

    #[test]
    fn pixel_probe_fails_when_visual_match_is_missing() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = Image::new(12, 10, vec![0; 12 * 10 * 4]);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().pixel_probe(PixelProbe::new(
                    PixelMatch::new([255, 0, 0, 255], 0, 255),
                    1,
                )),
            )
            .unwrap_err();

        assert_eq!(error, RegionResolveError::VisualMatchMissing);
    }

    #[test]
    fn pixel_probe_accepts_partial_alpha_reference_over_unknown_background() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_partial_alpha_reference_chip([160, 32, 32, 255]);
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().pixel_probe(PixelProbe::new(
                    PixelMatch::new([255, 0, 0, 128], 0, 255),
                    8,
                )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
    }

    #[test]
    fn pixel_probe_rejects_partial_alpha_reference_when_color_is_out_of_range() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_partial_alpha_reference_chip([100, 0, 0, 255]);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().pixel_probe(PixelProbe::new(
                    PixelMatch::new([255, 0, 0, 128], 0, 255),
                    1,
                )),
            )
            .unwrap_err();

        assert_eq!(error, RegionResolveError::VisualMatchMissing);
    }

    #[test]
    fn region_probe_finds_single_component() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_red_chip();
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().region_probe(RegionProbe::new(
                    PixelMatch::new([255, 0, 0, 255], 0, 255),
                    4,
                    1.0,
                )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(2.0, 1.0), Size::new(6.0, 6.0)));
    }

    #[test]
    fn region_probe_fails_when_multiple_components_match() {
        let scene = SceneSnapshot::new(Vec::new());
        let mut data = vec![0u8; 12 * 10 * 4];
        for y in 2..6 {
            for x in 1..5 {
                let base = ((y * 12 + x) * 4) as usize;
                data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
            for x in 7..11 {
                let base = ((y * 12 + x) * 4) as usize;
                data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let image = Image::new(12, 10, data);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().region_probe(RegionProbe::new(
                    PixelMatch::new([255, 0, 0, 255], 0, 255),
                    4,
                    0.0,
                )),
            )
            .unwrap_err();

        assert_eq!(error, RegionResolveError::VisualMatchAmbiguous { count: 2 });
    }

    #[test]
    fn region_probe_accepts_partial_alpha_reference_over_unknown_background() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_partial_alpha_reference_chip([200, 24, 24, 255]);
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().region_probe(RegionProbe::new(
                    PixelMatch::new([255, 0, 0, 128], 0, 255),
                    4,
                    0.0,
                )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
    }

    #[test]
    fn region_probe_rejects_partial_alpha_reference_when_channel_mix_is_invalid() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_partial_alpha_reference_chip([200, 200, 24, 255]);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().region_probe(RegionProbe::new(
                    PixelMatch::new([255, 0, 0, 128], 0, 255),
                    1,
                    0.0,
                )),
            )
            .unwrap_err();

        assert_eq!(error, RegionResolveError::VisualMatchMissing);
    }

    #[test]
    fn image_match_finds_template_with_alpha() {
        let mut template_data = vec![0u8; 4 * 4 * 4];
        for y in 0..4 {
            for x in 0..4 {
                let base = ((y * 4 + x) * 4) as usize;
                template_data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let template = Image::new(4, 4, template_data);
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_red_chip();
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 1.0,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
    }

    #[test]
    fn image_match_fails_below_threshold() {
        let template = Image::new(
            2,
            2,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        );
        let scene = SceneSnapshot::new(Vec::new());
        let image = Image::new(12, 10, vec![0; 12 * 10 * 4]);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 1.0,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RegionResolveError::MatchBelowThreshold {
                score: 0.0,
                threshold: 1.0
            }
        ));
    }

    #[test]
    fn image_match_accepts_partial_alpha_reference_over_unknown_background() {
        let template = Image::new(
            2,
            2,
            vec![
                255, 0, 0, 255, 255, 0, 0, 128, 255, 0, 0, 128, 255, 0, 0, 128,
            ],
        );
        let scene = SceneSnapshot::new(Vec::new());
        let mut image = image_with_partial_alpha_reference_chip([180, 40, 40, 255]);
        let base = ((2 * image.width + 3) * 4) as usize;
        image.data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 1.0,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(2.0, 2.0)));
    }

    #[test]
    fn image_match_reports_ambiguity_for_tied_best_matches() {
        let template = Image::new(
            4,
            4,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                255, 0, 0, 255,
            ],
        );
        let scene = SceneSnapshot::new(Vec::new());
        let mut data = vec![0u8; 12 * 10 * 4];
        for y in 2..6 {
            for x in 1..5 {
                let base = ((y * 12 + x) * 4) as usize;
                data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
            for x in 7..11 {
                let base = ((y * 12 + x) * 4) as usize;
                data[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let image = Image::new(12, 10, data);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 1.0,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap_err();

        assert_eq!(error, RegionResolveError::VisualMatchAmbiguous { count: 2 });
    }

    #[test]
    fn image_match_rejects_partial_alpha_reference_when_too_dark() {
        let template = Image::new(
            2,
            2,
            vec![
                255, 0, 0, 128, 255, 0, 0, 128, 255, 0, 0, 128, 255, 0, 0, 128,
            ],
        );
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_partial_alpha_reference_chip([100, 20, 20, 255]);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 1.0,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RegionResolveError::MatchBelowThreshold {
                score: s,
                threshold: 1.0
            } if s < 1.0
        ));
    }

    #[test]
    fn image_match_reports_touching_best_matches_as_ambiguous() {
        let template = Image::new(1, 1, vec![255, 0, 0, 255]);
        let scene = SceneSnapshot::new(Vec::new());
        let image = Image::new(2, 1, vec![255, 0, 0, 255, 255, 0, 0, 255]);
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(2.0, 1.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 1.0,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap_err();

        assert_eq!(error, RegionResolveError::VisualMatchAmbiguous { count: 2 });
    }

    #[test]
    fn image_match_rejects_false_positive_from_fully_transparent_padding() {
        let template = Image::new(
            2,
            2,
            vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        let scene = SceneSnapshot::new(Vec::new());
        let image = Image::new(
            4,
            4,
            vec![
                0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
                0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
                0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
            ],
        );
        let error = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(4.0, 4.0)),
                Some(&image),
                &RegionSpec::root().image_match(ImageMatch::new(
                    template,
                    CompareConfig {
                        channel_tolerance: 0,
                        match_threshold: 0.75,
                        generate_diff: false,
                    },
                )),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RegionResolveError::MatchBelowThreshold {
                score: 0.0,
                threshold: 0.75
            }
        ));
    }

    #[test]
    fn pixel_probe_reports_root_space_rect_when_search_region_is_clipped() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_red_chip();
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::rect(Rect::new(Point::new(-2.0, 1.0), Size::new(10.0, 7.0)))
                    .pixel_probe(PixelProbe::new(PixelMatch::new([255, 0, 0, 255], 0, 1), 1)),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
    }

    #[test]
    fn region_probe_reports_root_space_rect_when_search_region_is_clipped() {
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_red_chip();
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::rect(Rect::new(Point::new(-2.0, 1.0), Size::new(10.0, 7.0)))
                    .region_probe(RegionProbe::new(
                        PixelMatch::new([255, 0, 0, 255], 0, 1),
                        1,
                        0.0,
                    )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
    }

    #[test]
    fn image_match_reports_root_space_rect_when_search_region_is_clipped() {
        let template = Image::new(
            4,
            4,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                255, 0, 0, 255,
            ],
        );
        let scene = SceneSnapshot::new(Vec::new());
        let image = image_with_red_chip();
        let rect = scene
            .resolve_region_with_image(
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
                Some(&image),
                &RegionSpec::rect(Rect::new(Point::new(-2.0, 1.0), Size::new(10.0, 7.0)))
                    .image_match(ImageMatch::new(
                        template,
                        CompareConfig {
                            channel_tolerance: 0,
                            match_threshold: 1.0,
                            generate_diff: false,
                        },
                    )),
            )
            .unwrap();

        assert_eq!(rect, Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0)));
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
    }
}
