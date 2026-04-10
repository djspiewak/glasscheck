//! Top-level `glasscheck` crate.
//!
//! This crate re-exports the portable `glasscheck-core` APIs and, when the
//! `appkit` feature is enabled on macOS, the AppKit-backed test harness.

pub use glasscheck_core::*;

#[cfg(feature = "appkit")]
pub use glasscheck_appkit::*;
