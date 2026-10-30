//! Gaussian blur approximated by three box blurs (for soft shadows). cairo has no blur.

/// Box widths whose three passes approximate a Gaussian with `sigma`
/// (Kovesi, "Fast Almost-Gaussian Filtering").
fn box_radii(sigma: f64) -> [usize; 3] {
    const N: f64 = 3.0;
    let ideal = (12.0 * sigma * sigma / N + 1.0).sqrt();
    let mut lower = ideal.floor() as i64;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let upper = lower + 2;
    let l = lower as f64;
    let m = ((12.0 * sigma * sigma - N * l * l - 4.0 * N * l - 3.0 * N) / (-4.0 * l - 4.0)).round();
    let mut radii = [0; 3];
    for (i, r) in radii.iter_mut().enumerate() {
        let width = if (i as f64) < m { lower } else { upper };
        *r = ((width - 1) / 2) as usize;
    }
    radii
}

/// One box pass along rows (`horizontal`) or columns. Pixels outside count as 0.
fn box_pass(
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
    stride: usize,
    radius: usize,
    horizontal: bool,
) {
    let (lines, len) = if horizontal {
        (height, width)
    } else {
        (width, height)
    };
    let at = |line: usize, i: usize| -> usize {
        if horizontal {
            line * stride + i
        } else {
            i * stride + line
        }
    };
    let span = (2 * radius + 1) as u32;
    for line in 0..lines {
        let mut sum: u32 = 0;
        for i in 0..radius.min(len) {
            sum += u32::from(src.get(at(line, i)).copied().unwrap_or(0));
        }
        for i in 0..len {
            if i + radius < len {
                sum += u32::from(src.get(at(line, i + radius)).copied().unwrap_or(0));
            }
            if let Some(out) = dst.get_mut(at(line, i)) {
                *out = ((sum + span / 2) / span) as u8;
            }
            if i >= radius {
                sum -= u32::from(src.get(at(line, i - radius)).copied().unwrap_or(0));
            }
        }
    }
}

/// Blurs an 8-bit single-channel image (cairo `A8`) in place.
pub fn blur_a8(data: &mut [u8], width: usize, height: usize, stride: usize, sigma: f64) {
    if sigma < 0.5 || width == 0 || height == 0 || data.len() < stride * height {
        return;
    }
    let mut scratch = data.to_vec();
    for radius in box_radii(sigma) {
        if radius == 0 {
            continue;
        }
        box_pass(data, &mut scratch, width, height, stride, radius, true);
        box_pass(&scratch, data, width, height, stride, radius, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sigma_is_a_no_op() {
        let mut data = vec![0, 255, 0, 0];
        blur_a8(&mut data, 4, 1, 4, 0.2);
        assert_eq!(data, vec![0, 255, 0, 0]);
    }

    #[test]
    fn blur_spreads_symmetrically_and_keeps_mass() {
        let (w, h) = (41, 41);
        let mut data = vec![0u8; w * h];
        for y in 10..31 {
            for x in 10..31 {
                data[y * w + x] = 255;
            }
        }
        let before: u32 = data.iter().map(|&v| u32::from(v)).sum();
        blur_a8(&mut data, w, h, w, 3.0);
        let after: u32 = data.iter().map(|&v| u32::from(v)).sum();
        // Rounding loses or gains a little per pass; mass stays within 2 %.
        assert!((f64::from(after) - f64::from(before)).abs() / f64::from(before) < 0.02);
        // Symmetric spread around the square's centre.
        assert_eq!(data[20 * w + 5], data[20 * w + 35]);
        assert_eq!(data[5 * w + 20], data[35 * w + 20]);
        // Soft edge: something leaked out, the centre stays solid.
        assert!(data[20 * w + 8] > 0);
        assert!(data[20 * w + 20] > 250);
        assert!(data[20 * w + 20] > data[20 * w + 10]);
    }

    #[test]
    fn radii_grow_with_sigma() {
        let small: usize = box_radii(1.0).iter().sum();
        let large: usize = box_radii(8.0).iter().sum();
        assert!(large > small);
    }
}
