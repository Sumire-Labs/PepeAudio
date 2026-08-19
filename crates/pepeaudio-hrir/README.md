# pepeaudio-hrir

Strict, allocation-bounded loading of HeSuVi-compatible HRIR WAVE files.

This crate deliberately has a narrow first contract:

- accepts exactly 7- or 14-channel WAVE input;
- accepts 44.1 kHz or 48 kHz input without changing its sample rate;
- accepts 16-bit integer PCM or 32-bit IEEE float samples;
- normalizes samples to immutable planar `f32` data;
- exposes seven virtual directions, each with an explicit `(left ear, right ear)` pair;
- expands HeSuVi's 7-channel symmetric representation by mirroring the right-side directions;
- rejects empty, over-limit, non-finite, or unequal-length impulse responses before returning a preset.

Normal WAVE and `WAVE_FORMAT_EXTENSIBLE` are accepted to the extent supported by
`hound` 3.5.1. A file path is only an I/O locator: the returned preset does not
contain an ID, display name, or identity derived from that path.

## Intentionally out of scope

This crate does **not** implement sample-rate conversion, convolution, realtime
DSP, interpolation/spatial movement, content hashing, preset IDs, persistence,
or object storage. Those responsibilities belong to later pipeline layers. It
also does not bundle third-party HeSuVi presets; callers must provide files they
are permitted to use.
