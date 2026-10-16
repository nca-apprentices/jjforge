//! The IDs that `store.v1` gives objects: the BLAKE3 hash of their bytes, as
//! ADR 0001 decides, and the virtual roots that jj-lib expects.

use std::fmt;
use std::str::FromStr;

use jj_lib::backend::ChangeId;
use jj_lib::backend::CommitId;
use jj_lib::backend::Tree;
use jj_lib::backend::TreeId;
use jj_lib::hex_util::decode_hex;
use jj_lib::hex_util::encode_hex;
use jj_lib::object_id::ObjectId as _;
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

pub(crate) fn root_operation_id() -> jj_lib::op_store::OperationId {
    jj_lib::op_store::OperationId::new(vec![0; ID_LENGTH])
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

/// Bytes or text that aren't a 32-byte ID.
#[derive(Debug, thiserror::Error)]
#[error("{0:?} is not a 32-byte ID in hexadecimal")]
pub struct InvalidId(String);

/// Declares an ID type that holds a BLAKE3 hash. Each reads and writes the
/// bytes that `sync/v1` carries and the hexadecimal form that its paths and
/// `ETag` carry, so no caller handles a raw `Vec<u8>`.
macro_rules! hash_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name([u8; ID_LENGTH]);

        impl $name {
            /// The ID's bytes, as `sync/v1` carries them.
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                &self.0
            }

            /// The ID's bytes, owned, as a `sync/v1` message holds them.
            #[must_use]
            pub fn to_vec(&self) -> Vec<u8> {
                self.0.to_vec()
            }
        }

        impl TryFrom<&[u8]> for $name {
            type Error = InvalidId;

            fn try_from(bytes: &[u8]) -> Result<Self, InvalidId> {
                bytes
                    .try_into()
                    .map(Self)
                    .map_err(|_| InvalidId(encode_hex(bytes)))
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;

            fn from_str(hex: &str) -> Result<Self, InvalidId> {
                decode_hex(hex)
                    .and_then(|bytes| Self::try_from(bytes.as_slice()).ok())
                    .ok_or_else(|| InvalidId(hex.to_owned()))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&encode_hex(&self.0))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({self})", stringify!($name))
            }
        }
    };
}

hash_id! {
    /// An operation of a repository's log, which `HEAD` names.
    OperationId
}

hash_id! {
    /// An object of a repository: a file, a symlink, a tree, a copy history,
    /// or a commit.
    ObjectId
}

hash_id! {
    /// A segment, the objects of one upload, which a publish names.
    SegmentId
}

impl OperationId {
    /// The virtual root operation, at which every repository starts.
    #[must_use]
    pub fn root() -> Self {
        Self([0; ID_LENGTH])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_reads_back_its_hexadecimal_form() {
        let id = ObjectId::try_from([7; ID_LENGTH].as_slice()).unwrap();

        assert_eq!(id.to_string().parse::<ObjectId>().unwrap(), id);
    }

    #[test]
    fn an_id_refuses_a_short_hash() {
        assert!(OperationId::try_from([7; 31].as_slice()).is_err());
        assert!("0707".parse::<SegmentId>().is_err());
    }
}
