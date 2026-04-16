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

        pub fn attach_host(&self, id: impl Into<SurfaceId>, host: GtkWindowHost) {
            self.surfaces.borrow_mut().insert(id.into(), host);
        }

        pub fn attach_window(&self, id: impl Into<SurfaceId>, window: &Window) {
            self.attach_host(id, GtkWindowHost::from_window(window));
        }

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
            let Some(window) = Window::list_toplevels()
                .into_iter()
                .filter_map(|widget| widget.downcast::<Window>().ok())
                .find(|window| {
                    window
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
        pub fn open_transient_with_click(
            &self,
            id: impl Into<SurfaceId>,
            spec: &TransientSurfaceSpec,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            let Some(baseline) = ({
                let surfaces = self.surfaces.borrow();
                surfaces.get(&spec.owner).map(transient_candidate_ids)
            }) else {
                return self.harness.wait_until(options, || false);
            };
            let click_succeeded = {
                let surfaces = self.surfaces.borrow();
                surfaces
                    .get(&spec.owner)
                    .is_some_and(|host| host.click_node(&spec.opener).is_ok())
            };
            if !click_succeeded {
                return self.harness.wait_until(options, || false);
            }
            let Some(owner) = self.with_surface(&spec.owner, |host| {
                GtkWindowHost::from_window(host.window())
            }) else {
                return self.harness.wait_until(options, || false);
            };
            self.harness.wait_until(options, || {
                let Some(candidate) = discover_owned_transient_candidate(&owner, &baseline) else {
                    return false;
                };
                match candidate {
                    GtkTransientCandidate::Window(window) => {
                        self.attach_window(id.clone(), &window)
                    }
                    GtkTransientCandidate::Popover(popover) => {
                        self.attach_root(id.clone(), &popover, Some(owner.window()))
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
                self.remove_surface(id);
            }
            is_open
        }

        /// Waits for the named transient surface to dismiss and evicts it from the session.
        pub fn wait_for_surface_closed(
            &self,
            id: &SurfaceId,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            self.harness
                .wait_until(options, || !self.surface_is_open(id))
        }

        pub fn with_surface<R>(
            &self,
            id: &SurfaceId,
            f: impl FnOnce(&GtkWindowHost) -> R,
        ) -> Option<R> {
            self.surfaces.borrow().get(id).map(f)
        }

        #[must_use]
        pub fn snapshot_scene(&self, id: &SurfaceId) -> Option<Scene> {
            self.with_surface(id, GtkWindowHost::snapshot_scene)
        }

        pub fn click_node(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
        ) -> Option<Result<(), glasscheck_core::RegionResolveError>> {
            self.with_surface(id, |host| host.click_node(predicate))
        }

        pub fn hover_node(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
            search: &HitPointSearch,
        ) -> Option<Result<(), glasscheck_core::RegionResolveError>> {
            self.with_surface(id, |host| host.hover_node(predicate, search))
        }

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
            return root.is_visible() || host.window().is_visible();
        }
        host.window().is_visible()
    }

    fn transient_candidate_ids(owner: &GtkWindowHost) -> BTreeSet<usize> {
        owned_transient_candidates(owner)
            .into_iter()
            .map(|candidate| transient_candidate_id(&candidate))
            .collect()
    }

    fn discover_owned_transient_candidate(
        owner: &GtkWindowHost,
        baseline: &BTreeSet<usize>,
    ) -> Option<GtkTransientCandidate> {
        owned_transient_candidates(owner)
            .into_iter()
            .find(|candidate| !baseline.contains(&transient_candidate_id(candidate)))
    }

    fn owned_transient_candidates(owner: &GtkWindowHost) -> Vec<GtkTransientCandidate> {
        let owner_window = owner.window();
        let owner_window_ptr = owner_window.as_ptr() as usize;
        let mut candidates = Window::list_toplevels()
            .into_iter()
            .filter_map(|widget| widget.downcast::<Window>().ok())
            .filter(|window| {
                window.as_ptr() as usize != owner_window_ptr
                    && window.is_visible()
                    && window
                        .transient_for()
                        .as_ref()
                        .is_some_and(|parent| parent.as_ptr() as usize == owner_window_ptr)
            })
            .map(GtkTransientCandidate::Window)
            .collect::<Vec<_>>();
        if let Some(root) = owner.root_widget() {
            collect_visible_popovers(&root, &mut candidates);
        }
        candidates
    }

    fn collect_visible_popovers(root: &Widget, candidates: &mut Vec<GtkTransientCandidate>) {
        if let Ok(popover) = root.clone().downcast::<Popover>() {
            if popover.is_visible() {
                candidates.push(GtkTransientCandidate::Popover(popover));
            }
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
