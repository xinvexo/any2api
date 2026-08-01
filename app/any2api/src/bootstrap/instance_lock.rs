use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use any2api_storage::api::{ensure_private_directory, ensure_private_file};
use anyhow::{Context, Result};
use fs2::FileExt;

pub(super) struct InstanceLock {
    file: File,
}

impl InstanceLock {
    pub(super) fn acquire(data_directory: &Path) -> Result<Self> {
        ensure_private_directory(data_directory).with_context(|| {
            format!(
                "failed to create data directory {}",
                data_directory.display()
            )
        })?;
        let path = data_directory.join("any2api.instance.lock");
        ensure_private_file(&path)
            .with_context(|| format!("failed to protect instance lock {}", path.display()))?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open instance lock {}", path.display()))?;
        file.try_lock_exclusive().with_context(|| {
            format!(
                "another any2api process is using data directory {}",
                data_directory.display()
            )
        })?;
        Ok(Self { file })
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::InstanceLock;

    #[test]
    fn data_directory_lock_is_exclusive_until_drop() {
        let directory = tempdir().expect("temporary directory");
        let first = InstanceLock::acquire(directory.path()).expect("first lock");
        assert!(InstanceLock::acquire(directory.path()).is_err());
        drop(first);
        InstanceLock::acquire(directory.path()).expect("lock after release");
    }

    #[cfg(unix)]
    #[test]
    fn data_directory_and_lock_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().expect("temporary directory");
        let data = root.path().join("data");
        let _lock = InstanceLock::acquire(&data).expect("instance lock");
        assert_eq!(
            std::fs::metadata(&data)
                .expect("data directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(data.join("any2api.instance.lock"))
                .expect("lock metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
