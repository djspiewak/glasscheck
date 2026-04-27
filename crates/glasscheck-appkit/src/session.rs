#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use glasscheck_core::{
        PollError, PollOptions, Scene, Selector, SurfaceId, SurfaceQuery, TransientSurfaceSpec,
    };
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplication, NSWindow};
    use objc2_foundation::{MainThreadMarker, NSPoint};

    use crate::dialog::{self, AppKitDialogError, AppKitDialogKind, AppKitDialogQuery};
    use crate::{AppKitContextMenu, AppKitHarness, AppKitWindowHost, HitPointSearch};

    /// Coordinator for multi-surface AppKit test flows.
    pub struct AppKitSession {
        harness: AppKitHarness,
        surfaces: RefCell<BTreeMap<SurfaceId, AppKitWindowHost>>,
    }

    impl AppKitSession {
        #[must_use]
        pub fn new(harness: AppKitHarness) -> Self {
            Self {
                harness,
                surfaces: RefCell::new(BTreeMap::new()),
            }
        }

        /// Registers a pre-built [`AppKitWindowHost`] under the given surface ID.
        pub fn attach_host(&self, id: impl Into<SurfaceId>, host: AppKitWindowHost) {
            let id = id.into();
            debug_assert!(
                !self.surfaces.borrow().contains_key(&id),
                "surface id '{}' is already registered; use a distinct id or remove the existing surface first",
                id.as_str()
            );
            self.surfaces.borrow_mut().insert(id, host);
        }

        /// Wraps a raw [`NSWindow`] into a host and registers it under the given surface ID.
        pub fn attach_window(&self, id: impl Into<SurfaceId>, window: &NSWindow) {
            self.attach_host(id, self.harness.attach_window(window));
        }

        #[must_use]
        pub fn discover_window(&self, id: impl Into<SurfaceId>, query: &SurfaceQuery) -> bool {
            let registered_ids: std::collections::BTreeSet<usize> = {
                let surfaces = self.surfaces.borrow();
                surfaces
                    .values()
                    .map(|host| window_id(host.window()))
                    .collect()
            };
            let app = NSApplication::sharedApplication(self.harness.main_thread_marker());
            let Some(window) = app.orderedWindows().iter().find(|window| {
                !registered_ids.contains(&window_id(window))
                    && query.matches_title(&window.title().to_string())
            }) else {
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

        /// Polls until a native AppKit dialog or panel matching `query` is found and attached.
        pub fn wait_for_dialog(
            &self,
            id: impl Into<SurfaceId>,
            query: &AppKitDialogQuery,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            self.harness.wait_until(options, || {
                let (registered_ids, owner_windows): (BTreeSet<usize>, Vec<Retained<NSWindow>>) = {
                    let surfaces = self.surfaces.borrow();
                    (
                        surfaces
                            .values()
                            .map(|host| window_id(host.window()))
                            .collect(),
                        surfaces
                            .values()
                            .map(|host| unsafe {
                                Retained::retain(host.window() as *const NSWindow as *mut NSWindow)
                                    .expect("owner window retain")
                            })
                            .collect(),
                    )
                };
                let Some(window) =
                    dialog_window_candidates(self.harness.main_thread_marker(), &owner_windows)
                        .into_iter()
                        .find(|window| {
                            !registered_ids.contains(&window_id(window))
                                && query.matches_window(window)
                        })
                else {
                    return false;
                };
                self.attach_host(
                    id.clone(),
                    dialog::attach_dialog_window(self.harness, &window),
                );
                true
            })
        }

        /// Clicks an opener on the owner surface and attaches the newly opened transient.
        ///
        /// Returns `PollError::Timeout` if the transient never appears within
        /// `options`. Returns `PollError::Precondition` for precondition failures: owner surface not
        /// registered or opener click failed.
        pub fn open_transient_with_click(
            &self,
            id: impl Into<SurfaceId>,
            spec: &TransientSurfaceSpec,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            let id = id.into();
            debug_assert!(
                id != spec.owner,
                "transient id must not equal the owner surface id"
            );
            let Some((baseline, opener_point, owner_window, mtm)) = ({
                let surfaces = self.surfaces.borrow();
                surfaces.get(&spec.owner).map(|host| {
                    let opener_point = host
                        .resolve_hit_point(&spec.opener, &HitPointSearch::default())
                        .ok()
                        .map(|point| point_in_screen_space(host, point));
                    // opener_point is None if resolve_hit_point fails (e.g., zero-size frame),
                    // in which case window_is_anchored_near is skipped and only direct parent/
                    // child/sheet relationships are used for transient identification.
                    let baseline = transient_window_baseline(
                        host.window(),
                        host.main_thread_marker(),
                        opener_point,
                    );
                    let w = unsafe {
                        Retained::retain(host.window() as *const NSWindow as *mut NSWindow)
                            .expect("owner window retain")
                    };
                    (baseline, opener_point, w, host.main_thread_marker())
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
                        .is_some_and(|h| std::ptr::eq(h.window() as *const _, owner_window.as_ref() as *const _))
                },
                "owner surface was evicted or replaced between baseline capture and transient discovery"
            );
            // opener_point is intentionally captured once at baseline time. Background
            // test windows are expected to be layout-stable; recomputing each iteration
            // would require reborrowing surfaces inside wait_until which conflicts with
            // the reentrancy restriction.
            self.harness.wait_until(options, || {
                if !self.surfaces.borrow().contains_key(&spec.owner) {
                    return false;
                }
                let Some(window) =
                    discover_owned_transient_window(&owner_window, mtm, &baseline, opener_point)
                else {
                    return false;
                };
                self.attach_window(id.clone(), &window);
                true
            })
        }

        #[must_use]
        /// Returns whether the named surface is still available.
        pub fn surface_is_open(&self, id: &SurfaceId) -> bool {
            let is_open = {
                let surfaces = self.surfaces.borrow();
                surfaces.get(id).is_some_and(appkit_host_is_open)
            };
            if !is_open {
                let _ = self.remove_surface(id);
            }
            is_open
        }

        /// Waits for the named transient surface to dismiss and evicts it from the session.
        ///
        /// A transient is considered closed when its window is both invisible and unparented.
        /// For tracking non-transient surface closure, use `surface_is_open` instead.
        ///
        /// See the GTK counterpart for comparison: GTK uses visibility-only detection
        /// while AppKit uses invisible-AND-unparented.
        pub fn wait_for_surface_closed(
            &self,
            id: &SurfaceId,
            options: PollOptions,
        ) -> Result<usize, PollError> {
            if !{ self.surfaces.borrow().contains_key(id) } {
                return Ok(0);
            }
            self.harness.wait_until(options, || {
                let is_closed = {
                    let surfaces = self.surfaces.borrow();
                    surfaces.get(id).is_none_or(appkit_transient_is_closed)
                };
                if is_closed {
                    let _ = self.remove_surface(id);
                }
                is_closed
            })
        }

        /// Calls `f` with a reference to the host for the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed.
        ///
        /// # Panics
        ///
        /// Panics if `f` re-enters the session via any method that accesses the
        /// internal surface map, including `attach_host`, `attach_window`,
        /// `remove_surface`, `surface_is_open`,
        /// `snapshot_scene`, `click_node`, `context_click_node`, `hover_node`,
        /// `wait_for_surface_closed`, `discover_window`, `wait_for_discovered_window`,
        /// `open_transient_with_click`, or a nested `with_surface` call.
        /// Only `wait_until` (on the underlying harness) is safe to call from `f`.
        pub fn with_surface<R>(
            &self,
            id: &SurfaceId,
            f: impl FnOnce(&AppKitWindowHost) -> R,
        ) -> Option<R> {
            let is_open = {
                let surfaces = self.surfaces.borrow();
                surfaces.get(id).is_some_and(appkit_host_is_open)
            };
            if !is_open {
                let _ = self.remove_surface(id);
                return None;
            }
            let surfaces = self.surfaces.borrow();
            surfaces.get(id).map(f)
        }

        /// Snapshots the accessibility scene for the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed (and evicts it as a side effect).
        #[must_use]
        pub fn snapshot_scene(&self, id: &SurfaceId) -> Option<Scene> {
            self.with_surface(id, AppKitWindowHost::snapshot_scene)
        }

        /// Returns the native AppKit dialog kind for the named surface.
        pub fn dialog_kind(&self, id: &SurfaceId) -> Result<AppKitDialogKind, AppKitDialogError> {
            self.with_surface(id, |host| {
                dialog::classify_window(host.window()).ok_or(AppKitDialogError::NotDialog)
            })
            .ok_or(AppKitDialogError::MissingSurface)?
        }

        /// Snapshots the named dialog or panel into a semantic scene.
        pub fn snapshot_dialog_scene(&self, id: &SurfaceId) -> Result<Scene, AppKitDialogError> {
            self.with_surface(id, dialog::snapshot_dialog_scene)
                .ok_or(AppKitDialogError::MissingSurface)?
        }

        /// Synthesizes a click on a dialog button matching `predicate`.
        pub fn click_dialog_button(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
        ) -> Result<(), AppKitDialogError> {
            self.with_surface(id, |host| dialog::click_dialog_button(host, predicate))
                .ok_or(AppKitDialogError::MissingSurface)?
        }

        /// Sets text on a native text field or text view within a dialog.
        pub fn set_dialog_text(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
            text: &str,
        ) -> Result<(), AppKitDialogError> {
            self.with_surface(id, |host| dialog::set_dialog_text(host, predicate, text))
                .ok_or(AppKitDialogError::MissingSurface)?
        }

        /// Chooses a deterministic save destination in a live `NSSavePanel`.
        pub fn choose_save_panel_path(
            &self,
            id: &SurfaceId,
            path: &Path,
            options: PollOptions,
        ) -> Result<usize, AppKitDialogError> {
            self.with_surface(id, |host| {
                dialog::choose_save_panel_path(host, path, options)
            })
            .ok_or(AppKitDialogError::MissingSurface)?
        }

        /// Chooses deterministic paths in a live `NSOpenPanel` when the OS panel exposes them.
        pub fn choose_open_panel_paths(
            &self,
            id: &SurfaceId,
            paths: &[PathBuf],
            options: PollOptions,
        ) -> Result<usize, AppKitDialogError> {
            self.with_surface(id, |host| {
                dialog::choose_open_panel_paths(host, paths, options)
            })
            .ok_or(AppKitDialogError::MissingSurface)?
        }

        /// Cancels the named native dialog or panel.
        pub fn cancel_dialog(
            &self,
            id: &SurfaceId,
            options: PollOptions,
        ) -> Result<usize, AppKitDialogError> {
            self.with_surface(id, |host| dialog::cancel_dialog(host, options))
                .ok_or(AppKitDialogError::MissingSurface)?
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

        /// Opens the native AppKit context menu for a node in the named surface.
        ///
        /// Returns `None` if the surface is absent or has been closed (and evicts
        /// it as a side effect); `Some(Err(...))` if the node or menu can't be
        /// resolved.
        pub fn context_click_node(
            &self,
            id: &SurfaceId,
            predicate: &Selector,
        ) -> Option<Result<AppKitContextMenu, crate::AppKitContextMenuError>> {
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
        pub(crate) fn remove_surface(&self, id: &SurfaceId) -> Option<AppKitWindowHost> {
            self.surfaces.borrow_mut().remove(id)
        }
    }

    // Checks whether the window is still in NSApplication's window list.
    // NOTE: Because the host retains its NSWindow (and background test windows
    // set releasedWhenClosed=false), a window that has been closed but is still
    // retained by this host will continue to appear in app.windows(). This
    // cannot be resolved without tracking NSWindowWillCloseNotification.
    // For transient lifecycle use wait_for_surface_closed instead.
    fn appkit_host_is_open(host: &AppKitWindowHost) -> bool {
        let app = NSApplication::sharedApplication(host.main_thread_marker());
        let target_id = window_id(host.window());
        app.windows()
            .iter()
            .any(|window| window_id(&window) == target_id)
    }

    fn appkit_transient_is_closed(host: &AppKitWindowHost) -> bool {
        !host.window().isVisible() && host.window().parentWindow().is_none()
    }

    struct TransientWindowBaseline {
        owner_linked_ids: BTreeSet<usize>,
        ordered_window_ids: BTreeSet<usize>,
    }

    fn transient_window_baseline(
        owner_window: &NSWindow,
        mtm: MainThreadMarker,
        opener_point: Option<NSPoint>,
    ) -> TransientWindowBaseline {
        TransientWindowBaseline {
            owner_linked_ids: owner_linked_transient_window_candidates(
                owner_window,
                mtm,
                opener_point,
            )
            .into_iter()
            .map(|window| window_id(&window))
            .collect(),
            ordered_window_ids: ordered_window_candidates(owner_window, mtm)
                .into_iter()
                .map(|window| window_id(&window))
                .collect(),
        }
    }

    fn discover_owned_transient_window(
        owner_window: &NSWindow,
        mtm: MainThreadMarker,
        baseline: &TransientWindowBaseline,
        opener_point: Option<NSPoint>,
    ) -> Option<Retained<NSWindow>> {
        let owner_linked =
            owner_linked_transient_window_candidates(owner_window, mtm, opener_point);
        let ordered = ordered_window_candidates(owner_window, mtm);
        let selected_id = discover_transient_window_id(
            baseline,
            owner_linked.iter().map(|window| window_id(window)),
            ordered.iter().map(|window| window_id(window)),
        )?;
        owner_linked
            .into_iter()
            .chain(ordered)
            .find(|window| window_id(window) == selected_id)
    }

    fn dialog_window_candidates(
        mtm: MainThreadMarker,
        owner_windows: &[Retained<NSWindow>],
    ) -> Vec<Retained<NSWindow>> {
        let app = NSApplication::sharedApplication(mtm);
        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();
        for window in app.orderedWindows().iter().chain(app.windows().iter()) {
            let id = window_id(&window);
            if seen.insert(id) {
                candidates.push(window);
            }
        }
        for owner in owner_windows {
            if let Some(sheet) = owner.attachedSheet() {
                let id = window_id(&sheet);
                if seen.insert(id) {
                    candidates.push(sheet);
                }
            }
            if let Some(children) = owner.childWindows() {
                for child in children.iter() {
                    let id = window_id(&child);
                    if seen.insert(id) {
                        candidates.push(child);
                    }
                }
            }
        }
        candidates
    }

    fn ordered_window_candidates(
        owner_window: &NSWindow,
        mtm: MainThreadMarker,
    ) -> Vec<Retained<NSWindow>> {
        let app = NSApplication::sharedApplication(mtm);
        let owner_id = window_id(owner_window);
        let mut candidates = Vec::new();
        for window in app.orderedWindows().iter() {
            let candidate_id = window_id(&window);
            if candidate_id == owner_id {
                continue;
            }
            candidates.push(window);
        }
        candidates
    }

    fn owner_linked_transient_window_candidates(
        owner_window: &NSWindow,
        mtm: MainThreadMarker,
        opener_point: Option<NSPoint>,
    ) -> Vec<Retained<NSWindow>> {
        let owner_id = window_id(owner_window);
        let child_ids = owner_window
            .childWindows()
            .map(|windows| {
                windows
                    .iter()
                    .map(|window| window_id(&window))
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let attached_sheet_id = owner_window
            .attachedSheet()
            .map(|window| window_id(&window));
        let app = NSApplication::sharedApplication(mtm);
        let mut direct = Vec::new();
        let mut anchored = Vec::new();
        for window in app.orderedWindows().iter() {
            let candidate_id = window_id(&window);
            if candidate_id == owner_id {
                continue;
            }
            let parent_matches = window
                .parentWindow()
                .as_ref()
                .is_some_and(|parent| window_id(parent) == owner_id);
            let child_matches = child_ids.contains(&candidate_id);
            let sheet_matches = attached_sheet_id == Some(candidate_id);
            if parent_matches || child_matches || sheet_matches {
                direct.push(window);
                continue;
            }
            if opener_point.is_some_and(|point| window_is_anchored_near(&window, point)) {
                anchored.push(window);
                continue;
            }
        }
        direct.extend(anchored);
        direct
    }

    fn discover_transient_window_id(
        baseline: &TransientWindowBaseline,
        owner_linked_candidates: impl IntoIterator<Item = usize>,
        ordered_window_candidates: impl IntoIterator<Item = usize>,
    ) -> Option<usize> {
        owner_linked_candidates
            .into_iter()
            .find(|candidate_id| !baseline.owner_linked_ids.contains(candidate_id))
            .or_else(|| {
                ordered_window_candidates
                    .into_iter()
                    .find(|candidate_id| !baseline.ordered_window_ids.contains(candidate_id))
            })
    }

    fn window_is_anchored_near(window: &NSWindow, point: NSPoint) -> bool {
        let frame = window.frame();
        let min_x = frame.origin.x - 32.0;
        let max_x = frame.origin.x + frame.size.width + 32.0;
        let min_y = frame.origin.y - 32.0;
        let max_y = frame.origin.y + frame.size.height + 32.0;
        point.x >= min_x && point.x <= max_x && point.y >= min_y && point.y <= max_y
    }

    fn point_in_screen_space(owner: &AppKitWindowHost, point: glasscheck_core::Point) -> NSPoint {
        let frame = owner.window().frame();
        NSPoint::new(frame.origin.x + point.x, frame.origin.y + point.y)
    }

    // Uses raw pointer identity rather than windowNumber() because windowNumber()
    // returns 0 for any ordered-out window, causing collisions for background test
    // windows. The session holds Retained<NSWindow> for every tracked window, so
    // the address is stable for the session's lifetime. In the unlikely event that
    // a non-retained transient candidate is released and its address reused before
    // the next poll, discover_transient_window_id may miss the new transient; this
    // is accepted as an extremely-low-probability edge case.
    fn window_id(window: &NSWindow) -> usize {
        window as *const NSWindow as usize
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeSet;

        use super::{discover_transient_window_id, TransientWindowBaseline};

        #[test]
        fn prefers_new_owner_linked_transient_over_existing_peer_window() {
            let baseline = TransientWindowBaseline {
                owner_linked_ids: BTreeSet::from([11]),
                ordered_window_ids: BTreeSet::from([7, 11, 42]),
            };

            let selected = discover_transient_window_id(&baseline, [11, 13], [7, 11, 13, 42]);

            assert_eq!(selected, Some(13));
        }

        #[test]
        fn falls_back_to_new_ordered_window_when_owner_linked_set_is_unchanged() {
            let baseline = TransientWindowBaseline {
                owner_linked_ids: BTreeSet::from([11]),
                ordered_window_ids: BTreeSet::from([7, 11]),
            };

            let selected = discover_transient_window_id(&baseline, [11], [7, 11, 13]);

            assert_eq!(selected, Some(13));
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct AppKitSession;
}

pub use imp::AppKitSession;
