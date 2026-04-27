# glasscheck

Functional testing primitives for graphical native Rust UIs.

`glasscheck` is for in-process tests of graphical native UIs, not browser-based UIs. It is for tests that need more than string checks and less than full external automation. It focuses on three things:

- node queries over a scene
- image and rendered-text assertions against live UI pixels
- polling helpers for asynchronous UI state

`glasscheck` is the intended dependency for most users. It re-exports the core APIs and adds the supported native backends behind features. `glasscheck-core` is backend-neutral, but it is mainly useful for backend authors or specialized integrations.

Much of the scene/query/wait design in `glasscheck` is directly inspired by [Zed](https://github.com/zed-industries/zed)'s test harness.

## Shared Facade

When you depend on `glasscheck`, the crate exposes target-specific aliases for a common top-level facade:

- `glasscheck::Harness`
- `glasscheck::WindowHost`

The intended downstream pattern is:

1. build the native fixture using backend-specific toolkit code
2. return the aliased `glasscheck::WindowHost`
3. run the shared assertions through the common host surface

That shared post-mount surface is intentionally aligned across GTK and AppKit for:

- `snapshot_scene()`
- `capture()`
- `capture_region(&RegionSpec)`
- `resolve_region(&RegionSpec)`
- `click_node(&Selector)`
- `hover_node(&Selector, &HitPointSearch)`
- `input()`
- `text_renderer()`

This lets one shared test body compile unchanged after platform-specific fixture setup.

For multi-surface flows, the facade also exposes a target-specific `glasscheck::Session` alias plus `Harness::session()`. Sessions can attach named surfaces, open owned transient surfaces with `open_transient_with_click(...)`, wait for transient dismissal with `wait_for_surface_closed(...)`, delegate node interactions to a specific surface, and still fall back to passive title-based discovery through `SurfaceQuery` when that is the only signal available.

On AppKit, main-thread capability is still explicit for native construction and attachment, but it is carried by the harness/host internally so shared post-mount calls stay marker-free. Harness-managed AppKit windows are kept as background test windows, so capture and input can run without temporary windows surfacing on screen.

Backends with native context-menu support expose `context_click_node(...)`, `context_click_node_with_search(...)`, and `context_click_root_point(...)`. On AppKit these APIs return an `AppKitContextMenu` semantic handle over the native `NSMenu`; on GTK they return a `GtkContextMenu` handle over the opened popover menu. Tests can inspect menu items with `snapshot_scene()` and choose commands with `activate_item(...)`. AppKit also supports `activate_item_at_path(...)` for retained `NSMenu` hierarchies. Glasscheck keeps platform-specific secondary-click behavior inside each backend; callers should express the user intent as a context click.

Sessions also understand native dialogs and panels through the shared `DialogQuery`, `DialogKind`, `DialogCapability`, and `DialogError` API. Use `wait_for_dialog(...)` to attach `NSAlert`, `NSOpenPanel`, `NSSavePanel`, GTK `MessageDialog`, GTK `FileChooserDialog`, or generic panel/dialog surfaces, then inspect surface-backed dialogs with `snapshot_dialog_scene(...)`. Dialog scenes keep the existing core roles (`Window`, `Button`, `TextInput`, `Label`, and list items) and expose backend-specific properties such as `appkit:dialog_kind`, `appkit:view_path`, `gtk:dialog_kind`, and `gtk:widget_path`.

Alerts can be accepted or cancelled through semantic button selectors, and text fields can be edited with `set_dialog_text(...)`. Save/open surfaces use the portable `choose_save_dialog_path(...)` and `choose_open_dialog_paths(...)` names. GTK async dialog controllers can be attached with `GtkDialogController` when a dialog has metadata but no widget surface; unsupported capabilities are reported explicitly through `DialogError::UnsupportedCapability`.

## Crates

Use `glasscheck` unless you are building a new backend or integrating with an existing native test harness.

```toml
[dependencies]
glasscheck = { path = "crates/glasscheck" }
```

Use `glasscheck-core` only when you already have your own scene and pixel capture integration and do not want the built-in native backends.

```toml
[dependencies]
glasscheck-core = { path = "crates/glasscheck-core" }
```

For AppKit-specific setup, prefer `AppKitHarness::create_window`, `attach_window`, and `attach_root_view` over calling `AppKitWindowHost::from_*` directly. That keeps the harness as the public carrier for `MainThreadMarker` and preserves the backend's hidden-window test policy.

AppKit also exposes `AppKitHarness::menu_bar()` for tests that need the process main menu. `menu_bar().snapshot()` reads `NSApplication.mainMenu` into `Role::MenuBar`, `Role::Menu`, `Role::MenuItem`, and `Role::Divider` scene nodes. Top-level menus can be opened by title, index, or selector; an opened menu can be snapshotted, captured, or activated by selector.

Menu capture uses AppKit menu cells drawn into an offscreen bitmap. The default path does not open a native menu popup, does not make a menu visible on screen, and does not require screen-recording permission. `AppKitMenuCaptureOptions::allow_visible_fallback` defaults to `false`; keep it that way unless a test explicitly accepts a visible native fallback for an environment where offscreen rendering is unavailable.

On Linux/GTK, `GtkHarness::new()` returns `Result<_, glib::BoolError>`. GTK initialization depends on the process environment, so callers must handle setup failure explicitly instead of relying on a panic from inside the harness. GTK context menus are modeled as visible popover menus: `GtkContextMenu::snapshot_scene()` emits `Role::Menu`, `Role::MenuItem`, and `Role::Divider` nodes, and `activate_item(...)` activates button-backed items. Model-backed `PopoverMenu`/`gio::Menu` activation is not yet normalized. GTK does not expose an AppKit-style process-global main menu helper.

## Pick An API

Use `Scene` and `Selector` for most new tests. They support hierarchy, selectors, properties, state, hit testing, and scene diffs.

Prefer stable selectors and roles over snapshot-local IDs. IDs are exact, but they can be disambiguated during snapshot construction and should not be treated as cross-snapshot identities.

Use scene-local `id`s for structural relationships inside one scene, such as parent/child wiring. Use selectors for the public test-facing names you query across fresh scene captures.

## Contextual Scene Sources

When a provider needs host geometry, use the backend contextual scene-source API instead of duplicating coordinate conversion in the app:

- AppKit: `AppKitWindowHost::set_contextual_scene_source(...)` with `AppKitSnapshotContext`
- GTK: `GtkWindowHost::set_contextual_scene_source(...)` with `GtkSnapshotContext`

Context objects expose root bounds, view/widget rect conversion, visible rect lookup, text range rects, insertion caret rects, and selected text ranges. Legacy `set_scene_source(...)` and `SemanticProvider` remain supported and are adapted into the new snapshot model.

Providers now return a unified `SemanticSnapshot { nodes, recipes }` instead of splitting those concepts across separate host plumbing.

## Text Geometry And Interaction

Both native hosts expose backend-native text helpers for direct layout assertions and caret-driven interaction:

- `text_range_rect(...)`
- `insertion_caret_rect(...)`
- `selected_text_range(...)`
- `click_text_position(...)`

Prefer these helpers when tests need text-backed geometry or caret assertions rather than app-local adapters.

`host.input().key_press_queued(key, modifiers)` is the monitor-aware path: it routes key events through any application-level monitors or root-level controllers before the focused responder. Use `key_press` when direct controller delivery is sufficient.

## Scene Queries

```rust
use glasscheck_core::{
    Node, Selector, Point, PropertyValue, Rect, Role, Scene, Size,
};

let scene = Scene::new(vec![
    Node::new(
        "save-button",
        Role::Button,
        Rect::new(Point::new(20.0, 12.0), Size::new(80.0, 32.0)),
    )
    .with_selector("toolbar.save")
    .with_label("Save")
    .with_state("enabled", PropertyValue::Bool(true)),
]);

let save = scene.resolve(&Selector::selector_eq("toolbar.save")).unwrap();
assert_eq!(save.node.label.as_deref(), Some("Save"));
```

## Node Refinement

`glasscheck` can build nodes from whatever hooks the UI exposes: native widgets, declared geometry, or visual refinement over live pixels.

### Direct Geometry

Use this when you already know the exact bounds.

```rust
use glasscheck_core::{NodeRecipe, RegionLocator, Rect, Point, Role, Size};

let recipe = NodeRecipe::new(
    "back-button",
    Role::Button,
    RegionLocator::rect(Rect::new(Point::new(20.0, 20.0), Size::new(28.0, 28.0))),
)
.with_selector("browser.nav.back");
```

Here `id` and `selector` serve different purposes: the recipe `id` is the scene-local node identity used for parent/child relationships and internal resolution, while the selector is the stable test-facing query name.

Failure mode: the node still resolves, but later assertions fail if the real UI no longer draws or behaves correctly in that region.

### Nested Subregions

Use this when you know a stable outer node but want a tighter child region.

```rust
use glasscheck_core::{Selector, RegionLocator, RelativeBounds};

let title_region = RegionLocator::node(Selector::selector_eq("card"))
    .subregion(RelativeBounds::inset(0.1, 0.1, 0.1, 0.6));
```

Failure mode: if the outer node moves or disappears, region resolution fails at the anchor.

### Regions Outside Another Region

Use this for “nearby but not inside” regions, such as an affordance expected to appear 50px to the right of another node.

```rust
use glasscheck_core::{Selector, RegionLocator};

let search_space = RegionLocator::node(Selector::selector_eq("sidebar.item"))
    .right_of(50.0, 120.0);
```

Failure mode: the region still resolves geometrically, but follow-up probes or assertions fail if the UI regression means the visual target is not actually there.

### Pixel Probes

Use this when a control is custom-drawn and can be identified by a stable pixel signature. The expected pixel can be fully opaque or partially opaque over an unknown background.

```rust
use glasscheck_core::{NodeRecipe, PixelMatch, PixelProbe, RegionLocator, Role};

let recipe = NodeRecipe::new(
    "visual.red-chip",
    Role::Button,
    RegionLocator::root().pixel_probe(PixelProbe::new(
        PixelMatch::new([255, 0, 0, 255], 0, 255),
        8,
    )),
);
```

Failure mode: the node is omitted from the snapshot if the matching pixels are no longer present.

### Region Probes

Use this when you expect a single connected visual component and want ambiguity to fail loudly. Region probes use the same alpha-aware pixel matching as `PixelProbe`.

```rust
use glasscheck_core::{NodeRecipe, PixelMatch, RegionLocator, RegionProbe, Role};

let recipe = NodeRecipe::new(
    "visual.badge",
    Role::Button,
    RegionLocator::root().region_probe(RegionProbe::new(
        PixelMatch::new([255, 0, 0, 255], 8, 200),
        8,
        2.0,
    )),
);
```

Failure mode: the node is omitted if no component matches, and resolution fails if multiple components match.

### Alpha-Aware Image Matching

Use this for icons or custom affordances where a small template is more stable than a single-pixel signature. Fully transparent template pixels are ignored during scoring, and partially transparent template pixels are matched as foreground references over an unknown background.

```rust
use glasscheck_core::{CompareConfig, Image, ImageMatch, NodeRecipe, RegionLocator, Role};

let template = Image::new(2, 2, vec![
    255, 0, 0, 255, 255, 0, 0, 255,
    255, 0, 0, 255, 255, 0, 0, 255,
]);

let recipe = NodeRecipe::new(
    "visual.chevron",
    Role::Button,
    RegionLocator::root().image_match(ImageMatch::new(
        template,
        CompareConfig {
            channel_tolerance: 4,
            match_threshold: 0.98,
            generate_diff: false,
        },
    )),
);
```

Failure mode: resolution fails with a below-threshold match when the icon is present but visually wrong. Transparent padding does not help a weak match pass the threshold.

### Independent Hit Targets

Use this when the node bounds should stay coarse but clicks should land on a tighter visual sub-region.

```rust
use glasscheck_core::{NodeRecipe, PixelMatch, PixelProbe, RegionLocator, Role};

let hit_target = RegionLocator::root().pixel_probe(PixelProbe::new(
    PixelMatch::new([255, 0, 0, 255], 0, 255),
    8,
));

let recipe = NodeRecipe::new("visual.red-chip", Role::Button, hit_target.clone())
    .with_hit_target(hit_target);
```

Failure mode: when the hit target cannot be refined, clicks fall back to the node bounds unless you make the recipe itself depend on the same refined region.

## Wait For Real UI State

Use waits when the UI changes across frames or event-loop turns. The helpers return the last scene or match data on timeout, which is more useful than a bare sleep.

```rust
use glasscheck_core::{wait_for_exists, Selector, PollOptions, Role};

let scene = wait_for_exists(
    PollOptions::default(),
    || app.snapshot_scene(),
    &Selector::role_eq(Role::Button),
)
.unwrap();

assert!(scene.count(&Selector::role_eq(Role::Button)) >= 1);
```

## Rendered Text

Use rendered-text assertions when string extraction is not enough, such as custom text rendering, truncation, aliasing, or color-sensitive checks.

Prefer anchored expectations over fixed rectangles when the layout can move.

```rust
use glasscheck_core::{
    AnchoredTextExpectation, Selector, RegionLocator, RelativeBounds, Role, TextMatch,
};

let expectation = AnchoredTextExpectation::new(
    "Run",
    RegionLocator::node(Selector::and(vec![
        Selector::role_eq(Role::Button),
        Selector::label(TextMatch::contains("Run")),
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

`glasscheck-appkit` is for in-process AppKit tests on macOS. It provides window hosting, pixel capture, scenes, hit-point-based interaction, native context-menu inspection, shared native-dialog helpers, and text rendering. Standard `NSControl` clicks are activated through AppKit control APIs when available. Native context menus are exposed as retained `NSMenu` semantic handles rather than capturable popup window surfaces. `NSAlert`, `NSOpenPanel`, and `NSSavePanel` are driven through the shared `DialogQuery` and `choose_*_dialog_*` session APIs.

`glasscheck-gtk` is the Linux GTK4 backend. It provides the same overall testing model, including shared native-dialog helpers for `MessageDialog`, `FileChooserDialog`, generic `Dialog` surfaces, and metadata-only async dialog records through `GtkDialogController`. Higher-level semantic interactions such as `GtkWindowHost::click_node(...)` are best-effort and may use GTK widget/controller activation for registered widgets before falling back to native input synthesis. `context_click_node(...)` opens visible popover-backed context menus when the app exposes one; activation currently requires a button-backed menu item in the visible widget tree. Direct pointer synthesis through `GtkInputDriver` uses native X11 dispatch on X11-backed windows; `key_press_queued` also uses XTest-based X11 injection so events flow through root-level and legacy controllers before the focused widget, while `key_press` follows GTK controller and text APIs directly. Outside that direct-input support surface, the native input driver reports unavailability instead of silently degrading.

## AppKit Verification

Use the normal AppKit native contract command for default local and CI-style verification:

- `cargo test -p glasscheck-appkit --test appkit_contracts --features native-contract-tests`

That default run keeps live foreground-window contracts, live `NSAlert` sheet/modal contracts, and live `NSSavePanel`/`NSOpenPanel` interaction contracts skipped. Those contracts are separate because making native windows key/front, presenting AppKit modal UI, or presenting system file panels can interrupt focus, beep, launch macOS file-panel services, or surface visible system UI, which conflicts with hidden/background-only local verification.

Run the disruptive foreground, modal-dialog, and file-panel contracts only when focus interruption or visible/system AppKit behavior is acceptable in the current session:

- `GLASSCHECK_RUN_DISRUPTIVE_APPKIT_TESTS=all cargo test -p glasscheck-appkit --test appkit_contracts --features native-contract-tests`

For narrower local runs, `GLASSCHECK_RUN_DISRUPTIVE_APPKIT_TESTS` also accepts comma-separated categories: `foreground`, `modal-dialog`, and `file-panel`.

The macOS CI AppKit contracts step enables `GLASSCHECK_RUN_DISRUPTIVE_APPKIT_TESTS=all` to keep that coverage in CI while preserving the non-disruptive default local command.

## GTK Verification

Use real X11 when you need to validate live desktop behavior. Use `xvfb-run` when you need headless or CI-style verification.

- `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`
- `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
- `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`

Always force `GDK_BACKEND=x11` for these GTK verification paths.
