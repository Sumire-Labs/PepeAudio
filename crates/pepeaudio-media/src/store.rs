use std::path::{Path, PathBuf};

use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncWriteExt, ErrorKind},
};
use uuid::Uuid;

use crate::{
    ManagedMediaLeaseRegistry, StoreError, capacity::CapacityReservation,
    janitor::scan::is_link_or_reparse,
};

/// Operator-owned staging and cache directories for remote media.
#[derive(Clone, Debug)]
pub struct DownloadStore {
    root: PathBuf,
    registry: ManagedMediaLeaseRegistry,
}

impl DownloadStore {
    /// Uses a strictly initialized registry's canonical managed root.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::CapacityUnavailable`] for the internal unmetered
    /// registry used only by standalone janitor construction.
    pub fn new(registry: ManagedMediaLeaseRegistry) -> Result<Self, StoreError> {
        if registry.maximum_bytes().is_none() {
            return Err(StoreError::CapacityUnavailable);
        }
        Ok(Self {
            root: registry.canonical_root().to_path_buf(),
            registry,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn maximum_bytes(&self) -> u64 {
        self.registry
            .maximum_bytes()
            .expect("DownloadStore accepts only a metered registry")
    }

    pub(crate) fn reserve(&self, requested_bytes: u64) -> Result<CapacityReservation, StoreError> {
        self.registry.reserve(requested_bytes)
    }

    pub(crate) async fn begin(
        &self,
        reservation: CapacityReservation,
    ) -> Result<PartialDownload, StoreError> {
        let staging = self.root.join("staging");
        let objects = self.root.join("objects");
        fs::create_dir_all(&staging)
            .await
            .map_err(StoreError::Prepare)?;
        fs::create_dir_all(&objects)
            .await
            .map_err(StoreError::Prepare)?;

        for _ in 0..8 {
            let identifier = Uuid::new_v4().simple().to_string();
            let partial_path = staging.join(format!("{identifier}.part"));
            let final_path = objects.join(identifier);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial_path)
                .await
            {
                Ok(file) => {
                    return Ok(PartialDownload {
                        partial_path,
                        final_path,
                        file: Some(file),
                        reservation,
                        committed: false,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(StoreError::Allocate(error)),
            }
        }

        Err(StoreError::Allocate(std::io::Error::new(
            ErrorKind::AlreadyExists,
            "generated media identifiers repeatedly collided",
        )))
    }

    pub(crate) async fn discard_object(&self, path: &Path) -> Result<(), StoreError> {
        let metadata = fs::symlink_metadata(path)
            .await
            .map_err(StoreError::Cleanup)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(StoreError::UnsafeObject);
        }
        let canonical = fs::canonicalize(path).await.map_err(StoreError::Cleanup)?;
        if canonical.parent() != Some(self.root.join("objects").as_path()) || canonical != path {
            return Err(StoreError::UnsafeObject);
        }
        let permit = self
            .registry
            .begin_deletion(&canonical)
            .ok_or(StoreError::ObjectInUse)?;
        let verified = fs::symlink_metadata(&canonical)
            .await
            .map_err(StoreError::Cleanup)?;
        if !verified.is_file()
            || is_link_or_reparse(&verified)
            || verified.len() != metadata.len()
            || fs::canonicalize(&canonical)
                .await
                .map_err(StoreError::Cleanup)?
                != canonical
        {
            return Err(StoreError::UnsafeObject);
        }
        fs::remove_file(&canonical)
            .await
            .map_err(StoreError::Cleanup)?;
        permit.removed();
        Ok(())
    }
}

pub(crate) struct PartialDownload {
    partial_path: PathBuf,
    final_path: PathBuf,
    file: Option<File>,
    reservation: CapacityReservation,
    committed: bool,
}

impl PartialDownload {
    pub(crate) async fn write_all(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        self.file
            .as_mut()
            .expect("partial file remains open until commit or cleanup")
            .write_all(bytes)
            .await
            .map_err(StoreError::Write)
    }

    pub(crate) fn ensure_reserved(&mut self, bytes: u64) -> Result<(), StoreError> {
        self.reservation.ensure(bytes)
    }

    pub(crate) async fn commit(&mut self, actual_bytes: u64) -> Result<PathBuf, StoreError> {
        if let Some(mut file) = self.file.take() {
            file.flush().await.map_err(StoreError::Commit)?;
            drop(file);
        }
        fs::rename(&self.partial_path, &self.final_path)
            .await
            .map_err(StoreError::Commit)?;
        if let Err(error) = self
            .reservation
            .commit(self.final_path.clone(), actual_bytes)
        {
            let _ = std::fs::remove_file(&self.final_path);
            return Err(error);
        }
        self.committed = true;
        Ok(self.final_path.clone())
    }

    pub(crate) async fn cleanup(&mut self) -> Result<(), StoreError> {
        self.file.take();
        remove_if_present(&self.partial_path).await?;
        if !self.committed {
            remove_if_present(&self.final_path).await?;
        }
        Ok(())
    }
}

impl Drop for PartialDownload {
    fn drop(&mut self) {
        if !self.committed {
            self.file.take();
            let _ = std::fs::remove_file(&self.partial_path);
            let _ = std::fs::remove_file(&self.final_path);
            let retained = [&self.partial_path, &self.final_path]
                .into_iter()
                .find(|path| std::fs::symlink_metadata(path).is_ok());
            if let Some(path) = retained {
                // Keep the full reservation charged. The next serialized
                // janitor removal releases it; an unsafe replacement remains
                // charged and therefore fails admission closed.
                self.reservation.retain_failed_file(path.clone());
            }
        }
    }
}

async fn remove_if_present(path: &Path) -> Result<(), StoreError> {
    if let Err(error) = fs::remove_file(path).await
        && error.kind() != ErrorKind::NotFound
    {
        return Err(StoreError::Cleanup(error));
    }
    Ok(())
}
