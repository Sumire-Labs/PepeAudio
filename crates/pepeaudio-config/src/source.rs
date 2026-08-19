use std::{collections::BTreeMap, env};

use crate::{ConfigError, ConfigResult};

/// Read-only source of configuration values.
pub trait ConfigSource {
    /// Reads one variable without interpreting its contents.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::NotUnicode`] when the value is not valid Unicode.
    fn get(&self, name: &'static str) -> ConfigResult<Option<String>>;
}

/// Process environment configuration source.
#[derive(Clone, Copy, Debug, Default)]
pub struct Environment;

impl ConfigSource for Environment {
    fn get(&self, name: &'static str) -> ConfigResult<Option<String>> {
        match env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => Err(ConfigError::NotUnicode { name }),
        }
    }
}

/// Deterministic map-backed source useful for tests and embedded runtimes.
#[derive(Clone, Debug, Default)]
pub struct MapSource(BTreeMap<&'static str, String>);

impl MapSource {
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    #[must_use]
    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.0.insert(name, value.into());
        self
    }

    pub fn insert(&mut self, name: &'static str, value: impl Into<String>) {
        self.0.insert(name, value.into());
    }

    pub fn remove(&mut self, name: &'static str) {
        self.0.remove(name);
    }
}

impl ConfigSource for MapSource {
    fn get(&self, name: &'static str) -> ConfigResult<Option<String>> {
        Ok(self.0.get(name).cloned())
    }
}
