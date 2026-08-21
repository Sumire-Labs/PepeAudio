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

This crate supports horizontal HeSuVi rendering only. HeSuVi has no elevation
planes, so it is not a spherical HRTF renderer. Spatial audio keeps the stereo
pair fixed at the configured front position; it never moves with playback time.
`set_orbit_position` explicitly changes that position for the active and future
tracks. DAVE is supplied by Songbird's driver and is below this PCM input
boundary; it still requires validation in a real Discord voice channel.
