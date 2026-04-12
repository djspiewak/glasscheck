#[cfg(target_os = "macos")]
mod imp {
    use std::sync::Once;
    use std::time::Duration;

    use glasscheck_core::{Harness, PollError, PollOptions};
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::{MainThreadMarker, NSDate, NSRunLoop};

    use crate::window::AppKitWindowHost;

    static INIT_APP: Once = Once::new();

    #[derive(Clone, Copy)]
    /// Main-thread AppKit harness for creating windows and flushing the run loop.
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
        #[must_use]
        pub fn create_window(&self, width: f64, height: f64) -> AppKitWindowHost {
            AppKitWindowHost::new(self.mtm, width, height)
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
