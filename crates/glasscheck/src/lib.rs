//! Top-level `glasscheck` crate.
//!
//! This crate re-exports `glasscheck-core` and, when enabled for the current
//! target, a native backend harness. Use it when you want one dependency for
//! both portable assertions and platform-specific window hosting, capture, and
//! input helpers for graphical native UIs rather than browser-based UIs. This
//! is the intended dependency for most users. Supported native backends are
//! AppKit on macOS and GTK4 on Linux. The facade exposes shared session,
//! native-dialog, and context-click entry points where the backends support the
//! same capability; backend-specific types are still re-exported for capabilities
//! such as AppKit process main-menu testing or GTK async dialog metadata.

pub use glasscheck_core::*;

#[cfg(all(feature = "appkit", target_os = "macos"))]
pub use glasscheck_appkit::*;
#[cfg(feature = "gtk")]
pub use glasscheck_gtk::*;

#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type Harness = AppKitHarness;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type WindowHost = AppKitWindowHost;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type Session = AppKitSession;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type NativeInputDriver<'a> = AppKitInputDriver<'a>;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type NativeTextHarness<'a> = AppKitTextHarness<'a>;
#[cfg(all(feature = "appkit", target_os = "macos"))]
pub type NativeSnapshotContext<'a> = AppKitSnapshotContext<'a>;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type Harness = GtkHarness;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type WindowHost = GtkWindowHost;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type Session = GtkSession;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type NativeInputDriver<'a> = GtkInputDriver<'a>;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type NativeTextHarness<'a> = GtkTextHarness<'a>;
#[cfg(all(feature = "gtk", target_os = "linux"))]
pub type NativeSnapshotContext<'a> = GtkSnapshotContext<'a>;

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "appkit", target_os = "macos"))]
    #[test]
    fn appkit_public_api_is_enabled_on_macos() {
        assert!(cfg!(all(feature = "appkit", target_os = "macos")));
    }

    #[cfg(all(feature = "appkit", not(target_os = "macos")))]
    #[test]
    fn appkit_public_api_is_disabled_off_macos() {
        assert!(!cfg!(all(feature = "appkit", target_os = "macos")));
    }
}
