#[cfg(target_os = "linux")]
mod imp {
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use crate::window::GtkWindowHost;
    use crate::GtkSession;
    use glasscheck_core::{Harness, PollError, PollOptions};
    use gtk4::prelude::IsA;
    use gtk4::{Widget, Window};

    static INIT_GTK: OnceLock<Result<(), glib::BoolError>> = OnceLock::new();

    #[derive(Clone, Copy, Debug)]
    /// Main-context GTK harness for creating windows and flushing the event loop.
    ///
    /// Construct this with [`GtkHarness::new`] so GTK initialization remains
    /// checked. It keeps waits aligned with the GTK main context rather than
    /// arbitrary sleeps.
    ///
    /// ```compile_fail
    /// # use glasscheck_gtk::GtkHarness;
    /// let _ = GtkHarness::default();
    /// ```
    pub struct GtkHarness;

    impl GtkHarness {
        /// Initializes GTK4 for in-process tests.
        ///
        /// This is the only public constructor so initialization failures
        /// cannot be bypassed.
        pub fn new() -> Result<Self, glib::BoolError> {
            INIT_GTK.get_or_init(gtk4::init).clone().map(|()| Self)
        }

        /// Pumps the GTK main context once.
        pub fn flush(&self) {
            let context = glib::MainContext::default();
            while context.pending() {
                context.iteration(false);
            }
            context.iteration(false);
        }

        /// Flushes the main context for at least `frames` iterations.
        pub fn settle(&self, frames: usize) {
            for _ in 0..frames.max(1) {
                self.flush();
            }
        }

        /// Polls `predicate`, flushing the GTK main context between attempts.
        pub fn wait_until<F>(
            &self,
            options: PollOptions,
            mut predicate: F,
        ) -> Result<usize, PollError>
        where
            F: FnMut() -> bool,
        {
            let started = Instant::now();
            let mut attempts = 0usize;
            loop {
                attempts += 1;
                self.flush();
                if predicate() {
                    return Ok(attempts);
                }
                if started.elapsed() >= options.timeout {
                    return Err(PollError::Timeout {
                        elapsed: started.elapsed(),
                        attempts,
                    });
                }
                std::thread::sleep(options.interval);
            }
        }

        /// Creates a new GTK4 test window.
        #[must_use]
        pub fn create_window(&self, width: f64, height: f64) -> GtkWindowHost {
            GtkWindowHost::new(width, height)
        }

        /// Attaches a host to an existing GTK root widget and optional parent window.
        #[must_use]
        pub fn attach_root(
            &self,
            widget: &impl IsA<Widget>,
            window: Option<&Window>,
        ) -> GtkWindowHost {
            GtkWindowHost::from_root(widget, window)
        }

        /// Creates a session for coordinating multiple attached surfaces.
        #[must_use]
        pub fn session(&self) -> GtkSession {
            GtkSession::new(*self)
        }

        /// Runs the GTK main context for the given duration.
        pub fn wait_for_duration(&self, duration: Duration) {
            let deadline = Instant::now() + duration;
            while Instant::now() < deadline {
                self.flush();
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    impl Harness for GtkHarness {
        type WindowHost = GtkWindowHost;

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

#[cfg(not(target_os = "linux"))]
mod imp {
    #[derive(Clone, Copy, Debug)]
    pub struct GtkHarness;
}

pub use imp::GtkHarness;
