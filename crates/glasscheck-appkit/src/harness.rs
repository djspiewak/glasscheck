#[cfg(target_os = "macos")]
mod imp {
    use std::sync::Once;
    use std::time::Duration;

    use glasscheck_core::{Harness, PollError, PollOptions};
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_app_kit::{NSView, NSWindow};
    use objc2_foundation::{MainThreadMarker, NSDate, NSRunLoop};

    use crate::window::AppKitWindowHost;

    static INIT_APP: Once = Once::new();

    #[derive(Clone, Copy)]
    /// Main-thread AppKit harness for creating windows, attaching hosts, and flushing the run loop.
    ///
    /// Use this as the entry point for AppKit tests. It owns the main-thread
    /// capability used by AppKit construction and attachment APIs and keeps
    /// polling aligned with real run-loop progress. Prefer the harness-owned
    /// attachment helpers over calling `AppKitWindowHost::from_*` directly when
    /// you already have a harness in scope.
    pub struct AppKitHarness {
        mtm: MainThreadMarker,
    }

    impl AppKitHarness {
        /// Initializes the shared `NSApplication` and returns a harness handle.
        pub fn new(mtm: MainThreadMarker) -> Self {
            INIT_APP.call_once(|| {
                let app = NSApplication::sharedApplication(mtm);
                app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
                app.activate();
            });

            Self { mtm }
        }

        /// Returns the main-thread marker associated with this harness.
        ///
        /// Most downstream code should not need this after native fixture
        /// construction. It remains available for backend-specific view/control
        /// creation that cannot be expressed through the shared `glasscheck`
        /// facade.
        #[must_use]
        pub fn main_thread_marker(&self) -> MainThreadMarker {
            self.mtm
        }

        /// Pumps the AppKit run loop once.
        pub fn flush(&self) {
            let date = NSDate::dateWithTimeIntervalSinceNow(0.02);
            NSRunLoop::currentRunLoop().runUntilDate(&date);
        }

        /// Flushes the run loop for at least `frames` iterations.
        pub fn settle(&self, frames: usize) {
            for _ in 0..frames.max(1) {
                self.flush();
            }
        }

        /// Polls `predicate`, flushing the run loop between attempts.
        pub fn wait_until<F>(
            &self,
            options: PollOptions,
            mut predicate: F,
        ) -> Result<usize, PollError>
        where
            F: FnMut() -> bool,
        {
            Harness::wait_until(self, options, || predicate())
        }

        /// Creates a new test window with the requested content size.
        ///
        /// This is the usual starting point when tests need capture, semantic
        /// snapshots, or synthesized input against a real window.
        #[must_use]
        pub fn create_window(&self, width: f64, height: f64) -> AppKitWindowHost {
            AppKitWindowHost::new(self.mtm, width, height)
        }

        /// Attaches a host to an existing `NSWindow`.
        ///
        /// Prefer this over `AppKitWindowHost::from_window` in normal test code
        /// so the harness remains the carrier for the main-thread capability.
        #[must_use]
        pub fn attach_window(&self, window: &NSWindow) -> AppKitWindowHost {
            AppKitWindowHost::from_window(window, self.mtm)
        }

        /// Attaches a host to an existing root view and optional parent window.
        ///
        /// Prefer this over `AppKitWindowHost::from_root_view` in normal test
        /// code so the harness remains the carrier for the main-thread
        /// capability.
        #[must_use]
        pub fn attach_root_view(
            &self,
            view: &NSView,
            window: Option<&NSWindow>,
        ) -> AppKitWindowHost {
            AppKitWindowHost::from_root_view(view, window, self.mtm)
        }

        /// Runs the AppKit run loop for the given duration.
        pub fn wait_for_duration(&self, duration: Duration) {
            let date = NSDate::dateWithTimeIntervalSinceNow(duration.as_secs_f64());
            NSRunLoop::currentRunLoop().runUntilDate(&date);
        }
    }

    impl Harness for AppKitHarness {
        type WindowHost = AppKitWindowHost;

        fn flush(&self) {
            Self::flush(self);
        }

        fn create_window(&self, width: f64, height: f64) -> Self::WindowHost {
            Self::create_window(self, width, height)
        }

        fn wait_for_duration(&self, duration: Duration) {
            Self::wait_for_duration(self, duration);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    #[derive(Clone, Copy)]
    pub struct AppKitHarness;
}

pub use imp::AppKitHarness;
