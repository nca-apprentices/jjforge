//! vcs's side: the repositories that it serves, each under
//! `t/{org}/r/{repo}/` in the object store, as ADR 0002 lays it out:
//!
//! ```text
//! pool/seg/{segment}   the objects of one upload
//! pool/idx/{segment}   where each object of the segment starts and ends
//! op/{hash}            operations and views
//! HEAD                 the current operation and review operation, moved
//!                      together by compare-and-swap
//! SERVED               present once vcs created the repository
//! ```
//!
//! An operation names the segments it made visible, so the log alone finds
//! every object. vcs builds an operation with jj-lib over its own index,
//! which it keeps in a local cache, and moves `HEAD` itself.

mod bucket;
mod repo;
mod segment;

pub use repo::Author;
pub use repo::Repo;
pub use repo::RepoError;
pub use repo::Repos;
pub use repo::Signer;
