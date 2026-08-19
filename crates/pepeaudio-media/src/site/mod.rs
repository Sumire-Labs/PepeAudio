mod client;
mod command;
mod matching;
mod model;
mod parse;
#[cfg(test)]
mod tests;

pub use client::YtDlpClient;
pub use model::{
    SiteCollection, SiteError, SiteProvider, SiteReference, SiteResolvedTrack, SiteSearch,
    YtDlpConfig,
};
