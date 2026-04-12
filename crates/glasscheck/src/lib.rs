//! Top-level `glasscheck` crate.
//!
//! This crate re-exports the portable `glasscheck-core` APIs and, when the
//! relevant backend feature is enabled, a native platform-backed test harness.

pub use glasscheck_core::*;

#[cfg(feature = "appkit")]
pub use glasscheck_appkit::*;

#[cfg(feature = "gtk")]
pub use glasscheck_gtk::*;
