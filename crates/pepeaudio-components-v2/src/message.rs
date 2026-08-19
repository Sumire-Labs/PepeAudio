use std::collections::HashSet;

use serde::Serialize;

use crate::{Component, IS_COMPONENTS_V2, ValidationError, limits::MAX_COMPONENTS};

/// A message whose entire visible body is rendered using Components V2.
///
/// `content`, `embeds`, polls, and stickers intentionally do not exist on this
/// type because Discord rejects them when [`IS_COMPONENTS_V2`] is set.
#[derive(Clone, Debug, Serialize)]
pub struct Message {
    flags: u64,
    allowed_mentions: AllowedMentions,
    components: Vec<Component>,
}

impl Message {
    /// # Errors
    ///
    /// Returns [`ValidationError`] when the component tree violates a Discord
    /// Components V2 structural or size constraint modeled by this crate.
    pub fn new(components: Vec<Component>) -> Result<Self, ValidationError> {
        Self::with_flags(components, 0)
    }

    /// # Errors
    ///
    /// Returns [`ValidationError`] for the same structural limits as [`Self::new`].
    pub fn ephemeral(components: Vec<Component>) -> Result<Self, ValidationError> {
        const EPHEMERAL: u64 = 1 << 6;
        Self::with_flags(components, EPHEMERAL)
    }

    fn with_flags(
        components: Vec<Component>,
        additional_flags: u64,
    ) -> Result<Self, ValidationError> {
        let message = Self {
            flags: IS_COMPONENTS_V2 | additional_flags,
            allowed_mentions: AllowedMentions::default(),
            components,
        };
        message.validate()?;
        Ok(message)
    }

    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    /// # Errors
    ///
    /// Returns [`ValidationError`] when the component tree is not valid.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.components.is_empty() {
            return Err(ValidationError::EmptyMessage);
        }

        let component_count = self.components.iter().map(Component::count_tree).sum();
        if component_count > MAX_COMPONENTS {
            return Err(ValidationError::TooManyComponents {
                actual: component_count,
                maximum: MAX_COMPONENTS,
            });
        }

        for component in &self.components {
            component.validate()?;
        }

        let mut custom_ids = HashSet::new();
        for component in &self.components {
            component.collect_custom_ids(&mut custom_ids)?;
        }
        Ok(())
    }
}

/// Explicitly disables every form of mention parsing for untrusted metadata.
#[derive(Clone, Debug, Default, Serialize)]
struct AllowedMentions {
    parse: Vec<String>,
    roles: Vec<String>,
    users: Vec<String>,
    replied_user: bool,
}
