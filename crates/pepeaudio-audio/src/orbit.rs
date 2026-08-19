use pepeaudio_hrir::VirtualDirection;

use crate::{DspError, transition::equal_power_weights_finite};

const REAR_WRAP_START_DEGREES: f32 = 150.0;
const REAR_WRAP_END_DEGREES: f32 = 210.0;

const SORTED_ANCHORS: [(f32, VirtualDirection); 7] = [
    (-150.0, VirtualDirection::BackRight),
    (-90.0, VirtualDirection::SideRight),
    (-30.0, VirtualDirection::FrontRight),
    (0.0, VirtualDirection::FrontCenter),
    (30.0, VirtualDirection::FrontLeft),
    (90.0, VirtualDirection::SideLeft),
    (150.0, VirtualDirection::BackLeft),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionBlend {
    pub first: VirtualDirection,
    pub second: VirtualDirection,
    pub first_gain: f32,
    pub second_gain: f32,
}

/// Normalizes a finite angle to `[-180, 180]` degrees.
///
/// # Errors
///
/// Returns an error for NaN or infinity.
pub fn normalize_azimuth(degrees: f32) -> Result<f32, DspError> {
    if !degrees.is_finite() {
        return Err(DspError::InvalidAzimuth { actual: degrees });
    }
    Ok(normalize_finite_azimuth(degrees))
}

/// The seven anchors are FC 0°, FL/FR ±30°, SL/SR ±90°, and BL/BR
/// ±150°. The remaining rear interval wraps from BL +150° through ±180°
/// to BR -150°. This is a horizontal approximation only; it has no elevation.
///
/// # Errors
///
/// Returns an error for NaN or infinity.
pub fn blend_for_azimuth(degrees: f32) -> Result<DirectionBlend, DspError> {
    let azimuth = normalize_azimuth(degrees)?;
    if !(-150.0..150.0).contains(&azimuth) {
        let unwrapped = if azimuth < -150.0 {
            azimuth + 360.0
        } else {
            azimuth
        };
        return Ok(make_blend(
            VirtualDirection::BackLeft,
            VirtualDirection::BackRight,
            (unwrapped - REAR_WRAP_START_DEGREES)
                / (REAR_WRAP_END_DEGREES - REAR_WRAP_START_DEGREES),
        ));
    }

    for anchors in SORTED_ANCHORS.windows(2) {
        let (first_angle, first) = anchors[0];
        let (second_angle, second) = anchors[1];
        if (first_angle..second_angle).contains(&azimuth) {
            return Ok(make_blend(
                first,
                second,
                (azimuth - first_angle) / (second_angle - first_angle),
            ));
        }
    }

    // Exact +150° is handled by the rear interval; every other finite value
    // in [-150, 150) belongs to one of the windows above.
    Ok(make_blend(
        VirtualDirection::BackLeft,
        VirtualDirection::BackRight,
        0.0,
    ))
}

/// Positive azimuth points left/counter-clockwise. At the default center 0°
/// and width 60°, the left and right channels occupy FL +30° and FR -30°.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HorizontalStereoPair {
    center_degrees: f32,
    width_degrees: f32,
}

impl HorizontalStereoPair {
    /// Faithful fixed-front positions represented through the orbit renderer.
    pub const FRONT: Self = Self {
        center_degrees: 0.0,
        width_degrees: 60.0,
    };

    /// # Errors
    ///
    /// Returns an error for a non-finite center/width or width outside
    /// `0..=180` degrees.
    pub fn new(center_degrees: f32, width_degrees: f32) -> Result<Self, DspError> {
        let center_degrees = normalize_azimuth(center_degrees)?;
        if !width_degrees.is_finite() || !(0.0..=180.0).contains(&width_degrees) {
            return Err(DspError::InvalidStereoWidth {
                actual: width_degrees,
            });
        }
        Ok(Self {
            center_degrees,
            width_degrees,
        })
    }

    #[must_use]
    pub const fn center_degrees(self) -> f32 {
        self.center_degrees
    }

    #[must_use]
    pub const fn width_degrees(self) -> f32 {
        self.width_degrees
    }

    #[must_use]
    pub fn left_degrees(self) -> f32 {
        normalize_finite_azimuth(self.center_degrees + self.width_degrees * 0.5)
    }

    #[must_use]
    pub fn right_degrees(self) -> f32 {
        normalize_finite_azimuth(self.center_degrees - self.width_degrees * 0.5)
    }
}

fn normalize_finite_azimuth(degrees: f32) -> f32 {
    let normalized = degrees.rem_euclid(360.0);
    if normalized > 180.0 {
        normalized - 360.0
    } else {
        normalized
    }
}

fn make_blend(first: VirtualDirection, second: VirtualDirection, progress: f32) -> DirectionBlend {
    let (first_gain, second_gain) = equal_power_weights_finite(progress);
    DirectionBlend {
        first,
        second,
        first_gain,
        second_gain,
    }
}
