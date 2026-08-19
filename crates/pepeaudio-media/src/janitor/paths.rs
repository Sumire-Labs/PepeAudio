use std::path::{Path, PathBuf};

use tokio::fs;

use super::{JanitorError, scan::is_link_or_reparse};

#[derive(Clone, Debug)]
pub(crate) struct ManagedPaths {
    pub(crate) root: PathBuf,
    pub(crate) staging: PathBuf,
    pub(crate) objects: PathBuf,
}

impl ManagedPaths {
    pub(crate) async fn prepare(root: &Path) -> Result<Self, JanitorError> {
        fs::create_dir_all(root)
            .await
            .map_err(JanitorError::Prepare)?;
        let metadata = fs::symlink_metadata(root)
            .await
            .map_err(JanitorError::Prepare)?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(JanitorError::UnsafeRoot);
        }
        let root = fs::canonicalize(root)
            .await
            .map_err(JanitorError::Prepare)?;
        let staging = prepare_child(&root, "staging").await?;
        let objects = prepare_child(&root, "objects").await?;
        let paths = Self {
            root,
            staging,
            objects,
        };
        paths.validate().await?;
        Ok(paths)
    }

    pub(crate) async fn validate(&self) -> Result<(), JanitorError> {
        validate_directory(&self.root, &self.root)
            .await
            .map_err(|()| JanitorError::UnsafeRoot)?;
        validate_directory(&self.staging, &self.staging)
            .await
            .map_err(|()| JanitorError::UnsafeManagedDirectory("staging"))?;
        validate_directory(&self.objects, &self.objects)
            .await
            .map_err(|()| JanitorError::UnsafeManagedDirectory("objects"))?;
        if self.staging.parent() != Some(&self.root) || self.objects.parent() != Some(&self.root) {
            return Err(JanitorError::UnsafeRoot);
        }
        Ok(())
    }
}

async fn prepare_child(root: &Path, name: &'static str) -> Result<PathBuf, JanitorError> {
    let child = root.join(name);
    match fs::create_dir(&child).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(JanitorError::Prepare(error)),
    }
    validate_directory(&child, &child)
        .await
        .map_err(|()| JanitorError::UnsafeManagedDirectory(name))?;
    Ok(child)
}

async fn validate_directory(path: &Path, expected: &Path) -> Result<(), ()> {
    let metadata = fs::symlink_metadata(path).await.map_err(|_| ())?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(());
    }
    let canonical = fs::canonicalize(path).await.map_err(|_| ())?;
    if canonical != expected {
        return Err(());
    }
    Ok(())
}
