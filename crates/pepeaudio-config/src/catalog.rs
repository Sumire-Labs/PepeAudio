use std::num::NonZeroU32;

use crate::{
    ConfigError, ConfigResult, ConfigSource, SecretString, load::secret, validate::invalid,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogConfig {
    pub cross_service_matching_enabled: bool,
    pub spotify_public_metadata_enabled: bool,
    pub apple_music_public_metadata_enabled: bool,
    pub max_items: NonZeroU32,
    pub spotify: Option<SpotifyCatalogConfig>,
    pub apple_music: Option<AppleMusicCatalogConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpotifyCatalogConfig {
    pub client_id: SecretString,
    pub client_secret: SecretString,
    pub market: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleMusicCatalogConfig {
    pub team_id: String,
    pub key_id: String,
    pub private_key: SecretString,
}

pub(crate) fn load_catalog(
    source: &impl ConfigSource,
    maximum_player_items: NonZeroU32,
) -> ConfigResult<CatalogConfig> {
    let enabled = optional_bool(source, "PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING", false)?;
    let configured_items = optional_bounded_u32(source, "PEPEAUDIO_CATALOG_MAX_ITEMS", 25, 1, 100)?;
    let max_items = NonZeroU32::new(configured_items.min(maximum_player_items.get()))
        .expect("both catalog bounds are non-zero");
    if !enabled {
        return Ok(CatalogConfig {
            cross_service_matching_enabled: false,
            spotify_public_metadata_enabled: false,
            apple_music_public_metadata_enabled: false,
            max_items,
            spotify: None,
            apple_music: None,
        });
    }

    let spotify_public_metadata_enabled =
        optional_bool(source, "PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA", false)?;
    let apple_music_public_metadata_enabled = optional_bool(
        source,
        "PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA",
        false,
    )?;
    let spotify = load_spotify(source)?;
    let apple_music = load_apple_music(source)?;
    if spotify.is_none()
        && apple_music.is_none()
        && !spotify_public_metadata_enabled
        && !apple_music_public_metadata_enabled
    {
        return Err(ConfigError::Inconsistent {
            reason: "cross-service matching requires an explicitly enabled catalog provider",
        });
    }
    Ok(CatalogConfig {
        cross_service_matching_enabled: true,
        spotify_public_metadata_enabled,
        apple_music_public_metadata_enabled,
        max_items,
        spotify,
        apple_music,
    })
}

fn load_spotify(source: &impl ConfigSource) -> ConfigResult<Option<SpotifyCatalogConfig>> {
    let client_id = source.get("PEPEAUDIO_SPOTIFY_CLIENT_ID")?;
    let direct_secret = source.get("PEPEAUDIO_SPOTIFY_CLIENT_SECRET")?;
    let file_secret = source.get("PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE")?;
    if client_id.is_none() && direct_secret.is_none() && file_secret.is_none() {
        return Ok(None);
    }
    let client_id = client_id.ok_or(ConfigError::Inconsistent {
        reason: "Spotify catalog configuration is incomplete",
    })?;
    if client_id.trim().is_empty()
        || client_id.len() > 256
        || client_id.chars().any(char::is_control)
    {
        return Err(invalid(
            "PEPEAUDIO_SPOTIFY_CLIENT_ID",
            "must be a non-empty identifier of at most 256 bytes",
        ));
    }
    if direct_secret.is_none() && file_secret.is_none() {
        return Err(ConfigError::Inconsistent {
            reason: "Spotify catalog configuration is incomplete",
        });
    }
    let client_secret = secret(
        source,
        "PEPEAUDIO_SPOTIFY_CLIENT_SECRET",
        "PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE",
        1,
    )?;
    if client_secret.expose_secret().len() > 512 {
        return Err(invalid(
            "PEPEAUDIO_SPOTIFY_CLIENT_SECRET",
            "must not exceed 512 bytes",
        ));
    }
    let market = source
        .get("PEPEAUDIO_SPOTIFY_MARKET")?
        .unwrap_or_else(|| "JP".to_owned());
    if market.len() != 2 || !market.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(invalid(
            "PEPEAUDIO_SPOTIFY_MARKET",
            "must be a two-letter uppercase market",
        ));
    }
    Ok(Some(SpotifyCatalogConfig {
        client_id: SecretString::new(client_id),
        client_secret,
        market,
    }))
}

fn load_apple_music(source: &impl ConfigSource) -> ConfigResult<Option<AppleMusicCatalogConfig>> {
    let team_id = source.get("PEPEAUDIO_APPLE_MUSIC_TEAM_ID")?;
    let key_id = source.get("PEPEAUDIO_APPLE_MUSIC_KEY_ID")?;
    let direct_key = source.get("PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY")?;
    let file_key = source.get("PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_FILE")?;
    if team_id.is_none() && key_id.is_none() && direct_key.is_none() && file_key.is_none() {
        return Ok(None);
    }
    let team_id = apple_id(team_id, "PEPEAUDIO_APPLE_MUSIC_TEAM_ID")?;
    let key_id = apple_id(key_id, "PEPEAUDIO_APPLE_MUSIC_KEY_ID")?;
    if direct_key.is_none() && file_key.is_none() {
        return Err(ConfigError::Inconsistent {
            reason: "Apple Music catalog configuration is incomplete",
        });
    }
    let private_key = secret(
        source,
        "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY",
        "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_FILE",
        64,
    )?;
    if private_key.expose_secret().len() > 16 * 1024 {
        return Err(invalid(
            "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY",
            "must not exceed 16384 bytes",
        ));
    }
    Ok(Some(AppleMusicCatalogConfig {
        team_id,
        key_id,
        private_key,
    }))
}

fn apple_id(value: Option<String>, name: &'static str) -> ConfigResult<String> {
    let value = value.ok_or(ConfigError::Inconsistent {
        reason: "Apple Music catalog configuration is incomplete",
    })?;
    if value.len() != 10 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(invalid(
            name,
            "must be a 10-character alphanumeric identifier",
        ));
    }
    Ok(value)
}

fn optional_bool(
    source: &impl ConfigSource,
    name: &'static str,
    default: bool,
) -> ConfigResult<bool> {
    match source.get(name)?.as_deref() {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(invalid(name, "must be exactly true or false")),
    }
}

fn optional_bounded_u32(
    source: &impl ConfigSource,
    name: &'static str,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> ConfigResult<u32> {
    let value = source
        .get(name)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| invalid(name, "has an invalid value"))
        })
        .transpose()?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(invalid(name, "is outside the allowed range"));
    }
    Ok(value)
}
