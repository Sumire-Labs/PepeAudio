use std::{net::SocketAddr, num::NonZeroU64, path::PathBuf, str::FromStr};

use url::Url;

use crate::{ConfigError, ConfigResult};

pub(crate) fn http_url(name: &'static str, value: &str) -> ConfigResult<Url> {
    let url = Url::parse(value).map_err(|_| invalid(name, "must be an absolute HTTP(S) URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(invalid(name, "must be an absolute HTTP(S) URL"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(name, "must not contain user credentials"));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(invalid(name, "must not contain a query or fragment"));
    }
    Ok(url)
}

pub(crate) fn connection_url(
    name: &'static str,
    value: &str,
    schemes: &[&str],
    require_database_name: bool,
) -> ConfigResult<()> {
    let url = Url::parse(value).map_err(|_| invalid(name, "must be an absolute connection URL"))?;
    if !schemes.contains(&url.scheme()) {
        return Err(invalid(name, "uses an unsupported URL scheme"));
    }
    if url.host_str().is_none() {
        return Err(invalid(name, "must include a host"));
    }
    if url.fragment().is_some() {
        return Err(invalid(name, "must not contain a fragment"));
    }
    if require_database_name && url.path().trim_matches('/').is_empty() {
        return Err(invalid(name, "must include a database name"));
    }
    Ok(())
}

pub(crate) fn socket_addr(name: &'static str, value: &str) -> ConfigResult<SocketAddr> {
    SocketAddr::from_str(value).map_err(|_| invalid(name, "must be an IP address and port"))
}

pub(crate) fn nonzero_u64(name: &'static str, value: &str) -> ConfigResult<NonZeroU64> {
    value
        .parse::<NonZeroU64>()
        .map_err(|_| invalid(name, "must be a non-zero unsigned integer"))
}

pub(crate) fn instance_id(name: &'static str, value: String) -> ConfigResult<String> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(value)
    } else {
        Err(invalid(
            name,
            "must be 1-64 ASCII letters, digits, dots, underscores, or hyphens",
        ))
    }
}

pub(crate) fn keyspace(name: &'static str, value: String) -> ConfigResult<String> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-'));
    if valid {
        Ok(value)
    } else {
        Err(invalid(
            name,
            "must be 1-64 ASCII letters, digits, colons, underscores, or hyphens",
        ))
    }
}

pub(crate) fn nonempty_path(name: &'static str, value: String) -> ConfigResult<PathBuf> {
    if value.trim().is_empty() || value.contains('\0') {
        Err(invalid(name, "must be a non-empty filesystem path"))
    } else {
        Ok(PathBuf::from(value))
    }
}

pub(crate) const fn invalid(name: &'static str, reason: &'static str) -> ConfigError {
    ConfigError::Invalid { name, reason }
}
