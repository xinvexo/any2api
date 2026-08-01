use std::{fs, io, path::Path};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

const DIRECTORY_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

pub fn ensure_private_directory(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    if let Err(error) = fs::symlink_metadata(path) {
        if error.kind() != io::ErrorKind::NotFound {
            return Err(error);
        }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        builder.mode(DIRECTORY_MODE);
        builder.create(path)?;
    }
    protect(path, PathKind::Directory, DIRECTORY_MODE)
}

pub fn ensure_private_file(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => protect(path, PathKind::File, FILE_MODE),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut options = fs::OpenOptions::new();
            options.create_new(true).read(true).write(true);
            #[cfg(unix)]
            options.mode(FILE_MODE);
            match options.open(path) {
                Ok(file) => drop(file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            protect(path, PathKind::File, FILE_MODE)
        }
        Err(error) => Err(error),
    }
}

pub fn protect_private_file(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => protect(path, PathKind::File, FILE_MODE),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[derive(Clone, Copy)]
enum PathKind {
    Directory,
    File,
}

fn protect(path: &Path, kind: PathKind, mode: u32) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let valid_kind = match kind {
        PathKind::Directory => metadata.file_type().is_dir(),
        PathKind::File => metadata.file_type().is_file(),
    };
    if !valid_kind {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "local data path has an unsafe file type: {}",
                path.display()
            ),
        ));
    }
    #[cfg(unix)]
    {
        let current_uid = rustix::process::geteuid().as_raw();
        if metadata.uid() != current_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "local data path is not owned by the current user: {}",
                    path.display()
                ),
            ));
        }
        if metadata.permissions().mode() & 0o7777 != mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use tempfile::tempdir;

    use super::{ensure_private_directory, ensure_private_file};

    #[test]
    fn tightens_existing_paths_and_rejects_symlinks() {
        let root = tempdir().expect("temporary directory");
        let directory = root.path().join("data");
        std::fs::create_dir(&directory).expect("directory");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
            .expect("directory permissions");
        ensure_private_directory(&directory).expect("protect directory");
        assert_eq!(
            std::fs::metadata(&directory)
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let file = directory.join("database");
        std::fs::write(&file, b"").expect("file");
        ensure_private_file(&file).expect("protect file");
        assert_eq!(
            std::fs::metadata(&file)
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let link = directory.join("link");
        symlink(&file, &link).expect("symlink");
        assert!(ensure_private_file(&link).is_err());
    }
}
