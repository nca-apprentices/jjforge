//! The jj objects of a repository in the `store.v1` format, which vcs and
//! `jf` share, as ADR 0001 decides.

mod codec;

pub use codec::Error;
pub use codec::Object;
pub use codec::decode;
pub use codec::encode;
