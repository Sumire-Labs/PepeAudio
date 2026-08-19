use std::{env, process};

use sqlx::{migrate::Migrator, postgres::PgPoolOptions};
use thiserror::Error;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("PepeAudio migration failed: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), MigrationError> {
    let database_url = database_url()?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .map_err(MigrationError::Connect)?;
    MIGRATOR.run(&pool).await.map_err(MigrationError::Migrate)?;
    pool.close().await;
    println!("PepeAudio database migrations completed.");
    Ok(())
}

fn database_url() -> Result<String, MigrationError> {
    let namespaced = read_direct_or_file("PEPEAUDIO_DATABASE_URL", "PEPEAUDIO_DATABASE_URL_FILE")?;
    let conventional = read_direct_or_file("DATABASE_URL", "DATABASE_URL_FILE")?;
    match (namespaced, conventional) {
        (Some(_), Some(_)) => Err(MigrationError::ConflictingSources),
        (Some(value), None) | (None, Some(value)) => Ok(value),
        (None, None) => Err(MigrationError::MissingUrl),
    }
}

fn read_direct_or_file(
    direct_name: &'static str,
    file_name: &'static str,
) -> Result<Option<String>, MigrationError> {
    let direct = read_env(direct_name)?;
    let file = read_env(file_name)?;
    match (direct, file) {
        (Some(_), Some(_)) => Err(MigrationError::ConflictingPair {
            direct_name,
            file_name,
        }),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(path)) => read_secret_file(file_name, &path).map(Some),
        (None, None) => Ok(None),
    }
}

fn read_env(name: &'static str) -> Result<Option<String>, MigrationError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(MigrationError::NotUnicode { name }),
    }
}

fn read_secret_file(name: &'static str, path: &str) -> Result<String, MigrationError> {
    if path.trim().is_empty() || path.contains('\0') {
        return Err(MigrationError::InvalidFileVariable { name });
    }
    let mut value =
        std::fs::read_to_string(path).map_err(|_| MigrationError::SecretFile { name })?;
    if value.ends_with("\r\n") {
        value.truncate(value.len() - 2);
    } else if value.ends_with('\n') {
        value.pop();
    }
    if value.is_empty() {
        Err(MigrationError::SecretFile { name })
    } else {
        Ok(value)
    }
}

#[derive(Debug, Error)]
enum MigrationError {
    #[error("set exactly one supported database URL variable or file variable")]
    MissingUrl,
    #[error("namespaced and conventional database URL sources cannot both be set")]
    ConflictingSources,
    #[error("{direct_name} and {file_name} cannot both be set")]
    ConflictingPair {
        direct_name: &'static str,
        file_name: &'static str,
    },
    #[error("{name} is not valid Unicode")]
    NotUnicode { name: &'static str },
    #[error("{name} must identify one readable file")]
    InvalidFileVariable { name: &'static str },
    #[error("secret file configured by {name} could not be read")]
    SecretFile { name: &'static str },
    #[error("could not connect to PostgreSQL")]
    Connect(#[source] sqlx::Error),
    #[error("PostgreSQL rejected a migration")]
    Migrate(#[source] sqlx::migrate::MigrateError),
}
