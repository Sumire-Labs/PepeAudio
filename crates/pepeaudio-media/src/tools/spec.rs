use std::{ffi::OsString, fmt, path::PathBuf};

/// Direct process invocation expressed as a program and unjoined arguments.
#[derive(Clone, Eq, PartialEq)]
pub struct CommandSpec {
    program: PathBuf,
    arguments: Vec<OsString>,
    deno_directory: Option<PathBuf>,
    classify_unavailable_media: bool,
}

impl CommandSpec {
    #[must_use]
    pub fn new(program: impl Into<PathBuf>, arguments: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            arguments,
            deno_directory: None,
            classify_unavailable_media: false,
        }
    }

    /// Adds the only tool-specific environment value accepted by the process
    /// boundary. The runner still clears the inherited environment first.
    #[must_use]
    pub fn with_deno_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.deno_directory = Some(directory.into());
        self
    }

    /// Enables conservative classification of a `yt-dlp` item-level
    /// unavailable error. Only site-track resolution commands opt in.
    #[must_use]
    pub(crate) const fn classify_unavailable_media(mut self) -> Self {
        self.classify_unavailable_media = true;
        self
    }

    #[must_use]
    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    /// Individual, non-shell command arguments.
    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    #[must_use]
    pub fn deno_directory(&self) -> Option<&std::path::Path> {
        self.deno_directory.as_deref()
    }

    pub(crate) const fn should_classify_unavailable_media(&self) -> bool {
        self.classify_unavailable_media
    }
}

impl fmt::Debug for CommandSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandSpec")
            .field("program", &self.program)
            .field("argument_count", &self.arguments.len())
            .field("has_deno_directory", &self.deno_directory.is_some())
            .field(
                "classify_unavailable_media",
                &self.classify_unavailable_media,
            )
            .finish()
    }
}
