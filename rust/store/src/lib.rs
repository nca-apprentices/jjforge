//! The jj objects of a repository in the `store.v1` format, which vcs and
//! `jf` share, as ADR 0001 decides. jj-lib reads and writes them through the
//! store type `jjforge`.
//!
//! Each crate that uses the store has a module of its own, as ADR 0003
//! decides:
//!
//! - [`client`] is `jf`'s side: the store type for jj-cli, and the fetch
//!   and push of a clone over a [`client::Transport`].
//! - [`server`] is vcs's side: the [`server::Repos`] in the object store
//!   that `sync/v1` serves.
//!
//! Both sides name operations, objects, and segments by the IDs at the
//! root, and speak the messages of `sync/v1` from `jjforge-proto`.

#![deny(missing_docs)]

pub mod client;
pub mod server;

mod backend;
mod bucket;
mod codec;
mod ids;
mod lazy;
mod op_heads;
mod op_store;
mod segment;
mod shelf;
mod workspace;

pub use ids::InvalidId;
pub use ids::ObjectId;
pub use ids::OperationId;
pub use ids::SegmentId;

/// What a publish did, as `sync/v1` answers it.
#[derive(Debug, PartialEq, Eq)]
pub enum Published {
    /// `HEAD` moved to the new operation, with this ID.
    Moved(OperationId),
    /// The client's view was stale: `HEAD`, with this ID, or a bookmark had
    /// moved. Nothing was written that anyone reads.
    Stale(OperationId),
}

/// The store type, which jj-lib writes to `.jj/repo/*/type`.
const NAME: &str = "jjforge";
