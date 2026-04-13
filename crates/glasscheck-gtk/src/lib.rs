//! GTK4 backend for in-process testing of native Linux UIs.
//!
//! Use this crate when tests can run inside the GTK main context and need scene
//! snapshots, capture, or direct widget-level interaction. Some low-level input
//! paths are still best-effort, so semantic queries and text/image assertions
//! are usually the more stable choice.

mod harness;
mod input;
mod screen;
mod text;
mod window;

pub use glasscheck_core::InstrumentedNode;
pub use glasscheck_core::InstrumentedNode as InstrumentedWidget;
pub use harness::GtkHarness;
pub use input::GtkInputDriver;
pub use text::{GtkAnchoredTextError, GtkTextError, GtkTextHarness};
pub use window::GtkWindowHost;
