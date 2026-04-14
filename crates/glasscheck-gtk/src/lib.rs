//! GTK4 backend for in-process testing of native Linux UIs.
//!
//! Use this crate when tests can run inside the GTK main context and need scene
//! snapshots, capture, or direct widget-level interaction. On X11-backed
//! windows, pointer input uses native X11 dispatch while keyboard and text
//! input still follow GTK controller and text APIs. Other GTK backends report
//! input unavailability rather than silently degrading.

mod harness;
mod input;
mod screen;
mod text;
mod window;

pub use glasscheck_core::InstrumentedNode as InstrumentedWidget;
pub use glasscheck_core::{HitPointSearch, HitPointStrategy, InstrumentedNode};
pub use harness::GtkHarness;
pub use input::GtkInputDriver;
pub use text::{GtkAnchoredTextError, GtkTextError, GtkTextHarness};
pub use window::GtkWindowHost;
