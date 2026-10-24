//! Where a store keeps its encoded objects, each under the hash of its
//! bytes. `jf` keeps them on its disk, and vcs in the object store.

use std::fmt::Debug;
use std::fs;
use std::io;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use jj_lib::hex_util::encode_hex;
use tempfile::NamedTempFile;

pub(crate) type Source = Box<dyn std::error::Error + Send + Sync>;

/// Encoded objects by ID.
#[async_trait]
pub(crate) trait Shelf: Send + Sync + Debug {
    /// The bytes named `id`, or `None` when the shelf lacks them.
    async fn get(&self, id: &[u8]) -> Result<Option<Vec<u8>>, Source>;

    /// Stores `bytes` under `id`, the hash of the bytes. Bytes already there
    /// stay, because they are the same.
    async fn put(&self, id: &[u8], bytes: Vec<u8>) -> Result<(), Source>;

    /// The IDs whose hexadecimal form starts with `prefix`.
    async fn ids_with_prefix(&self, prefix: &str) -> Result<Vec<Vec<u8>>, Source>;
}

/// A directory with one file per object, named by its ID in hexadecimal.
#[derive(Debug)]
pub(crate) struct Disk {
    dir: PathBuf,
}

impl Disk {
    pub(crate) fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
        }
    }

    fn path(&self, id: &[u8]) -> PathBuf {
        self.dir.join(encode_hex(id))
    }
}

#[async_trait]
impl Shelf for Disk {
    async fn get(&self, id: &[u8]) -> Result<Option<Vec<u8>>, Source> {
        match fs::read(self.path(id)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    async fn put(&self, id: &[u8], bytes: Vec<u8>) -> Result<(), Source> {
        let path = self.path(id);
        if path.try_exists()? {
            return Ok(());
        }
        fs::create_dir_all(&self.dir)?;

        // A reader sees the whole object or none of it, and a concurrent
        // writer of the same object wins with the same bytes.
        let mut file = NamedTempFile::new_in(&self.dir)?;
        file.write_all(&bytes)?;
        match file.persist_noclobber(&path) {
            Ok(_) => Ok(()),
            Err(_) if path.try_exists()? => Ok(()),
            Err(err) => Err(err.error.into()),
        }
    }

    async fn ids_with_prefix(&self, prefix: &str) -> Result<Vec<Vec<u8>>, Source> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };
        let mut ids = vec![];
        for entry in entries {
            let name = entry?.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            // A temporary file's name starts with a dot and is no hex.
            if let Some(id) = name
                .starts_with(prefix)
                .then(|| jj_lib::hex_util::decode_hex(name))
                .flatten()
            {
                ids.push(id);
            }
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn put_keeps_the_bytes_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::new(dir.path());

        block_on(disk.put(&[0xab], b"first".to_vec())).unwrap();
        block_on(disk.put(&[0xab], b"second".to_vec())).unwrap();

        assert_eq!(
            block_on(disk.get(&[0xab])).unwrap(),
            Some(b"first".to_vec())
        );
        assert_eq!(block_on(disk.get(&[0xcd])).unwrap(), None);
    }

    #[test]
    fn ids_with_prefix_skips_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::new(dir.path());
        for id in [[0xab, 1], [0xab, 2], [0xcd, 3]] {
            block_on(disk.put(&id, vec![])).unwrap();
        }
        fs::write(dir.path().join(".tmpab01"), b"").unwrap();

        let mut ids = block_on(disk.ids_with_prefix("ab")).unwrap();
        ids.sort();

        assert_eq!(ids, [vec![0xab, 1], vec![0xab, 2]]);
    }
}
