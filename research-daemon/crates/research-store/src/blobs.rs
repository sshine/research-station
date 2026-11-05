use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::StoreError;

/// A stored blob, addressed by the SHA-256 of its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    pub hash: String,
    pub path: PathBuf,
}

/// Content-addressed blob store.
///
/// The same bytes always map to the same path, so repeated provider responses
/// are deduplicated. Writes are atomic (temporary file plus rename).
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Store bytes and return their hash and path.
    pub fn put(&self, bytes: &[u8]) -> Result<BlobRef, StoreError> {
        let hash = hex::encode(Sha256::digest(bytes));
        let path = self.path_for(&hash)?;

        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, bytes)?;
            match std::fs::rename(&tmp, &path) {
                Ok(()) => {}
                Err(err) if path.exists() => {
                    // Another writer won the race; the content is identical.
                    let _ = std::fs::remove_file(&tmp);
                    let _ = err;
                }
                Err(err) => return Err(err.into()),
            }
        }

        Ok(BlobRef { hash, path })
    }

    /// Read a blob by hash.
    pub fn get(&self, hash: &str) -> Result<Vec<u8>, StoreError> {
        let path = self.path_for(hash)?;
        Ok(std::fs::read(path)?)
    }

    fn path_for(&self, hash: &str) -> Result<PathBuf, StoreError> {
        if hash.len() < 4 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(StoreError::InvalidHash(hash.to_owned()));
        }

        Ok(self.root.join(&hash[0..2]).join(&hash[2..4]).join(hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blobs_are_content_addressed_and_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path());

        let a = store.put(b"hello").unwrap();
        let b = store.put(b"hello").unwrap();
        let c = store.put(b"world").unwrap();

        assert_eq!(a.hash, b.hash);
        assert_eq!(a.path, b.path);
        assert_ne!(a.hash, c.hash);
        assert_eq!(store.get(&a.hash).unwrap(), b"hello");
        assert!(store.get("nope").is_err());
    }
}
