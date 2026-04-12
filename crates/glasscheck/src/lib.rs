//! Top-level `glasscheck` crate.
//!
//! This crate re-exports the portable `glasscheck-core` APIs and, when the
//! relevant backend feature is enabled, a native platform-backed test harness.

pub use glasscheck_core::*;

#[cfg(feature = "appkit")]
pub use glasscheck_appkit::*;

#[cfg(feature = "gtk")]
pub use glasscheck_gtk::*;

#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type Harness = AppKitHarness;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type WindowHost = AppKitWindowHost;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type NativeInputDriver<'a> = AppKitInputDriver<'a>;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type NativeTextHarness<'a> = AppKitTextHarness<'a>;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type Harness = GtkHarness;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type WindowHost = GtkWindowHost;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type NativeInputDriver<'a> = GtkInputDriver<'a>;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type NativeTextHarness<'a> = GtkTextHarness<'a>;
