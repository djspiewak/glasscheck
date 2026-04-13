//! AppKit backend for in-process functional testing of native macOS UIs.
//!
//! These APIs integrate the portable `glasscheck-core` assertions with AppKit
//! windows, views, input synthesis, capture, and text rendering.

mod capture;
mod harness;
mod input;
mod screen;
mod text;
mod window;

pub use capture::capture_view_image;
pub use glasscheck_core::InstrumentedNode;
pub use glasscheck_core::InstrumentedNode as InstrumentedView;
pub use harness::AppKitHarness;
pub use input::AppKitInputDriver;
pub use text::{AppKitAnchoredTextError, AppKitTextError, AppKitTextHarness};
pub use window::AppKitWindowHost;
