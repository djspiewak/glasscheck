pub(crate) fn transparent_reference_pixel_constrains_match(expected: &[u8]) -> bool {
    expected.get(3).copied().unwrap_or_default() != 0
}

pub(crate) fn transparent_reference_pixel_matches(
    actual: &[u8],
    expected: &[u8],
    tolerance: i16,
) -> bool {
    if (i16::from(actual[3]) - 255).abs() > tolerance {
        return false;
    }

    let alpha = expected[3];
    channel_matches_transparent_reference(actual[0], expected[0], alpha, tolerance)
        && channel_matches_transparent_reference(actual[1], expected[1], alpha, tolerance)
        && channel_matches_transparent_reference(actual[2], expected[2], alpha, tolerance)
}

fn channel_matches_transparent_reference(
    actual: u8,
    foreground: u8,
    alpha: u8,
    tolerance: i16,
) -> bool {
    if alpha == 0 {
        return true;
    }

    let alpha = f64::from(alpha) / 255.0;
    let inverse = 1.0 - alpha;
    let minimum = composite_channel(foreground, 0, alpha, inverse);
    let maximum = composite_channel(foreground, 255, alpha, inverse);
    let actual = i16::from(actual);
    actual >= minimum - tolerance && actual <= maximum + tolerance
}

fn composite_channel(foreground: u8, background: u8, alpha: f64, inverse: f64) -> i16 {
    (f64::from(foreground) * alpha + f64::from(background) * inverse).round() as i16
}
