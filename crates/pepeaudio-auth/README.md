# pepeaudio-auth

Production-oriented Discord OAuth2 and opaque browser sessions for PepeAudio.
This crate is intentionally separate from the generic API transport so OAuth
credentials and Valkey session details do not enter the domain model.

## Discord Developer Portal

Register the **exact** HTTPS callback URI passed to `DiscordOAuthConfig::new`,
for example:

```text
https://audio.example.com/auth/callback
```

The URI must match byte-for-byte in the authorization request, token exchange,
Developer Portal OAuth2 redirect list, and reverse-proxy public URL. This crate
rejects callbacks with a query, fragment, user information, or a non-HTTPS
scheme. Request only the `identify guilds` scopes. Discord documents the
[authorization-code flow and state binding](https://docs.discord.com/developers/topics/oauth2)
and the [`/users/@me/guilds` projection](https://docs.discord.com/developers/resources/user#get-current-user-guilds).

Supply the client secret and Valkey URL through the deployment secret-file
configuration. Never put either value in the image, source tree, command line,
or browser build.

## API assembly

```rust,ignore
use std::sync::Arc;

use pepeaudio_api::SessionAuthenticator;
use pepeaudio_auth::{
    AuthService, DiscordOAuthClient, SessionGuildAuthorizer, SystemAuthClock,
    ValkeyAuthStore, build_auth_router,
};

let store = ValkeyAuthStore::connect(&valkey_url, &auth_config).await?;
let discord = DiscordOAuthClient::new(auth_config.discord().clone())?;
let presence = Arc::new(gateway_bot_presence);
let service = AuthService::with_discord_client(
    auth_config,
    discord,
    Arc::new(store.clone()),
    Arc::new(store.clone()),
    presence.clone(),
    Arc::new(SystemAuthClock),
);

let authenticator = SessionAuthenticator::new(store.clone());
let authorizer = SessionGuildAuthorizer::new(Arc::new(store), presence);
let app = pepeaudio_api::build_router(api_state).merge(build_auth_router(service));
```

The API's `Principal` contains a user ID and a server-only SHA-256 fingerprint
of the opaque session, but deliberately no raw session token. On every request
and periodic SSE authorization check, `SessionGuildAuthorizer` requires that
exact fingerprint to still own the Valkey `user-current-session` pointer. A new
login, logout, or session expiry therefore invalidates older requests and live
event streams for authorization.

Read, SSE subscription, and player control require OAuth-time guild membership
plus current bot presence. They do **not** require `MANAGE_GUILD`; voice-channel,
DJ, and owner policy must be rechecked by the authoritative player/Bot adapter.
Settings administration remains deliberately unimplemented.

## Security properties

- 32-byte random `state`, PKCE verifier, session token, and CSRF token.
- OAuth state is `SET NX` with a short TTL and consumed with atomic Lua
  `GET` + `DEL`; an HttpOnly SameSite=Lax `__Host-` cookie binds it to the browser.
- Discord HTTP uses fixed HTTPS endpoints, no redirects, connect/request
  timeouts, and bounded response bodies. Rate limits and 5xx responses fail
  closed without reflecting Discord bodies.
- Access and refresh tokens are zeroized after identity/guild projection and
  never persisted. Sessions retain only bounded browser-safe profile fields
  (username, display name, and avatar hash) alongside guild membership.
  Session expiry requires a fresh login.
- Valkey stores only SHA-256 hashes of opaque session cookie values. Session
  JSON has absolute and sliding idle expiry and contains no OAuth token.
- `__Host-pepeaudio_session` is Secure, HttpOnly, SameSite=Lax, Path=/, and has
  no Domain attribute. Logout is POST-only and requires the session CSRF token.
- All auth responses set `Cache-Control: no-store` and
  `Referrer-Policy: no-referrer`.

## Verification boundary

Unit and fake-backed router tests cover state/cookie binding, session rotation,
CSRF logout, guild authorization, and response secrecy. Live Discord OAuth,
Developer Portal configuration, reverse-proxy TLS, Valkey failover, browser
cookie behavior, and production bot-presence freshness still require staging
verification. The OAuth projection is a login-time snapshot; session expiry is
the bounded refresh mechanism because user tokens are intentionally not kept.
The absolute session lifetime has a hard maximum of 30 minutes, so a guild
departure or kick can remain authorized for at most that window. Immediate
revocation still requires a future live membership-query boundary. Session
loads also clamp records created by older releases to the current policy, so a
deployment does not preserve a previous longer authorization window.
