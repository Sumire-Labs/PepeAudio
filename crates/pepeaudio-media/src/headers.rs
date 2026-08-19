use std::{collections::BTreeMap, fmt};

use crate::{SiteError, SiteProvider};

const MAX_HEADER_VALUE_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafeHeaderName {
    UserAgent,
    Referer,
    Origin,
    Accept,
    AcceptLanguage,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct SafeHttpHeaders(Vec<(SafeHeaderName, String)>);

impl SafeHttpHeaders {
    /// Copies only the small request-header subset needed by direct provider
    /// media. Authentication material is rejected rather than ignored.
    pub(crate) fn from_ytdlp(
        raw: BTreeMap<String, String>,
        provider: SiteProvider,
    ) -> Result<Self, SiteError> {
        let mut headers = Vec::new();
        for (name, value) in raw {
            let normalized = name.trim().to_ascii_lowercase();
            if matches!(
                normalized.as_str(),
                "cookie" | "authorization" | "proxy-authorization" | "host"
            ) {
                return Err(SiteError::UnsafeHeader);
            }
            let Some(name) = allowed_name(&normalized) else {
                continue;
            };
            validate_value(&value)?;
            if matches!(name, SafeHeaderName::Referer | SafeHeaderName::Origin) {
                validate_provider_origin(&value, provider)?;
            }
            if headers.iter().any(|(existing, _)| *existing == name) {
                return Err(SiteError::UnsafeHeader);
            }
            headers.push((name, value));
        }
        Ok(Self(headers))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (SafeHeaderName, &str)> {
        self.0.iter().map(|(name, value)| (*name, value.as_str()))
    }
}

impl fmt::Debug for SafeHttpHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SafeHttpHeaders")
            .field("count", &self.0.len())
            .finish()
    }
}

fn allowed_name(name: &str) -> Option<SafeHeaderName> {
    match name {
        "user-agent" => Some(SafeHeaderName::UserAgent),
        "referer" => Some(SafeHeaderName::Referer),
        "origin" => Some(SafeHeaderName::Origin),
        "accept" => Some(SafeHeaderName::Accept),
        "accept-language" => Some(SafeHeaderName::AcceptLanguage),
        _ => None,
    }
}

fn validate_value(value: &str) -> Result<(), SiteError> {
    if value.is_empty()
        || value.len() > MAX_HEADER_VALUE_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(SiteError::UnsafeHeader);
    }
    Ok(())
}

fn validate_provider_origin(value: &str, provider: SiteProvider) -> Result<(), SiteError> {
    let url = url::Url::parse(value).map_err(|_| SiteError::UnsafeHeader)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || !provider.accepts_page_host(url.host_str().unwrap_or_default())
    {
        return Err(SiteError::UnsafeHeader);
    }
    Ok(())
}
