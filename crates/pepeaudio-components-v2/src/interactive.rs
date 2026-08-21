use std::collections::HashSet;

use serde::Serialize;

use crate::{
    ValidationError,
    limits::{
        ACTION_ROW, BUTTON, DANGER_BUTTON, LINK_BUTTON, MAX_BUTTON_LABEL_CHARS,
        MAX_BUTTON_URL_BYTES, MAX_BUTTONS_PER_ROW, MAX_CUSTOM_ID_CHARS, MAX_SELECT_OPTIONS,
        PRIMARY_BUTTON, SECONDARY_BUTTON, STRING_SELECT, SUCCESS_BUTTON,
    },
};

/// An action row containing either buttons or one string select.
#[derive(Clone, Debug, Serialize)]
pub struct ActionRowComponent {
    #[serde(rename = "type")]
    kind: u8,
    components: Vec<InteractiveComponent>,
}

impl ActionRowComponent {
    /// # Errors
    ///
    /// Returns [`ValidationError`] when the row or a button is invalid.
    pub fn buttons(buttons: Vec<ButtonComponent>) -> Result<Self, ValidationError> {
        if buttons.is_empty() || buttons.len() > MAX_BUTTONS_PER_ROW {
            return Err(ValidationError::InvalidButtonCount(buttons.len()));
        }
        let row = Self {
            kind: ACTION_ROW,
            components: buttons
                .into_iter()
                .map(InteractiveComponent::Button)
                .collect(),
        };
        row.validate()?;
        Ok(row)
    }

    /// # Errors
    ///
    /// Returns [`ValidationError`] when the select is invalid.
    pub fn select(select: StringSelectComponent) -> Result<Self, ValidationError> {
        let row = Self {
            kind: ACTION_ROW,
            components: vec![InteractiveComponent::StringSelect(select)],
        };
        row.validate()?;
        Ok(row)
    }

    pub(crate) fn child_count(&self) -> usize {
        self.components.len()
    }

    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        match self.components.as_slice() {
            [] => Err(ValidationError::EmptyActionRow),
            [InteractiveComponent::StringSelect(select)] => select.validate(),
            components
                if components.len() <= MAX_BUTTONS_PER_ROW
                    && components
                        .iter()
                        .all(|component| matches!(component, InteractiveComponent::Button(_))) =>
            {
                for component in components {
                    if let InteractiveComponent::Button(button) = component {
                        button.validate()?;
                    }
                }
                Ok(())
            }
            _ => Err(ValidationError::MixedActionRow),
        }
    }

    pub(crate) fn collect_custom_ids(
        &self,
        custom_ids: &mut HashSet<String>,
    ) -> Result<(), ValidationError> {
        for component in &self.components {
            let custom_id = match component {
                InteractiveComponent::Button(button) => button.custom_id.as_ref(),
                InteractiveComponent::StringSelect(select) => Some(&select.custom_id),
            };
            if let Some(custom_id) = custom_id
                && !custom_ids.insert(custom_id.clone())
            {
                return Err(ValidationError::DuplicateCustomId(custom_id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum InteractiveComponent {
    Button(ButtonComponent),
    StringSelect(StringSelectComponent),
}

#[derive(Clone, Debug, Serialize)]
pub struct ButtonComponent {
    #[serde(rename = "type")]
    kind: u8,
    style: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    label: String,
    disabled: bool,
}

impl ButtonComponent {
    #[must_use]
    pub fn neutral(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::action(custom_id, label, SECONDARY_BUTTON)
    }

    #[must_use]
    pub fn primary(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::action(custom_id, label, PRIMARY_BUTTON)
    }

    #[must_use]
    pub fn success(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::action(custom_id, label, SUCCESS_BUTTON)
    }

    #[must_use]
    pub fn danger(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::action(custom_id, label, DANGER_BUTTON)
    }

    fn action(custom_id: impl Into<String>, label: impl Into<String>, style: u8) -> Self {
        Self {
            kind: BUTTON,
            style,
            custom_id: Some(custom_id.into()),
            url: None,
            label: label.into(),
            disabled: false,
        }
    }

    /// Creates a link-style button after validating its public HTTPS URL.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] for an unsafe URL or invalid label.
    pub fn link(url: impl Into<String>, label: impl Into<String>) -> Result<Self, ValidationError> {
        let button = Self {
            kind: BUTTON,
            style: LINK_BUTTON,
            custom_id: None,
            url: Some(url.into()),
            label: label.into(),
            disabled: false,
        };
        button.validate()?;
        Ok(button)
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn validate(&self) -> Result<(), ValidationError> {
        match (&self.custom_id, &self.url) {
            (Some(custom_id), None)
                if matches!(
                    self.style,
                    PRIMARY_BUTTON | SECONDARY_BUTTON | SUCCESS_BUTTON | DANGER_BUTTON
                ) =>
            {
                validate_custom_id(custom_id)?;
            }
            (None, Some(url)) if self.style == LINK_BUTTON && valid_link_url(url) => {}
            _ => return Err(ValidationError::InvalidButtonUrl),
        }
        if self.label.is_empty() {
            return Err(ValidationError::EmptyButtonLabel);
        }
        if self.label.chars().count() > MAX_BUTTON_LABEL_CHARS {
            return Err(ValidationError::ButtonLabelTooLong);
        }
        Ok(())
    }
}

fn valid_link_url(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_BUTTON_URL_BYTES || value.chars().any(char::is_control)
    {
        return false;
    }
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct StringSelectComponent {
    #[serde(rename = "type")]
    kind: u8,
    custom_id: String,
    options: Vec<SelectOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    placeholder: Option<String>,
    min_values: u8,
    max_values: u8,
    disabled: bool,
}

impl StringSelectComponent {
    /// # Errors
    ///
    /// Returns [`ValidationError`] when its ID, options, placeholder, or
    /// default selection is invalid.
    pub fn single(
        custom_id: impl Into<String>,
        options: Vec<SelectOption>,
        placeholder: Option<String>,
    ) -> Result<Self, ValidationError> {
        let select = Self {
            kind: STRING_SELECT,
            custom_id: custom_id.into(),
            options,
            placeholder,
            min_values: 1,
            max_values: 1,
            disabled: false,
        };
        select.validate()?;
        Ok(select)
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn validate(&self) -> Result<(), ValidationError> {
        validate_custom_id(&self.custom_id)?;
        if self.options.is_empty() || self.options.len() > MAX_SELECT_OPTIONS {
            return Err(ValidationError::InvalidSelectOptionCount(
                self.options.len(),
            ));
        }
        if self
            .placeholder
            .as_ref()
            .is_some_and(|value| value.chars().count() > 150)
        {
            return Err(ValidationError::SelectPlaceholderTooLong);
        }
        for option in &self.options {
            option.validate()?;
        }
        if self.options.iter().filter(|option| option.selected).count() > 1 {
            return Err(ValidationError::MultipleDefaultSelectOptions);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SelectOption {
    label: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "default")]
    selected: bool,
}

impl SelectOption {
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: None,
            selected: false,
        }
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.label.is_empty() || self.value.is_empty() {
            return Err(ValidationError::EmptySelectOption);
        }
        if self.label.chars().count() > 100
            || self.value.chars().count() > 100
            || self
                .description
                .as_ref()
                .is_some_and(|value| value.chars().count() > 100)
        {
            return Err(ValidationError::SelectOptionTooLong);
        }
        Ok(())
    }
}

fn validate_custom_id(custom_id: &str) -> Result<(), ValidationError> {
    let length = custom_id.chars().count();
    if !(1..=MAX_CUSTOM_ID_CHARS).contains(&length) {
        return Err(ValidationError::InvalidCustomIdLength(length));
    }
    Ok(())
}
