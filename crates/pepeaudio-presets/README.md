# pepeaudio-presets

Startup-time catalog for operator-supplied HeSuVi HRIR WAV files.

- Put direct `.wav` files in `PEPEAUDIO_HRIR_DIRECTORY`; subdirectories and
  non-WAV attribution documents are ignored.
- Each filename stem remains the stable preset ID.
- An optional direct `info.csv` in HeSuVi's `id;description` format supplies
  human-readable selector names. Additional `/n/n` paragraphs become the
  secondary description; missing entries fall back to the filename stem.
- WAV symlinks, unsafe paths, over-limit assets, invalid 7/14-channel layouts,
  invalid samples, and unsafe DSP coefficients are rejected before Discord
  gateway startup.
- 44.1 kHz inputs are prepared once at 48 kHz. Realtime audio workers receive
  immutable in-memory coefficients and never read the filesystem.
- Third-party presets are not bundled. Operators remain responsible for source,
  license, attribution, and redistribution terms.

`info.csv` is read only during startup and is bounded like the WAV catalog. Do
not commit third-party HRIRs or their metadata to the PepeAudio source tree;
mount them as operator-managed runtime data.

Validate one file without starting the Bot:

```text
cargo run -p pepeaudio-presets --bin pepeaudio-hrir-check -- path/to/preset.wav
```

The catalog is intentionally immutable for one process lifetime. Install or
remove files, validate them, then restart the Bot for an atomic catalog reload.
