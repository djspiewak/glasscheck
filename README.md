# glasscheck

Functional testing primitives for graphical native Rust UIs.

`glasscheck` is for in-process tests of graphical native UIs, not browser-based UIs. It is for tests that need more than string checks and less than full external automation. It focuses on three things:

- semantic queries over a scene snapshot
- image and rendered-text assertions against live UI pixels
- polling helpers for asynchronous UI state

`glasscheck` is the intended dependency for most users. It re-exports the core APIs and adds the supported native backends behind features. `glasscheck-core` is backend-neutral, but it is mainly useful for backend authors or specialized integrations.

Much of the semantic scene/query/wait design in `glasscheck` is directly inspired by [Zed](https://github.com/zed-industries/zed)'s test harness.

## Crates

Use `glasscheck` unless you are building a new backend or integrating with an existing native test harness.

```toml
[dependencies]
glasscheck = { path = "crates/glasscheck" }
```

Use `glasscheck-core` only when you already have your own scene snapshot and pixel capture integration and do not want the built-in native backends.

```toml
[dependencies]
glasscheck-core = { path = "crates/glasscheck-core" }
```

## Pick An API

Use `SceneSnapshot` and `NodePredicate` for most new tests. They support hierarchy, selectors, properties, state, hit testing, and scene diffs.

Use `QueryRoot` and `Selector` only when you need the older flat metadata model or want a very small compatibility layer. The tradeoff is less expressive matching.

Prefer stable selectors and semantic roles over snapshot-local IDs. IDs are exact, but they can be disambiguated during snapshot construction and should not be treated as cross-snapshot identities.

## Scene Queries

```rust
use glasscheck_core::{
    NodePredicate, Point, PropertyValue, Rect, Role, SceneSnapshot, SemanticNode, Size,
};

let scene = SceneSnapshot::new(vec![
    SemanticNode::new(
        "save-button",
        Role::Button,
        Rect::new(Point::new(20.0, 12.0), Size::new(80.0, 32.0)),
    )
    .with_selector("toolbar.save")
    .with_label("Save")
    .with_state("enabled", PropertyValue::Bool(true)),
]);

let save = scene.resolve(&NodePredicate::selector_eq("toolbar.save")).unwrap();
assert_eq!(save.node.label.as_deref(), Some("Save"));
```

## Wait For Real UI State

Use waits when the UI changes across frames or event-loop turns. The helpers return the last scene or match data on timeout, which is more useful than a bare sleep.

```rust
use glasscheck_core::{wait_for_exists, NodePredicate, PollOptions, Role};

let scene = wait_for_exists(
    PollOptions::default(),
    || app.snapshot_scene(),
    &NodePredicate::role_eq(Role::Button),
)
.unwrap();

assert!(scene.count(&NodePredicate::role_eq(Role::Button)) >= 1);
```

## Rendered Text

Use rendered-text assertions when string extraction is not enough, such as custom text rendering, truncation, aliasing, or color-sensitive checks.

Prefer anchored expectations over fixed rectangles when the layout can move.

```rust
use glasscheck_core::{
    AnchoredTextExpectation, NodePredicate, RegionSpec, RelativeBounds, Role, TextMatch,
};

let expectation = AnchoredTextExpectation::new(
    "Run",
    RegionSpec::node(NodePredicate::and(vec![
        NodePredicate::role_eq(Role::Button),
        NodePredicate::label(TextMatch::contains("Run")),
    ]))
    .subregion(RelativeBounds::inset(0.1, 0.1, 0.1, 0.1)),
);
```

## Image Baselines

Use direct image comparison for screenshots, icons, or custom drawing where semantic metadata is too coarse.

```rust
use glasscheck_core::{compare_images, CompareConfig, Image};

let actual = Image::new(1, 1, vec![255, 255, 255, 255]);
let expected = Image::new(1, 1, vec![255, 255, 255, 255]);

assert!(compare_images(&actual, &expected, &CompareConfig::default()).passed);
```

## Native Backends

Only AppKit on macOS and GTK4 on Linux are supported native backends today.

`glasscheck-appkit` is for in-process AppKit tests on macOS. It provides window hosting, pixel capture, semantic scene snapshots, hit-point-based clicks, and text rendering.

`glasscheck-gtk` is the Linux GTK4 backend. It provides the same overall testing model, but some low-level input paths remain best-effort and may fall back to widget activation or focus routing.

## GTK Verification

Use real X11 when you need to validate live desktop behavior. Use `xvfb-run` when you need headless or CI-style verification.

- `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`
- `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`

Always force `GDK_BACKEND=x11` for these GTK verification paths.
