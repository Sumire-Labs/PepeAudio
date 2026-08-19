use std::collections::HashSet;

use serde::Serialize;

use crate::{
    ActionRowComponent, ButtonComponent, StringSelectComponent, ValidationError,
    limits::{CONTAINER, SECTION, SEPARATOR, TEXT_DISPLAY, THUMBNAIL, is_false},
};

/// A supported top-level or container child component.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Component {
    ActionRow(ActionRowComponent),
    Section(SectionComponent),
    TextDisplay(TextDisplayComponent),
    Separator(SeparatorComponent),
    Container(ContainerComponent),
}

impl Component {
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::TextDisplay(TextDisplayComponent::new(content))
    }

    #[must_use]
    pub fn separator() -> Self {
        Self::Separator(SeparatorComponent::default())
    }

    /// # Errors
    ///
    /// Returns [`ValidationError`] when the row is invalid.
    pub fn buttons(buttons: Vec<ButtonComponent>) -> Result<Self, ValidationError> {
        Ok(Self::ActionRow(ActionRowComponent::buttons(buttons)?))
    }

    /// # Errors
    ///
    /// Returns [`ValidationError`] when the select is invalid.
    pub fn select(select: StringSelectComponent) -> Result<Self, ValidationError> {
        Ok(Self::ActionRow(ActionRowComponent::select(select)?))
    }

    #[must_use]
    pub fn container(children: Vec<Self>) -> Self {
        Self::Container(ContainerComponent::new(children))
    }

    pub(crate) fn count_tree(&self) -> usize {
        match self {
            Self::ActionRow(row) => 1 + row.child_count(),
            Self::Section(section) => 1 + section.text.len() + 1,
            Self::Container(container) => {
                1 + container
                    .components
                    .iter()
                    .map(Self::count_tree)
                    .sum::<usize>()
            }
            Self::TextDisplay(_) | Self::Separator(_) => 1,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::ActionRow(row) => row.validate(),
            Self::Section(section) => section.validate(),
            Self::TextDisplay(text) => text.validate(),
            Self::Separator(_) => Ok(()),
            Self::Container(container) => container.validate(),
        }
    }

    pub(crate) fn collect_custom_ids(
        &self,
        custom_ids: &mut HashSet<String>,
    ) -> Result<(), ValidationError> {
        match self {
            Self::ActionRow(row) => row.collect_custom_ids(custom_ids),
            Self::Container(container) => {
                for component in &container.components {
                    component.collect_custom_ids(custom_ids)?;
                }
                Ok(())
            }
            Self::Section(_) | Self::TextDisplay(_) | Self::Separator(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TextDisplayComponent {
    #[serde(rename = "type")]
    kind: u8,
    content: String,
}

impl TextDisplayComponent {
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            kind: TEXT_DISPLAY,
            content: content.into(),
        }
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.content.is_empty() {
            return Err(ValidationError::EmptyTextDisplay);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ContainerComponent {
    #[serde(rename = "type")]
    kind: u8,
    components: Vec<Component>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accent_color: Option<u32>,
    #[serde(skip_serializing_if = "is_false")]
    spoiler: bool,
}

impl ContainerComponent {
    #[must_use]
    pub fn new(components: Vec<Component>) -> Self {
        Self {
            kind: CONTAINER,
            components,
            accent_color: None,
            spoiler: false,
        }
    }

    #[must_use]
    pub fn children(&self) -> &[Component] {
        &self.components
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.components.is_empty() {
            return Err(ValidationError::EmptyContainer);
        }
        for component in &self.components {
            if matches!(component, Component::Container(_)) {
                return Err(ValidationError::NestedContainer);
            }
            component.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SectionComponent {
    #[serde(rename = "type")]
    kind: u8,
    #[serde(rename = "components")]
    text: Vec<TextDisplayComponent>,
    accessory: SectionAccessory,
}

impl SectionComponent {
    /// # Errors
    ///
    /// Returns [`ValidationError`] when its text or media fields are invalid.
    pub fn with_thumbnail(
        text: Vec<TextDisplayComponent>,
        image_url: impl Into<String>,
        description: Option<String>,
    ) -> Result<Self, ValidationError> {
        let section = Self {
            kind: SECTION,
            text,
            accessory: SectionAccessory::Thumbnail(ThumbnailComponent::new(image_url, description)),
        };
        section.validate()?;
        Ok(section)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if !(1..=3).contains(&self.text.len()) {
            return Err(ValidationError::InvalidSectionTextCount(self.text.len()));
        }
        for text in &self.text {
            text.validate()?;
        }
        self.accessory.validate()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum SectionAccessory {
    Thumbnail(ThumbnailComponent),
}

impl SectionAccessory {
    fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Thumbnail(thumbnail) => thumbnail.validate(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct ThumbnailComponent {
    #[serde(rename = "type")]
    kind: u8,
    media: UnfurledMediaItem,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    spoiler: bool,
}

impl ThumbnailComponent {
    fn new(url: impl Into<String>, description: Option<String>) -> Self {
        Self {
            kind: THUMBNAIL,
            media: UnfurledMediaItem { url: url.into() },
            description,
            spoiler: false,
        }
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.media.url.is_empty() {
            return Err(ValidationError::EmptyMediaUrl);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
struct UnfurledMediaItem {
    url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeparatorComponent {
    #[serde(rename = "type")]
    kind: u8,
    divider: bool,
    spacing: u8,
}

impl Default for SeparatorComponent {
    fn default() -> Self {
        Self {
            kind: SEPARATOR,
            divider: true,
            spacing: 1,
        }
    }
}
