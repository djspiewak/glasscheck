//! AppKit backend for in-process testing of native macOS UIs.
//!
//! Use this crate when tests can run on the main thread and need direct access
//! to real AppKit windows, semantic snapshots, capture, or input synthesis.
//! Compared with external UI automation, the setup cost is lower and the
//! assertions are more precise, but tests must opt into explicit
//! instrumentation. Prefer `AppKitHarness` as the entry point: it owns the
//! AppKit main-thread capability for window creation and attachment, while
//! post-mount host operations such as `text_renderer()` remain marker-free.
//!
//! `AppKitHarness::menu_bar()` exposes `NSApplication.mainMenu` for semantic
//! assertions, offscreen menu capture, and AppKit menu-item activation without
//! showing a native menu popup on the default capture path.
//! `AppKitSession` also exposes AppKit-only dialog helpers for `NSAlert`,
//! `NSOpenPanel`, and `NSSavePanel` surfaces using semantic scenes plus public
//! AppKit operations. Live file-panel contract tests are opt-in with
//! `GLASSCHECK_RUN_NATIVE_FILE_PANEL_TESTS=1` because presenting system file
//! panels can launch macOS services and visible UI.

mod capture;
mod dialog;
mod harness;
mod input;
mod menu;
mod screen;
mod session;
mod text;
mod window;

pub use capture::capture_view_image;
pub use dialog::{AppKitDialogError, AppKitDialogKind, AppKitDialogQuery};
pub use glasscheck_core::{HitPointSearch, HitPointStrategy, InstrumentedNode};
pub use harness::AppKitHarness;
pub use input::AppKitInputDriver;
pub use menu::{
    AppKitContextMenu, AppKitContextMenuError, AppKitMenuBar, AppKitMenuCapture,
    AppKitMenuCaptureOptions, AppKitMenuError, AppKitMenuTarget, AppKitOpenedMenu,
};
pub use session::AppKitSession;
pub use text::{AppKitAnchoredTextError, AppKitTextError, AppKitTextHarness};
pub use window::{AppKitSceneSource, AppKitSnapshotContext, AppKitWindowHost, InstrumentedView};
