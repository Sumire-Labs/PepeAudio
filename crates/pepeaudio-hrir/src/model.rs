/// Stable order used by [`HesuviPreset::pairs`].
pub const ALL_DIRECTIONS: [VirtualDirection; 7] = [
    VirtualDirection::FrontLeft,
    VirtualDirection::FrontRight,
    VirtualDirection::FrontCenter,
    VirtualDirection::BackLeft,
    VirtualDirection::BackRight,
    VirtualDirection::SideLeft,
    VirtualDirection::SideRight,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VirtualDirection {
    FrontLeft = 0,
    FrontRight = 1,
    FrontCenter = 2,
    BackLeft = 3,
    BackRight = 4,
    SideLeft = 5,
    SideRight = 6,
}

impl VirtualDirection {
    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HesuviSampleRate {
    Hz44100,
    Hz48000,
}

impl HesuviSampleRate {
    #[must_use]
    pub const fn as_hz(self) -> u32 {
        match self {
            Self::Hz44100 => 44_100,
            Self::Hz48000 => 48_000,
        }
    }
}

/// Representation used by the source file before normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceLayout {
    /// Seven stored channels; right-side directions were mirrored on load.
    SevenChannelMirrored,
    /// Fourteen independent speaker-ear impulse responses.
    FourteenChannelIndependent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HrirPair {
    left_ear: Box<[f32]>,
    right_ear: Box<[f32]>,
}

impl HrirPair {
    pub(crate) fn from_planes(left_ear: &[f32], right_ear: &[f32]) -> Self {
        Self {
            left_ear: left_ear.into(),
            right_ear: right_ear.into(),
        }
    }

    #[must_use]
    pub fn left_ear(&self) -> &[f32] {
        &self.left_ear
    }

    #[must_use]
    pub fn right_ear(&self) -> &[f32] {
        &self.right_ear
    }

    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.left_ear.len()
    }
}

/// A validated `HeSuVi` HRIR normalized to seven `(left ear, right ear)` pairs.
///
/// No path, display name, content hash, or storage identity is retained. The
/// caller must assign those concepts in a higher layer.
#[derive(Debug, Clone, PartialEq)]
pub struct HesuviPreset {
    sample_rate: HesuviSampleRate,
    source_layout: SourceLayout,
    frame_count: usize,
    pairs: [HrirPair; 7],
}

impl HesuviPreset {
    pub(crate) const fn new(
        sample_rate: HesuviSampleRate,
        source_layout: SourceLayout,
        frame_count: usize,
        pairs: [HrirPair; 7],
    ) -> Self {
        Self {
            sample_rate,
            source_layout,
            frame_count,
            pairs,
        }
    }

    #[must_use]
    pub const fn sample_rate(&self) -> HesuviSampleRate {
        self.sample_rate
    }

    #[must_use]
    pub const fn source_layout(&self) -> SourceLayout {
        self.source_layout
    }

    #[must_use]
    pub const fn frame_count(&self) -> usize {
        self.frame_count
    }

    #[must_use]
    pub fn pair(&self, direction: VirtualDirection) -> &HrirPair {
        &self.pairs[direction.index()]
    }

    /// All direction pairs in [`ALL_DIRECTIONS`] order.
    #[must_use]
    pub const fn pairs(&self) -> &[HrirPair; 7] {
        &self.pairs
    }
}
