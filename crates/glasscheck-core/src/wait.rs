use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::Image;

#[derive(Clone, Copy, Debug)]
pub struct PollOptions {
    pub timeout: Duration,
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

#[derive(Debug)]
pub enum PollError {
    Timeout { elapsed: Duration, attempts: usize },
    CaptureFailed(&'static str),
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
        }
    }
}

impl std::error::Error for PollError {}

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
        if start.elapsed() >= options.timeout {
            return Err(PollError::Timeout {
                elapsed: start.elapsed(),
                attempts,
            });
        }
        sleep(options.interval);
    }
}

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

        if required == 1 || run_length + 1 >= required && previous.as_ref() == Some(&current) {
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

        if start.elapsed() >= options.timeout {
            return Err(PollError::Timeout {
                elapsed: start.elapsed(),
                attempts,
            });
        }
        sleep(options.interval);
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaitArtifacts {
    pub frames: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(value: u8) -> Image {
        Image::new(1, 1, vec![value, value, value, 255])
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
}
