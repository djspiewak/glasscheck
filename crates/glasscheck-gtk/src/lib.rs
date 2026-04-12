//! GTK4 backend for in-process functional testing of native Linux UIs.

mod harness;
mod input;
mod text;
mod window;

pub use glasscheck_core::InstrumentedNode;
pub use glasscheck_core::InstrumentedNode as InstrumentedWidget;
pub use harness::GtkHarness;
pub use input::GtkInputDriver;
pub use text::{GtkAnchoredTextError, GtkTextError, GtkTextHarness};
pub use window::GtkWindowHost;
