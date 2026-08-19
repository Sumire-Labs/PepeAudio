# pepeaudio-presets

Startup-time catalog for operator-supplied HeSuVi HRIR WAV files.

- Put direct `.wav` files in `PEPEAUDIO_HRIR_DIRECTORY`; subdirectories and
  non-WAV attribution documents are ignored.
- Each filename stem becomes the stable preset ID and visible selector label.
- WAV symlinks, unsafe paths, over-limit assets, invalid 7/14-channel layouts,
  invalid samples, and unsafe DSP coefficients are rejected before Discord
  gateway startup.
- 44.1 kHz inputs are prepared once at 48 kHz. Realtime audio workers receive
  immutable in-memory coefficients and never read the filesystem.
- Third-party presets are not bundled. Operators remain responsible for source,
  license, attribution, and redistribution terms.

Validate one file without starting the Bot:

```text
cargo run -p pepeaudio-presets --bin pepeaudio-hrir-check -- path/to/preset.wav
```

The catalog is intentionally immutable for one process lifetime. Install or
remove files, validate them, then restart the Bot for an atomic catalog reload.
