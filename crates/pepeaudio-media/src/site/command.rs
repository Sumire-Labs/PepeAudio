use std::ffi::OsString;

use crate::CommandSpec;

use super::{SiteProvider, SiteReference, YtDlpConfig};

pub(crate) fn discover(
    config: &YtDlpConfig,
    provider: SiteProvider,
    input: &str,
    maximum_plus_one: usize,
) -> CommandSpec {
    let mut arguments = base(config, provider);
    arguments.extend(strings([
        "--flat-playlist",
        "--playlist-items",
        &format!("1:{maximum_plus_one}"),
        "--dump-single-json",
        "--skip-download",
        "--",
        input,
    ]));
    specification(config, arguments)
}

pub(crate) fn resolve(config: &YtDlpConfig, reference: &SiteReference) -> CommandSpec {
    let mut arguments = base(config, reference.provider);
    arguments.extend(strings([
        "--no-playlist",
        "--format",
        reference.provider.format_selector(),
        "--dump-single-json",
        "--skip-download",
        "--",
        reference.page_url(),
    ]));
    specification(config, arguments).classify_unavailable_media()
}

pub(crate) fn search(config: &YtDlpConfig, provider: SiteProvider, query: &str) -> CommandSpec {
    let input = format!("{}{query}", provider.search_prefix());
    discover(config, provider, &input, 5)
}

pub(crate) fn resolve_query(
    config: &YtDlpConfig,
    provider: SiteProvider,
    query: &str,
) -> CommandSpec {
    let input = format!("{}{query}", provider.single_search_prefix());
    let mut arguments = base(config, provider);
    arguments.extend(strings([
        "--no-playlist",
        "--playlist-items",
        "1",
        "--format",
        provider.format_selector(),
        "--dump-single-json",
        "--skip-download",
        "--",
        &input,
    ]));
    specification(config, arguments).classify_unavailable_media()
}

pub(crate) fn ytdlp_version(config: &YtDlpConfig) -> CommandSpec {
    CommandSpec::new(&config.executable, strings(["--version"]))
        .with_deno_directory(&config.deno_directory)
}

pub(crate) fn deno_version(config: &YtDlpConfig) -> CommandSpec {
    CommandSpec::new(&config.deno_executable, strings(["--version"]))
        .with_deno_directory(&config.deno_directory)
}

fn specification(config: &YtDlpConfig, arguments: Vec<OsString>) -> CommandSpec {
    CommandSpec::new(&config.executable, arguments).with_deno_directory(&config.deno_directory)
}

fn base(config: &YtDlpConfig, provider: SiteProvider) -> Vec<OsString> {
    let runtime = format!("deno:{}", config.deno_executable.display());
    let cache_directory = config.deno_directory.join("yt-dlp-cache");
    let mut arguments = strings([
        "--ignore-config",
        "--no-config-locations",
        "--no-plugin-dirs",
        "--no-remote-components",
        "--no-cookies",
        "--cache-dir",
    ]);
    arguments.push(cache_directory.into_os_string());
    arguments.extend(strings([
        "--no-exec",
        "--quiet",
        "--no-warnings",
        "--no-progress",
        "--color",
        "no_color",
        "--socket-timeout",
        "10",
        "--retries",
        "1",
        "--extractor-retries",
        "1",
        "--no-wait-for-video",
        "--js-runtimes",
        &runtime,
        "--use-extractors",
        provider.extractor_allowlist(),
    ]));
    arguments
}

fn strings<const N: usize>(values: [&str; N]) -> Vec<OsString> {
    values.into_iter().map(OsString::from).collect()
}
