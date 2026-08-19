mod development;
mod error;
mod hrir_catalog;
mod production;
mod production_lifecycle;
mod readiness;
mod shutdown;

use std::{env, process::ExitCode};

use error::StartupError;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pepeaudio-api startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), StartupError> {
    match env::var("PEPEAUDIO_API_AUTH_MODE").as_deref() {
        Ok("development") => development::run().await,
        Ok("production") | Err(env::VarError::NotPresent) => production::run().await,
        Ok(_) | Err(env::VarError::NotUnicode(_)) => {
            Err(StartupError::InvalidEnvironment("PEPEAUDIO_API_AUTH_MODE"))
        }
    }
}
