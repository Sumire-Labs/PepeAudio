//! Minimal, typed Discord Components V2 payloads.
//!
//! Serenity 0.12 does not yet expose all Components V2 layout and content
//! types. This crate isolates that wire format so the rest of the bot can stay
//! on released Serenity, Poise, and Songbird versions.

mod error;
mod interactive;
mod layout;
mod limits;
mod message;

pub use error::ValidationError;
pub use interactive::{ActionRowComponent, ButtonComponent, SelectOption, StringSelectComponent};
pub use layout::{
    Component, ContainerComponent, SectionComponent, SeparatorComponent, TextDisplayComponent,
};
pub use message::Message;

/// Discord's irreversible flag for component-only messages (`1 << 15`).
pub const IS_COMPONENTS_V2: u64 = 1 << 15;
