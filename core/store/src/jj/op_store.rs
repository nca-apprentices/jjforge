//! jj-lib's `OpStore` over `store.v1` operations and views on a shelf, as
//! ADR 0002 lays them out under `op/{hash}`.

use std::time::SystemTime;

use async_trait::async_trait;
use jj_lib::backend::CommitId;
use jj_lib::object_id::HexPrefix;
use jj_lib::object_id::ObjectId;
use jj_lib::object_id::PrefixResolution;
use jj_lib::op_store::OpStore;
use jj_lib::op_store::OpStoreError;
use jj_lib::op_store::OpStoreResult;
use jj_lib::op_store::Operation;
use jj_lib::op_store::OperationId;
use jj_lib::op_store::View;
use jj_lib::op_store::ViewId;

use super::shelf::Shelf;
use super::shelf::Source;
use crate::codec;
use crate::codec::Object;
use crate::ids;

#[derive(Debug)]
pub(crate) struct Store {
    shelf: Box<dyn Shelf>,
    root_operation_id: OperationId,
    root_view_id: ViewId,
    root_commit_id: CommitId,
}

impl Store {
    /// A store over `shelf`. A new one needs nothing written, because the
    /// root operation and its view are virtual, as in jj-lib's own op store.
    pub(crate) fn new(shelf: Box<dyn Shelf>) -> Self {
        Self {
            shelf,
            root_operation_id: ids::root_operation_id(),
            root_view_id: ids::root_view_id(),
            root_commit_id: ids::root_commit_id(),
        }
    }

    async fn get(&self, id: &impl ObjectId) -> OpStoreResult<Object> {
        let bytes = self
            .shelf
            .get(id.as_bytes())
            .await
            .map_err(|err| read_error(id, err))?
            .ok_or_else(|| OpStoreError::ObjectNotFound {
                object_type: id.object_type(),
                hash: id.hex(),
                source: None,
            })?;
        if ids::hash(&bytes) != id.as_bytes() {
            return Err(read_error(id, "the bytes don't match their ID"));
        }
        codec::decode(&bytes).map_err(|err| read_error(id, err))
    }

    async fn put(&self, object: &Object, object_type: &'static str) -> OpStoreResult<Vec<u8>> {
        let write_error = |source: Source| OpStoreError::WriteObject {
            object_type,
            source,
        };
        let bytes = codec::encode(object).map_err(|err| write_error(err.into()))?;
        let id = ids::hash(&bytes);
        self.shelf.put(&id, bytes).await.map_err(write_error)?;
        Ok(id)
    }
}

fn read_error(id: &impl ObjectId, source: impl Into<Source>) -> OpStoreError {
    OpStoreError::ReadObject {
        object_type: id.object_type(),
        hash: id.hex(),
        source: source.into(),
    }
}

/// The ID names an object of another kind.
fn other_kind(id: &impl ObjectId) -> OpStoreError {
    read_error(id, format!("the object is not a {}", id.object_type()))
}

#[async_trait]
impl OpStore for Store {
    fn name(&self) -> &str {
        crate::NAME
    }

    fn root_operation_id(&self) -> &OperationId {
        &self.root_operation_id
    }

    async fn read_view(&self, id: &ViewId) -> OpStoreResult<View> {
        if *id == self.root_view_id {
            return Ok(View::make_root(self.root_commit_id.clone()));
        }
        let Object::View(view) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(*view)
    }

    async fn write_view(&self, contents: &View) -> OpStoreResult<ViewId> {
        let id = self
            .put(&Object::View(Box::new(contents.clone())), "view")
            .await?;
        Ok(ViewId::new(id))
    }

    async fn read_operation(&self, id: &OperationId) -> OpStoreResult<Operation> {
        if *id == self.root_operation_id {
            return Ok(Operation::make_root(self.root_view_id.clone()));
        }
        let Object::Operation(operation) = self.get(id).await? else {
            return Err(other_kind(id));
        };
        Ok(*operation)
    }

    /// Only the root operation has no parents.
    async fn write_operation(&self, contents: &Operation) -> OpStoreResult<OperationId> {
        if contents.parents.is_empty() {
            return Err(OpStoreError::WriteObject {
                object_type: "operation",
                source: "an operation needs a parent".into(),
            });
        }
        let id = self
            .put(&Object::Operation(Box::new(contents.clone())), "operation")
            .await?;
        Ok(OperationId::new(id))
    }

    async fn resolve_operation_id_prefix(
        &self,
        prefix: &HexPrefix,
    ) -> OpStoreResult<PrefixResolution<OperationId>> {
        let mut matched = prefix
            .matches(&self.root_operation_id)
            .then(|| self.root_operation_id.clone());
        let candidates = self
            .shelf
            .ids_with_prefix(&prefix.hex())
            .await
            .map_err(OpStoreError::Other)?;
        for id in candidates {
            let id = OperationId::new(id);
            // Views share the shelf, and only an operation matches.
            if !matches!(self.get(&id).await?, Object::Operation(_)) {
                continue;
            }
            if matched.is_some() {
                return Ok(PrefixResolution::AmbiguousMatch);
            }
            matched = Some(id);
        }
        Ok(matched.map_or(PrefixResolution::NoMatch, PrefixResolution::SingleMatch))
    }

    /// Nothing is collected, as ADR 0002 decides.
    async fn gc(&self, _head_ids: &[OperationId], _keep_newer: SystemTime) -> OpStoreResult<()> {
        Ok(())
    }
}

/// Ported from jj-lib's tests of `SimpleOpStore`, without the git refs and
/// git heads that `store.v1` leaves out.
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::HashSet;

    use futures::executor::block_on;
    use jj_lib::backend::MillisSinceEpoch;
    use jj_lib::backend::Timestamp;
    use jj_lib::op_store::OperationMetadata;
    use jj_lib::op_store::RefTarget;
    use jj_lib::op_store::RemoteRef;
    use jj_lib::op_store::RemoteRefState;
    use jj_lib::op_store::RemoteView;
    use jj_lib::op_store::TimestampRange;
    use jj_lib::ref_name::WorkspaceName;
    use tempfile::TempDir;

    use super::*;
    use crate::jj::Disk;

    fn store() -> (TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Box::new(Disk::new(dir.path())));
        (dir, store)
    }

    fn view() -> View {
        fn remote_ref(hex: &'static str, state: RemoteRefState) -> RemoteRef {
            RemoteRef {
                target: RefTarget::normal(CommitId::from_hex(hex)),
                state,
            }
        }
        View {
            head_ids: HashSet::from([CommitId::from_hex("aaa111"), CommitId::from_hex("aaa222")]),
            local_bookmarks: BTreeMap::from([(
                "main".into(),
                RefTarget::normal(CommitId::from_hex("ccc111")),
            )]),
            local_tags: BTreeMap::from([(
                "v1.0".into(),
                RefTarget::normal(CommitId::from_hex("ddd111")),
            )]),
            remote_views: BTreeMap::from([(
                "origin".into(),
                RemoteView {
                    bookmarks: BTreeMap::from([
                        ("main".into(), remote_ref("ccc222", RemoteRefState::Tracked)),
                        ("deleted".into(), remote_ref("ccc333", RemoteRefState::New)),
                    ]),
                    tags: BTreeMap::from([
                        ("v1.0".into(), remote_ref("ddd222", RemoteRefState::Tracked)),
                        ("deleted".into(), remote_ref("ddd333", RemoteRefState::New)),
                    ]),
                },
            )]),
            git_refs: BTreeMap::new(),
            git_heads: BTreeMap::new(),
            wc_commit_ids: BTreeMap::from([
                (
                    WorkspaceName::DEFAULT.to_owned(),
                    CommitId::from_hex("abc111"),
                ),
                ("test".into(), CommitId::from_hex("abc222")),
            ]),
        }
    }

    fn operation() -> Operation {
        let at = |millis| Timestamp {
            timestamp: MillisSinceEpoch(millis),
            tz_offset: 3600,
        };
        Operation {
            view_id: ViewId::new(vec![0xaa; ids::ID_LENGTH]),
            parents: vec![
                OperationId::new(vec![0xb1; ids::ID_LENGTH]),
                OperationId::new(vec![0xb2; ids::ID_LENGTH]),
            ],
            metadata: OperationMetadata {
                time: TimestampRange {
                    start: at(123_456_789),
                    end: at(123_456_800),
                },
                description: "check out foo".to_owned(),
                hostname: "some.host.example.com".to_owned(),
                username: "someone".to_owned(),
                is_snapshot: false,
                workspace_name: Some("test".into()),
                attributes: BTreeMap::from([("key1".to_owned(), "value1".to_owned())]),
            },
            commit_predecessors: Some(BTreeMap::from([
                (CommitId::from_hex("111111"), vec![]),
                (
                    CommitId::from_hex("222222"),
                    vec![CommitId::from_hex("333333"), CommitId::from_hex("444444")],
                ),
            ])),
        }
    }

    #[test]
    fn read_write_view() {
        let (_dir, store) = store();
        let view = view();

        let id = block_on(store.write_view(&view)).unwrap();

        assert_eq!(block_on(store.read_view(&id)).unwrap(), view);
    }

    #[test]
    fn read_write_operation() {
        let (_dir, store) = store();
        let operation = operation();

        let id = block_on(store.write_operation(&operation)).unwrap();

        assert_eq!(block_on(store.read_operation(&id)).unwrap(), operation);
    }

    #[test]
    fn root_operation_and_view_are_virtual() {
        let (_dir, store) = store();

        let root = block_on(store.read_operation(store.root_operation_id())).unwrap();

        assert_eq!(root.parents, []);
        let view = block_on(store.read_view(&root.view_id)).unwrap();
        assert_eq!(view, View::make_root(ids::root_commit_id()));
    }

    #[test]
    fn write_operation_without_parents_fails() {
        let (_dir, store) = store();
        let mut operation = operation();
        operation.parents = vec![];

        assert!(block_on(store.write_operation(&operation)).is_err());
    }

    #[test]
    fn resolve_prefix_finds_operations_only() {
        let (_dir, store) = store();
        let id = block_on(store.write_operation(&operation())).unwrap();
        block_on(store.write_view(&view())).unwrap();

        let full = block_on(store.resolve_operation_id_prefix(&HexPrefix::from_id(&id))).unwrap();
        let none = block_on(
            store.resolve_operation_id_prefix(&HexPrefix::try_from_hex("ffffffffffff").unwrap()),
        )
        .unwrap();

        assert_eq!(full, PrefixResolution::SingleMatch(id));
        assert_eq!(none, PrefixResolution::NoMatch);
    }
}
