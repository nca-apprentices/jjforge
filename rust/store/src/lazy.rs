//! `jf`'s shelf, which reads the trees and files of a clone only when jj
//! needs them, as #90 asks. A miss fetches the object from the forge and
//! keeps it on disk, so nothing is fetched twice.

use std::error::Error as StdError;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use crate::codec;
use crate::codec::Object;
use crate::ids;
use crate::shelf::Disk;
use crate::shelf::Shelf;
use crate::shelf::Source;

/// Fetches objects from the forge. `jf` implements it over `sync/v1`.
pub trait Fetch: Send + Sync + Debug {
    /// Returns the encoded objects named by `ids`, in any order.
    ///
    /// # Errors
    ///
    /// Fails when the forge can't answer or lacks an object.
    fn fetch(&self, ids: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, Box<dyn StdError + Send + Sync>>;
}

#[derive(Debug)]
pub(crate) struct Lazy {
    disk: Disk,
    fetch: Option<Arc<dyn Fetch>>,
}

impl Lazy {
    /// A shelf on `disk` that fetches what it lacks through `fetch`, or
    /// fetches nothing without it.
    pub(crate) fn new(disk: Disk, fetch: Option<Arc<dyn Fetch>>) -> Self {
        Self { disk, fetch }
    }

    /// Fetches `ids`, checks each object against its name, and keeps them.
    async fn fetch_and_keep(&self, fetch: &dyn Fetch, ids: &[Vec<u8>]) -> Result<(), Source> {
        for bytes in fetch.fetch(ids)? {
            let id = ids::hash(&bytes);
            if !ids.contains(&id) {
                return Err("the forge sent an object that wasn't asked for".into());
            }
            self.disk.put(&id, bytes).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Shelf for Lazy {
    /// A miss fetches the object. A tree also brings the entries it names
    /// that the disk lacks, in one more request, since a checkout or a diff
    /// reads them next.
    async fn get(&self, id: &[u8]) -> Result<Option<Vec<u8>>, Source> {
        if let Some(bytes) = self.disk.get(id).await? {
            return Ok(Some(bytes));
        }
        let Some(fetch) = &self.fetch else {
            return Ok(None);
        };

        self.fetch_and_keep(fetch.as_ref(), &[id.to_vec()]).await?;
        let Some(bytes) = self.disk.get(id).await? else {
            return Err("the forge didn't send an object it was asked for".into());
        };

        if let Ok(tree @ Object::Tree(_)) = codec::decode(&bytes) {
            let mut wanted = vec![];
            for entry in codec::references(&tree) {
                if !ids::is_virtual(&entry) && self.disk.get(&entry).await?.is_none() {
                    wanted.push(entry);
                }
            }
            if !wanted.is_empty() {
                self.fetch_and_keep(fetch.as_ref(), &wanted).await?;
            }
        }
        Ok(Some(bytes))
    }

    async fn put(&self, id: &[u8], bytes: Vec<u8>) -> Result<(), Source> {
        self.disk.put(id, bytes).await
    }

    async fn ids_with_prefix(&self, prefix: &str) -> Result<Vec<Vec<u8>>, Source> {
        self.disk.ids_with_prefix(prefix).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use futures::executor::block_on;
    use jj_lib::backend::CopyId;
    use jj_lib::backend::FileId;
    use jj_lib::backend::Tree;
    use jj_lib::backend::TreeValue;
    use jj_lib::repo_path::RepoPathComponentBuf;

    use super::*;

    /// A forge that holds `objects` and records each request.
    #[derive(Debug, Default)]
    struct Forge {
        objects: HashMap<Vec<u8>, Vec<u8>>,
        requests: Mutex<Vec<Vec<Vec<u8>>>>,
    }

    impl Forge {
        fn add(&mut self, bytes: Vec<u8>) -> Vec<u8> {
            let id = ids::hash(&bytes);
            self.objects.insert(id.clone(), bytes);
            id
        }
    }

    impl Fetch for Forge {
        fn fetch(&self, ids: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, Box<dyn StdError + Send + Sync>> {
            self.requests.lock().unwrap().push(ids.to_vec());
            ids.iter()
                .map(|id| self.objects.get(id).cloned().ok_or_else(|| "absent".into()))
                .collect()
        }
    }

    fn file(content: &str) -> Vec<u8> {
        codec::encode(&Object::File(content.as_bytes().to_vec())).unwrap()
    }

    #[test]
    fn a_tree_brings_its_entries_and_nothing_is_fetched_twice() {
        let mut forge = Forge::default();
        let a = forge.add(file("a"));
        let b = forge.add(file("b"));
        let entry = |id: &[u8]| TreeValue::File {
            id: FileId::new(id.to_vec()),
            executable: false,
            copy_id: CopyId::placeholder(),
        };
        let tree = Tree::from_sorted_entries(vec![
            (RepoPathComponentBuf::new("a").unwrap(), entry(&a)),
            (RepoPathComponentBuf::new("b").unwrap(), entry(&b)),
        ]);
        let tree = forge.add(codec::encode(&Object::Tree(tree)).unwrap());
        let forge = Arc::new(forge);
        let dir = tempfile::tempdir().unwrap();
        let lazy = Lazy::new(Disk::new(dir.path()), Some(forge.clone()));

        block_on(lazy.get(&tree)).unwrap().unwrap();
        block_on(lazy.get(&a)).unwrap().unwrap();
        block_on(lazy.get(&b)).unwrap().unwrap();
        block_on(lazy.get(&tree)).unwrap().unwrap();

        let requests = forge.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0], [tree]);
        assert_eq!(requests[1].len(), 2);
    }

    #[test]
    fn an_object_that_doesnt_match_its_name_fails() {
        #[derive(Debug)]
        struct Liar;
        impl Fetch for Liar {
            fn fetch(
                &self,
                _: &[Vec<u8>],
            ) -> Result<Vec<Vec<u8>>, Box<dyn StdError + Send + Sync>> {
                Ok(vec![file("not what was asked")])
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let lazy = Lazy::new(Disk::new(dir.path()), Some(Arc::new(Liar)));

        assert!(block_on(lazy.get(&ids::hash(&file("wanted")))).is_err());
    }

    #[test]
    fn without_a_forge_a_miss_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let lazy = Lazy::new(Disk::new(dir.path()), None);

        assert_eq!(block_on(lazy.get(&[1; 32])).unwrap(), None);
    }
}
