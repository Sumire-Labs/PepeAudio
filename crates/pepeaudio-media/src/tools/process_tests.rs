use std::{ffi::OsString, path::Path};

use super::{
    CommandSpec, ProcessOutput,
    process::{configure_environment, reports_unavailable_media},
};

#[test]
fn process_output_debug_omits_tool_output() {
    let output = ProcessOutput {
        status_code: Some(0),
        stdout: b"https://cdn.example/?token=sentinel".to_vec(),
        stderr: b"cookie=sentinel".to_vec(),
    };
    let debug = format!("{output:?}");
    assert!(!debug.contains("sentinel"));
    assert!(debug.contains("stdout_bytes"));
}

#[test]
fn classifies_only_explicit_item_unavailability_diagnostics() {
    for unavailable in [
        "ERROR: [youtube] abc: Private video. Sign in if you've been granted access",
        "ERROR: [youtube] abc: Video unavailable. This content isn't available",
        "ERROR: [youtube] abc: The uploader has not made this video available in your country",
        "ERROR: [soundcloud] artist/track: This track is no longer available",
    ] {
        assert!(reports_unavailable_media(unavailable.as_bytes()));
    }
    for operational in [
        "ERROR: Unable to download API page: timed out",
        "ERROR: HTTP Error 403: Forbidden",
        "ERROR: [youtube] JavaScript runtime was not found",
        "ERROR: Unable to extract track URL; please report this issue",
    ] {
        assert!(!reports_unavailable_media(operational.as_bytes()));
    }
}

#[test]
fn tool_environment_keeps_only_runtime_prerequisites_and_explicit_deno_dir() {
    let mut command = tokio::process::Command::new("unused");
    command
        .env("COOKIE", "sentinel-cookie")
        .env("AWS_SECRET_ACCESS_KEY", "sentinel-secret");
    let specification = CommandSpec::new("unused", Vec::<OsString>::new())
        .with_deno_directory("/tmp/pepeaudio-deno");

    configure_environment(&mut command, &specification);

    let environment = command
        .as_std()
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect::<Vec<_>>();
    assert!(!environment.iter().any(|(name, value)| {
        matches!(name.to_str(), Some("COOKIE" | "AWS_SECRET_ACCESS_KEY")) && value.is_some()
    }));
    assert!(environment.iter().any(|(name, value)| {
        name == "DENO_DIR" && value.as_deref() == Some(std::ffi::OsStr::new("/tmp/pepeaudio-deno"))
    }));
    assert_eq!(
        specification.deno_directory(),
        Some(Path::new("/tmp/pepeaudio-deno"))
    );
}
