use std::f64::consts::PI;

use crate::DspError;

/// Internal and Discord Voice sample rate.
pub const OUTPUT_SAMPLE_RATE_HZ: u32 = 48_000;

const SOURCE_RATE_HZ: usize = 44_100;
const TARGET_RATE_HZ: usize = 48_000;
const GCD_HZ: usize = 300;
const RATIO_NUMERATOR: usize = TARGET_RATE_HZ / GCD_HZ;
const RATIO_DENOMINATOR: usize = SOURCE_RATE_HZ / GCD_HZ;
const LANCZOS_RADIUS: i32 = 16;
const RATIO_NUMERATOR_F64: f64 = 160.0;
const LANCZOS_RADIUS_F64: f64 = 16.0;

/// Every ear and direction uses this exact stateless transform. No plane is
/// trimmed, aligned, or normalized independently, preserving their relative
/// time origin and level relationship.
pub(crate) fn resample_44_1_to_48(input: &[f32]) -> Result<Box<[f32]>, DspError> {
    let scaled = input
        .len()
        .checked_mul(RATIO_NUMERATOR)
        .ok_or(DspError::ResampleLengthOverflow)?;
    let output_len = scaled
        .checked_add(RATIO_DENOMINATOR / 2)
        .ok_or(DspError::ResampleLengthOverflow)?
        / RATIO_DENOMINATOR;
    let output_len = output_len.max(1);

    let mut output = Vec::with_capacity(output_len);
    for output_index in 0..output_len {
        let position_numerator = output_index
            .checked_mul(RATIO_DENOMINATOR)
            .ok_or(DspError::ResampleLengthOverflow)?;
        let center = position_numerator / RATIO_NUMERATOR;
        let fraction_numerator = position_numerator % RATIO_NUMERATOR;
        let fraction_numerator =
            u16::try_from(fraction_numerator).map_err(|_| DspError::ResampleLengthOverflow)?;
        let fraction = f64::from(fraction_numerator) / RATIO_NUMERATOR_F64;
        output.push(interpolate(input, center, fraction));
    }
    Ok(output.into_boxed_slice())
}

#[allow(clippy::cast_possible_truncation)]
fn interpolate(input: &[f32], center: usize, fraction: f64) -> f32 {
    let center = i64::try_from(center).unwrap_or(i64::MAX);
    let mut weighted_sum = 0.0_f64;
    let mut weight_sum = 0.0_f64;

    for offset in (-LANCZOS_RADIUS + 1)..=LANCZOS_RADIUS {
        let source_index = center.saturating_add(i64::from(offset));
        let Ok(source_index) = usize::try_from(source_index) else {
            continue;
        };
        let Some(&sample) = input.get(source_index) else {
            continue;
        };
        let distance = f64::from(offset) - fraction;
        let weight = lanczos_weight(distance);
        weighted_sum += f64::from(sample) * weight;
        weight_sum += weight;
    }

    if weight_sum.abs() <= f64::EPSILON {
        0.0
    } else {
        // Coefficients and inputs are bounded; the output is intentionally the
        // crate's f32 realtime sample format.
        (weighted_sum / weight_sum) as f32
    }
}

fn lanczos_weight(distance: f64) -> f64 {
    if distance.abs() <= f64::EPSILON {
        return 1.0;
    }
    if distance.abs() >= LANCZOS_RADIUS_F64 {
        return 0.0;
    }
    sinc(distance) * sinc(distance / LANCZOS_RADIUS_F64)
}

fn sinc(value: f64) -> f64 {
    let radians = PI * value;
    radians.sin() / radians
}
