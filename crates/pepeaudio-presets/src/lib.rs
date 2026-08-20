//! Startup-time catalog for operator-installed `HeSuVi` HRIR assets.
//!
//! Files are parsed and prepared before they enter the realtime playback path.
//! The catalog retains only immutable 48 kHz coefficients and public metadata;
//! audio workers never perform filesystem I/O.

#![forbid(unsafe_code)]

mod catalog;
mod error;
mod limits;
mod metadata;

pub use catalog::{HrirCatalog, HrirDescriptor};
pub use error::{CatalogError, CatalogResult};
pub use limits::CatalogLimits;
