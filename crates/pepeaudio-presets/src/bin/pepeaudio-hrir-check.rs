use std::{env, process::ExitCode};

use pepeaudio_audio::PreparedHrir;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("HRIR validation failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let source = arguments
        .next()
        .ok_or("usage: pepeaudio-hrir-check <hesuvi.wav>")?;
    if arguments.next().is_some() {
        return Err("usage: pepeaudio-hrir-check <hesuvi.wav>".into());
    }

    let preset = pepeaudio_hrir::load_hesuvi_wav_file(source)?;
    let prepared = PreparedHrir::from_hesuvi(&preset)?;
    println!(
        "valid HeSuVi HRIR: source={}Hz layout={:?} source_frames={} prepared_frames={} output=48000Hz",
        preset.sample_rate().as_hz(),
        preset.source_layout(),
        preset.frame_count(),
        prepared.frame_count(),
    );
    Ok(())
}
