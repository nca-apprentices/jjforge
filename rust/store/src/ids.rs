//! The IDs that `store.v1` gives objects: the BLAKE3 hash of their bytes, as
//! ADR 0001 decides, and the virtual roots that jj-lib expects.

use jj_lib::backend::ChangeId;
use jj_lib::backend::CommitId;
use jj_lib::backend::Tree;
use jj_lib::backend::TreeId;
use jj_lib::object_id::ObjectId as _;
use jj_lib::op_store::OperationId;
use jj_lib::op_store::ViewId;

use crate::codec;
use crate::codec::Object;

/// Every ID but a change ID is a 32-byte BLAKE3 hash.
pub(crate) const ID_LENGTH: usize = 32;
pub(crate) const CHANGE_ID_LENGTH: usize = 16;

/// The name of encoded bytes.
pub(crate) fn hash(bytes: &[u8]) -> Vec<u8> {
    blake3::hash(bytes).as_bytes().to_vec()
}

/// The root commit, operation, and view are virtual, so their IDs are zeros,
/// as in jj-lib's own stores.
pub(crate) fn root_commit_id() -> CommitId {
    CommitId::new(vec![0; ID_LENGTH])
}

pub(crate) fn root_change_id() -> ChangeId {
    ChangeId::new(vec![0; CHANGE_ID_LENGTH])
}

pub(crate) fn root_operation_id() -> OperationId {
    OperationId::new(vec![0; ID_LENGTH])
}

pub(crate) fn root_view_id() -> ViewId {
    ViewId::new(vec![0; ID_LENGTH])
}

/// The empty tree is virtual too, but it is an ordinary encoded object, so
/// its ID is the hash of its bytes.
pub(crate) fn empty_tree_id() -> TreeId {
    TreeId::new(hash(&empty_tree()))
}

pub(crate) fn empty_tree() -> Vec<u8> {
    // An empty tree has no entry that store.v1 could refuse.
    codec::encode(&Object::Tree(Tree::default())).unwrap_or_default()
}

/// Whether an ID names an object that every repository holds without
/// storing it.
pub(crate) fn is_virtual(id: &[u8]) -> bool {
    id == root_commit_id().as_bytes() || id == empty_tree_id().as_bytes()
}
