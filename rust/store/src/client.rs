//! `jf`'s side: the store type that jj-cli loads, and the sync of a clone
//! with its [`Forge`], over the [`Transport`] that `jf` implements with
//! `sync/v1`.

mod lazy;
mod sync;
mod workspace;

pub use lazy::Fetch;
pub use lazy::TransportError;
pub use sync::Forge;
pub use sync::SyncError;
pub use sync::Synced;
pub use sync::Transport;
pub use sync::follow_working_copy;
pub use sync::forge_url;
pub use sync::set_forge_url;
pub use sync::start_on_main;
pub use sync::undo_fetch;
pub use workspace::FindForge;
pub use workspace::config;
pub use workspace::factories;
pub use workspace::init;
