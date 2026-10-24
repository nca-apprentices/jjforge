//! jj-lib's stores over a [`Shelf`] of encoded objects, which `jf` keeps on
//! disk and vcs in the object store.

mod backend;
mod op_heads;
mod op_store;
mod shelf;

pub(crate) use backend::Store as Backend;
pub(crate) use op_heads::Store as OperationHeads;
pub(crate) use op_store::Store as OperationStore;
pub(crate) use shelf::Disk;
pub(crate) use shelf::Shelf;
pub(crate) use shelf::Source;
