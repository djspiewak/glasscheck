use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::{Image, LayoutTolerance, PropertyValue, Rect, ResolvedNode, Scene, Selector};

/// Polling configuration for asynchronous UI assertions.
///
/// Use short intervals when a backend flush is cheap and longer intervals when
/// capture is expensive. The default targets frame-scale UI settling.
#[derive(Clone, Copy, Debug)]
pub struct PollOptions {
    /// Maximum amount of time to wait before failing.
    pub timeout: Duration,
    /// Delay between polling attempts.
    pub interval: Duration,
}

impl Default for PollOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            interval: Duration::from_millis(16),
        }
    }
}

/// Errors returned by polling helpers.
#[derive(Debug)]
pub enum PollError {
    /// The condition did not succeed before the timeout elapsed.
    Timeout { elapsed: Duration, attempts: usize },
    /// A capture source could not provide an image.
    CaptureFailed(&'static str),
    /// A required precondition was not met; polling was not attempted.
    Precondition(&'static str),
}

impl std::fmt::Display for PollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { elapsed, attempts } => {
                write!(
                    f,
                    "condition timed out after {:?} ({} attempts)",
                    elapsed, attempts
                )
            }
            Self::CaptureFailed(message) => write!(f, "capture failed: {message}"),
            Self::Precondition(reason) => write!(f, "precondition not met: {reason}"),
        }
    }
}

impl std::error::Error for PollError {}

/// Errors returned by semantic wait helpers.
#[derive(Clone, Debug, PartialEq)]
pub enum WaitError {
    /// The condition did not succeed before the timeout elapsed.
    Timeout {
        elapsed: Duration,
        attempts: usize,
        last_scene: Option<Scene>,
        last_matches: Vec<ResolvedNode>,
    },
    /// A capture source could not provide a scene.
    CaptureFailed(&'static str),
}

impl std::fmt::Display for WaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout {
                elapsed, attempts, ..
            } => {
                write!(
                    f,
                    "condition timed out after {:?} ({} attempts)",
                    elapsed, attempts
                )
            }
            Self::CaptureFailed(message) => write!(f, "capture failed: {message}"),
        }
    }
}

impl std::error::Error for WaitError {}

/// Repeatedly evaluates `predicate` until it returns `true` or times out.
///
/// Returns the number of attempts performed.
pub fn wait_for_condition<F>(options: PollOptions, mut predicate: F) -> Result<usize, PollError>
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    let mut attempts = 0;

    loop {
        attempts += 1;
        if predicate() {
            return Ok(attempts);
        }
        let remaining = options.timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(PollError::Timeout {
                elapsed: start.elapsed(),
                attempts,
            });
        }
        sleep(remaining.min(options.interval));
    }
}

/// Captures images until the output remains unchanged for `stable_frames`.
///
/// This is useful for animations, deferred drawing, or raster content that
/// settles after multiple event-loop turns. Returns the stable image once the
/// requirement is met.
pub fn wait_for_image_stability<F>(
    options: PollOptions,
    stable_frames: usize,
    mut capture: F,
) -> Result<Image, PollError>
where
    F: FnMut() -> Option<Image>,
{
    let start = Instant::now();
    let mut attempts = 0usize;
    let required = stable_frames.max(1);
    let mut run_length = 0usize;
    let mut previous: Option<Image> = None;

    loop {
        attempts += 1;
        let current = capture().ok_or(PollError::CaptureFailed("image source returned None"))?;

        if required == 1 || (run_length + 1 >= required && previous.as_ref() == Some(&current)) {
            return Ok(current);
        }

        if let Some(previous) = previous.as_ref() {
            if previous == &current {
                run_length += 1;
            } else {
                run_length = 1;
            }
        } else {
            run_length = 1;
        }

        previous = Some(current);

        let remaining = options.timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(PollError::Timeout {
                elapsed: start.elapsed(),
                attempts,
            });
        }
        sleep(remaining.min(options.interval));
    }
}

/// Captures scenes until the result remains unchanged for `stable_polls`.
///
/// Prefer this over image stability when tests care about semantic state rather
/// than exact pixels.
pub fn wait_for_scene_stability<F>(
    options: PollOptions,
    stable_polls: usize,
    mut capture: F,
) -> Result<Scene, PollError>
where
    F: FnMut() -> Option<Scene>,
{
    let start = Instant::now();
    let mut attempts = 0usize;
    let required = stable_polls.max(1);
    let mut run_length = 0usize;
    let mut previous: Option<Scene> = None;

    loop {
        attempts += 1;
        let current = capture().ok_or(PollError::CaptureFailed("scene source returned None"))?;
        if required == 1 || (run_length + 1 >= required && previous.as_ref() == Some(&current)) {
            return Ok(current);
        }

        if let Some(previous) = previous.as_ref() {
            if previous == &current {
                run_length += 1;
            } else {
                run_length = 1;
            }
        } else {
            run_length = 1;
        }

        previous = Some(current);

        let remaining = options.timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(PollError::Timeout {
                elapsed: start.elapsed(),
                attempts,
            });
        }
        sleep(remaining.min(options.interval));
    }
}

#[allow(dead_code)]
/// Optional artifact container for wait-related debugging output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaitArtifacts {
    /// Paths to captured frames.
    pub frames: Vec<PathBuf>,
}

/// Waits for a predicate to exist in the captured scene.
///
/// Returns the last successful scene so follow-up assertions can reuse it.
pub fn wait_for_exists<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
) -> Result<Scene, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_scene_match(options, capture, predicate, |scene, predicate| {
        scene.exists(predicate)
    })
}

/// Waits for a predicate to become absent from the captured scene.
pub fn wait_for_absent<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
) -> Result<Scene, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_scene_match(options, capture, predicate, |scene, predicate| {
        !scene.exists(predicate)
    })
}

/// Waits for a predicate to resolve to a visible node.
pub fn wait_for_visible<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_resolved(options, capture, predicate, |resolved| {
        resolved.visible_bounds.is_some()
    })
}

/// Waits for a predicate to resolve to a hit-testable node.
pub fn wait_for_hit_testable<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_resolved(options, capture, predicate, |resolved| {
        resolved.interactability.is_hit_testable()
    })
}

/// Waits for a predicate to match the expected count.
pub fn wait_for_count<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
    expected: usize,
) -> Result<Scene, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_scene_match(options, capture, predicate, |scene, predicate| {
        scene.count(predicate) == expected
    })
}

/// Waits for a predicate to expose a property value.
pub fn wait_for_property<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
    key: &str,
    expected: &PropertyValue,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_resolved(options, capture, predicate, |resolved| {
        resolved.node.properties.get(key) == Some(expected)
    })
}

/// Waits for a predicate to expose a state value.
pub fn wait_for_state<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
    key: &str,
    expected: &PropertyValue,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_resolved(options, capture, predicate, |resolved| {
        resolved.node.state.get(key) == Some(expected)
    })
}

/// Waits for a predicate to reach the expected bounds.
pub fn wait_for_bounds<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
    expected: Rect,
    tolerance: LayoutTolerance,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_resolved(options, capture, predicate, |resolved| {
        (resolved.bounds.origin.x - expected.origin.x).abs() <= tolerance.position
            && (resolved.bounds.origin.y - expected.origin.y).abs() <= tolerance.position
            && (resolved.bounds.size.width - expected.size.width).abs() <= tolerance.size
            && (resolved.bounds.size.height - expected.size.height).abs() <= tolerance.size
    })
}

/// Waits for a predicate to become interactable.
pub fn wait_for_interactable<F>(
    options: PollOptions,
    capture: F,
    predicate: &Selector,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
{
    wait_for_resolved(options, capture, predicate, |resolved| {
        matches!(
            resolved.interactability,
            crate::Interactability::Interactable { .. }
        )
    })
}

fn wait_for_scene_match<F, P>(
    options: PollOptions,
    mut capture: F,
    predicate: &Selector,
    mut matches_scene: P,
) -> Result<Scene, WaitError>
where
    F: FnMut() -> Option<Scene>,
    P: FnMut(&Scene, &Selector) -> bool,
{
    let start = Instant::now();
    let mut attempts = 0usize;
    loop {
        attempts += 1;
        let scene = capture().ok_or(WaitError::CaptureFailed("scene source returned None"))?;
        if matches_scene(&scene, predicate) {
            return Ok(scene);
        }
        let remaining = options.timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            let last_matches = scene
                .resolve_all(predicate)
                .into_iter()
                .map(ResolvedNode::from)
                .collect();
            return Err(WaitError::Timeout {
                elapsed: start.elapsed(),
                attempts,
                last_scene: Some(scene),
                last_matches,
            });
        }
        sleep(remaining.min(options.interval));
    }
}

fn wait_for_resolved<F, P>(
    options: PollOptions,
    mut capture: F,
    predicate: &Selector,
    mut matches_resolved: P,
) -> Result<ResolvedNode, WaitError>
where
    F: FnMut() -> Option<Scene>,
    P: FnMut(&ResolvedNode) -> bool,
{
    let start = Instant::now();
    let mut attempts = 0usize;
    loop {
        attempts += 1;
        let scene = capture().ok_or(WaitError::CaptureFailed("scene source returned None"))?;
        if let Ok(resolved) = scene.resolve(predicate) {
            let resolved = ResolvedNode::from(resolved);
            if matches_resolved(&resolved) {
                return Ok(resolved);
            }
        }
        let remaining = options.timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            let last_matches = scene
                .resolve_all(predicate)
                .into_iter()
                .map(ResolvedNode::from)
                .collect();
            return Err(WaitError::Timeout {
                elapsed: start.elapsed(),
                attempts,
                last_scene: Some(scene),
                last_matches,
            });
        }
        sleep(remaining.min(options.interval));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(value: u8) -> Image {
        Image::new(1, 1, vec![value, value, value, 255])
    }

    fn scene(value: i64) -> Scene {
        Scene::new(vec![crate::SemanticNode::new(
            format!("node-{value}"),
            crate::Role::Container,
            crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(1.0, 1.0)),
        )
        .with_property("value", crate::PropertyValue::Integer(value))])
    }

    #[test]
    fn wait_for_condition_returns_after_eventual_success() {
        let mut calls = 0;
        let attempts = wait_for_condition(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            || {
                calls += 1;
                calls >= 3
            },
        )
        .unwrap();
        assert!(attempts >= 3);
    }

    #[test]
    fn wait_for_image_stability_detects_stable_tail() {
        let mut frames = vec![image(1), image(2), image(2)].into_iter();
        let stable = wait_for_image_stability(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            2,
            || frames.next(),
        )
        .unwrap();

        assert_eq!(stable, image(2));
    }

    #[test]
    fn wait_for_image_stability_honors_three_frame_requirement() {
        let mut frames = vec![image(1), image(2), image(2), image(2)].into_iter();
        let stable = wait_for_image_stability(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            3,
            || frames.next(),
        )
        .unwrap();

        assert_eq!(stable, image(2));
    }

    #[test]
    fn wait_for_image_stability_returns_first_frame_when_one_is_required() {
        let mut frames = vec![image(7)].into_iter();
        let stable = wait_for_image_stability(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            1,
            || frames.next(),
        )
        .unwrap();

        assert_eq!(stable, image(7));
    }

    #[test]
    fn wait_for_image_stability_times_out_when_frames_keep_alternating() {
        let mut next = 0usize;
        let error = wait_for_image_stability(
            PollOptions {
                timeout: Duration::from_millis(5),
                interval: Duration::from_millis(1),
            },
            2,
            || {
                next += 1;
                Some(if next % 2 == 0 { image(2) } else { image(1) })
            },
        )
        .unwrap_err();

        assert!(matches!(error, PollError::Timeout { .. }));
    }

    #[test]
    fn wait_for_scene_stability_detects_stable_tail() {
        let mut scenes = vec![scene(1), scene(2), scene(2)].into_iter();
        let stable = wait_for_scene_stability(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            2,
            || scenes.next(),
        )
        .unwrap();

        assert_eq!(stable, scene(2));
    }

    #[test]
    fn wait_for_scene_stability_times_out_when_scene_keeps_changing() {
        let mut next = 0i64;
        let error = wait_for_scene_stability(
            PollOptions {
                timeout: Duration::from_millis(5),
                interval: Duration::from_millis(1),
            },
            2,
            || {
                next += 1;
                Some(scene(next))
            },
        )
        .unwrap_err();

        assert!(matches!(error, PollError::Timeout { .. }));
    }

    #[test]
    fn semantic_waits_cover_success_and_failure_cases() {
        let mut scenes = vec![
            scene(1),
            scene(2),
            Scene::new(vec![crate::SemanticNode::new(
                "node-3",
                crate::Role::Container,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target")
            .with_property("value", crate::PropertyValue::Integer(3))]),
        ]
        .into_iter();

        let resolved = wait_for_exists(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            || scenes.next(),
            &crate::Selector::selector_eq("target"),
        )
        .unwrap();
        assert!(resolved.exists(&crate::Selector::selector_eq("target")));

        let stable_missing = Scene::new(vec![crate::SemanticNode::new(
            "node",
            crate::Role::Container,
            crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
        )]);
        let error = wait_for_exists(
            PollOptions {
                timeout: Duration::from_millis(5),
                interval: Duration::from_millis(1),
            },
            || Some(stable_missing.clone()),
            &crate::Selector::selector_eq("missing"),
        )
        .unwrap_err();
        assert!(matches!(error, WaitError::Timeout { .. }));
    }

    #[test]
    fn semantic_wait_for_property_and_absence_cover_pass_and_fail_cases() {
        let mut scenes = vec![
            Scene::new(vec![crate::SemanticNode::new(
                "node",
                crate::Role::Container,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target")]),
            Scene::new(vec![crate::SemanticNode::new(
                "node",
                crate::Role::Container,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target")
            .with_property("ready", crate::PropertyValue::Bool(true))]),
        ]
        .into_iter();

        assert!(wait_for_property(
            PollOptions {
                timeout: Duration::from_millis(50),
                interval: Duration::from_millis(1),
            },
            || scenes.next(),
            &crate::Selector::selector_eq("target"),
            "ready",
            &crate::PropertyValue::Bool(true),
        )
        .is_ok());

        let mut scenes = vec![Scene::new(Vec::new())].into_iter();
        assert!(wait_for_absent(
            PollOptions {
                timeout: Duration::from_millis(20),
                interval: Duration::from_millis(1),
            },
            || scenes.next(),
            &crate::Selector::selector_eq("target"),
        )
        .is_ok());

        let error = wait_for_visible(
            PollOptions {
                timeout: Duration::from_millis(20),
                interval: Duration::from_millis(1),
            },
            || None,
            &crate::Selector::selector_eq("target"),
        )
        .unwrap_err();
        assert!(matches!(error, WaitError::CaptureFailed(_)));
    }

    #[test]
    fn wait_for_visible_supports_visible_nodes_without_known_visible_rect_and_times_out_when_hidden(
    ) {
        let mut visible_scenes = vec![Scene::new(vec![crate::SemanticNode::new(
            "node",
            crate::Role::Button,
            crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
        )
        .with_selector("provider.button")])]
        .into_iter();

        let resolved = wait_for_visible(
            PollOptions {
                timeout: Duration::from_millis(20),
                interval: Duration::from_millis(1),
            },
            || visible_scenes.next(),
            &crate::Selector::selector_eq("provider.button"),
        )
        .unwrap();
        assert_eq!(
            resolved.visible_bounds,
            Some(crate::Rect::new(
                crate::Point::new(0.0, 0.0),
                crate::Size::new(10.0, 10.0),
            ))
        );

        let hidden_scene = Scene::new(vec![crate::SemanticNode {
            visible: false,
            ..crate::SemanticNode::new(
                "node",
                crate::Role::Button,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("provider.button")
        }]);
        let error = wait_for_visible(
            PollOptions {
                timeout: Duration::from_millis(5),
                interval: Duration::from_millis(1),
            },
            || Some(hidden_scene.clone()),
            &crate::Selector::selector_eq("provider.button"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WaitError::Timeout {
                last_matches,
                ..
            } if last_matches.len() == 1 && last_matches[0].visible_bounds.is_none()
        ));
    }

    #[test]
    fn wait_for_hit_testable_accepts_occluded_nodes_and_rejects_not_hit_testable_ones() {
        let mut occluded_scenes = vec![Scene::new(vec![
            crate::SemanticNode::new(
                "root",
                crate::Role::Container,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(20.0, 20.0)),
            ),
            crate::SemanticNode::new(
                "target",
                crate::Role::Button,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target")
            .with_parent("root", 0),
            crate::SemanticNode::new(
                "overlay",
                crate::Role::Button,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_parent("root", 1),
        ])]
        .into_iter();

        let resolved = wait_for_hit_testable(
            PollOptions {
                timeout: Duration::from_millis(20),
                interval: Duration::from_millis(1),
            },
            || occluded_scenes.next(),
            &crate::Selector::selector_eq("target"),
        )
        .unwrap();
        assert!(matches!(
            resolved.interactability,
            crate::Interactability::Occluded { .. }
        ));

        let non_hit_testable = Scene::new(vec![crate::SemanticNode {
            hit_testable: false,
            ..crate::SemanticNode::new(
                "target",
                crate::Role::Button,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target")
        }]);
        let error = wait_for_hit_testable(
            PollOptions {
                timeout: Duration::from_millis(5),
                interval: Duration::from_millis(1),
            },
            || Some(non_hit_testable.clone()),
            &crate::Selector::selector_eq("target"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WaitError::Timeout {
                last_matches,
                ..
            } if last_matches.len() == 1
                && matches!(
                    last_matches[0].interactability,
                    crate::Interactability::NotHitTestable
                )
        ));
    }

    #[test]
    fn resolved_waits_do_not_succeed_while_predicate_is_ambiguous() {
        let ambiguous_scene = Scene::new(vec![
            crate::SemanticNode::new(
                "first",
                crate::Role::Button,
                crate::Rect::new(crate::Point::new(0.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target"),
            crate::SemanticNode::new(
                "second",
                crate::Role::Button,
                crate::Rect::new(crate::Point::new(20.0, 0.0), crate::Size::new(10.0, 10.0)),
            )
            .with_selector("target"),
        ]);

        let error = wait_for_visible(
            PollOptions {
                timeout: Duration::from_millis(5),
                interval: Duration::from_millis(1),
            },
            || Some(ambiguous_scene.clone()),
            &crate::Selector::selector_eq("target"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WaitError::Timeout {
                last_matches,
                ..
            } if last_matches.len() == 2
        ));
    }
}
