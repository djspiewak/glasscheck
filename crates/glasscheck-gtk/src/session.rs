#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use glasscheck_core::{
        DialogCapability, DialogError, DialogKind, DialogQuery, PollError, PollOptions, Scene,
        Selector, SurfaceId, SurfaceQuery, TransientSurfaceSpec,
    };
    use glib::object::Cast;
    use gtk4::prelude::*;
    use gtk4::{Popover, Widget, Window};

    use crate::{dialog, GtkDialogController, GtkHarness, GtkWindowHost, HitPointSearch};

    /// Coordinator for multi-surface GTK test flows.
    pub struct GtkSession {
        harness: GtkHarness,
        surfaces: RefCell<BTreeMap<SurfaceId, GtkWindowHost>>,
        dialog_controllers: RefCell<BTreeMap<SurfaceId, GtkDialogController>>,
    }

    impl GtkSession {
        #[must_use]
        pub fn new(harness: GtkHarness) -> Self {
            Self {
                harness,
                surfaces: RefCell::new(BTreeMap::new()),
                dialog_controllers: RefCell::new(BTreeMap::new()),
            }
        }

        /// Registers a pre-built [`GtkWindowHost`] under the given surface ID.
        ///
        /// # Panics
        ///
        /// Panics if `id` converts from an empty string into a [`SurfaceId`].
        ///
        /// Panics if `id` is already registered in this session. Each attached
        /// surface must use a distinct [`SurfaceId`].
        pub fn attach_host(&self, id: impl Into<SurfaceId>, host: GtkWindowHost) {
            let id = id.into();
            assert!(
                !self.surfaces.borrow().contains_key(&id),
                "surface id '{}' is already registered; use a distinct id or remove the existing surface first",
                id.as_str()
            );
            self.dialog_controllers.borrow_mut().remove(&id);
            self.surfaces.borrow_mut().insert(id, host);
        }

        /// Wraps a raw [`Window`] into a host and registers it under the given surface ID.
        ///
        /// # Panics
        ///
        /// Panics if `id` converts from an empty string into a [`SurfaceId`].
        ///
        /// Panics if `id` is already registered in this session. Each attached
        /// surface must use a distinct [`SurfaceId`].
        pub fn attach_window(&self, id: impl Into<SurfaceId>, window: &Window) {
            self.attach_host(id, GtkWindowHost::from_window(window));
        }

        /// Registers async dialog metadata under the given surface ID.
        ///
        /// Controller-only dialogs support kind/title tracking but not live UI
        /// operations such as scene snapshots, button clicks, text edits, path
        /// selection, or cancellation.
        pub fn attach_dialog_controller(
            &self,
            id: impl Into<SurfaceId>,
            controller: GtkDialogController,
        ) {
            let id = id.into();
            debug_assert!(
                !self.surfaces.borrow().contains_key(&id)
                    && !self.dialog_controllers.borrow().contains_key(&id),
                "surface id '{}' is already registered; use a distinct id or remove the existing surface first",
                id.as_str()
            );
            self.dialog_controllers.borrow_mut().insert(id, controller);
        }

        /// Wraps an arbitrary root widget (and optional parent window) into a host and registers it.
        ///
        /// # Panics
        ///
        /// Panics if `id` converts from an empty string into a [`SurfaceId`].
        ///
        /// Panics if `id` is already registered in this session. Each attached
        /// surface must use a distinct [`SurfaceId`].
        pub fn attach_root(
            &self,
            id: impl Into<SurfaceId>,
            widget: &impl IsA<Widget>,
            window: Option<&Window>,
        ) {
            self.attach_host(id, self.harness.attach_root(widget, window));
        }

        /// Discovers a matching unregistered toplevel window and attaches it.
        ///
        /// # Panics
        ///
        /// Panics if a matching window is found and `id` converts from an empty
        /// string into a [`SurfaceId`].
        ///
        /// Panics if a matching window is found and `id` is already registered
        /// in this session. Each attached surface must use a distinct
        /// [`SurfaceId`].
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
        ///
        /// # Panics
        ///
        /// Panics if `id` converts from an empty string into a [`SurfaceId`].
        ///
        /// Panics if a matching window is found and `id` is already registered
        /// in this session. Each attached surface must use a distinct
        /// [`SurfaceId`].
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
        ///
        /// # Panics
        ///
        /// Panics if `id` converts from an empty string into a [`SurfaceId`].
        ///
        /// Panics if `id` is equal to `spec.owner`. Transient surfaces must be
        /// attached under a different [`SurfaceId`] from their owner surface.
        ///
        /// Panics if the transient is discovered and `id` is already registered
        /// in this session. Each attached surface must use a distinct
        /// [`SurfaceId`].
        pub fn open_transient_with_click(
            &self,
            id: impl Into<SurfaceId>,
            spec: &TransientSurfaceSpec,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            assert!(
                id != spec.owner,
                "transient id must not equal the owner surface id"
            );
            let Some((baseline, owner_window, owner_root)) = ({
                let surfaces = self.surfaces.borrow();
                surfaces.get(&spec.owner).map(|host| {
                    let baseline =
                        transient_candidate_ids(host.window(), host.root_widget().as_ref());
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
                let Some(candidate) = discover_owned_transient_candidate(
                    &owner_window,
                    owner_root.as_ref(),
                    &baseline,
                ) else {
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
                    || self.dialog_controllers.borrow().contains_key(id)
            };
            if !is_open {
                let _ = self.remove_surface(id);
            }
            is_open
        }

        /// Waits for the named transient surface to dismiss and evicts it from the session.
        ///
        /// Controller-only dialog metadata has no GTK host to poll, so it is
        /// treated as already closed and evicted immediately.
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
                self.dialog_controllers.borrow_mut().remove(id);
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

        /// Polls until a surface-backed GTK dialog matching `query` is found and attached.
        ///
        /// This discovers visible `MessageDialog`, `FileChooserDialog`, and
        /// generic `Dialog` windows. Use `attach_dialog_controller` for async
        /// dialog objects that expose metadata but no widget surface.
        pub fn wait_for_dialog(
            &self,
            id: impl Into<SurfaceId>,
            query: &DialogQuery,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            self.harness.wait_until(options, || {
                let registered_ptrs: BTreeSet<usize> = {
                    let surfaces = self.surfaces.borrow();
                    surfaces
                        .values()
                        .map(|host| host.window().as_ptr() as usize)
                        .collect()
                };
                let Some((window, kind)) = Window::list_toplevels()
                    .into_iter()
                    .filter_map(|widget| widget.downcast::<Window>().ok())
                    .filter(|window| window.is_visible())
                    .filter(|window| !registered_ptrs.contains(&(window.as_ptr() as usize)))
                    .filter_map(|window| {
                        let kind = dialog::classify_window(&window)?;
                        let title = dialog::dialog_title(&window, kind);
                        query.matches_dialog(kind, &title).then_some((window, kind))
                    })
                    .next()
                else {
                    return false;
                };
                match dialog::attach_dialog_window(&window) {
                    Ok(host) => {
                        self.attach_host(id.clone(), host);
                        true
                    }
                    Err(_) => {
                        let _ = kind;
                        false
                    }
                }
            })
        }

        /// Returns the native dialog kind for the named surface or controller.
        pub fn dialog_kind(&self, id: &SurfaceId) -> Result<DialogKind, DialogError> {
            if let Some(kind) = self
                .dialog_controllers
                .borrow()
                .get(id)
                .map(GtkDialogController::kind)
            {
                return Ok(kind);
            }
            if let Some(kind) = self.with_surface(id, |host| {
                dialog::classify_window(host.window()).ok_or(DialogError::NotDialog)
            }) {
                return kind;
            }
            Err(DialogError::MissingSurface)
        }

        /// Snapshots the named surface-backed GTK dialog.
        pub fn snapshot_dialog_scene(&self, id: &SurfaceId) -> Result<Scene, DialogError> {
            if self.dialog_controllers.borrow().contains_key(id) {
                return Err(DialogError::UnsupportedCapability(
                    DialogCapability::SceneSnapshot,
                ));
            }
            self.with_surface(id, dialog::snapshot_dialog_scene)
                .ok_or(DialogError::MissingSurface)?
        }

        /// Synthesizes a click on a dialog button matching `predicate`.
        pub fn click_dialog_button(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
        ) -> Result<(), DialogError> {
            if self.dialog_controllers.borrow().contains_key(id) {
                return Err(DialogError::UnsupportedCapability(
                    DialogCapability::ButtonClick,
                ));
            }
            self.with_surface(id, |host| dialog::click_dialog_button(host, predicate))
                .ok_or(DialogError::MissingSurface)?
        }

        /// Sets text on a GTK text field or text view within a dialog.
        pub fn set_dialog_text(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
            text: &str,
        ) -> Result<(), DialogError> {
            if self.dialog_controllers.borrow().contains_key(id) {
                return Err(DialogError::UnsupportedCapability(
                    DialogCapability::TextEdit,
                ));
            }
            self.with_surface(id, |host| dialog::set_dialog_text(host, predicate, text))
                .ok_or(DialogError::MissingSurface)?
        }

        /// Chooses a deterministic save destination in a GTK file chooser dialog.
        pub fn choose_save_dialog_path(
            &self,
            id: &SurfaceId,
            path: &Path,
            options: PollOptions,
        ) -> Result<usize, DialogError> {
            if self.dialog_controllers.borrow().contains_key(id) {
                return Err(DialogError::UnsupportedCapability(
                    DialogCapability::SavePathSelection,
                ));
            }
            self.with_surface(id, |host| {
                dialog::choose_save_dialog_path(host, path, options)
            })
            .ok_or(DialogError::MissingSurface)?
        }

        /// Chooses deterministic open paths in a GTK file chooser dialog.
        pub fn choose_open_dialog_paths(
            &self,
            id: &SurfaceId,
            paths: &[PathBuf],
            options: PollOptions,
        ) -> Result<usize, DialogError> {
            if self.dialog_controllers.borrow().contains_key(id) {
                return Err(DialogError::UnsupportedCapability(
                    DialogCapability::OpenPathSelection,
                ));
            }
            self.with_surface(id, |host| {
                dialog::choose_open_dialog_paths(host, paths, options)
            })
            .ok_or(DialogError::MissingSurface)?
        }

        /// Cancels the named surface-backed GTK dialog.
        ///
        /// Controller-only dialogs return
        /// `DialogError::UnsupportedCapability(DialogCapability::Cancel)`.
        pub fn cancel_dialog(
            &self,
            id: &SurfaceId,
            options: PollOptions,
        ) -> Result<usize, DialogError> {
            if self.dialog_controllers.borrow().contains_key(id) {
                return Err(DialogError::UnsupportedCapability(DialogCapability::Cancel));
            }
            self.with_surface(id, |host| dialog::cancel_dialog(host, options))
                .ok_or(DialogError::MissingSurface)?
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

        /// Opens a visible popover-backed GTK context menu on the node matching `predicate`.
        pub fn context_click_node(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
        ) -> Option<Result<crate::GtkContextMenu, crate::GtkContextMenuError>> {
            self.with_surface(id, |host| host.context_click_node(predicate))
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
            self.dialog_controllers.borrow_mut().remove(id);
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

    fn transient_candidate_ids(
        owner_window: &Window,
        owner_root: Option<&Widget>,
    ) -> BTreeSet<usize> {
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
