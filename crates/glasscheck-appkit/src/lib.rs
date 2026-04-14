//! AppKit backend for in-process testing of native macOS UIs.
//!
//! Use this crate when tests can run on the main thread and need direct access
//! to real AppKit windows, semantic snapshots, capture, or input synthesis.
//! Compared with external UI automation, the setup cost is lower and the
//! assertions are more precise, but tests must opt into explicit
//! instrumentation. Prefer `AppKitHarness` as the entry point: it owns the
//! AppKit main-thread capability for window creation and attachment, while
//! post-mount host operations such as `text_renderer()` remain marker-free.

mod capture;
mod harness;
mod input;
mod screen;
mod text;
mod window;

pub use capture::capture_view_image;
pub use glasscheck_core::{HitPointSearch, HitPointStrategy, InstrumentedNode};
pub use harness::AppKitHarness;
pub use input::AppKitInputDriver;
pub use text::{AppKitAnchoredTextError, AppKitTextError, AppKitTextHarness};
pub use window::{AppKitWindowHost, InstrumentedView};
