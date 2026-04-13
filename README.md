# glasscheck

Functional testing primitives for native UI applications written in Rust.

Much of the semantic scene/query/wait functionality in `glasscheck` is directly inspired by
[Zed](https://github.com/zed-industries/zed)'s test harness design.

The intended use-case is stable, maintainable, sandbox-friendly assertions around real UI state:

- query instrumented views by semantic metadata
- capture rendered output without external automation tooling
- assert text and image output with tolerances
- wait for asynchronous UI state to settle before asserting

`glasscheck-core` provides the portable assertion model.
`glasscheck` re-exports the core crate and, on macOS, the AppKit backend for in-process testing.

## Add It

```toml
[dependencies]
glasscheck = { path = "crates/glasscheck" }
```

Core-only:

```toml
[dependencies]
glasscheck-core = { path = "crates/glasscheck-core" }
```

## Testing Flow

Model the views you care about with semantic IDs, roles, and labels:

```rust
use glasscheck_core::{
    NodeMetadata, Point, QueryRoot, Rect, Role, Selector, Size,
};

let root = QueryRoot::new(vec![NodeMetadata {
    id: Some("editor".into()),
    role: Some(Role::TextInput),
    label: Some("Editor".into()),
    rect: Rect::new(Point::new(0.0, 0.0), Size::new(320.0, 160.0)),
}]);

let editor = root.find(&Selector::by_id("editor")).unwrap();
assert_eq!(editor.label.as_deref(), Some("Editor"));
```

Wait for the UI to reach a stable state before asserting:

```rust
use glasscheck_core::{wait_for_condition, PollOptions};
use std::time::Duration;

let mut attempts = 0;
wait_for_condition(
    PollOptions {
        timeout: Duration::from_millis(50),
        interval: Duration::from_millis(1),
    },
    || {
        attempts += 1;
        attempts >= 3
    },
)
.unwrap();
```

Assert rendered text against a reference renderer instead of brittle string-only checks:

```rust
use glasscheck_core::{
    assert_text_renders, CompareConfig, Image, Point, Rect, RgbaColor, Size,
    TextAssertionConfig, TextExpectation, TextRenderer,
};

#[derive(Debug)]
struct StubError;

impl std::fmt::Display for StubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stub error")
    }
}

impl std::error::Error for StubError {}

struct StubRenderer;

impl TextRenderer for StubRenderer {
    type Error = StubError;

    fn render_text_reference(&self, _: &TextExpectation) -> Result<Image, Self::Error> {
        Ok(Image::new(1, 1, vec![255, 255, 255, 255]))
    }

    fn capture_text_region(&self, _: &TextExpectation) -> Result<Image, Self::Error> {
        Ok(Image::new(1, 1, vec![255, 255, 255, 255]))
    }
}

let expectation = TextExpectation::new(
    "Hello",
    Rect::new(Point::new(0.0, 0.0), Size::new(120.0, 40.0)),
)
.with_point_size(14.0)
.with_foreground(RgbaColor::new(0, 0, 0, 255));

assert_text_renders(
    &StubRenderer,
    &expectation,
    std::path::Path::new("target/text-artifacts"),
    &TextAssertionConfig {
        compare: CompareConfig::default(),
        write_diff: true,
    },
)
.unwrap();
```

Compare captured output with configurable tolerance:

```rust
use glasscheck_core::{compare_images, CompareConfig, Image};

let actual = Image::new(1, 1, vec![255, 255, 255, 255]);
let expected = Image::new(1, 1, vec![255, 255, 255, 255]);

let result = compare_images(&actual, &expected, &CompareConfig::default());
assert!(result.passed);
```

Prefer semantic regions over hard-coded pixel rectangles when possible:

```rust
use glasscheck_core::{
    AnchoredTextExpectation, NodePredicate, RegionSpec, RelativeBounds, Role,
    TextMatch,
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

## macOS AppKit

On macOS, `glasscheck` also exposes an in-process AppKit harness for native UI tests:

- `AppKitHarness`
- `AppKitWindowHost`
- `AppKitInputDriver`
- `AppKitTextHarness`
- `AppKitWindowHost::capture_region`
- `AppKitTextHarness::assert_text_renders_anchored`

## Linux GTK Verification

For the GTK backend on Linux, there are two useful X11 verification modes:

- Real X11 display, for validating live desktop behavior such as ensuring test windows do not visibly flash:
  - `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
  - `env GDK_BACKEND=x11 cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`
- Hidden X11 via `xvfb-run`, for CI-style or headless execution:
  - `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_smoke --features native-smoke-tests`
  - `env GDK_BACKEND=x11 xvfb-run -a cargo test -p glasscheck-gtk --test gtk_contracts --features native-contract-tests`

Always force `GDK_BACKEND=x11` for these GTK verification paths. Depending on the environment, plain `xvfb-run` can otherwise select a different backend.
