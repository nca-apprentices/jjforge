//! jj-lib's `Backend` over `store.v1` objects on a shelf, as ADR 0001
//! decides.

use std::pin::Pin;

use async_trait::async_trait;
use futures::AsyncRead;
use futures::AsyncReadExt as _;
use futures::StreamExt as _;
use futures::io::Cursor;
use futures::stream;
use futures::stream::BoxStream;
use jj_lib::backend::Backend;
use jj_lib::backend::BackendError;
use jj_lib::backend::BackendResult;
use jj_lib::backend::ChangeId;
use jj_lib::backend::Commit;
use jj_lib::backend::CommitId;
use jj_lib::backend::CopyHistory;
use jj_lib::backend::CopyId;
use jj_lib::backend::CopyRecord;
use jj_lib::backend::FileId;
use jj_lib::backend::RelatedCopy;
use jj_lib::backend::SecureSig;
use jj_lib::backend::SigningFn;
use jj_lib::backend::SymlinkId;
use jj_lib::backend::Tree;
use jj_lib::backend::TreeId;
use jj_lib::backend::make_root_commit;
use jj_lib::object_id::ObjectId;
use jj_lib::repo_path::RepoPath;
use jj_lib::repo_path::RepoPathBuf;

use super::shelf::Shelf;
use super::shelf::Source;
use crate::codec;
use crate::codec::Object;
use crate::ids;

/// The objects of one repository.
#[derive(Debug)]
pub(crate) struct Store {
    shelf: Box<dyn Shelf>,
    concurrency: usize,
    root_commit_id: CommitId,
    root_change_id: ChangeId,
    empty_tree_id: TreeId,
}

impl Store {
    /// A store over `shelf`, which jj-lib reads `concurrency` objects of at
    /// once.
    pub(crate) fn new(shelf: Box<dyn Shelf>, concurrency: usize) -> Self {
        Self {
            shelf,
            concurrency,
            root_commit_id: ids::root_commit_id(),
            root_change_id: ids::root_change_id(),
            empty_tree_id: ids::empty_tree_id(),
        }
    }

    async fn get(&self, id: &impl ObjectId) -> BackendResult<Object> {
        let bytes = self
            .shelf
            .get(id.as_bytes())
            .await
            .map_err(|err| read_error(id, err))?
            .ok_or_else(|| BackendError::ObjectNotFound {
                object_type: id.object_type(),
                hash: id.hex(),
                source: None,
            })?;
        if ids::hash(&bytes) != id.as_bytes() {
            return Err(read_error(id, "the bytes don't match their ID"));
        }
        codec::decode(&bytes).map_err(|err| read_error(id, err))
    }

    /// Writes `object` and returns its ID and its bytes.
    async fn put(
        &self,
        object: &Object,
        object_type: &'static str,
    ) -> BackendResult<(Vec<u8>, Vec<u8>)> {
        let write_error = |source: Source| BackendError::WriteObject {
            object_type,
            source,
        };
        let bytes = codec::encode(object).map_err(|err| write_error(err.into()))?;
        let id = ids::hash(&bytes);
        self.shelf
            .put(&id, bytes.clone())
            .await
            .map_err(write_error)?;
        Ok((id, bytes))
    }
}

fn read_error(id: &impl ObjectId, source: impl Into<Source>) -> BackendError {
    BackendError::ReadObject {
        object_type: id.object_type(),
        hash: id.hex(),
        source: source.into(),
    }
}

/// The ID names an object of another kind.
fn other_kind(id: &impl ObjectId) -> BackendError {
    read_error(id, format!("the object is not a {}", id.object_type()))
}

#[async_trait]
impl Backend for Store {
    fn name(&self) -> &str {
        crate::NAME
    }

    fn commit_id_length(&self) -> usize {
        ids::ID_LENGTH
    }

    fn change_id_length(&self) -> usize {
        ids::CHANGE_ID_LENGTH
    }

    fn root_commit_id(&self) -> &CommitId {
        &self.root_commit_id
    }

    fn root_change_id(&self) -> &ChangeId {
        &self.root_change_id
    }

    fn empty_tree_id(&self) -> &TreeId {
        &self.empty_tree_id
    }

    fn concurrency(&self) -> usize {
        self.concurrency
    }

    async fn read_file(
        &self,
        _path: &RepoPath,
        id: &FileId,
    ) -> BackendResult<Pin<Box<dyn AsyncRead + Send>>> {
        let Object::File(content) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(Box::pin(Cursor::new(content)))
    }

    async fn write_file(
        &self,
        _path: &RepoPath,
        contents: &mut (dyn AsyncRead + Send + Unpin),
    ) -> BackendResult<FileId> {
        let mut content = vec![];
        contents
            .read_to_end(&mut content)
            .await
            .map_err(|err| BackendError::WriteObject {
                object_type: "file",
                source: err.into(),
            })?;
        let (id, _) = self.put(&Object::File(content), "file").await?;
        Ok(FileId::new(id))
    }

    async fn read_symlink(&self, _path: &RepoPath, id: &SymlinkId) -> BackendResult<String> {
        let Object::Symlink(target) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(target)
    }

    async fn write_symlink(&self, _path: &RepoPath, target: &str) -> BackendResult<SymlinkId> {
        let (id, _) = self
            .put(&Object::Symlink(target.to_owned()), "symlink")
            .await?;
        Ok(SymlinkId::new(id))
    }

    async fn read_copy(&self, id: &CopyId) -> BackendResult<CopyHistory> {
        let Object::CopyHistory(history) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(history)
    }

    async fn write_copy(&self, copy: &CopyHistory) -> BackendResult<CopyId> {
        let (id, _) = self
            .put(&Object::CopyHistory(copy.clone()), "copy history")
            .await?;
        Ok(CopyId::new(id))
    }

    /// Finding descendants needs an index of copies, which the store doesn't
    /// keep.
    async fn get_related_copies(&self, _copy_id: &CopyId) -> BackendResult<Vec<RelatedCopy>> {
        Err(BackendError::Unsupported(
            "the jjforge store doesn't find related copies".to_owned(),
        ))
    }

    async fn read_tree(&self, _path: &RepoPath, id: &TreeId) -> BackendResult<Tree> {
        if *id == self.empty_tree_id {
            return Ok(Tree::default());
        }
        let Object::Tree(tree) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(tree)
    }

    async fn write_tree(&self, _path: &RepoPath, contents: &Tree) -> BackendResult<TreeId> {
        let (id, _) = self.put(&Object::Tree(contents.clone()), "tree").await?;
        Ok(TreeId::new(id))
    }

    async fn read_commit(&self, id: &CommitId) -> BackendResult<Commit> {
        if *id == self.root_commit_id {
            return Ok(make_root_commit(
                self.root_change_id.clone(),
                self.empty_tree_id.clone(),
            ));
        }
        let Object::Commit(commit) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(*commit)
    }

    /// Returns the commit as `store.v1` holds it, which has no
    /// `predecessors`.
    async fn write_commit(
        &self,
        mut contents: Commit,
        sign_with: Option<&mut SigningFn>,
    ) -> BackendResult<(CommitId, Commit)> {
        if contents.parents.is_empty() {
            return Err(BackendError::Other(
                "Cannot write a commit with no parents".into(),
            ));
        }
        if let Some(sign) = sign_with {
            let data = codec::signed_data(&contents);
            let sig = sign(&data).map_err(|err| BackendError::WriteObject {
                object_type: "commit",
                source: err.into(),
            })?;
            contents.secure_sig = Some(SecureSig { data, sig });
        }

        let (id, bytes) = self
            .put(&Object::Commit(Box::new(contents)), "commit")
            .await?;
        let id = CommitId::new(id);
        let Object::Commit(stored) = codec::decode(&bytes).map_err(|err| read_error(&id, err))?
        else {
            return Err(other_kind(&id));
        };
        Ok((id, *stored))
    }

    /// The store records no copies yet.
    fn get_copy_records(
        &self,
        _paths: Option<&[RepoPathBuf]>,
        _root: &CommitId,
        _head: &CommitId,
    ) -> BackendResult<BoxStream<'_, BackendResult<CopyRecord>>> {
        Ok(stream::empty().boxed())
    }
}

/// Ported from jj-lib's tests of `SimpleBackend`, since jj-lib publishes no
/// conformance tests for a backend, as ADR 0001 decides.
#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use jj_lib::backend::MillisSinceEpoch;
    use jj_lib::backend::Signature;
    use jj_lib::backend::Timestamp;
    use jj_lib::backend::TreeValue;
    use jj_lib::merge::Merge;
    use jj_lib::repo_path::RepoPathComponentBuf;
    use tempfile::TempDir;

    use super::*;
    use crate::jj::Disk;

    fn store() -> (TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Box::new(Disk::new(dir.path())), 1);
        (dir, store)
    }

    fn signature() -> Signature {
        Signature {
            name: "Someone".to_owned(),
            email: "someone@example.com".to_owned(),
            timestamp: Timestamp {
                timestamp: MillisSinceEpoch(0),
                tz_offset: 0,
            },
        }
    }

    fn commit(store: &Store) -> Commit {
        Commit {
            parents: vec![store.root_commit_id().clone()],
            predecessors: vec![],
            root_tree: Merge::resolved(store.empty_tree_id().clone()),
            conflict_labels: Merge::resolved(String::new()),
            change_id: ChangeId::new(vec![7; ids::CHANGE_ID_LENGTH]),
            description: String::new(),
            author: signature(),
            committer: signature(),
            secure_sig: None,
        }
    }

    #[test]
    fn root_commit_and_empty_tree_need_no_write() {
        let (_dir, store) = store();

        let root = block_on(store.read_commit(store.root_commit_id())).unwrap();
        let tree = block_on(store.read_tree(RepoPath::root(), store.empty_tree_id())).unwrap();

        assert_eq!(root.parents, []);
        assert_eq!(root.change_id, *store.root_change_id());
        assert_eq!(tree, Tree::default());
    }

    #[test]
    fn write_commit_parents() {
        let (_dir, store) = store();
        let mut commit = commit(&store);

        commit.parents = vec![];
        assert!(block_on(store.write_commit(commit.clone(), None)).is_err());

        commit.parents = vec![store.root_commit_id().clone()];
        let first_id = block_on(store.write_commit(commit.clone(), None))
            .unwrap()
            .0;
        let first = block_on(store.read_commit(&first_id)).unwrap();
        assert_eq!(first, commit);

        commit.parents = vec![first_id.clone()];
        let second_id = block_on(store.write_commit(commit.clone(), None))
            .unwrap()
            .0;
        commit.parents = vec![first_id, second_id];
        let merge_id = block_on(store.write_commit(commit.clone(), None))
            .unwrap()
            .0;
        assert_eq!(block_on(store.read_commit(&merge_id)).unwrap(), commit);
    }

    #[test]
    fn write_commit_returns_it_without_predecessors() {
        let (_dir, store) = store();
        let mut commit = commit(&store);
        commit.predecessors = vec![CommitId::new(vec![9; ids::ID_LENGTH])];

        let (id, stored) = block_on(store.write_commit(commit, None)).unwrap();

        assert_eq!(stored.predecessors, []);
        assert_eq!(block_on(store.read_commit(&id)).unwrap(), stored);
    }

    #[test]
    fn write_commit_signs_the_commit_without_its_signature() {
        let (_dir, store) = store();
        let mut sign = |data: &[u8]| Ok(data.iter().rev().copied().collect());

        let (id, stored) = block_on(store.write_commit(commit(&store), Some(&mut sign))).unwrap();

        let secure_sig = stored.secure_sig.clone().unwrap();
        assert_eq!(
            secure_sig.sig,
            secure_sig.data.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(block_on(store.read_commit(&id)).unwrap(), stored);
    }

    #[test]
    fn files_symlinks_and_trees_return_equal() {
        let (_dir, store) = store();
        let path = RepoPath::root();

        let file = block_on(store.write_file(path, &mut Cursor::new(b"content".to_vec()))).unwrap();
        let mut read = String::new();
        block_on(
            block_on(store.read_file(path, &file))
                .unwrap()
                .read_to_string(&mut read),
        )
        .unwrap();
        assert_eq!(read, "content");

        let symlink = block_on(store.write_symlink(path, "../target")).unwrap();
        assert_eq!(
            block_on(store.read_symlink(path, &symlink)).unwrap(),
            "../target"
        );

        let tree = Tree::from_sorted_entries(vec![
            (
                RepoPathComponentBuf::new("file").unwrap(),
                TreeValue::File {
                    id: file,
                    executable: false,
                    copy_id: CopyId::placeholder(),
                },
            ),
            (
                RepoPathComponentBuf::new("link").unwrap(),
                TreeValue::Symlink(symlink),
            ),
        ]);
        let tree_id = block_on(store.write_tree(path, &tree)).unwrap();
        assert_eq!(block_on(store.read_tree(path, &tree_id)).unwrap(), tree);
    }

    #[test]
    fn read_of_another_kind_fails() {
        let (_dir, store) = store();
        let file = block_on(
            store.write_file(RepoPath::root(), &mut Cursor::new(b"not a commit".to_vec())),
        )
        .unwrap();

        let result = block_on(store.read_commit(&CommitId::new(file.to_bytes())));

        assert!(matches!(result, Err(BackendError::ReadObject { .. })));
    }

    #[test]
    fn read_of_an_absent_object_is_not_found() {
        let (_dir, store) = store();

        let result = block_on(store.read_commit(&CommitId::new(vec![1; ids::ID_LENGTH])));

        assert!(matches!(result, Err(BackendError::ObjectNotFound { .. })));
    }
}
