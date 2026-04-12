//! GTK backend stub for in-process functional testing of native Linux UIs.
//!
//! This crate reserves the Linux backend API surface without providing a GTK
//! implementation yet.

mod harness;
mod input;
mod text;
mod window;

pub use harness::GtkHarness;
pub use input::GtkInputDriver;
pub use text::{GtkAnchoredTextError, GtkTextError, GtkTextHarness};
pub use window::{GtkWindowHost, InstrumentedWidget};
