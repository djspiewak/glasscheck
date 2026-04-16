#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};

    use glasscheck_core::{
        PollError, PollOptions, Scene, Selector, SurfaceId, SurfaceQuery, TransientSurfaceSpec,
    };
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplication, NSWindow};
    use objc2_foundation::NSPoint;

    use crate::{AppKitHarness, AppKitWindowHost, HitPointSearch};

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

        pub fn attach_host(&self, id: impl Into<SurfaceId>, host: AppKitWindowHost) {
            self.surfaces.borrow_mut().insert(id.into(), host);
        }

        pub fn attach_window(&self, id: impl Into<SurfaceId>, window: &NSWindow) {
            self.attach_host(id, self.harness.attach_window(window));
        }

        #[must_use]
        pub fn discover_window(&self, id: impl Into<SurfaceId>, query: &SurfaceQuery) -> bool {
            let app = NSApplication::sharedApplication(self.harness.main_thread_marker());
            let Some(window) = app
                .orderedWindows()
                .iter()
                .find(|window| query.matches_title(&window.title().to_string()))
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
            let Some((baseline, opener_point)) = ({
                let surfaces = self.surfaces.borrow();
                surfaces.get(&spec.owner).map(|host| {
                    let opener_point = host
                        .resolve_hit_point(&spec.opener, &HitPointSearch::default())
                        .ok()
                        .map(|point| point_in_screen_space(host, point));
                    (transient_window_baseline(host, opener_point), opener_point)
                })
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
                self.harness.attach_window(host.window())
            }) else {
                return self.harness.wait_until(options, || false);
            };
            self.harness.wait_until(options, || {
                let Some(window) = discover_owned_transient_window(&owner, &baseline, opener_point)
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
        pub fn wait_for_surface_closed(
            &self,
            id: &SurfaceId,
            options: PollOptions,
        ) -> Result<usize, PollError> {
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

        #[must_use]
        pub fn snapshot_scene(&self, id: &SurfaceId) -> Option<Scene> {
            self.with_surface(id, AppKitWindowHost::snapshot_scene)
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
        pub(crate) fn remove_surface(&self, id: &SurfaceId) -> Option<AppKitWindowHost> {
            self.surfaces.borrow_mut().remove(id)
        }
    }

    fn appkit_host_is_open(host: &AppKitWindowHost) -> bool {
        let app = NSApplication::sharedApplication(host.main_thread_marker());
        let target_id = window_id(host.window());
        app.orderedWindows()
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
        owner: &AppKitWindowHost,
        opener_point: Option<NSPoint>,
    ) -> TransientWindowBaseline {
        TransientWindowBaseline {
            owner_linked_ids: owner_linked_transient_window_candidates(owner, opener_point)
                .into_iter()
                .map(|window| window_id(&window))
                .collect(),
            ordered_window_ids: ordered_window_candidates(owner)
                .into_iter()
                .map(|window| window_id(&window))
                .collect(),
        }
    }

    fn discover_owned_transient_window(
        owner: &AppKitWindowHost,
        baseline: &TransientWindowBaseline,
        opener_point: Option<NSPoint>,
    ) -> Option<Retained<NSWindow>> {
        let owner_linked = owner_linked_transient_window_candidates(owner, opener_point);
        let ordered = ordered_window_candidates(owner);
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

    fn ordered_window_candidates(owner: &AppKitWindowHost) -> Vec<Retained<NSWindow>> {
        let app = NSApplication::sharedApplication(owner.main_thread_marker());
        let mut candidates = Vec::new();
        for window in app.orderedWindows().iter() {
            let candidate_id = window_id(&window);
            let owner_id = window_id(owner.window());
            if candidate_id == owner_id {
                continue;
            }
            candidates.push(window);
        }
        candidates
    }

    fn owner_linked_transient_window_candidates(
        owner: &AppKitWindowHost,
        opener_point: Option<NSPoint>,
    ) -> Vec<Retained<NSWindow>> {
        let owner_window = owner.window();
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
        let app = NSApplication::sharedApplication(owner.main_thread_marker());
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
