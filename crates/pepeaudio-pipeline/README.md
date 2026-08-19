# pepeaudio-pipeline

Production playback bridge from inspected, managed media files to Discord
voice. It restarts a bounded FFmpeg `f32le` decoder for play and seek, processes
48 kHz stereo blocks through `pepeaudio-audio`, and feeds Songbird 0.6 through
its `AsyncAdapterStream` plus `RawAdapter` input path.

The pipeline does not resolve or download URLs. `TrackResolver` implementations
must return a trusted local object, normally produced by `pepeaudio-media`.
`ManagedMediaResolver` is provided for generated cache paths and rejects files
whose canonical path escapes its configured root.

Track PCM and Songbird adapter buffers are bounded. Decoder reads and writes
apply backpressure, and every replacement, seek, stop, or disconnect explicitly
cancels the worker and awaits its FFmpeg kill/reap path. A bounded shutdown
timeout and drop-time kill fallback are retained for a wedged child, but normal
shutdown must use the async playback methods.

`PipelineDependencies` accepts public resolver, decoder, and prepared-HRIR
ports. `LookupHrirProvider::new(move |id| catalog.get(id))` adapts an immutable
`pepeaudio-presets` catalog without a dependency cycle. Subscribe to
`SongbirdPlayback::subscribe_events` before moving the port into the player
actor, then forward current `PlaybackEvent::TrackEnded` identities to
`PlayerHandle::playback_ended`; pipeline generations already suppress stale
Songbird events from replacement, repeat, and FFmpeg-respawn seeks.

The `symphonia` PCM codec feature is explicit because Songbird's raw format
adapter alone does not enable the `f32le` decoder. Runtime images must provide a
compatible `ffmpeg` executable. The executable and all input paths are passed as
argument arrays without a shell.

This crate supports horizontal 360-degree orbit only. HeSuVi has no elevation
planes, so it is not a spherical HRTF renderer. DAVE is supplied by Songbird's
driver and is below this PCM input boundary; it still requires validation in a
real Discord voice channel.

While spatial audio is enabled, the wet output follows a continuous clockwise
horizontal orbit. One revolution is 60 seconds by default and can be configured
from 1 second through 10 minutes with `PipelineConfig::orbit_period`. The orbit
is driven by processed 48 kHz PCM frames, not wall time: pause freezes the
audible sequence and resume continues it; bounded PCM already prepared before a
pause retains its matching position. Seek creates a fresh worker at the seeked
track-time phase, while a new track begins at the configured front/origin phase.
Disabling spatial audio bypasses the wet signal but keeps the sample clock
aligned, so re-enabling joins the current track-time phase instead of restarting
or jumping to wall time.
`set_orbit_position` rebases the active orbit immediately and stores that
center/width as the origin for later track generations.
