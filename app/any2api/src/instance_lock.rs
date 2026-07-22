use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use anyhow::{Context, Result};
use fs2::FileExt;

pub(crate) struct InstanceLock {
    file: File,
}

impl InstanceLock {
    pub(crate) fn acquire(data_directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_directory).with_context(|| {
            format!(
                "failed to create data directory {}",
                data_directory.display()
            )
        })?;
        let path = data_directory.join("any2api.instance.lock");
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
}
