# pepeaudio-media

`pepeaudio-media` is the transport-neutral ingestion boundary for PepeAudio.
It validates untrusted remote locations, downloads bounded media into managed
storage, inspects containers with `ffprobe`, and spawns `ffmpeg` decoders that
emit 48 kHz stereo `f32le` PCM.

## Supported inputs

- A direct `http` or `https` media URL supplied through `/play`'s `url` option.
- A Discord attachment URL supplied through `/play`'s `file` option. Attachment
  metadata is treated only as an untrusted hint; the URL follows the exact same
  validation, DNS, redirect, and byte-limit path as every other remote URL.
- A YouTube or SoundCloud page or playlist when the operator enables the site
  resolver. Playlists import a bounded prefix and report skipped/truncated
  entries instead of expanding without limit.

The site resolver invokes a pinned `yt-dlp` executable only to inspect public
metadata and select one direct HTTPS, audio-only, non-manifest format. It never
asks yt-dlp to download media, run commands, load cookies or user configuration,
install plugins, or use remote components. The selected URL and a small
allowlist of safe request headers return to `MediaFetcher`, so DNS pinning,
redirect checks, quotas, managed storage, and `ffprobe` remain mandatory.

Spotify and Apple Music API clients live in `pepeaudio-catalog`; the bot may
convert their structured metadata into a `SiteSearch`. The media crate remains
provider-neutral at that boundary and rejects weak or ambiguous matches.

## Security boundary

Only `http` and `https` are accepted. User information and URL fragments are
rejected. Every redirect is re-parsed and re-resolved. All DNS answers must be
publicly routable, and the production HTTP adapter pins each request to the
inspected answers with automatic redirects and environment proxies disabled.
Downloads have redirect, header, wall-clock, and byte limits. Files are written
under operator-provided managed roots with an extensionless generated name;
the remote filename and extension never determine media validity.

`ffprobe` and `ffmpeg` are invoked directly with argument arrays, never through
a shell. The same rule applies to yt-dlp and Deno. Child environments are
cleared and rebuilt from an explicit allowlist; only the configured Deno cache
directory is passed to yt-dlp. These tools are external runtime dependencies
and must be installed and patched by the deployment image. Callers must
explicitly shut down decoders; the child is also configured to be killed if its
Rust handle is dropped.

Application checks complement, but do not replace, container egress rules and
an OS-level sandbox for FFmpeg. Before accepting traffic, production performs
an exact, bounded scan of both managed directories. Unknown names, links,
reparse points, non-regular files, unsafe canonical paths, unreadable metadata,
arithmetic overflow, and an exceeded entry bound all fail startup closed.

`DownloadStore` then enforces a process-local hard admission quota. A direct
URL reserves the full per-download limit, and an attachment reserves its
declared size, before URL parsing, DNS, or HTTP. Reservations grow before the
corresponding bytes are written, shrink to the actual size on commit, and are
released synchronously on failure, cancellation, or drop. Existing objects,
crash-left staging files, and in-flight reservations are counted together.
The per-download maximum must not exceed the complete managed-media budget.
If cancellation happens after an object commits while inspection is still in
progress, the completed object remains charged as used capacity. It has no
playback lease and is reclaimed by the same age-bounded janitor path below.

`ManagedDownloadJanitor` separately supplies bounded retention cleanup. Its
defaults retain an unleased completed object for at least five minutes before capacity
eviction, expire objects after seven days, and expire staging partials after
one hour. It never follows or removes links, reparse points, non-regular files,
unknown names, or paths outside the canonical managed root. A bounded janitor
scan can report `scan_limit_reached`; this affects cleanup latency, not hard
admission accounting. Run once at startup and periodically thereafter. Dry-run
reports use the same selection rules without deleting files.

Production shares one `ManagedMediaLeaseRegistry` between ingestion and the
janitor. Each completed object receives an opaque, Arc-backed lease embedded in
its `PlaybackSource`; clones held by the current track, queue, history, or audio
pipeline therefore keep the object protected. Dropping the final clone performs
no filesystem work: a later janitor pass reclaims the now-unreferenced object.
The janitor excludes leased objects during scan selection and atomically takes
a deletion permit immediately before `remove_file`, so a concurrent lease
acquisition cannot win between the final check and deletion. Successful
janitor removals and explicit command-failure discards release the same hard
quota ledger only after the exact verified object has been removed.

These leases are process-local. Exactly one process-local registry must govern
every janitor and resolver using a managed root. The production Bot derives an
instance-private subtree as `<upload root>/<PEPEAUDIO_INSTANCE_ID>` before it
constructs either component, so non-overlapping shard processes may share the
parent volume without scanning or deleting each other's objects. Reusing one
instance ID concurrently still requires distributed lease/fencing and is
forbidden by the stop-before-start deployment contract.

Current playback, queue, repeat history, and audio-pipeline ownership retain an
opaque lease, so their object lifetime is independent of the five-minute
unleased capacity-eviction floor. `/play` followed by `/stop` drops ownership;
the resulting object becomes eligible after that short floor instead of being
pinned for the maximum possible queue dwell. Production runs periodic cleanup
every 15 minutes and, after a pre-network quota rejection, runs one serialized
admission-targeted cleanup pass followed by one reservation retry.

FFmpeg's redistribution terms depend on its build configuration (for example,
whether GPL-only codecs were enabled), so the final Ubuntu/Docker image needs a
separate binary and codec-license audit.
