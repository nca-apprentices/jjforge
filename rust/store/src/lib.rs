//! The jj objects of a repository in the `store.v1` format, which vcs and
//! `jf` share, as ADR 0001 decides. jj-lib reads and writes them through the
//! store type `jjforge`.

mod backend;
mod bucket;
mod client;
mod codec;
mod forge;
mod ids;
mod lazy;
mod op_heads;
mod op_store;
mod segment;
mod shelf;

use std::error::Error as StdError;
use std::path::Path;
use std::sync::Arc;

pub use client::Fetched;
pub use client::ForgeDir;
pub use client::Pushed;
pub use client::Remote;
pub use client::RemoteError;
pub use client::SyncError;
pub use client::fetch;
pub use client::follow_working_copy;
pub use client::push;
pub use client::start_on_main;
pub use client::undo_fetch;
pub use codec::Error;
pub use codec::Object;
pub use codec::decode;
pub use codec::encode;
pub use forge::Author;
pub use forge::Head;
pub use forge::Published;
pub use forge::Repo;
pub use forge::RepoError;
pub use forge::Repos;
use jj_lib::backend::BackendLoadError;
use jj_lib::config::ConfigLayer;
use jj_lib::config::ConfigSource;
use jj_lib::default_backend_factories::default_working_copy_factory;
use jj_lib::ref_name::WorkspaceName;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::StoreFactories;
use jj_lib::settings::UserSettings;
use jj_lib::signing_factory::signer_from_settings;
use jj_lib::workspace::Workspace;
use jj_lib::workspace::WorkspaceInitError;
pub use lazy::Fetch;

use crate::lazy::Lazy;
use crate::shelf::Disk;

/// The store type, which jj-lib writes to `.jj/repo/*/type`.
const NAME: &str = "jjforge";

/// Finds the forge of the repository in a directory, or `None` for a
/// repository that has none. `jf` implements it over `sync/v1`.
pub type FindForge =
    dyn Fn(&Path) -> Result<Option<Arc<dyn Fetch>>, Box<dyn StdError + Send + Sync>>;

/// Returns the jj-lib `StoreFactories` that load the store type `jjforge`.
/// The backend asks `find_forge` for the forge of the repository, and
/// fetches what it lacks from there.
#[must_use]
pub fn factories(find_forge: Arc<FindForge>) -> StoreFactories {
    let mut factories = StoreFactories::empty();
    factories.add_backend(
        NAME,
        Box::new(move |_, dir| {
            let repo = dir.parent().unwrap_or(dir);
            let fetch = find_forge(repo).map_err(BackendLoadError)?;
            Ok(Box::new(local_backend(dir, fetch)))
        }),
    );
    factories.add_op_store(
        NAME,
        Box::new(|_, dir, _| Ok(Box::new(op_store::Store::new(Box::new(Disk::new(dir)))))),
    );
    factories.add_op_heads_store(
        NAME,
        Box::new(|_, dir| Ok(Box::new(op_heads::Store::load(dir)))),
    );
    factories
}

fn local_backend(dir: &Path, fetch: Option<Arc<dyn Fetch>>) -> backend::Store {
    // A local disk serves one read at a time as fast as several.
    backend::Store::new(Box::new(Lazy::new(Disk::new(dir), fetch)), 1)
}

/// Creates a workspace in `root` on a new repository of the store type
/// `jjforge`. The index, the working copy, and the workspace store are
/// jj-lib's defaults.
///
/// # Errors
///
/// Fails when `root` already holds a workspace, when a store can't be
/// written, or when the signing settings are wrong.
pub async fn init(
    settings: &UserSettings,
    root: &Path,
) -> Result<(Workspace, Arc<ReadonlyRepo>), WorkspaceInitError> {
    Workspace::init_with_factories(
        settings,
        root,
        &|_, dir| Ok(Box::new(local_backend(dir, None))),
        signer_from_settings(settings)?,
        ReadonlyRepo::default_workspace_store_initializer(),
        &|_, dir, _| Ok(Box::new(op_store::Store::new(Box::new(Disk::new(dir))))),
        &|_, dir, root_operation_id| Ok(Box::new(op_heads::Store::init(dir, root_operation_id)?)),
        ReadonlyRepo::default_index_store_initializer(),
        ReadonlyRepo::default_submodule_store_initializer(),
        &*default_working_copy_factory(),
        WorkspaceName::DEFAULT.to_owned(),
    )
    .await
}

/// Returns the configuration that `jf` adds to jj's defaults.
///
/// `store.v1` drops a commit's `predecessors`, which jj deprecates, so jj-lib
/// records them only in the operation.
#[must_use]
pub fn config() -> ConfigLayer {
    let mut layer = ConfigLayer::empty(ConfigSource::Default);
    // An empty layer takes any value at a valid name, so this never fails.
    layer
        .set_value("experimental.record-predecessors-in-commit", false)
        .ok();
    layer
}

#[cfg(test)]
mod tests {
    use jj_lib::config::StackedConfig;

    use super::*;

    #[test]
    fn config_turns_off_predecessors_in_commits() {
        let mut stacked = StackedConfig::with_defaults();
        stacked.add_layer(config());

        let record = stacked.get::<bool>("experimental.record-predecessors-in-commit");

        assert!(!record.unwrap());
    }
}
