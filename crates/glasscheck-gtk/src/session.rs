#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};

    use glasscheck_core::{
        PollError, PollOptions, Scene, Selector, SurfaceId, SurfaceQuery, TransientSurfaceSpec,
    };
    use glib::object::Cast;
    use gtk4::prelude::*;
    use gtk4::{Popover, Widget, Window};

    use crate::{GtkHarness, GtkWindowHost, HitPointSearch};

    /// Coordinator for multi-surface GTK test flows.
    pub struct GtkSession {
        harness: GtkHarness,
        surfaces: RefCell<BTreeMap<SurfaceId, GtkWindowHost>>,
    }

    impl GtkSession {
        #[must_use]
        pub fn new(harness: GtkHarness) -> Self {
            Self {
                harness,
                surfaces: RefCell::new(BTreeMap::new()),
            }
        }

        /// Registers a pre-built [`GtkWindowHost`] under the given surface ID.
        pub fn attach_host(&self, id: impl Into<SurfaceId>, host: GtkWindowHost) {
            let id = id.into();
            debug_assert!(
                !self.surfaces.borrow().contains_key(&id),
                "surface id '{}' is already registered; use a distinct id or remove the existing surface first",
                id.as_str()
            );
            self.surfaces.borrow_mut().insert(id, host);
        }

        /// Wraps a raw [`Window`] into a host and registers it under the given surface ID.
        pub fn attach_window(&self, id: impl Into<SurfaceId>, window: &Window) {
            self.attach_host(id, GtkWindowHost::from_window(window));
        }

        /// Wraps an arbitrary root widget (and optional parent window) into a host and registers it.
        pub fn attach_root(
            &self,
            id: impl Into<SurfaceId>,
            widget: &impl IsA<Widget>,
            window: Option<&Window>,
        ) {
            self.attach_host(id, self.harness.attach_root(widget, window));
        }

        #[must_use]
        pub fn discover_window(&self, id: impl Into<SurfaceId>, query: &SurfaceQuery) -> bool {
            let registered_ptrs: std::collections::BTreeSet<usize> = {
                let surfaces = self.surfaces.borrow();
                surfaces
                    .values()
                    .map(|host| host.window().as_ptr() as usize)
                    .collect()
            };
            let Some(window) = Window::list_toplevels()
                .into_iter()
                .filter_map(|widget| widget.downcast::<Window>().ok())
                .find(|window| {
                    !registered_ptrs.contains(&(window.as_ptr() as usize))
                        && window
                            .title()
                            .as_ref()
                            .is_some_and(|title| query.matches_title(title.as_str()))
                })
            else {
                return false;
            };
            self.attach_window(id, &window);
            true
        }

        /// Polls until a window matching `query` is found and attached; returns the poll attempt count.
        pub fn wait_for_discovered_window(
            &self,
            id: impl Into<SurfaceId>,
            query: &SurfaceQuery,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            self.harness
                .wait_until(options, || self.discover_window(id.clone(), query))
        }

        /// Clicks an opener on the owner surface and attaches the newly opened transient.
        ///
        /// Returns `PollError::Timeout` if the transient never appears within
        /// `options`. Returns `PollError::Precondition` for precondition failures: owner surface not
        /// registered or opener click failed.
        ///
        /// If a transient from a previous test step is still visible when this method
        /// is called, it will appear in the baseline and will not be detected as
        /// "newly opened", producing a timeout. Callers must ensure prior transients
        /// are fully dismissed (e.g., via `wait_for_surface_closed`) before calling
        /// this method again with the same opener.
        pub fn open_transient_with_click(
            &self,
            id: impl Into<SurfaceId>,
            spec: &TransientSurfaceSpec,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            debug_assert!(id != spec.owner, "transient id must not equal the owner surface id");
            let Some((baseline, owner_window, owner_root)) = ({
                let surfaces = self.surfaces.borrow();
                surfaces.get(&spec.owner).map(|host| {
                    let baseline = transient_candidate_ids(host.window(), host.root_widget().as_ref());
                    let owner_window = host.window().clone();
                    let owner_root = host.root_widget().clone();
                    (baseline, owner_window, owner_root)
                })
            }) else {
                return Err(PollError::Precondition("owner surface not registered"));
            };
            let click_succeeded = {
                let surfaces = self.surfaces.borrow();
                surfaces
                    .get(&spec.owner)
                    .is_some_and(|host| host.click_node(&spec.opener).is_ok())
            };
            if !click_succeeded {
                return Err(PollError::Precondition("opener click failed"));
            }
            debug_assert!(
                {
                    let surfaces = self.surfaces.borrow();
                    surfaces
                        .get(&spec.owner)
                        .is_some_and(|h| h.window().as_ptr() == owner_window.as_ptr())
                },
                "owner surface was evicted or replaced between baseline capture and transient discovery"
            );
            self.harness.wait_until(options, || {
                if !self.surfaces.borrow().contains_key(&spec.owner) {
                    return false;
                }
                let Some(candidate) = discover_owned_transient_candidate(&owner_window, owner_root.as_ref(), &baseline) else {
                    return false;
                };
                match candidate {
                    GtkTransientCandidate::Window(window) => {
                        self.attach_window(id.clone(), &window)
                    }
                    GtkTransientCandidate::Popover(popover) => {
                        self.attach_root(id.clone(), &popover, Some(&owner_window))
                    }
                }
                true
            })
        }

        #[must_use]
        /// Returns whether the named surface is still available.
        pub fn surface_is_open(&self, id: &SurfaceId) -> bool {
            let is_open = {
                let surfaces = self.surfaces.borrow();
                surfaces.get(id).is_some_and(gtk_host_is_open)
            };
            if !is_open {
                let _ = self.remove_surface(id);
            }
            is_open
        }

        /// Waits for the named transient surface to dismiss and evicts it from the session.
        ///
        /// A transient is considered closed when `gtk_host_is_open` returns false (i.e.,
        /// the GTK window is no longer visible or has been destroyed). This uses the same
        /// liveness check as `surface_is_open`, unlike the AppKit backend which uses a
        /// transient-specific invisible-and-unparented condition.
        ///
        /// See the AppKit counterpart for comparison: AppKit uses an invisible-AND-unparented
        /// condition while GTK uses visibility-only detection via `gtk_host_is_open`.
        pub fn wait_for_surface_closed(
            &self,
            id: &SurfaceId,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            if !{ self.surfaces.borrow().contains_key(id) } {
                return Ok(0);
            }
            self.harness
                .wait_until(options, || !self.surface_is_open(id))
        }

        /// Calls `f` with a reference to the host for the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed.
        ///
        /// # Panics
        ///
        /// Panics if `f` re-enters the session via any method that accesses the
        /// internal surface map, including `attach_host`, `attach_window`,
        /// `attach_root`, `remove_surface`, `surface_is_open`,
        /// `snapshot_scene`, `click_node`, `hover_node`, `wait_for_surface_closed`,
        /// `discover_window`, `wait_for_discovered_window`,
        /// `open_transient_with_click`, or a nested `with_surface` call.
        /// Only `wait_until` (on the underlying harness) is safe to call from `f`.
        pub fn with_surface<R>(
            &self,
            id: &SurfaceId,
            f: impl FnOnce(&GtkWindowHost) -> R,
        ) -> Option<R> {
            let is_open = {
                let surfaces = self.surfaces.borrow();
                surfaces.get(id).is_some_and(gtk_host_is_open)
            };
            if !is_open {
                let _ = self.remove_surface(id);
                return None;
            }
            self.surfaces.borrow().get(id).map(f)
        }

        /// Snapshots the accessibility scene for the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed (and evicts it as a side effect).
        #[must_use]
        pub fn snapshot_scene(&self, id: &SurfaceId) -> Option<Scene> {
            self.with_surface(id, GtkWindowHost::snapshot_scene)
        }

        /// Synthesizes a click on the node matching `predicate` in the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed (and evicts it as a side effect); `Some(Err(...))` if the node can't be located.
        pub fn click_node(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
        ) -> Option<Result<(), glasscheck_core::RegionResolveError>> {
            self.with_surface(id, |host| host.click_node(predicate))
        }

        /// Synthesizes a mouse-hover over the node matching `predicate` in the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed (and evicts it as a side effect); `Some(Err(...))` if the node can't be located.
        pub fn hover_node(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Option<Result<(), glasscheck_core::RegionResolveError>> {
            self.with_surface(id, |host| host.hover_node(predicate, search))
        }

        /// Delegates to the harness's poll loop; succeeds when `predicate` returns true.
        pub fn wait_until<F>(
            &self,
            options: PollOptions,
            mut predicate: F,
        ) -> Result<usize, PollError>
        where
            F: FnMut(&Self) -> bool,
        {
            self.harness.wait_until(options, || predicate(self))
        }

        #[must_use]
        pub(crate) fn remove_surface(&self, id: &SurfaceId) -> Option<GtkWindowHost> {
            self.surfaces.borrow_mut().remove(id)
        }
    }

    enum GtkTransientCandidate {
        Window(Window),
        Popover(Popover),
    }

    fn gtk_host_is_open(host: &GtkWindowHost) -> bool {
        if let Some(root) = host.root_widget() {
            if let Ok(popover) = root.clone().downcast::<Popover>() {
                return popover.is_visible();
            }
            return root.is_visible();
        }
        host.window().is_visible()
    }

    fn transient_candidate_ids(owner_window: &Window, owner_root: Option<&Widget>) -> BTreeSet<usize> {
        owned_transient_candidates(owner_window, owner_root)
            .into_iter()
            .map(|candidate| transient_candidate_id(&candidate))
            .collect()
    }

    fn discover_owned_transient_candidate(
        owner_window: &Window,
        owner_root: Option<&Widget>,
        baseline: &BTreeSet<usize>,
    ) -> Option<GtkTransientCandidate> {
        owned_transient_candidates(owner_window, owner_root)
            .into_iter()
            .find(|candidate| !baseline.contains(&transient_candidate_id(candidate)))
    }

    fn owned_transient_candidates(
        owner_window: &Window,
        owner_root: Option<&Widget>,
    ) -> Vec<GtkTransientCandidate> {
        let owner_window_ptr = owner_window.as_ptr() as usize;

        // Collect visible popovers first so their backing windows can be excluded below.
        let mut candidates: Vec<GtkTransientCandidate> = Vec::new();
        if let Some(root) = owner_root {
            collect_visible_popovers(root, &mut candidates);
        }

        // Build the set of Window ptrs that are native backing surfaces for known popovers.
        // These will be excluded from the Window toplevel scan to avoid double-attaching.
        let popover_backed_window_ptrs: std::collections::BTreeSet<usize> = candidates
            .iter()
            .filter_map(|c| {
                if let GtkTransientCandidate::Popover(popover) = c {
                    popover
                        .native()
                        .and_then(|native| native.dynamic_cast::<Window>().ok())
                        .map(|w| w.as_ptr() as usize)
                } else {
                    None
                }
            })
            .collect();

        for widget in Window::list_toplevels() {
            let Ok(window) = widget.downcast::<Window>() else {
                continue;
            };
            let ptr = window.as_ptr() as usize;
            if ptr == owner_window_ptr {
                continue;
            }
            if !window.is_visible() {
                continue;
            }
            if !window
                .transient_for()
                .as_ref()
                .is_some_and(|parent| parent.as_ptr() as usize == owner_window_ptr)
            {
                continue;
            }
            if popover_backed_window_ptrs.contains(&ptr) {
                continue;
            }
            candidates.push(GtkTransientCandidate::Window(window));
        }

        // Dedup by ptr as a final safety net for any remaining duplicate representations.
        let mut seen = std::collections::BTreeSet::new();
        candidates.retain(|c| seen.insert(transient_candidate_id(c)));
        candidates
    }

    fn collect_visible_popovers(root: &Widget, candidates: &mut Vec<GtkTransientCandidate>) {
        if let Ok(popover) = root.clone().downcast::<Popover>() {
            if popover.is_visible() {
                candidates.push(GtkTransientCandidate::Popover(popover));
            }
            return;
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            collect_visible_popovers(&widget, candidates);
            child = widget.next_sibling();
        }
    }

    fn transient_candidate_id(candidate: &GtkTransientCandidate) -> usize {
        match candidate {
            GtkTransientCandidate::Window(window) => window.as_ptr() as usize,
            GtkTransientCandidate::Popover(popover) => popover.as_ptr() as usize,
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub struct GtkSession;
}

pub use imp::GtkSession;
