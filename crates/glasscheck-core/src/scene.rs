use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{
    anchor::RegionSpec, image::Image, Interactability, Point, QueryError, QueryMatch, Rect,
    Selector,
};

const PAINT_ORDER_PATH_PROPERTY: &str = "glasscheck:paint_order_path";
const SOURCE_ID_PROPERTY: &str = "glasscheck:source_id";
const AMBIGUOUS_PARENT_ID_PROPERTY: &str = "glasscheck:ambiguous_parent_id";
const HIT_POINT_X_PROPERTY: &str = "glasscheck:hit_point_x";
const HIT_POINT_Y_PROPERTY: &str = "glasscheck:hit_point_y";
const HIT_RECT_X_PROPERTY: &str = "glasscheck:hit_rect_x";
const HIT_RECT_Y_PROPERTY: &str = "glasscheck:hit_rect_y";
const HIT_RECT_WIDTH_PROPERTY: &str = "glasscheck:hit_rect_width";
const HIT_RECT_HEIGHT_PROPERTY: &str = "glasscheck:hit_rect_height";

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

/// Immutable handle to a node within a specific scene.
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
/// Use this as the primary testing model. It supports hierarchy, selectors,
/// properties, state, visibility, and hit-testing semantics in one snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeProvenanceKind {
    Native,
    Declared,
    Geometric,
    Probed,
    Matched,
    Custom,
}

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
    /// How this node was constructed.
    pub provenance: NodeProvenanceKind,
    /// Per-state provenance metadata.
    pub state_provenance: BTreeMap<String, NodeProvenanceKind>,
    /// Per-property provenance metadata.
    pub property_provenance: BTreeMap<String, NodeProvenanceKind>,
}

impl SemanticNode {
    /// Creates a node with the required fields and sensible defaults.
    ///
    /// `id` is the scene-local identity for this node instance. Use it for
    /// exact lookup within one scene and for structural relationships such as
    /// `parent_id`, but prefer selectors for stable cross-scene queries.
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
            provenance: NodeProvenanceKind::Declared,
            state_provenance: BTreeMap::new(),
            property_provenance: BTreeMap::new(),
        }
    }

    /// Sets the user-visible label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a stable test-facing selector.
    ///
    /// Selectors are the preferred query surface for tests because they are
    /// caller-defined and remain stable even when snapshot-local IDs must be
    /// disambiguated during scene construction.
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
    ///
    /// `parent_id` refers to the scene-local `id` of the parent node, not one
    /// of its selectors.
    #[must_use]
    pub fn with_parent(mut self, parent_id: impl Into<String>, child_index: usize) -> Self {
        self.parent_id = Some(parent_id.into());
        self.child_index = child_index;
        self
    }

    /// Adds a property entry.
    #[must_use]
    pub fn with_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        let key = key.into();
        self.properties.insert(key.clone(), value);
        self.property_provenance.insert(key, self.provenance);
        self
    }

    /// Adds a state entry.
    #[must_use]
    pub fn with_state(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        let key = key.into();
        self.state.insert(key.clone(), value);
        self.state_provenance.insert(key, self.provenance);
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

    /// Sets the node provenance.
    #[must_use]
    pub fn with_provenance(mut self, provenance: NodeProvenanceKind) -> Self {
        self.provenance = provenance;
        self.state_provenance
            .values_mut()
            .for_each(|field| *field = provenance);
        self.property_provenance
            .values_mut()
            .for_each(|field| *field = provenance);
        self
    }
}

/// Declarative node recipe resolved against the current scene and live pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRecipe {
    pub id: String,
    pub locator: RegionSpec,
    pub hit_target: Option<RegionSpec>,
    pub role: Role,
    pub selectors: BTreeSet<String>,
    pub label: Option<String>,
    pub value: Option<String>,
    pub parent_id: Option<String>,
    pub child_index: usize,
    pub z_index: i32,
    pub visible: bool,
    pub hit_testable: bool,
    pub classes: Vec<String>,
    pub tags: Vec<String>,
    pub state: BTreeMap<String, PropertyValue>,
    pub properties: BTreeMap<String, PropertyValue>,
    pub provenance: NodeProvenanceKind,
}

impl NodeRecipe {
    /// Creates a declarative recipe for a node resolved from `locator`.
    ///
    /// `id` becomes the scene-local identity of the resolved node. Use
    /// `with_selector` to attach stable test-facing query names.
    #[must_use]
    pub fn new(id: impl Into<String>, role: Role, locator: RegionSpec) -> Self {
        Self {
            id: id.into(),
            locator,
            hit_target: None,
            role,
            selectors: BTreeSet::new(),
            label: None,
            value: None,
            parent_id: None,
            child_index: 0,
            z_index: 0,
            visible: true,
            hit_testable: true,
            classes: Vec::new(),
            tags: Vec::new(),
            state: BTreeMap::new(),
            properties: BTreeMap::new(),
            provenance: NodeProvenanceKind::Declared,
        }
    }

    /// Adds a stable test-facing selector to the resolved node.
    ///
    /// This is separate from the recipe `id`: the `id` is scene-local
    /// structural identity, while selectors are the preferred public query
    /// names for tests and waits.
    #[must_use]
    pub fn with_selector(mut self, selector: impl Into<String>) -> Self {
        self.selectors.insert(selector.into());
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[must_use]
    pub fn with_parent(mut self, parent_id: impl Into<String>, child_index: usize) -> Self {
        self.parent_id = Some(parent_id.into());
        self.child_index = child_index;
        self
    }

    #[must_use]
    pub fn with_hit_target(mut self, hit_target: RegionSpec) -> Self {
        self.hit_target = Some(hit_target);
        self
    }

    #[must_use]
    pub fn with_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn with_state(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.state.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn with_provenance(mut self, provenance: NodeProvenanceKind) -> Self {
        self.provenance = provenance;
        self
    }

    #[must_use]
    pub fn requires_image(&self) -> bool {
        self.locator.requires_image()
            || self
                .hit_target
                .as_ref()
                .is_some_and(crate::RegionSpec::requires_image)
    }

    pub fn resolve(
        &self,
        scene: &Scene,
        root_bounds: Rect,
        image: Option<&Image>,
    ) -> Result<SemanticNode, crate::RegionResolveError> {
        let rect = scene.resolve_region_with_image(root_bounds, image, &self.locator)?;
        let mut node = SemanticNode::new(self.id.clone(), self.role.clone(), rect)
            .with_provenance(self.provenance);
        node.selectors = self.selectors.clone();
        node.label = self.label.clone();
        node.value = self.value.clone();
        node.parent_id = self.parent_id.clone();
        node.child_index = self.child_index;
        node.z_index = self.z_index;
        node.visible = self.visible;
        node.hit_testable = self.hit_testable;
        node.classes = self.classes.clone();
        node.tags = self.tags.clone();
        node.state = self.state.clone();
        node.properties = self.properties.clone();
        node.state_provenance = self
            .state
            .keys()
            .cloned()
            .map(|key| (key, self.provenance))
            .collect();
        node.property_provenance = self
            .properties
            .keys()
            .cloned()
            .map(|key| (key, self.provenance))
            .collect();
        if let Some(hit_target) = self.hit_target.as_ref() {
            if let Ok(rect) = scene.resolve_region_with_image(root_bounds, image, hit_target) {
                let point = Point::new(
                    rect.origin.x + rect.size.width / 2.0,
                    rect.origin.y + rect.size.height / 2.0,
                );
                node.properties.insert(
                    HIT_POINT_X_PROPERTY.into(),
                    PropertyValue::Integer(point.x.round() as i64),
                );
                node.properties.insert(
                    HIT_POINT_Y_PROPERTY.into(),
                    PropertyValue::Integer(point.y.round() as i64),
                );
                node.properties.insert(
                    HIT_RECT_X_PROPERTY.into(),
                    PropertyValue::Integer(rect.origin.x.round() as i64),
                );
                node.properties.insert(
                    HIT_RECT_Y_PROPERTY.into(),
                    PropertyValue::Integer(rect.origin.y.round() as i64),
                );
                node.properties.insert(
                    HIT_RECT_WIDTH_PROPERTY.into(),
                    PropertyValue::Integer(rect.size.width.round() as i64),
                );
                node.properties.insert(
                    HIT_RECT_HEIGHT_PROPERTY.into(),
                    PropertyValue::Integer(rect.size.height.round() as i64),
                );
                node.property_provenance
                    .insert(HIT_POINT_X_PROPERTY.into(), self.provenance);
                node.property_provenance
                    .insert(HIT_POINT_Y_PROPERTY.into(), self.provenance);
                node.property_provenance
                    .insert(HIT_RECT_X_PROPERTY.into(), self.provenance);
                node.property_provenance
                    .insert(HIT_RECT_Y_PROPERTY.into(), self.provenance);
                node.property_provenance
                    .insert(HIT_RECT_WIDTH_PROPERTY.into(), self.provenance);
                node.property_provenance
                    .insert(HIT_RECT_HEIGHT_PROPERTY.into(), self.provenance);
            }
        }
        Ok(node)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeRecipeResolutionError {
    pub recipe_id: String,
    pub error: crate::RegionResolveError,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedNodeRecipes {
    pub nodes: Vec<SemanticNode>,
    pub errors: Vec<NodeRecipeResolutionError>,
}

/// Resolves recipe nodes in declaration order, allowing later recipes to anchor
/// against earlier resolved nodes from the same source.
#[must_use]
pub fn resolve_node_recipes(
    existing_nodes: Vec<SemanticNode>,
    root_bounds: Rect,
    image: Option<&Image>,
    recipes: &[NodeRecipe],
) -> ResolvedNodeRecipes {
    let mut nodes = existing_nodes;
    let mut errors = Vec::new();
    let recipe_id_counts = recipes.iter().fold(BTreeMap::new(), |mut counts, recipe| {
        *counts.entry(recipe.id.clone()).or_default() += 1;
        counts
    });
    let mut used_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    for recipe in recipes {
        let normalized_recipe = normalize_recipe(recipe, &nodes, &recipe_id_counts, &mut used_ids);
        let scene = Scene::new(nodes.clone());
        match normalized_recipe.resolve(&scene, root_bounds, image) {
            Ok(node) => nodes.push(node),
            Err(error) => errors.push(NodeRecipeResolutionError {
                recipe_id: recipe.id.clone(),
                error,
            }),
        }
    }
    ResolvedNodeRecipes { nodes, errors }
}

fn normalize_recipe(
    recipe: &NodeRecipe,
    existing_nodes: &[SemanticNode],
    recipe_id_counts: &BTreeMap<String, usize>,
    used_ids: &mut BTreeSet<String>,
) -> NodeRecipe {
    let mut normalized = recipe.clone();
    let occupied_raw_ids = existing_nodes
        .iter()
        .filter_map(node_source_id)
        .collect::<BTreeSet<_>>();
    let needs_namespace = used_ids.contains(&recipe.id)
        || occupied_raw_ids.contains(&recipe.id)
        || recipe_id_counts.get(&recipe.id).copied().unwrap_or(0) > 1;
    normalized.id = if needs_namespace {
        unique_node_id(&format!("provider::{}", recipe.id), used_ids)
    } else {
        used_ids.insert(recipe.id.clone());
        recipe.id.clone()
    };
    normalized
        .properties
        .entry(SOURCE_ID_PROPERTY.into())
        .or_insert_with(|| PropertyValue::string(recipe.id.clone()));

    let alias_map = recipe_aliases(existing_nodes);
    normalized.locator = remap_region_spec_ids(&normalized.locator, &alias_map);
    normalized.hit_target = normalized
        .hit_target
        .as_ref()
        .map(|region| remap_region_spec_ids(region, &alias_map));
    if let Some(parent_id) = normalized.parent_id.clone() {
        if alias_map.exact_ids.contains(&parent_id) {
            normalized.parent_id = Some(parent_id);
        } else if let Some(mapped) = alias_map.aliases.get(&parent_id) {
            normalized.parent_id = Some(mapped.clone());
        } else if alias_map.ambiguous_aliases.contains(&parent_id) {
            normalized.properties.insert(
                AMBIGUOUS_PARENT_ID_PROPERTY.into(),
                PropertyValue::string(parent_id),
            );
            normalized.parent_id = None;
        }
    }

    normalized
}

struct RecipeAliasMap {
    exact_ids: BTreeSet<String>,
    aliases: BTreeMap<String, String>,
    ambiguous_aliases: BTreeSet<String>,
}

fn recipe_aliases(nodes: &[SemanticNode]) -> RecipeAliasMap {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut ids = BTreeMap::<String, String>::new();
    let exact_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    for node in nodes {
        let Some(source_id) = node_source_id(node) else {
            continue;
        };
        *counts.entry(source_id.clone()).or_default() += 1;
        ids.insert(source_id, node.id.clone());
    }

    let aliases = counts
        .iter()
        .filter(|(_, count)| **count == 1)
        .filter(|(source_id, _)| !exact_ids.contains(*source_id))
        .filter_map(|(source_id, _)| ids.get(source_id).map(|id| (source_id.clone(), id.clone())))
        .collect::<BTreeMap<_, _>>();
    let ambiguous_aliases = counts
        .into_iter()
        .filter_map(|(source_id, count)| (count > 1).then_some(source_id))
        .collect::<BTreeSet<_>>();

    RecipeAliasMap {
        exact_ids,
        aliases,
        ambiguous_aliases,
    }
}

fn node_source_id(node: &SemanticNode) -> Option<String> {
    let PropertyValue::String(source_id) = node.properties.get(SOURCE_ID_PROPERTY)? else {
        return None;
    };
    Some(source_id.clone())
}

fn unique_node_id(base_id: &str, used_ids: &mut BTreeSet<String>) -> String {
    if used_ids.insert(base_id.to_string()) {
        return base_id.to_string();
    }

    let mut suffix = 1usize;
    loop {
        let candidate = format!("{base_id}#{suffix}");
        if used_ids.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn remap_region_spec_ids(region: &RegionSpec, aliases: &RecipeAliasMap) -> RegionSpec {
    RegionSpec {
        anchor: remap_anchor_ids(&region.anchor, aliases),
        bounds: region.bounds,
        absolute: region.absolute,
    }
}

fn remap_anchor_ids(
    anchor: &crate::anchor::Anchor,
    aliases: &RecipeAliasMap,
) -> crate::anchor::Anchor {
    match anchor {
        crate::anchor::Anchor::Root => crate::anchor::Anchor::Root,
        crate::anchor::Anchor::Rect(rect) => crate::anchor::Anchor::Rect(*rect),
        crate::anchor::Anchor::Node(predicate) => {
            crate::anchor::Anchor::Node(remap_node_predicate_ids(predicate, aliases))
        }
        crate::anchor::Anchor::Handle(handle) => crate::anchor::Anchor::Handle(*handle),
        crate::anchor::Anchor::Region(region) => {
            crate::anchor::Anchor::Region(Box::new(remap_region_spec_ids(region, aliases)))
        }
        crate::anchor::Anchor::PixelProbe { region, probe } => crate::anchor::Anchor::PixelProbe {
            region: Box::new(remap_region_spec_ids(region, aliases)),
            probe: *probe,
        },
        crate::anchor::Anchor::RegionProbe { region, probe } => {
            crate::anchor::Anchor::RegionProbe {
                region: Box::new(remap_region_spec_ids(region, aliases)),
                probe: *probe,
            }
        }
        crate::anchor::Anchor::ImageMatch { region, matcher } => {
            crate::anchor::Anchor::ImageMatch {
                region: Box::new(remap_region_spec_ids(region, aliases)),
                matcher: matcher.clone(),
            }
        }
        crate::anchor::Anchor::Custom { region, refiner } => crate::anchor::Anchor::Custom {
            region: Box::new(remap_region_spec_ids(region, aliases)),
            refiner: refiner.clone(),
        },
    }
}

fn remap_node_predicate_ids(predicate: &Selector, aliases: &RecipeAliasMap) -> Selector {
    match predicate {
        Selector::IdEq(id) => {
            if aliases.exact_ids.contains(id) {
                Selector::IdEq(id.clone())
            } else {
                aliases
                    .aliases
                    .get(id)
                    .cloned()
                    .map(Selector::IdEq)
                    .unwrap_or_else(|| Selector::IdEq(id.clone()))
            }
        }
        Selector::SelectorEq(selector) => Selector::SelectorEq(selector.clone()),
        Selector::AnySelector(matcher) => Selector::AnySelector(matcher.clone()),
        Selector::RoleEq(role) => Selector::RoleEq(role.clone()),
        Selector::Label(matcher) => Selector::Label(matcher.clone()),
        Selector::Value(matcher) => Selector::Value(matcher.clone()),
        Selector::ClassEq(class) => Selector::ClassEq(class.clone()),
        Selector::TagEq(tag) => Selector::TagEq(tag.clone()),
        Selector::PropertyEq(key, value) => Selector::PropertyEq(key.clone(), value.clone()),
        Selector::StateEq(key, value) => Selector::StateEq(key.clone(), value.clone()),
        Selector::Parent(inner) => {
            Selector::Parent(Box::new(remap_node_predicate_ids(inner, aliases)))
        }
        Selector::Ancestor(inner) => {
            Selector::Ancestor(Box::new(remap_node_predicate_ids(inner, aliases)))
        }
        Selector::Not(inner) => Selector::Not(Box::new(remap_node_predicate_ids(inner, aliases))),
        Selector::And(predicates) => Selector::And(
            predicates
                .iter()
                .map(|predicate| remap_node_predicate_ids(predicate, aliases))
                .collect(),
        ),
        Selector::Or(predicates) => Selector::Or(
            predicates
                .iter()
                .map(|predicate| remap_node_predicate_ids(predicate, aliases))
                .collect(),
        ),
    }
}

/// Serializable semantic scene under test.
///
/// A scene is the main query surface for semantic assertions, waits, and
/// scene diffs. Build one per assertion step when the UI can change over time.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    nodes: Vec<SemanticNode>,
    recipe_errors: Vec<NodeRecipeResolutionError>,
    id_index: BTreeMap<String, Vec<usize>>,
    selector_index: BTreeMap<String, Vec<usize>>,
    children_index: BTreeMap<String, Vec<usize>>,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Scene {
    /// Creates a scene from collected semantic nodes.
    ///
    /// The constructor builds indexes for exact ID lookup, selector lookup, and
    /// parent-child traversal.
    #[must_use]
    pub fn new(nodes: Vec<SemanticNode>) -> Self {
        Self::with_recipe_errors(nodes, Vec::new())
    }

    /// Creates a scene from collected semantic nodes and recipe diagnostics.
    #[must_use]
    pub fn with_recipe_errors(
        nodes: Vec<SemanticNode>,
        recipe_errors: Vec<NodeRecipeResolutionError>,
    ) -> Self {
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
            recipe_errors,
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

    /// Returns non-fatal recipe resolution failures captured during snapshot construction.
    #[must_use]
    pub fn recipe_errors(&self) -> &[NodeRecipeResolutionError] {
        &self.recipe_errors
    }

    /// Returns the node referenced by `handle`.
    #[must_use]
    pub fn node(&self, handle: NodeHandle) -> Option<&SemanticNode> {
        self.nodes.get(handle.index)
    }

    /// Finds all handles matching `predicate`.
    #[must_use]
    pub fn find_all(&self, predicate: &crate::Selector) -> Vec<NodeHandle> {
        match predicate {
            crate::Selector::IdEq(id) => {
                return self
                    .id_index
                    .get(id)
                    .into_iter()
                    .flat_map(|indices| indices.iter().copied())
                    .map(|index| NodeHandle { index })
                    .collect();
            }
            crate::Selector::SelectorEq(selector) => {
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
    pub fn find(&self, predicate: &crate::Selector) -> Result<NodeHandle, crate::QueryError> {
        let matches = self.find_all(predicate);
        match matches.as_slice() {
            [] => Err(crate::QueryError::NotFound(predicate.clone())),
            [handle] => Ok(*handle),
            _ => Err(crate::QueryError::MultipleMatches {
                selector: predicate.clone(),
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
    pub fn resolve(&self, predicate: &Selector) -> Result<QueryMatch<'_>, QueryError> {
        let handle = self.find(predicate)?;
        self.resolve_handle(handle)
            .ok_or(QueryError::NotFound(predicate.clone()))
    }

    /// Resolves all semantic matches with rich metadata.
    #[must_use]
    pub fn resolve_all(&self, predicate: &Selector) -> Vec<QueryMatch<'_>> {
        self.find_all(predicate)
            .into_iter()
            .filter_map(|handle| self.resolve_handle(handle))
            .collect()
    }

    /// Returns whether the predicate matches any nodes.
    #[must_use]
    pub fn exists(&self, predicate: &Selector) -> bool {
        !self.find_all(predicate).is_empty()
    }

    /// Returns the number of nodes matching the predicate.
    #[must_use]
    pub fn count(&self, predicate: &Selector) -> usize {
        self.find_all(predicate).len()
    }

    /// Returns the raw bounds of the unique match.
    pub fn bounds(&self, predicate: &Selector) -> Result<Rect, QueryError> {
        Ok(self.resolve(predicate)?.bounds)
    }

    /// Returns the visible bounds of the unique match.
    pub fn visible_bounds(&self, predicate: &Selector) -> Result<Rect, QueryError> {
        self.resolve(predicate)?
            .visible_bounds
            .ok_or(QueryError::NotFound(predicate.clone()))
    }

    /// Returns the center point of the unique match's raw bounds.
    pub fn center(&self, predicate: &Selector) -> Result<Point, QueryError> {
        Ok(rect_center(self.bounds(predicate)?))
    }

    /// Returns the center point of the unique match's visible bounds.
    pub fn visible_center(&self, predicate: &Selector) -> Result<Point, QueryError> {
        Ok(rect_center(self.visible_bounds(predicate)?))
    }

    /// Returns all raw bounds for all matches.
    #[must_use]
    pub fn all_bounds(&self, predicate: &Selector) -> Vec<Rect> {
        self.resolve_all(predicate)
            .into_iter()
            .map(|resolved| resolved.bounds)
            .collect()
    }

    /// Returns the interactability classification for the unique match.
    pub fn interactability(&self, predicate: &Selector) -> Result<Interactability, QueryError> {
        Ok(self.resolve(predicate)?.interactability)
    }

    /// Returns the preferred hit point for the unique match.
    pub fn preferred_hit_point(&self, predicate: &Selector) -> Result<Point, QueryError> {
        self.interactability(predicate)?
            .preferred_hit_point()
            .ok_or(QueryError::NotFound(predicate.clone()))
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
                let rect = hit_test_bounds_for_node(node)?;
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
        let preferred_hit_point = interactability
            .preferred_hit_point()
            .or_else(|| explicit_hit_point(node));
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
    /// Produces the current semantic snapshot for the live scene.
    fn snapshot(&self) -> crate::SemanticSnapshot {
        crate::SemanticSnapshot::new(self.snapshot_nodes(), self.snapshot_recipes())
    }

    /// Produces the current semantic nodes for the live scene.
    fn snapshot_nodes(&self) -> Vec<SemanticNode>;

    /// Produces declarative node recipes resolved against the live scene and pixels.
    fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
        Vec::new()
    }
}

fn classify_interactability(
    scene: &Scene,
    handle: NodeHandle,
    node: &SemanticNode,
) -> Interactability {
    if !node.visible {
        return Interactability::Hidden;
    }
    if explicit_hit_rect(node).is_none()
        && (node.rect.size.width <= 0.0 || node.rect.size.height <= 0.0)
    {
        return Interactability::ZeroSized;
    }
    let Some(hit_bounds) = hit_test_bounds_for_node(node) else {
        return Interactability::FullyClipped;
    };
    if node.state.get("disabled") == Some(&PropertyValue::Bool(true))
        || node.properties.get("disabled") == Some(&PropertyValue::Bool(true))
    {
        return Interactability::Disabled;
    }
    if !node.hit_testable {
        return Interactability::NotHitTestable;
    }
    let mut occluded = None;
    let mut probe_points = explicit_hit_point(node).into_iter().collect::<Vec<_>>();
    for point in interactability_probe_points(hit_bounds) {
        if !probe_points.contains(&point) {
            probe_points.push(point);
        }
    }
    for hit_point in probe_points {
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

fn explicit_hit_point(node: &SemanticNode) -> Option<Point> {
    match (
        node.properties.get(HIT_POINT_X_PROPERTY),
        node.properties.get(HIT_POINT_Y_PROPERTY),
    ) {
        (Some(PropertyValue::Integer(x)), Some(PropertyValue::Integer(y))) => {
            Some(Point::new(*x as f64, *y as f64))
        }
        _ => None,
    }
}

fn explicit_hit_rect(node: &SemanticNode) -> Option<Rect> {
    match (
        node.properties.get(HIT_RECT_X_PROPERTY),
        node.properties.get(HIT_RECT_Y_PROPERTY),
        node.properties.get(HIT_RECT_WIDTH_PROPERTY),
        node.properties.get(HIT_RECT_HEIGHT_PROPERTY),
    ) {
        (
            Some(PropertyValue::Integer(x)),
            Some(PropertyValue::Integer(y)),
            Some(PropertyValue::Integer(width)),
            Some(PropertyValue::Integer(height)),
        ) if *width > 0 && *height > 0 => Some(Rect::new(
            Point::new(*x as f64, *y as f64),
            crate::Size::new(*width as f64, *height as f64),
        )),
        _ => None,
    }
}

fn hit_test_bounds_for_node(node: &SemanticNode) -> Option<Rect> {
    explicit_hit_rect(node).or_else(|| {
        let rect = node.visible_rect.unwrap_or(node.rect);
        (rect.size.width > 0.0 && rect.size.height > 0.0).then_some(rect)
    })
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
    scene: &Scene,
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
    scene: &Scene,
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

fn paint_order_path(scene: &Scene, index: usize) -> Vec<(i32, usize, usize)> {
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

fn is_ancestor(scene: &Scene, ancestor_index: usize, descendant_index: usize) -> bool {
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
    fn has_ancestor_matching(&self, node: &SemanticNode, predicate: &crate::Selector) -> bool;
}

struct ScenePredicateContext<'a> {
    scene: &'a Scene,
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

    fn has_ancestor_matching(&self, node: &SemanticNode, predicate: &crate::Selector) -> bool {
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

pub(crate) fn scene_context(scene: &Scene, index: usize) -> impl PredicateContext + '_ {
    let _ = index;
    ScenePredicateContext { scene }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Image, PixelMatch, PixelProbe, Point, QueryError, RegionResolveError, RegionSpec, Selector,
        Size,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    fn rect() -> Rect {
        Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))
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

    #[test]
    fn ancestor_predicates_follow_parent_chains() {
        let scene = Scene::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("branch", Role::Container, rect()).with_parent("root", 0),
            SemanticNode::new("leaf", Role::Label, rect()).with_parent("branch", 0),
        ]);

        let handle = scene
            .find(&Selector::ancestor(Selector::id_eq("branch")))
            .unwrap();
        assert_eq!(scene.node(handle).unwrap().id, "leaf");
    }

    #[test]
    fn ancestor_predicates_stop_on_parent_cycles() {
        let scene = Scene::new(vec![
            SemanticNode::new("a", Role::Container, rect()).with_parent("b", 0),
            SemanticNode::new("b", Role::Container, rect()).with_parent("a", 0),
        ]);
        let predicate = Selector::ancestor(Selector::id_eq("missing"));
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = scene.find(&predicate);
            tx.send(result).unwrap();
        });

        let result = rx
            .recv_timeout(Duration::from_millis(200))
            .expect("ancestor lookup should terminate even on cycles");
        assert!(matches!(result, Err(QueryError::NotFound(_))));
    }

    #[test]
    fn children_and_parent_queries_use_indexes() {
        let scene = Scene::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("first", Role::Label, rect()).with_parent("root", 0),
            SemanticNode::new("second", Role::Label, rect()).with_parent("root", 1),
        ]);

        let root = scene.find(&Selector::id_eq("root")).unwrap();
        let children = scene.children_of(root);

        assert_eq!(children.len(), 2);
        assert_eq!(scene.node(children[0]).unwrap().id, "first");
        assert_eq!(scene.node(children[1]).unwrap().id, "second");
        assert_eq!(scene.parent_of(children[0]), Some(root));
    }

    #[test]
    fn duplicate_parent_ids_do_not_merge_children() {
        let scene = Scene::new(vec![
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("root", Role::Container, rect()),
            SemanticNode::new("child", Role::Label, rect()).with_parent("root", 0),
        ]);

        let duplicates = scene.find_all(&Selector::id_eq("root"));

        assert_eq!(duplicates.len(), 2);
        assert!(scene.children_of(duplicates[0]).is_empty());
        assert_eq!(
            scene.parent_of(scene.find(&Selector::id_eq("child")).unwrap()),
            None
        );
    }

    #[test]
    fn selector_indexes_support_exact_and_fuzzy_lookup() {
        let scene = Scene::new(vec![
            SemanticNode::new("run", Role::Button, rect())
                .with_selector("toolbar.run")
                .with_selector("run"),
            SemanticNode::new("stop", Role::Button, rect()).with_selector("toolbar.stop"),
        ]);

        assert_eq!(scene.count(&Selector::selector_eq("toolbar.run")), 1);
        assert_eq!(
            scene
                .find(&Selector::any_selector(crate::TextMatch::contains("stop")))
                .unwrap()
                .index(),
            1
        );
        assert!(!scene.exists(&Selector::selector_eq("missing")));
    }

    #[test]
    fn resolve_helpers_expose_bounds_centers_and_counts() {
        let scene = Scene::new(vec![
            SemanticNode::new("run", Role::Button, rect()).with_selector("toolbar.run"),
            SemanticNode::new(
                "secondary",
                Role::Button,
                Rect::new(Point::new(20.0, 0.0), Size::new(10.0, 10.0)),
            ),
        ]);

        assert_eq!(scene.count(&Selector::role_eq(Role::Button)), 2);
        assert_eq!(
            scene.bounds(&Selector::selector_eq("toolbar.run")).unwrap(),
            rect()
        );
        assert_eq!(
            scene.center(&Selector::selector_eq("toolbar.run")).unwrap(),
            Point::new(5.0, 5.0)
        );
        assert_eq!(scene.all_bounds(&Selector::role_eq(Role::Button)).len(), 2);
    }

    #[test]
    fn visible_bounds_fall_back_to_raw_bounds_when_visible_rect_is_unknown() {
        let visible =
            Scene::new(vec![SemanticNode::new("provider", Role::Button, rect())
                .with_selector("provider.button")]);

        assert_eq!(
            visible
                .visible_bounds(&Selector::selector_eq("provider.button"))
                .unwrap(),
            rect()
        );
        assert_eq!(
            visible
                .resolve(&Selector::selector_eq("provider.button"))
                .unwrap()
                .visible_bounds,
            Some(rect())
        );

        let hidden = Scene::new(vec![SemanticNode {
            visible: false,
            ..SemanticNode::new("provider", Role::Button, rect()).with_selector("provider.button")
        }]);
        assert!(hidden
            .visible_bounds(&Selector::selector_eq("provider.button"))
            .is_err());
        assert_eq!(
            hidden
                .resolve(&Selector::selector_eq("provider.button"))
                .unwrap()
                .visible_bounds,
            None
        );
    }

    #[test]
    fn hit_path_orders_descendants_above_their_ancestors_within_one_subtree() {
        let scene = Scene::new(vec![
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
        let scene = Scene::new(vec![
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
        let scene = Scene::new(vec![
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
            scene.interactability(&Selector::id_eq("left-leaf")),
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
        let interactable_scene = Scene::new(vec![target.clone()]);
        assert!(matches!(
            interactable_scene.interactability(&Selector::id_eq("target")),
            Ok(Interactability::Interactable { .. })
        ));

        let occluded_scene = Scene::new(vec![
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
            occluded_scene.interactability(&Selector::id_eq("target")),
            Ok(Interactability::Occluded { topmost, .. })
                if occluded_scene.node(topmost).unwrap().id == "overlay"
        ));
    }

    #[test]
    fn interactability_uses_fallback_probe_points_when_center_is_occluded() {
        let scene = Scene::new(vec![
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
            scene.interactability(&Selector::id_eq("target")),
            Ok(Interactability::Interactable { hit_point })
                if hit_point != Point::new(6.0, 6.0)
        ));
    }

    #[test]
    fn explicit_hit_targets_drive_hit_testing_and_interactability() {
        let resolved = resolve_node_recipes(
            Vec::new(),
            Rect::new(Point::new(0.0, 0.0), Size::new(40.0, 40.0)),
            None,
            &[NodeRecipe::new(
                "outside-hit",
                Role::Button,
                RegionSpec::rect(Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0))),
            )
            .with_hit_target(RegionSpec::rect(Rect::new(
                Point::new(20.0, 20.0),
                Size::new(4.0, 4.0),
            )))],
        );
        let scene = Scene::new(resolved.nodes);
        let handle = scene.find(&Selector::id_eq("outside-hit")).unwrap();
        let hit_point = Point::new(22.0, 22.0);

        assert_eq!(scene.hit_path_at(hit_point), vec![handle]);
        assert_eq!(scene.topmost_at(hit_point), Some(handle));
        assert_eq!(
            scene
                .preferred_hit_point(&Selector::id_eq("outside-hit"))
                .unwrap(),
            hit_point
        );
        assert!(matches!(
            scene.interactability(&Selector::id_eq("outside-hit")),
            Ok(Interactability::Interactable { hit_point: actual }) if actual == hit_point
        ));
        assert!(scene.hit_path_at(Point::new(5.0, 5.0)).is_empty());
    }

    #[test]
    fn preferred_hit_point_tracks_fallback_probe_when_explicit_point_is_occluded() {
        let scene = Scene::new(vec![
            SemanticNode::new(
                "target",
                Role::Button,
                Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 12.0)),
            )
            .with_property(HIT_POINT_X_PROPERTY, PropertyValue::Integer(6))
            .with_property(HIT_POINT_Y_PROPERTY, PropertyValue::Integer(6)),
            SemanticNode::new(
                "overlay",
                Role::Button,
                Rect::new(Point::new(4.0, 4.0), Size::new(4.0, 4.0)),
            )
            .with_property(PAINT_ORDER_PATH_PROPERTY, PropertyValue::string("1")),
        ]);

        let preferred = scene
            .preferred_hit_point(&Selector::id_eq("target"))
            .expect("scene should report the usable fallback hit point");

        assert_ne!(preferred, Point::new(6.0, 6.0));
        assert!(matches!(
            scene.interactability(&Selector::id_eq("target")),
            Ok(Interactability::Interactable { hit_point }) if hit_point == preferred
        ));
    }

    #[test]
    fn with_provenance_updates_existing_field_provenance() {
        let node = SemanticNode::new(
            "node",
            Role::Button,
            Rect::new(Point::new(0.0, 0.0), Size::new(10.0, 10.0)),
        )
        .with_property("kind", PropertyValue::string("button"))
        .with_state("enabled", PropertyValue::Bool(true))
        .with_provenance(NodeProvenanceKind::Matched);

        assert_eq!(node.provenance, NodeProvenanceKind::Matched);
        assert_eq!(
            node.property_provenance.get("kind"),
            Some(&NodeProvenanceKind::Matched)
        );
        assert_eq!(
            node.state_provenance.get("enabled"),
            Some(&NodeProvenanceKind::Matched)
        );
    }

    #[test]
    fn missing_hit_target_refinement_falls_back_to_locator_bounds() {
        let resolved = resolve_node_recipes(
            Vec::new(),
            Rect::new(Point::new(0.0, 0.0), Size::new(40.0, 40.0)),
            None,
            &[NodeRecipe::new(
                "fallback-hit",
                Role::Button,
                RegionSpec::rect(Rect::new(Point::new(4.0, 6.0), Size::new(12.0, 8.0))),
            )
            .with_hit_target(RegionSpec::node(Selector::id_eq("missing-hit-target")))],
        );
        let scene = Scene::with_recipe_errors(resolved.nodes, resolved.errors.clone());
        let handle = scene.find(&Selector::id_eq("fallback-hit")).unwrap();

        assert!(scene.recipe_errors().is_empty());
        assert!(scene
            .node(handle)
            .unwrap()
            .properties
            .get(HIT_POINT_X_PROPERTY)
            .is_none());
        assert_eq!(
            scene
                .preferred_hit_point(&Selector::id_eq("fallback-hit"))
                .unwrap(),
            Point::new(10.0, 10.0)
        );
        assert_eq!(scene.hit_path_at(Point::new(10.0, 10.0)), vec![handle]);
        assert!(matches!(
            scene.interactability(&Selector::id_eq("fallback-hit")),
            Ok(Interactability::Interactable { hit_point }) if hit_point == Point::new(10.0, 10.0)
        ));
    }

    #[test]
    fn resolve_node_recipes_builds_clickable_visual_probe_node() {
        let locator = RegionSpec::root().pixel_probe(PixelProbe::new(
            PixelMatch::new([255, 0, 0, 255], 0, 255),
            8,
        ));
        let image = image_with_red_chip();
        let resolved = resolve_node_recipes(
            Vec::new(),
            Rect::new(Point::new(0.0, 0.0), Size::new(12.0, 10.0)),
            Some(&image),
            &[
                NodeRecipe::new("visual.red-chip", Role::Button, locator.clone())
                    .with_selector("visual.red-chip")
                    .with_hit_target(locator),
            ],
        );
        let scene = Scene::with_recipe_errors(resolved.nodes, resolved.errors.clone());
        let handle = scene
            .find(&Selector::selector_eq("visual.red-chip"))
            .unwrap();

        assert!(scene.recipe_errors().is_empty());
        assert_eq!(
            scene.node(handle).unwrap().rect,
            Rect::new(Point::new(3.0, 2.0), Size::new(4.0, 4.0))
        );
        assert_eq!(
            scene
                .preferred_hit_point(&Selector::selector_eq("visual.red-chip"))
                .unwrap(),
            Point::new(5.0, 4.0)
        );
        assert_eq!(scene.hit_path_at(Point::new(5.0, 4.0)), vec![handle]);
        assert!(matches!(
            scene.interactability(&Selector::selector_eq("visual.red-chip")),
            Ok(Interactability::Interactable { hit_point }) if hit_point == Point::new(5.0, 4.0)
        ));
    }

    #[test]
    fn resolve_node_recipes_normalizes_provider_references_and_ids() {
        let resolved = resolve_node_recipes(
            vec![SemanticNode::new(
                "provider::anchor",
                Role::Container,
                Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 10.0)),
            )
            .with_property("glasscheck:source_id", PropertyValue::string("anchor"))],
            Rect::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0)),
            None,
            &[NodeRecipe::new(
                "anchor",
                Role::Button,
                RegionSpec::node(Selector::id_eq("anchor")).right_of(10.0, 12.0),
            )
            .with_parent("anchor", 0)],
        );

        assert!(resolved.errors.is_empty());
        let node = resolved.nodes.last().unwrap();
        assert_eq!(node.id, "provider::anchor#1");
        assert_eq!(node.parent_id.as_deref(), Some("provider::anchor"));
        assert_eq!(
            node.properties.get("glasscheck:source_id"),
            Some(&PropertyValue::string("anchor"))
        );
        assert_eq!(
            node.rect,
            Rect::new(Point::new(30.0, 0.0), Size::new(12.0, 10.0))
        );
    }

    #[test]
    fn resolve_node_recipes_keeps_exact_ids_reachable_when_provider_source_ids_collide() {
        let resolved = resolve_node_recipes(
            vec![
                SemanticNode::new(
                    "anchor",
                    Role::Container,
                    Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 10.0)),
                ),
                SemanticNode::new(
                    "provider::anchor",
                    Role::Container,
                    Rect::new(Point::new(40.0, 0.0), Size::new(20.0, 10.0)),
                )
                .with_property("glasscheck:source_id", PropertyValue::string("anchor")),
            ],
            Rect::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0)),
            None,
            &[NodeRecipe::new(
                "child",
                Role::Button,
                RegionSpec::node(Selector::id_eq("anchor")).right_of(5.0, 12.0),
            )
            .with_parent("anchor", 0)],
        );

        assert!(resolved.errors.is_empty());
        let node = resolved.nodes.last().unwrap();
        assert_eq!(node.parent_id.as_deref(), Some("anchor"));
        assert_eq!(
            node.rect,
            Rect::new(Point::new(25.0, 0.0), Size::new(12.0, 10.0))
        );
    }

    #[test]
    fn resolve_node_recipes_rejects_ambiguous_raw_provider_references() {
        let resolved = resolve_node_recipes(
            vec![
                SemanticNode::new(
                    "provider::anchor",
                    Role::Container,
                    Rect::new(Point::new(0.0, 0.0), Size::new(20.0, 10.0)),
                )
                .with_property("glasscheck:source_id", PropertyValue::string("anchor")),
                SemanticNode::new(
                    "provider::anchor#1",
                    Role::Container,
                    Rect::new(Point::new(30.0, 0.0), Size::new(20.0, 10.0)),
                )
                .with_property("glasscheck:source_id", PropertyValue::string("anchor")),
            ],
            Rect::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0)),
            None,
            &[NodeRecipe::new(
                "child",
                Role::Button,
                RegionSpec::node(Selector::id_eq("anchor")).right_of(10.0, 12.0),
            )],
        );

        assert_eq!(resolved.errors.len(), 1);
        assert_eq!(resolved.errors[0].recipe_id, "child");
        assert!(matches!(
            resolved.errors[0].error,
            RegionResolveError::NotFound(_)
        ));
    }

    #[test]
    fn resolve_node_recipes_preserves_resolution_errors() {
        let resolved = resolve_node_recipes(
            Vec::new(),
            Rect::new(Point::new(0.0, 0.0), Size::new(40.0, 20.0)),
            None,
            &[NodeRecipe::new(
                "missing-anchor",
                Role::Button,
                RegionSpec::node(Selector::selector_eq("provider.anchor")),
            )
            .with_selector("provider.missing")],
        );

        assert!(resolved.nodes.is_empty());
        assert_eq!(resolved.errors.len(), 1);
        assert_eq!(resolved.errors[0].recipe_id, "missing-anchor");
        assert!(matches!(
            resolved.errors[0].error,
            RegionResolveError::NotFound(_)
        ));

        let scene = Scene::with_recipe_errors(resolved.nodes, resolved.errors.clone());
        assert_eq!(scene.recipe_errors(), resolved.errors.as_slice());
    }
}
