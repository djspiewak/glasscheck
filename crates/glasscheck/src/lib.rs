//! Top-level `glasscheck` crate.
//!
//! This crate re-exports the portable `glasscheck-core` APIs and, when the
//! relevant backend feature is enabled, a native platform-backed test harness.

pub use glasscheck_core::*;

#[cfg(all(feature = "appkit", target_os = "macos"))]
pub use glasscheck_appkit::*;
#[cfg(feature = "gtk")]
pub use glasscheck_gtk::*;

#[cfg(test)]
const fn appkit_public_api_enabled() -> bool {
    cfg!(all(feature = "appkit", target_os = "macos"))
}

#[cfg(test)]
mod tests {
    use super::appkit_public_api_enabled;

    #[cfg(all(feature = "appkit", target_os = "macos"))]
    #[test]
    fn appkit_public_api_is_enabled_on_macos() {
        assert!(appkit_public_api_enabled());
    }

    #[cfg(all(feature = "appkit", not(target_os = "macos")))]
    #[test]
    fn appkit_public_api_is_disabled_off_macos() {
        assert!(!appkit_public_api_enabled());
    }
}
