# Runtime secrets

`compose.yaml` reads secrets from files in this directory. Files without the
`.example` suffix are ignored by Git and the Docker build context.

Run `scripts/init-dev-secrets.ps1` on Windows or
`scripts/init-dev-secrets.sh` on Linux to create local PostgreSQL, Valkey,
and connection-URL secrets. Sessions are opaque random values created by the
API and stored only in Valkey, so no session signing secret is generated.
Discord credentials are never generated;
copy the corresponding `.example` files and fill them from the Discord
Developer Portal before enabling the `discord` Compose profile.

When using the host-managed Cloudflare Tunnel deployment, its token is not a
PepeAudio Compose secret. Store it separately at `/etc/cloudflared/token` with
owner `root:root` and mode `0400`; the systemd `cloudflared` service reads it.
Do not copy that token into this directory or pass it to the production secret
preparation scripts.

The checked-in `.example` values are deliberately invalid placeholders. The
Bot and API reject them at startup, so copying an example without replacing
`replace-me` cannot accidentally launch a public service with a known token or
component-signing key. Generate the component-signing key with the helper and
replace both Discord credential files with values from your own application.

Production secret values must never be committed or included in an image. On
the supported Ubuntu rootful-Docker host, `prepare-production-secrets.sh`
applies the `root:10001` and `0440` file contract before Compose bind-mounts
them as secrets.
