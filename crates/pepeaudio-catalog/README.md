# pepeaudio-catalog

Metadata adapters for copied Spotify and Apple Music links. This crate never
downloads audio and never returns a provider stream URL. It produces bounded,
provider-neutral search metadata for the separate YouTube/SoundCloud resolver.

Cross-service matching is an explicit, default-off application feature. Using
these APIs does not by itself make cross-service matching compliant with a
provider's current terms; deployments must review the terms for their use case
and keep source attribution visible.

## Supported catalog access

- Spotify track and album metadata use the client credentials flow.
- Spotify playlist links are recognized so the application can return a clear
  `SpotifyPlaylistAccessDenied` error. The current playlist-items API requires
  a user token for the owner or a collaborator; this project intentionally has
  no Spotify user OAuth flow. Client credentials therefore support tracks and
  albums only, not arbitrary or public playlists.
- Apple Music songs, albums, and catalog playlists use an ES256 developer
  token. Apple Music user-library playlists are not supported.
- Short-link scraping, browser cookies, and user tokens are not accepted.

Albums and playlists process 25 source items by default. Applications may
choose a limit from 1 through the hard maximum of 100. Results say when the
source was truncated or contained unsupported items.

```rust,no_run
use pepeaudio_catalog::{CatalogResolverBuilder, SpotifyCatalog};
use url::Url;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let spotify = SpotifyCatalog::new("client-id", "client-secret", "JP")?;
let resolver = CatalogResolverBuilder::new()
    .spotify(spotify)
    .build();
let source = Url::parse(
    "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
)?;
let metadata = resolver.resolve_url(&source).await?;
let search_requests = metadata.search_requests();
# Ok(())
# }
```

Current API behavior is based on the official
[Spotify client credentials guide](https://developer.spotify.com/documentation/web-api/tutorials/client-credentials-flow),
[Spotify playlist-items reference](https://developer.spotify.com/documentation/web-api/reference/get-playlists-items),
[Apple developer-token guide](https://developer.apple.com/documentation/applemusicapi/generating-developer-tokens),
and [Apple catalog request documentation](https://developer.apple.com/documentation/applemusicapi/handling-requests-and-responses).
