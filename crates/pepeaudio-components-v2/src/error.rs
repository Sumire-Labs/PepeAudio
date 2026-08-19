use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("a Components V2 message requires at least one component")]
    EmptyMessage,
    #[error("a Components V2 message may contain at most {maximum} components; got {actual}")]
    TooManyComponents { actual: usize, maximum: usize },
    #[error("a text display may not be empty")]
    EmptyTextDisplay,
    #[error("a container may not be empty")]
    EmptyContainer,
    #[error("a container cannot contain another container")]
    NestedContainer,
    #[error("a section requires between one and three text displays; got {0}")]
    InvalidSectionTextCount(usize),
    #[error("a thumbnail URL may not be empty")]
    EmptyMediaUrl,
    #[error("an action row may not be empty")]
    EmptyActionRow,
    #[error("an action row must contain only buttons or exactly one select")]
    MixedActionRow,
    #[error("a button row requires between one and five buttons; got {0}")]
    InvalidButtonCount(usize),
    #[error("a button label may not be empty")]
    EmptyButtonLabel,
    #[error("a button label may not exceed 80 characters")]
    ButtonLabelTooLong,
    #[error("a link button requires a canonical HTTPS URL of at most 512 bytes")]
    InvalidButtonUrl,
    #[error("a component custom_id must be 1 to 100 characters; got {0}")]
    InvalidCustomIdLength(usize),
    #[error("component custom_id values must be unique within a message; duplicate: {0}")]
    DuplicateCustomId(String),
    #[error("a string select requires between one and 25 options; got {0}")]
    InvalidSelectOptionCount(usize),
    #[error("a string select placeholder may not exceed 150 characters")]
    SelectPlaceholderTooLong,
    #[error("a select option label and value may not be empty")]
    EmptySelectOption,
    #[error("a select option label, value, and description may not exceed 100 characters")]
    SelectOptionTooLong,
    #[error("a single-value string select cannot have more than one default option")]
    MultipleDefaultSelectOptions,
}
