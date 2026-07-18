use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretBox, SecretString, zeroize::Zeroizing};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::SecretVaultError;

const DOCUMENT_FORMAT: &str = "any2api-master-key";
const DOCUMENT_VERSION: u16 = 1;
const ALGORITHM: &str = "xchacha20poly1305";
const KEY_LENGTH: usize = 32;
const MAX_DOCUMENT_BYTES: u64 = 4_096;

pub(super) struct MasterKey {
    bytes: SecretBox<[u8; KEY_LENGTH]>,
    key_id: String,
}

impl MasterKey {
    pub(super) fn load_or_create(
        path: &Path,
        allow_create: bool,
    ) -> Result<Self, SecretVaultError> {
        match Self::load(path) {
            Ok(key) => Ok(key),
            Err(SecretVaultError::MasterKeyMissing { .. }) if allow_create => Self::create(path),
            Err(error) => Err(error),
        }
    }

    pub(super) fn expose(&self) -> &[u8; KEY_LENGTH] {
        self.bytes.expose_secret()
    }

    pub(super) fn key_id(&self) -> &str {
        &self.key_id
    }

    fn load(path: &Path) -> Result<Self, SecretVaultError> {
        let mut file = File::open(path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                SecretVaultError::MasterKeyMissing {
                    path: path.to_path_buf(),
                }
            } else {
                SecretVaultError::ReadMasterKey {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;
        verify_permissions(path, &file)?;
        let metadata = file
            .metadata()
            .map_err(|source| SecretVaultError::ReadMasterKey {
                path: path.to_path_buf(),
                source,
            })?;
        if metadata.len() > MAX_DOCUMENT_BYTES {
            return Err(SecretVaultError::InvalidMasterKeyFormat);
        }
        let mut contents = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
        file.read_to_end(&mut contents)
            .map_err(|source| SecretVaultError::ReadMasterKey {
                path: path.to_path_buf(),
                source,
            })?;
        Self::parse(&contents)
    }

    fn create(path: &Path) -> Result<Self, SecretVaultError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| {
                SecretVaultError::CreateMasterKeyDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
        let mut generated = Zeroizing::new([0_u8; KEY_LENGTH]);
        getrandom::fill(generated.as_mut()).map_err(|_| SecretVaultError::RandomGeneration)?;
        let bytes = SecretBox::init_with_mut(|value: &mut [u8; KEY_LENGTH]| {
            value.copy_from_slice(generated.as_ref())
        });
        let key_id = derive_key_id(bytes.expose_secret());
        let document = serialize_document(bytes.expose_secret())?;

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(path) {
            Ok(file) => file,
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                return Self::load(path);
            }
            Err(source) => {
                return Err(SecretVaultError::CreateMasterKey {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        if let Err(source) = file.write_all(&document).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(SecretVaultError::WriteMasterKey {
                path: path.to_path_buf(),
                source,
            });
        }
        verify_permissions(path, &file)?;
        Ok(Self { bytes, key_id })
    }

    fn parse(contents: &[u8]) -> Result<Self, SecretVaultError> {
        let document: MasterKeyDocument = serde_json::from_slice(contents)
            .map_err(|_| SecretVaultError::InvalidMasterKeyFormat)?;
        if document.format != DOCUMENT_FORMAT {
            return Err(SecretVaultError::InvalidMasterKeyFormat);
        }
        if document.version != DOCUMENT_VERSION {
            return Err(SecretVaultError::UnsupportedMasterKeyVersion);
        }
        if document.algorithm != ALGORITHM {
            return Err(SecretVaultError::UnsupportedMasterKeyAlgorithm);
        }
        let mut decoded = Zeroizing::new([0_u8; KEY_LENGTH]);
        let length = URL_SAFE_NO_PAD
            .decode_slice(document.key.expose_secret(), decoded.as_mut())
            .map_err(|_| SecretVaultError::InvalidMasterKeyFormat)?;
        if length != KEY_LENGTH {
            return Err(SecretVaultError::InvalidMasterKeyFormat);
        }
        let bytes = SecretBox::init_with_mut(|value: &mut [u8; KEY_LENGTH]| {
            value.copy_from_slice(decoded.as_ref())
        });
        let key_id = derive_key_id(bytes.expose_secret());
        Ok(Self { bytes, key_id })
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MasterKey")
            .field("key_id", &self.key_id)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MasterKeyDocument {
    format: String,
    version: u16,
    algorithm: String,
    key: SecretString,
}

#[derive(Serialize)]
struct MasterKeyDocumentRef<'a> {
    format: &'static str,
    version: u16,
    algorithm: &'static str,
    key: &'a str,
}

fn serialize_document(key: &[u8; KEY_LENGTH]) -> Result<Zeroizing<Vec<u8>>, SecretVaultError> {
    let encoded = SecretString::from(URL_SAFE_NO_PAD.encode(key));
    let document = MasterKeyDocumentRef {
        format: DOCUMENT_FORMAT,
        version: DOCUMENT_VERSION,
        algorithm: ALGORITHM,
        key: encoded.expose_secret(),
    };
    let mut contents = Zeroizing::new(
        serde_json::to_vec_pretty(&document)
            .map_err(|_| SecretVaultError::InvalidMasterKeyFormat)?,
    );
    contents.push(b'\n');
    Ok(contents)
}

fn derive_key_id(key: &[u8; KEY_LENGTH]) -> String {
    let digest = Sha256::digest(key);
    format!("mk1_{}", URL_SAFE_NO_PAD.encode(&digest[..12]))
}

#[cfg(unix)]
fn verify_permissions(path: &Path, file: &File) -> Result<(), SecretVaultError> {
    use std::os::unix::fs::MetadataExt;

    let mode = file
        .metadata()
        .map_err(|source| SecretVaultError::ReadMasterKey {
            path: path.to_path_buf(),
            source,
        })?
        .mode();
    if mode & 0o077 != 0 {
        return Err(SecretVaultError::UnsafeMasterKeyPermissions {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_permissions(_path: &Path, _file: &File) -> Result<(), SecretVaultError> {
    Ok(())
}
