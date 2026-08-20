# Third-party licensing and source patches

## Project license boundary

Code and documentation authored for PepeAudio-rs are available under the MIT
License unless a file or directory says otherwise. The root `LICENSE` file does
not relicense dependencies, vendored sources, FFmpeg, fonts, icons, HRIR
presets, or media supplied by operators and users. Those materials remain under
their respective licenses and terms.

The Rust runtime image carries the project license at
`/usr/share/licenses/pepeaudio/LICENSE`. The Caddy image carries the same file
there and serves it as `/LICENSE.txt`. These copies cover PepeAudio-rs itself;
they do not replace license notices supplied by base images or dependencies.

The Rust runtime image also carries this document and the patched `hpke-rs`
source, provenance, and MPL-2.0 text under `/usr/share`. The Caddy image serves
this document as `/THIRD-PARTY.md`. The standalone Web build and Caddy image
expose the approved browser-runtime notices under `/licenses/`, including a
machine-readable `/licenses/manifest.json`. The same tree is retained inside
the Caddy image at `/usr/share/licenses/pepeaudio-web-dependencies`.

No third-party HRIR preset is bundled merely because PepeAudio can read the
HeSuVi format. Operators must verify the source, attribution, modification, and
redistribution terms for each preset before importing or distributing it.

## Web runtime packages

The frozen production dependency graph reported by
`pnpm licenses list --prod --json` is the following exact allowlist:

| Package | Version | Declared license |
| --- | ---: | --- |
| `@astryxdesign/core` | 0.3.0 | MIT |
| `@astryxdesign/theme-neutral` | 0.3.0 | MIT |
| `@dnd-kit/accessibility` | 3.1.1 | MIT |
| `@dnd-kit/core` | 6.3.1 | MIT |
| `@dnd-kit/sortable` | 10.0.0 | MIT |
| `@dnd-kit/utilities` | 3.2.2 | MIT |
| `@formatjs/fast-memoize` | 3.1.7 | MIT |
| `@formatjs/icu-messageformat-parser` | 3.5.16 | MIT |
| `@formatjs/icu-skeleton-parser` | 2.1.11 | MIT |
| `@stylexjs/stylex` | 0.19.0 | MIT |
| `css-mediaquery` | 0.1.2 | BSD |
| `intl-messageformat` | 11.2.13 | BSD-3-Clause |
| `invariant` | 2.2.4 | MIT |
| `js-tokens` | 4.0.0 | MIT |
| `loose-envify` | 1.4.0 | MIT |
| `lucide-react` | 1.31.0 | ISC |
| `react` | 19.2.8 | MIT |
| `react-dom` | 19.2.8 | MIT |
| `scheduler` | 0.27.0 | MIT |
| `styleq` | 0.2.1 | MIT |
| `tslib` | 2.8.1 | 0BSD |

The build copies one full notice to `/licenses/<package-name>/LICENSE` for each
entry. Scoped package names retain their scope directory. CI rejects changes to
the package name, version, declared license, notice content, or SHA-256 digest,
so dependency upgrades require an explicit review of this inventory.

Two upstream npm archives declare MIT but omit their own license file. These
exceptions are explicit and hash-pinned rather than silently ignored:

- `@astryxdesign/core@0.3.0` uses the installed
  `@astryxdesign/theme-neutral@0.3.0` license text. Both packages come from the
  same Astryx release, and the text matches the
  [Astryx v0.3.0 root license](https://github.com/facebook/astryx/blob/v0.3.0/LICENSE).
- `@stylexjs/stylex@0.19.0` uses the installed `react@19.2.8` license text, which
  matches the
  [StyleX 0.19.0 root license](https://github.com/facebook/stylex/blob/0.19.0/LICENSE).

The verifier requires these package-local files to remain absent; if either
upstream begins shipping a notice, the fallback must be removed and the new
file reviewed. Base images and operating-system packages remain subject to
their own notices even though they are not browser assets.

## Discord TLS backend

Serenity 0.12.5 and Songbird 0.6.0 otherwise select the older Rustls 0.22
WebSocket stack. PepeAudio selects their supported native TLS features instead:
SChannel on Windows and OpenSSL in the Linux image. The Docker builder carries
the OpenSSL headers, and the runtime image explicitly carries `libssl3`.
The separately resolved Rustls 0.23 stack used by project HTTP clients remains
current; this workaround does not pin it to the older WebSocket dependency.

## hpke-rs 0.6.1

Songbird's DAVE support currently reaches `hpke-rs` 0.6.1 through Davey and
OpenMLS. PepeAudio carries a local, source-compatible manifest patch until a
fixed stable release reaches that dependency chain. The patch updates the
vulnerable SHA-3 primitive, removes an unused optional crypto backend from
dependency resolution, and deletes only that backend's dead re-export. The
HPKE implementation is unchanged.

The vendored source, exact archive digest, changes, upstream link, and
MPL-2.0 license are recorded in
`vendor/hpke-rs-0.6.1-security-patch/PATCH.md`.

## Bot media tools

The bot image, and only the bot image, adds two version-pinned executables:

| Tool | Version | Distributed artifact | SHA-256 | License material in image |
| --- | ---: | --- | --- | --- |
| yt-dlp | 2026.06.09 | architecture-neutral `yt-dlp` zipimport executable | `e5d57466682cfa9d61e9cf7c8a4f09b00f4a62af37d3bbdc4bcffdf63615feac` | `/usr/share/licenses/yt-dlp` |
| Deno | 2.8.1 | `deno-x86_64-unknown-linux-gnu.zip` | `2d7bb6195226ac832e0bf7109a115f0af65ee69ac797a4bbde5b27a06cc242d9` | `/usr/share/licenses/deno` |
| Deno | 2.8.1 | `deno-aarch64-unknown-linux-gnu.zip` | `67e9df91870fd0af700df924173e3009ea7ff6956e2c3c3bb86065d6070d0fd6` | `/usr/share/licenses/deno` |

All artifacts come from their immutable upstream GitHub releases and are
SHA-256 checked during the image build. The yt-dlp generic executable is used
with Debian Python rather than the PyInstaller Linux bundles: the upstream
release identifies additional compiled-in licenses for those bundles. yt-dlp's
Unlicense text and complete release `THIRD_PARTY_LICENSES.txt` are retained in
the image. Its release page also identifies the generic executable's embedded
ISC- and MIT-licensed code.

Deno's tagged MIT license and a provenance notice are retained in the image.
The audited `v2.8.1` source tag does not publish one consolidated third-party
notice; its source/lockfile remains the upstream dependency-license record and
the release workflow emits an image SBOM.

- [yt-dlp 2026.06.09 release](https://github.com/yt-dlp/yt-dlp/releases/tag/2026.06.09)
- [yt-dlp 2026.06.09 license](https://github.com/yt-dlp/yt-dlp/blob/2026.06.09/LICENSE)
- [yt-dlp 2026.06.09 bundled notices](https://github.com/yt-dlp/yt-dlp/blob/2026.06.09/THIRD_PARTY_LICENSES.txt)
- [Deno 2.8.1 release](https://github.com/denoland/deno/releases/tag/v2.8.1)
- [Deno 2.8.1 license](https://github.com/denoland/deno/blob/v2.8.1/LICENSE.md)

## Cloudflare Tunnel connector

The optional `compose.cloudflare-tunnel.yaml` deployment expects a separately
installed, systemd-managed `cloudflared` connector on the Ubuntu host.
`cloudflared` is licensed under Apache-2.0 and is not copied into, distributed
with, or relicensed by PepeAudio's MIT images. The host operator is responsible
for installing and updating it and for complying with the terms of the
Cloudflare service used by the connector.

- [cloudflared source and license](https://github.com/cloudflare/cloudflared)
- [Cloudflare Tunnel documentation](https://developers.cloudflare.com/tunnel/)

## Dependency audit policy

CI installs the pinned `cargo-audit` 0.22.2 release and rejects known
vulnerabilities. Maintenance-status notices are reviewed separately because
RustSec does not provide a patched version for every notice. In particular,
`derivative` remains an active build-time dependency of the stable Poise and
Songbird releases, `instant` remains behind Davey's OpenMLS `js` feature, and
`proc-macro-error2` is used by Libcrux's verification macros. These notices do
not describe known exploitable vulnerabilities, but all three must be
rechecked when those upstream projects publish compatible releases.

We do not suppress vulnerability advisories to make the audit pass. Packages
which appear only because Cargo resolves an unused optional feature are still
removed from the lockfile when a narrow, source-compatible patch is practical.
