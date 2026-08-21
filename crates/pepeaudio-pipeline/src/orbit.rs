use pepeaudio_audio::HorizontalStereoPair;

/// Stable horizontal position shared by the active and pending HRIR renderers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialPosition {
    position: HorizontalStereoPair,
}

impl SpatialPosition {
    pub(crate) const fn new(position: HorizontalStereoPair) -> Self {
        Self { position }
    }

    pub(crate) const fn position(self) -> HorizontalStereoPair {
        self.position
    }

    pub(crate) fn rebase(&mut self, position: HorizontalStereoPair) {
        self.position = position;
    }
}

#[cfg(test)]
mod tests {
    use super::SpatialPosition;
    use pepeaudio_audio::HorizontalStereoPair;

    #[test]
    fn position_is_stable_until_explicitly_rebased() {
        let mut position = SpatialPosition::new(HorizontalStereoPair::FRONT);
        assert_eq!(position.position(), HorizontalStereoPair::FRONT);
        let right = HorizontalStereoPair::new(-90.0, 60.0).expect("position");
        position.rebase(right);
        assert_eq!(position.position(), right);
    }
}
