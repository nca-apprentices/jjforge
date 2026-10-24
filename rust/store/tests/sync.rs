//! Clones push to and fetch from one forge repository, in process. The
//! forge keeps its objects in memory and answers through the same calls vcs
//! serves over `sync/v1`.

// The helpers panic like the tests that call them, since a panic here is the
// test failing, as clippy.toml allows inside a test.
#![allow(clippy::unwrap_used, reason = "a panic in a test is the failure")]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use futures::AsyncReadExt as _;
use futures::executor::block_on;
use jj_lib::backend::CopyId;
use jj_lib::backend::TreeValue;
use jj_lib::config::ConfigLayer;
use jj_lib::config::ConfigSource;
use jj_lib::config::StackedConfig;
use jj_lib::default_backend_factories::default_backend_factories;
use jj_lib::merge::Merge;
use jj_lib::merged_tree_builder::MergedTreeBuilder;
use jj_lib::object_id::ObjectId as _;
use jj_lib::op_store::RefTarget;
use jj_lib::ref_name::RefName;
use jj_lib::ref_name::RemoteName;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo as _;
use jj_lib::repo::RepoLoader;
use jj_lib::repo_path::RepoPath;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::settings::UserSettings;
use jjforge_proto::sync::v1::PublishRequest;
use jjforge_store::ObjectId;
use jjforge_store::OperationId;
use jjforge_store::Published;
use jjforge_store::SegmentId;
use jjforge_store::client;
use jjforge_store::client::Fetch;
use jjforge_store::client::SyncError;
use jjforge_store::client::Synced;
use jjforge_store::client::Transport;
use jjforge_store::client::TransportError;
use jjforge_store::server::Author;
use jjforge_store::server::Repo;
use jjforge_store::server::Repos;
use tempfile::TempDir;
use tracing_subscriber::layer::SubscriberExt as _;
use url::Url;

/// The forge repository, called as `jf` would call it over HTTP.
#[derive(Debug)]
struct Forge {
    repo: Repo,
    runtime: tokio::runtime::Runtime,
    author: Author,
    fetches: std::sync::Mutex<usize>,
    uploads: std::sync::Mutex<usize>,
    /// Answers the next publish as stale, as if another push won the race.
    lose_next: std::sync::Mutex<bool>,
}

impl Fetch for Forge {
    fn fetch(&self, ids: &[ObjectId]) -> Result<Vec<Vec<u8>>, TransportError> {
        *self.fetches.lock().unwrap() += 1;
        Ok(self.runtime.block_on(self.repo.fetch(ids))?)
    }
}

impl Transport for Forge {
    fn head(&self) -> Result<OperationId, TransportError> {
        Ok(self.runtime.block_on(self.repo.head())?)
    }

    fn operations(&self, since: Option<OperationId>) -> Result<Vec<Vec<u8>>, TransportError> {
        Ok(self.runtime.block_on(self.repo.operations_since(since))?)
    }

    fn missing(&self, ids: &[ObjectId]) -> Result<Vec<ObjectId>, TransportError> {
        Ok(self.runtime.block_on(self.repo.missing(ids))?)
    }

    fn upload(&self, objects: Vec<Vec<u8>>) -> Result<SegmentId, TransportError> {
        *self.uploads.lock().unwrap() += 1;
        Ok(self.runtime.block_on(self.repo.upload(objects))?)
    }

    fn publish(&self, request: &PublishRequest) -> Result<Published, TransportError> {
        if std::mem::take(&mut *self.lose_next.lock().unwrap()) {
            return Ok(Published::Stale(self.runtime.block_on(self.repo.head())?));
        }
        Ok(self
            .runtime
            .block_on(self.repo.publish(request.clone(), self.author.clone()))?)
    }
}

fn settings() -> UserSettings {
    let mut config = StackedConfig::with_defaults();
    let mut layer = ConfigLayer::empty(ConfigSource::User);
    layer.set_value("user.name", "Ada").unwrap();
    layer.set_value("user.email", "ada@example.com").unwrap();
    config.add_layer(layer);
    config.add_layer(client::config());
    UserSettings::from_config_and_home_dir(config, None).unwrap()
}

/// One forge repository, created empty, which each clone calls as its own
/// person.
struct World {
    _cache: TempDir,
    repos: Repos,
}

impl World {
    fn new() -> Self {
        let cache = tempfile::tempdir().unwrap();
        let repos = Repos::open(&Url::parse("memory:///").unwrap(), cache.path()).unwrap();
        Self::with(cache, repos)
    }

    fn with(cache: TempDir, repos: Repos) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime
            .block_on(repos.get("acme", "forge").unwrap().create())
            .unwrap();
        Self {
            _cache: cache,
            repos,
        }
    }

    fn forge(&self, subject: &str) -> Arc<Forge> {
        Arc::new(Forge {
            repo: self.repos.get("acme", "forge").unwrap(),
            runtime: tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap(),
            author: Author {
                subject: subject.to_owned(),
                delegation: vec![],
            },
            fetches: std::sync::Mutex::default(),
            uploads: std::sync::Mutex::default(),
            lose_next: std::sync::Mutex::default(),
        })
    }
}

/// A clone: a workspace on a jjforge repository whose backend fetches from
/// `forge`.
struct Clone {
    _dir: TempDir,
    repo_path: PathBuf,
    forge: Arc<Forge>,
    repo: Arc<ReadonlyRepo>,
}

impl Clone {
    fn new(forge: Arc<Forge>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings();
        let (workspace, _) = block_on(client::init(&settings, dir.path())).unwrap();
        let repo_path = workspace.repo_path().to_owned();
        client::set_forge_url(&repo_path, "https://forge.example/sync/v1/acme/forge").unwrap();
        let repo = load(&repo_path, &forge);
        let mut clone = Self {
            _dir: dir,
            repo_path,
            forge,
            repo,
        };
        clone.fetch().unwrap();
        clone
    }

    fn fetch(&mut self) -> Result<bool, SyncError> {
        let forge = client::Forge::new(&self.repo_path, self.forge.as_ref());
        Ok(self.take(block_on(forge.fetch(&self.repo))?))
    }

    fn push(&mut self, name: &str) -> Result<(), SyncError> {
        let forge = client::Forge::new(&self.repo_path, self.forge.as_ref());
        self.take(block_on(forge.push(&self.repo, &[name.to_owned()]))?);
        Ok(())
    }

    /// Moves to the repository a sync left, and says whether it changed.
    fn take(&mut self, synced: Synced) -> bool {
        let Synced::Changed(repo) = synced else {
            return false;
        };
        self.repo = repo;
        true
    }

    /// Commits a file on top of `parent` and points `bookmark` at it.
    fn commit(
        &mut self,
        parent: Option<&str>,
        bookmark: &str,
        path: &str,
        content: &str,
    ) -> jj_lib::commit::Commit {
        let repo = self.repo.clone();
        let parent = match parent {
            Some(name) => repo
                .store()
                .get_commit(
                    repo.view()
                        .get_local_bookmark(RefName::new(name))
                        .as_normal()
                        .unwrap(),
                )
                .unwrap(),
            None => repo.store().root_commit(),
        };
        let mut tx = repo.start_transaction();
        let file = block_on(repo.store().write_file(
            RepoPath::from_internal_string(path).unwrap(),
            &mut content.as_bytes(),
        ))
        .unwrap();
        let mut builder = MergedTreeBuilder::new(parent.tree());
        builder.set_or_remove(
            RepoPathBuf::from_internal_string(path).unwrap(),
            Merge::normal(TreeValue::File {
                id: file,
                executable: false,
                copy_id: CopyId::placeholder(),
            }),
        );
        let tree = block_on(builder.write_tree()).unwrap();
        let commit = block_on(
            tx.repo_mut()
                .new_commit(vec![parent.id().clone()], tree)
                .set_description(format!("{bookmark}: {content}"))
                .write(),
        )
        .unwrap();
        tx.repo_mut().set_local_bookmark_target(
            RefName::new(bookmark),
            RefTarget::normal(commit.id().clone()),
        );
        self.repo = block_on(tx.commit("commit")).unwrap();
        commit
    }

    /// Rewrites the commit `bookmark` points at with `content` in `path`, as
    /// a snapshot of the working copy does.
    fn rewrite_file(&mut self, bookmark: &str, path: &str, content: &str) {
        let repo = self.repo.clone();
        let old = repo
            .store()
            .get_commit(
                repo.view()
                    .get_local_bookmark(RefName::new(bookmark))
                    .as_normal()
                    .unwrap(),
            )
            .unwrap();
        let file = block_on(repo.store().write_file(
            RepoPath::from_internal_string(path).unwrap(),
            &mut content.as_bytes(),
        ))
        .unwrap();
        let mut builder = MergedTreeBuilder::new(old.tree());
        builder.set_or_remove(
            RepoPathBuf::from_internal_string(path).unwrap(),
            Merge::normal(TreeValue::File {
                id: file,
                executable: false,
                copy_id: CopyId::placeholder(),
            }),
        );
        let tree = block_on(builder.write_tree()).unwrap();
        let mut tx = repo.start_transaction();
        block_on(tx.repo_mut().rewrite_commit(&old).set_tree(tree).write()).unwrap();
        block_on(tx.repo_mut().rebase_descendants()).unwrap();
        self.repo = block_on(tx.commit("snapshot")).unwrap();
    }

    /// Rewrites the commit `bookmark` points at with a new description.
    fn reword(&mut self, bookmark: &str, description: &str) {
        let repo = self.repo.clone();
        let old = repo
            .store()
            .get_commit(
                repo.view()
                    .get_local_bookmark(RefName::new(bookmark))
                    .as_normal()
                    .unwrap(),
            )
            .unwrap();
        let mut tx = repo.start_transaction();
        block_on(
            tx.repo_mut()
                .rewrite_commit(&old)
                .set_description(description)
                .write(),
        )
        .unwrap();
        block_on(tx.repo_mut().rebase_descendants()).unwrap();
        self.repo = block_on(tx.commit("reword")).unwrap();
    }

    /// Abandons the commit `bookmark` points at, and deletes the bookmark.
    fn abandon(&mut self, bookmark: &str) {
        let repo = self.repo.clone();
        let commit = repo
            .store()
            .get_commit(self.bookmark(bookmark).as_normal().unwrap())
            .unwrap();
        let mut tx = repo.start_transaction();
        tx.repo_mut().record_abandoned_commit(&commit);
        block_on(tx.repo_mut().rebase_descendants()).unwrap();
        tx.repo_mut()
            .set_local_bookmark_target(RefName::new(bookmark), RefTarget::absent());
        self.repo = block_on(tx.commit("abandon")).unwrap();
    }

    /// Restores the view of the operation `op`, as `jj op restore` does.
    fn restore(&mut self, op: &jj_lib::op_store::OperationId) {
        let loader = self.repo.loader();
        let old = block_on(loader.load_at(&block_on(loader.load_operation(op)).unwrap())).unwrap();
        let mut tx = self.repo.start_transaction();
        tx.repo_mut().set_view(old.view().store_view().clone());
        self.repo = block_on(tx.commit("restore")).unwrap();
    }

    /// Points `bookmark` at the commit `other` points at.
    fn copy_bookmark(&mut self, other: &str, bookmark: &str) {
        let target = self.bookmark(other);
        let mut tx = self.repo.start_transaction();
        tx.repo_mut()
            .set_local_bookmark_target(RefName::new(bookmark), target);
        self.repo = block_on(tx.commit("copy bookmark")).unwrap();
    }

    /// Rebases the commit `bookmark` points at onto the root commit, which
    /// rewrites it without its parents.
    fn rebase_onto_root(&mut self, bookmark: &str) {
        let repo = self.repo.clone();
        let old = repo
            .store()
            .get_commit(
                repo.view()
                    .get_local_bookmark(RefName::new(bookmark))
                    .as_normal()
                    .unwrap(),
            )
            .unwrap();
        let mut tx = repo.start_transaction();
        let root = repo.store().root_commit_id().clone();
        block_on(
            tx.repo_mut()
                .rewrite_commit(&old)
                .set_parents(vec![root])
                .write(),
        )
        .unwrap();
        block_on(tx.repo_mut().rebase_descendants()).unwrap();
        self.repo = block_on(tx.commit("rebase")).unwrap();
    }

    fn bookmark(&self, name: &str) -> RefTarget {
        self.repo
            .view()
            .get_local_bookmark(RefName::new(name))
            .clone()
    }

    fn remote_bookmark(&self, name: &str) -> RefTarget {
        self.repo
            .view()
            .get_remote_bookmark(RefName::new(name).to_remote_symbol(RemoteName::new("forge")))
            .target
            .clone()
    }

    fn read(&self, bookmark: &str, path: &str) -> String {
        let commit = self
            .repo
            .store()
            .get_commit(self.bookmark(bookmark).as_normal().unwrap())
            .unwrap();
        let value = block_on(
            commit
                .tree()
                .path_value(RepoPath::from_internal_string(path).unwrap()),
        )
        .unwrap();
        let Some(Some(TreeValue::File { id, .. })) = value.as_resolved() else {
            panic!("{path} is not a file");
        };
        let mut content = String::new();
        block_on(async {
            self.repo
                .store()
                .read_file(RepoPath::from_internal_string(path).unwrap(), id)
                .await
                .unwrap()
                .read_to_string(&mut content)
                .await
        })
        .unwrap();
        content
    }

    /// Writes a second commit of the change that `bookmark` points at, with
    /// `path` added, and points `twin` at it, so the change is divergent.
    fn twin(&mut self, bookmark: &str, twin: &str, path: &str) -> jj_lib::commit::Commit {
        let repo = self.repo.clone();
        let original = repo
            .store()
            .get_commit(
                repo.view()
                    .get_local_bookmark(RefName::new(bookmark))
                    .as_normal()
                    .unwrap(),
            )
            .unwrap();
        let file = block_on(repo.store().write_file(
            RepoPath::from_internal_string(path).unwrap(),
            &mut "twin\n".as_bytes(),
        ))
        .unwrap();
        let mut builder = MergedTreeBuilder::new(original.tree());
        builder.set_or_remove(
            RepoPathBuf::from_internal_string(path).unwrap(),
            Merge::normal(TreeValue::File {
                id: file,
                executable: false,
                copy_id: CopyId::placeholder(),
            }),
        );
        let tree = block_on(builder.write_tree()).unwrap();
        let mut tx = repo.start_transaction();
        let commit = block_on(
            tx.repo_mut()
                .new_commit(original.parent_ids().to_vec(), tree)
                .set_change_id(original.change_id().clone())
                .set_description("twin")
                .write(),
        )
        .unwrap();
        tx.repo_mut()
            .set_local_bookmark_target(RefName::new(twin), RefTarget::normal(commit.id().clone()));
        self.repo = block_on(tx.commit("twin")).unwrap();
        commit
    }

    /// Commits on top of `bookmark` with `removes` gone and `adds` written,
    /// and moves `bookmark` to it.
    fn change(&mut self, bookmark: &str, removes: &[&str], adds: &[(&str, &str)]) {
        let repo = self.repo.clone();
        let parent = repo
            .store()
            .get_commit(
                repo.view()
                    .get_local_bookmark(RefName::new(bookmark))
                    .as_normal()
                    .unwrap(),
            )
            .unwrap();
        let mut builder = MergedTreeBuilder::new(parent.tree());
        for path in removes {
            builder.set_or_remove(
                RepoPathBuf::from_internal_string(*path).unwrap(),
                Merge::absent(),
            );
        }
        for (path, content) in adds {
            let file = block_on(repo.store().write_file(
                RepoPath::from_internal_string(path).unwrap(),
                &mut content.as_bytes(),
            ))
            .unwrap();
            builder.set_or_remove(
                RepoPathBuf::from_internal_string(*path).unwrap(),
                Merge::normal(TreeValue::File {
                    id: file,
                    executable: false,
                    copy_id: CopyId::placeholder(),
                }),
            );
        }
        let tree = block_on(builder.write_tree()).unwrap();
        let mut tx = repo.start_transaction();
        let commit = block_on(
            tx.repo_mut()
                .new_commit(vec![parent.id().clone()], tree)
                .set_description("change")
                .write(),
        )
        .unwrap();
        tx.repo_mut().set_local_bookmark_target(
            RefName::new(bookmark),
            RefTarget::normal(commit.id().clone()),
        );
        self.repo = block_on(tx.commit("change")).unwrap();
    }

    /// Commits a conflict at `path`, of `ours` and `theirs` from `base`, on
    /// the root, and points `bookmark` at it.
    fn conflict(&mut self, bookmark: &str, path: &str, base: &str, ours: &str, theirs: &str) {
        let repo = self.repo.clone();
        let file = |content: &str| {
            let id = block_on(repo.store().write_file(
                RepoPath::from_internal_string(path).unwrap(),
                &mut content.as_bytes(),
            ))
            .unwrap();
            Some(TreeValue::File {
                id,
                executable: false,
                copy_id: CopyId::placeholder(),
            })
        };
        let value = Merge::from_vec(vec![file(ours), file(base), file(theirs)]);
        let root = repo.store().root_commit();
        let mut builder = MergedTreeBuilder::new(root.tree());
        builder.set_or_remove(RepoPathBuf::from_internal_string(path).unwrap(), value);
        let tree = block_on(builder.write_tree()).unwrap();
        let mut tx = repo.start_transaction();
        let commit = block_on(
            tx.repo_mut()
                .new_commit(vec![root.id().clone()], tree)
                .set_description("conflict")
                .write(),
        )
        .unwrap();
        tx.repo_mut().set_local_bookmark_target(
            RefName::new(bookmark),
            RefTarget::normal(commit.id().clone()),
        );
        self.repo = block_on(tx.commit("conflict")).unwrap();
    }
}

/// Loads the repository with a backend that fetches from `forge`.
fn load(repo_path: &Path, forge: &Arc<Forge>) -> Arc<ReadonlyRepo> {
    let forge = forge.clone();
    let mut factories = default_backend_factories();
    factories.merge(client::factories(Arc::new(move |_| {
        Ok(Some(forge.clone() as Arc<dyn Fetch>))
    })));
    let loader = RepoLoader::init_from_file_system(&settings(), repo_path, &factories).unwrap();
    block_on(loader.load_at_head()).unwrap()
}

#[test]
fn a_pushed_change_reaches_another_clone_with_its_ids() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let pushed = ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();

    let bob = Clone::new(world.forge("bob"));

    let main = bob.bookmark("main");
    assert_eq!(main.as_normal(), Some(pushed.id()));
    assert_eq!(bob.remote_bookmark("main").as_normal(), Some(pushed.id()));
    let commit = bob.repo.store().get_commit(pushed.id()).unwrap();
    assert_eq!(commit.change_id(), pushed.change_id());
    assert_eq!(ada.remote_bookmark("main").as_normal(), Some(pushed.id()));
}

#[test]
fn a_clone_fetches_files_only_when_it_reads_them() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();

    let bob = Clone::new(world.forge("bob"));
    let before = *bob.forge.fetches.lock().unwrap();
    let content = bob.read("main", "README");
    let after = *bob.forge.fetches.lock().unwrap();

    assert_eq!(content, "hello\n");
    assert!(after > before, "reading a file fetched nothing");
    assert_eq!(bob.read("main", "README"), "hello\n");
    assert_eq!(*bob.forge.fetches.lock().unwrap(), after);
}

#[test]
fn the_forge_names_who_made_each_operation() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();

    let forge = world.forge("check");
    let head = forge.head().unwrap();
    let bob = Clone::new(forge);
    let operation = block_on(
        bob.repo
            .loader()
            .load_operation(&jj_lib::op_store::OperationId::new(head.to_vec())),
    )
    .unwrap();

    assert_eq!(operation.metadata().username, "ada");
    assert_eq!(operation.metadata().hostname, "jjforge");
    assert_eq!(operation.metadata().attributes["jjforge.principal"], "ada");
}

#[test]
fn pushes_of_different_bookmarks_both_land() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let mut bob = Clone::new(world.forge("bob"));
    let a = ada.commit(None, "ada", "a", "a\n");
    let b = bob.commit(None, "bob", "b", "b\n");

    ada.push("ada").unwrap();
    bob.push("bob").unwrap();

    let check = Clone::new(world.forge("check"));
    assert_eq!(check.bookmark("ada").as_normal(), Some(a.id()));
    assert_eq!(check.bookmark("bob").as_normal(), Some(b.id()));
    assert_eq!(bob.bookmark("ada").as_normal(), Some(a.id()));
}

#[test]
fn a_bookmark_moved_on_both_sides_stops_the_push() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "base\n");
    ada.push("main").unwrap();
    let mut bob = Clone::new(world.forge("bob"));
    ada.commit(Some("main"), "main", "README", "ada\n");
    bob.commit(Some("main"), "main", "README", "bob\n");

    ada.push("main").unwrap();
    let result = bob.push("main");

    assert!(matches!(result, Err(SyncError::Conflict(names)) if names == ["main"]));
}

#[test]
fn a_rewritten_change_replaces_its_old_commit_everywhere() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let old = ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();
    let mut bob = Clone::new(world.forge("bob"));

    ada.reword("main", "a better description");
    ada.push("main").unwrap();
    assert!(bob.fetch().unwrap());

    let new = bob
        .repo
        .store()
        .get_commit(bob.bookmark("main").as_normal().unwrap())
        .unwrap();
    assert_eq!(new.change_id(), old.change_id());
    assert_ne!(new.id(), old.id());
    assert!(!bob.repo.view().heads().contains(old.id()));
    let history: Vec<_> = block_on(futures::TryStreamExt::try_collect::<Vec<_>>(
        jj_lib::evolution::walk_predecessors(&bob.repo, &[new.id().clone()]),
    ))
    .unwrap()
    .into_iter()
    .map(|entry| entry.commit.id().hex())
    .collect();
    assert_eq!(history, [new.id().hex(), old.id().hex()]);
}

#[test]
fn a_second_fetch_without_news_changes_nothing() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();
    let mut bob = Clone::new(world.forge("bob"));
    let op = bob.repo.op_id().clone();

    assert!(!bob.fetch().unwrap());
    assert_eq!(bob.repo.op_id(), &op);
}

#[test]
fn a_clone_holds_the_forges_operations_under_their_ids() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    ada.push("main").unwrap();
    ada.commit(Some("main"), "main", "README", "two\n");
    ada.push("main").unwrap();

    let forge = world.forge("check");
    let head = forge.head().unwrap();
    let bob = Clone::new(forge);

    let ancestors: Vec<_> = block_on(futures::TryStreamExt::try_collect::<Vec<_>>(
        jj_lib::op_walk::walk_ancestors(std::slice::from_ref(bob.repo.operation())),
    ))
    .unwrap()
    .into_iter()
    .map(|op| op.id().to_bytes())
    .collect();
    assert!(ancestors.contains(&head.to_vec()));
}

#[test]
fn a_bookmark_on_a_replaced_commit_keeps_it_visible() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let old = ada.commit(None, "main", "README", "hello\n");
    ada.copy_bookmark("main", "keep");
    ada.push("main").unwrap();
    ada.push("keep").unwrap();

    ada.reword("main", "reworded");
    ada.push("main").unwrap();

    let bob = Clone::new(world.forge("bob"));
    assert_eq!(bob.remote_bookmark("keep").as_normal(), Some(old.id()));
    assert!(bob.repo.view().heads().contains(old.id()));
}

#[test]
fn a_change_rebased_away_from_unpushed_parents_reaches_a_clone() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "work", "a", "a\n");
    ada.commit(Some("work"), "work", "b", "b\n");
    ada.commit(Some("work"), "work", "c", "c\n");
    ada.rebase_onto_root("work");
    ada.push("work").unwrap();

    let bob = Clone::new(world.forge("bob"));

    // The version before the rebase never left the clone, so the forge
    // knows only the pushed one.
    let pushed = bob.bookmark("work");
    let history: Vec<_> = block_on(futures::TryStreamExt::try_collect::<Vec<_>>(
        jj_lib::evolution::walk_predecessors(&bob.repo, &[pushed.as_normal().unwrap().clone()]),
    ))
    .unwrap();
    assert_eq!(history.len(), 1);
}

#[test]
fn a_publish_that_names_an_incomplete_object_is_refused() {
    let world = World::new();
    let forge = world.forge("ada");
    let tree = jj_lib::backend::TreeId::new(vec![7; 32]);
    let commit = jj_lib::backend::Commit {
        parents: vec![jj_lib::backend::CommitId::new(vec![0; 32])],
        predecessors: vec![],
        root_tree: Merge::resolved(tree),
        conflict_labels: Merge::resolved(String::new()),
        change_id: jj_lib::backend::ChangeId::new(vec![1; 16]),
        description: "no tree\n".to_owned(),
        author: jj_lib::backend::Signature {
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            timestamp: jj_lib::backend::Timestamp {
                timestamp: jj_lib::backend::MillisSinceEpoch(0),
                tz_offset: 0,
            },
        },
        committer: jj_lib::backend::Signature {
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            timestamp: jj_lib::backend::Timestamp {
                timestamp: jj_lib::backend::MillisSinceEpoch(0),
                tz_offset: 0,
            },
        },
        secure_sig: None,
    };
    let ada = Clone::new(forge.clone());
    let (id, _) = block_on(ada.repo.store().backend().write_commit(commit, None)).unwrap();
    let bytes = std::fs::read(ada.repo_path.join("store").join(id.hex())).unwrap();
    let segment = forge.upload(vec![bytes]).unwrap();

    let request = PublishRequest {
        expected_operation_id: forge.head().unwrap().to_vec(),
        segment_ids: vec![segment.to_vec()],
        ..PublishRequest::default()
    };

    assert!(forge.publish(&request).is_err());
}

#[test]
fn a_publish_that_changes_nothing_writes_no_operation() {
    let world = World::new();
    let forge = world.forge("ada");
    let head = forge.head().unwrap();

    let published = forge
        .publish(&PublishRequest {
            expected_operation_id: head.to_vec(),
            ..PublishRequest::default()
        })
        .unwrap();

    assert!(matches!(published, Published::Moved(id) if id == head));
    assert_eq!(forge.head().unwrap(), head);
}

#[test]
fn a_push_of_a_bookmark_that_exists_nowhere_fails() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));

    let result = ada.push("nowhere");

    assert!(matches!(result, Err(SyncError::NoSuchBookmark(names)) if names == ["nowhere"]));
}

#[test]
fn a_push_deletes_a_bookmark_that_exists_only_on_the_forge() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();
    let mut tx = ada.repo.start_transaction();
    tx.repo_mut()
        .set_local_bookmark_target(RefName::new("main"), RefTarget::absent());
    ada.repo = block_on(tx.commit("delete bookmark")).unwrap();

    ada.push("main").unwrap();

    let check = Clone::new(world.forge("check"));
    assert!(check.bookmark("main").is_absent());
}

#[test]
fn an_abandoned_change_stays_hidden_after_the_push() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "kept\n");
    let abandoned = ada.commit(Some("main"), "topic", "README", "abandoned\n");
    ada.push("main").unwrap();
    ada.push("topic").unwrap();

    ada.abandon("topic");
    ada.push("topic").unwrap();

    let check = Clone::new(world.forge("check"));
    assert!(!check.repo.view().heads().contains(abandoned.id()));
}

#[test]
fn a_fetch_after_a_restore_catches_up_with_the_forge() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    ada.commit(Some("main"), "topic", "topic", "t\n");
    ada.push("main").unwrap();
    ada.push("topic").unwrap();
    let mut bob = Clone::new(world.forge("bob"));
    let before = bob.repo.op_id().clone();
    let mut tx = ada.repo.start_transaction();
    tx.repo_mut()
        .set_local_bookmark_target(RefName::new("topic"), RefTarget::absent());
    ada.repo = block_on(tx.commit("delete topic")).unwrap();
    ada.push("topic").unwrap();
    let two = ada.commit(Some("main"), "main", "README", "two\n");
    ada.push("main").unwrap();
    assert!(bob.fetch().unwrap());

    bob.restore(&before);
    assert!(bob.fetch().unwrap());

    assert_eq!(bob.bookmark("main").as_normal(), Some(two.id()));
    assert_eq!(bob.remote_bookmark("main").as_normal(), Some(two.id()));
    assert!(bob.bookmark("topic").is_absent());
    assert!(bob.remote_bookmark("topic").is_absent());
}

#[test]
fn a_push_after_a_restore_moves_no_bookmark_back() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    ada.push("main").unwrap();
    let before = ada.repo.op_id().clone();
    let two = ada.commit(Some("main"), "main", "README", "two\n");
    ada.push("main").unwrap();

    ada.restore(&before);
    ada.push("main").unwrap();

    let check = Clone::new(world.forge("check"));
    assert_eq!(check.bookmark("main").as_normal(), Some(two.id()));
}

#[test]
fn a_replica_forgets_a_deleted_repository() {
    let bucket = tempfile::tempdir().unwrap();
    let url = Url::from_directory_path(bucket.path()).unwrap();
    let (cache1, cache2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let one = Repos::open(&url, cache1.path()).unwrap();
    let two = Repos::open(&url, cache2.path()).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let first = one.get("acme", "forge").unwrap();
    runtime.block_on(first.create()).unwrap();
    let forge = Arc::new(Forge {
        repo: first,
        runtime: tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap(),
        author: Author {
            subject: "ada".to_owned(),
            delegation: vec![],
        },
        fetches: std::sync::Mutex::default(),
        uploads: std::sync::Mutex::default(),
        lose_next: std::sync::Mutex::default(),
    });
    let mut ada = Clone::new(forge.clone());
    let pushed = ada.commit(None, "main", "README", "hello\n");
    ada.push("main").unwrap();
    let id = ObjectId::try_from(pushed.id().as_bytes()).unwrap();
    let other = two.get("acme", "forge").unwrap();
    assert_eq!(runtime.block_on(other.missing(&[id])).unwrap(), []);

    runtime.block_on(forge.repo.delete()).unwrap();
    runtime.block_on(forge.repo.create()).unwrap();

    // As in the crate's own tests, so no callsite is cached as unwanted.
    let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry());
    let messages = Messages::default();
    let subscriber = tracing_subscriber::registry().with(messages.clone());
    let missing = tracing::subscriber::with_default(subscriber, || {
        runtime.block_on(other.missing(&[id])).unwrap()
    });
    assert_eq!(missing, [id]);
    assert!(
        messages
            .0
            .lock()
            .unwrap()
            .contains(&"catalog starts over".to_owned()),
        "the trace shows that the replica dropped what it knew"
    );
}

/// Keeps the message of each event, so a test can check what a trace shows.
#[derive(Clone, Default)]
struct Messages(Arc<std::sync::Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Messages {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct Message(String);
        impl tracing::field::Visit for Message {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0.lock().unwrap().push(message.0);
    }
}

#[test]
fn a_push_publishes_no_version_that_never_left_the_clone() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "config", "token=hunter2\n");
    ada.rewrite_file("main", "config", "token=redacted\n");
    ada.push("main").unwrap();

    // A file's ID is the hash of its content, so writing it again names it.
    let secret = block_on(ada.repo.store().write_file(
        RepoPath::from_internal_string("config").unwrap(),
        &mut "token=hunter2\n".as_bytes(),
    ))
    .unwrap();
    let id = ObjectId::try_from(secret.as_bytes()).unwrap();
    assert_eq!(ada.forge.missing(&[id]).unwrap(), [id]);
}

#[test]
fn a_retry_after_a_lost_race_uploads_nothing_again() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "hello\n");
    *ada.forge.lose_next.lock().unwrap() = true;

    ada.push("main").unwrap();

    assert_eq!(*ada.forge.uploads.lock().unwrap(), 1);
    let bob = Clone::new(world.forge("bob"));
    assert_eq!(bob.read("main", "README"), "hello\n");
}

#[test]
fn a_new_repository_has_no_review_yet() {
    let world = World::new();
    let repo = world.repos.get("acme", "forge").unwrap();

    let (head, review) = block_on_tokio(repo.heads()).unwrap();

    assert_eq!(head, OperationId::root());
    assert_eq!(review, Vec::<u8>::new());
}

/// Runs a future of the store on a runtime of its own, as vcs does.
fn block_on_tokio<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// The reads of `source/v1` on a repository with two pushes: README, then
/// src/lib.rs on top.
#[test]
fn the_server_reads_history_files_and_diffs_of_a_pushed_repository() {
    use jjforge_proto::source::v1::diff_response::Status;
    use jjforge_proto::source::v1::tree_entry::Kind;

    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let first = ada.commit(None, "main", "README", "one\n");
    ada.push("main").unwrap();
    let second = ada.commit(Some("main"), "main", "src/lib.rs", "fn main() {}\n");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let log = block_on_tokio(repo.log("", "", 0)).unwrap();
    let ids: Vec<_> = log
        .items
        .iter()
        .map(|change| change.commit_id.clone())
        .collect();
    assert_eq!(ids, [second.id().hex(), first.id().hex()]);
    assert_eq!(log.items[0].bookmarks, ["main"]);
    assert_eq!(log.next, "");

    let change = block_on_tokio(repo.change("main")).unwrap();
    let files: Vec<_> = change
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.status(), file.conflicted))
        .collect();
    assert_eq!(files, [("src/lib.rs", Status::Added, false)]);

    let change_id = jj_lib::hex_util::encode_reverse_hex(first.change_id().as_bytes());
    let resolved = block_on_tokio(repo.resolve(&change_id[..8])).unwrap();
    assert_eq!(resolved.commit_id, first.id().hex());
    assert!(matches!(
        block_on_tokio(repo.resolve("nothing")),
        Err(jjforge_store::server::RepoError::NotFound(_))
    ));

    let root = block_on_tokio(repo.tree("main", "", "", 0)).unwrap();
    let entries: Vec<_> = root
        .items
        .iter()
        .map(|entry| (entry.name.as_str(), entry.kind(), entry.size))
        .collect();
    assert_eq!(
        entries,
        [("src", Kind::Directory, 0), ("README", Kind::File, 4)]
    );

    let file = block_on_tokio(repo.file("main", "src/lib.rs", 3, 4)).unwrap();
    assert_eq!(
        (file.size, file.binary, file.conflicted),
        (13, false, false)
    );
    assert_eq!(bytes(file), b"main");
    let whole = block_on_tokio(repo.file("main", "src/lib.rs", 0, 0)).unwrap();
    assert_eq!(bytes(whole), b"fn main() {}\n");

    let diff = diff_files(block_on_tokio(repo.diff("main", "", "", "", 0)).unwrap());
    assert_eq!(diff.len(), 1);
    assert_eq!(
        (diff[0].path.as_str(), diff[0].status()),
        ("src/lib.rs", Status::Added)
    );
    assert_eq!(diff[0].hunks[0].lines[0].text, "fn main() {}\n");

    let operations = block_on_tokio(repo.operation_page("", 1)).unwrap();
    assert_eq!(operations.items.len(), 1);
    assert_eq!(operations.items[0].principal_id, "ada");
    assert!(operations.next.ends_with(".1"), "{}", operations.next);

    let bookmarks = block_on_tokio(repo.bookmarks("", 0)).unwrap();
    assert_eq!(bookmarks.items[0].name, "main");

    let evolution = block_on_tokio(repo.evolution("main", "", 0)).unwrap();
    assert_eq!(evolution.items[0].commit_id, second.id().hex());
}

#[test]
fn a_watch_yields_each_push_in_order() {
    use futures::StreamExt as _;
    use futures::TryStreamExt as _;

    let world = World::new();
    let repo = world.repos.get("acme", "forge").unwrap();
    let watcher = std::thread::spawn(move || {
        block_on_tokio(async move {
            repo.watch(None, std::time::Duration::from_millis(10))
                .take(2)
                .try_collect::<Vec<_>>()
                .await
        })
    });
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    ada.push("main").unwrap();
    ada.commit(Some("main"), "main", "README", "two\n");
    ada.push("main").unwrap();

    let operations = watcher.join().unwrap().unwrap();

    assert_eq!(operations[1].parent_ids, [operations[0].id.clone()]);
    assert_eq!(operations[0].principal_id, "ada");
}

#[test]
fn a_watch_from_an_operation_outside_the_log_fails() {
    use futures::StreamExt as _;

    let world = World::new();
    let repo = world.repos.get("acme", "forge").unwrap();
    let elsewhere: OperationId = "07".repeat(32).parse().unwrap();

    let first = block_on_tokio(async move {
        repo.watch(Some(elsewhere), std::time::Duration::from_millis(10))
            .next()
            .await
    });

    assert!(matches!(
        first,
        Some(Err(jjforge_store::server::RepoError::NotFound(_)))
    ));
}

impl Clone {
    /// Merges the commits that `left` and `right` point at, adds `path` on
    /// top, and points `bookmark` at the merge.
    fn merge(
        &mut self,
        left: &str,
        right: &str,
        bookmark: &str,
        path: &str,
    ) -> jj_lib::commit::Commit {
        let repo = self.repo.clone();
        let parent = |name: &str| {
            repo.store()
                .get_commit(
                    repo.view()
                        .get_local_bookmark(RefName::new(name))
                        .as_normal()
                        .unwrap(),
                )
                .unwrap()
        };
        let parents = [parent(left), parent(right)];
        let merged =
            block_on(jj_lib::rewrite::merge_commit_trees(repo.as_ref(), &parents)).unwrap();
        let file = block_on(repo.store().write_file(
            RepoPath::from_internal_string(path).unwrap(),
            &mut "merged\n".as_bytes(),
        ))
        .unwrap();
        let mut builder = MergedTreeBuilder::new(merged);
        builder.set_or_remove(
            RepoPathBuf::from_internal_string(path).unwrap(),
            Merge::normal(TreeValue::File {
                id: file,
                executable: false,
                copy_id: CopyId::placeholder(),
            }),
        );
        let tree = block_on(builder.write_tree()).unwrap();
        let mut tx = repo.start_transaction();
        let commit = block_on(
            tx.repo_mut()
                .new_commit(parents.iter().map(|p| p.id().clone()).collect(), tree)
                .set_description("merge")
                .write(),
        )
        .unwrap();
        tx.repo_mut().set_local_bookmark_target(
            RefName::new(bookmark),
            RefTarget::normal(commit.id().clone()),
        );
        self.repo = block_on(tx.commit("merge")).unwrap();
        commit
    }
}

#[test]
fn the_diff_of_a_merge_shows_only_what_the_merge_added() {
    use jjforge_proto::source::v1::diff_response::Status;

    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "left", "README", "one\n");
    ada.commit(None, "right", "NOTES", "two\n");
    ada.merge("left", "right", "main", "MERGED");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let diff = diff_files(block_on_tokio(repo.diff("main", "", "", "", 0)).unwrap());

    let files: Vec<_> = diff
        .iter()
        .map(|file| (file.path.as_str(), file.status()))
        .collect();
    assert_eq!(files, [("MERGED", Status::Added)]);
}

/// gRPC's default limit on a message, which the server's client keeps.
const GRPC_MESSAGE_LIMIT: usize = 4 << 20;

#[test]
fn the_diff_of_a_large_file_fits_in_one_grpc_message() {
    use prost::Message as _;

    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let numbers = (1..=400_000)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    ada.commit(None, "main", "numbers", &numbers);
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let diff = diff_files(block_on_tokio(repo.diff("main", "", "", "", 0)).unwrap());

    assert_eq!(diff.len(), 1);
    assert!(diff[0].encoded_len() < GRPC_MESSAGE_LIMIT);
    assert!(diff[0].truncated);
    assert_eq!(diff[0].hunks.len(), 0);
}

#[test]
fn a_log_continues_on_the_page_its_token_names() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let first = ada.commit(None, "main", "README", "one\n");
    let second = ada.commit(Some("main"), "main", "README", "two\n");
    let third = ada.commit(Some("main"), "main", "README", "three\n");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let one = block_on_tokio(repo.log("main", "", 2)).unwrap();
    let two = block_on_tokio(repo.log("main", &one.next, 2)).unwrap();

    let ids = |page: &jjforge_store::server::Page<jjforge_proto::source::v1::Change>| {
        page.items
            .iter()
            .map(|change| change.commit_id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&one), [third.id().hex(), second.id().hex()]);
    assert_eq!(ids(&two), [first.id().hex()]);
    assert_eq!(two.next, "");
}

#[test]
fn a_diff_continues_on_the_page_its_token_names() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let base = ada.commit(None, "main", "a", "a\n");
    ada.commit(Some("main"), "main", "b", "b\n");
    ada.commit(Some("main"), "main", "c", "c\n");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();
    let from = base.id().hex();

    let one = block_on_tokio(repo.diff("", &from, "main", "", 1)).unwrap();
    let next = one.next.clone();
    let two = block_on_tokio(repo.diff("", &from, "main", &next, 1)).unwrap();
    let last = two.next.clone();

    assert_eq!(diff_files(one)[0].path, "b");
    assert_eq!(diff_files(two)[0].path, "c");
    assert_eq!(last, "");
}

#[test]
fn a_divergent_change_is_ambiguous_with_both_commits_as_candidates() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let one = ada.commit(None, "main", "README", "one\n");
    let two = ada.twin("main", "twin", "NOTES");
    ada.push("main").unwrap();
    ada.push("twin").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();
    let change_id = jj_lib::hex_util::encode_reverse_hex(one.change_id().as_bytes());

    let refused = block_on_tokio(repo.resolve(&change_id));

    let Err(jjforge_store::server::RepoError::Ambiguous(rev, candidates)) = refused else {
        panic!("{:?}", refused.map(|resolved| resolved.commit_id));
    };
    let mut expected = [one.id().hex(), two.id().hex()];
    expected.sort();
    let ids: Vec<_> = candidates
        .iter()
        .map(|change| change.commit_id.clone())
        .collect();
    assert_eq!(rev, change_id);
    assert_eq!(ids, expected);
    assert!(
        candidates
            .iter()
            .all(|change| change.change_id == change_id)
    );
}

#[test]
fn every_page_of_a_log_reads_the_operation_of_the_first() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let first = ada.commit(None, "main", "README", "one\n");
    let second = ada.commit(Some("main"), "main", "README", "two\n");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let one = block_on_tokio(repo.log("main", "", 1)).unwrap();
    ada.commit(Some("main"), "main", "README", "three\n");
    ada.push("main").unwrap();
    let two = block_on_tokio(repo.log("main", &one.next, 1)).unwrap();

    assert_eq!(one.items[0].commit_id, second.id().hex());
    assert_eq!(two.items[0].commit_id, first.id().hex());
    assert_eq!(two.next, "");
}

#[test]
fn a_page_token_of_another_repository_is_invalid() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    ada.commit(Some("main"), "main", "README", "two\n");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();
    let elsewhere = format!("{}.1", "07".repeat(32));

    let refused = block_on_tokio(repo.log("main", &elsewhere, 1));

    assert!(matches!(
        refused,
        Err(jjforge_store::server::RepoError::Invalid(_))
    ));
}

#[test]
fn a_moved_file_shows_as_a_rename_with_or_without_edits() {
    use jjforge_proto::source::v1::diff_response::Status;

    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let text = "one\ntwo\nthree\nfour\nfive\nsix\n";
    ada.commit(None, "main", "old/same", "unchanged\n");
    ada.commit(Some("main"), "main", "old/edited", text);
    ada.commit(Some("main"), "main", "kept", "kept\n");
    ada.change(
        "main",
        &["old/same", "old/edited", "kept"],
        &[
            ("new/same", "unchanged\n"),
            ("new/edited", &text.replace("six", "SIX")),
            ("other", "nothing alike\n"),
        ],
    );
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let diff = diff_files(block_on_tokio(repo.diff("main", "", "", "", 0)).unwrap());
    let change = block_on_tokio(repo.change("main")).unwrap();

    let shown: Vec<_> = diff
        .iter()
        .map(|file| (file.path.as_str(), file.old_path.as_str(), file.status()))
        .collect();
    assert_eq!(
        shown,
        [
            ("kept", "", Status::Deleted),
            ("new/edited", "old/edited", Status::Renamed),
            ("new/same", "old/same", Status::Renamed),
            ("other", "", Status::Added),
        ]
    );
    assert_eq!(diff[1].hunks.len(), 1);
    assert_eq!(diff[2].hunks.len(), 0);
    let listed: Vec<_> = change
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.old_path.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("kept", ""),
            ("new/edited", "old/edited"),
            ("new/same", "old/same"),
            ("other", ""),
        ]
    );
}

#[test]
fn a_conflicted_file_reads_with_conflict_markers() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.conflict("main", "README", "base\n", "ada\n", "bob\n");
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let read = block_on_tokio(repo.file("main", "README", 0, 0)).unwrap();

    assert!(read.conflicted);
    let text = String::from_utf8(bytes(read)).unwrap();
    assert!(text.starts_with("<<<<<<<"), "{text}");
    assert!(text.contains("ada\n") && text.contains("bob\n"), "{text}");
}

#[test]
fn any_range_of_a_large_file_reads_alone() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let content = numbers(300_000);
    ada.commit(None, "main", "numbers", &content);
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();
    let at = 2_000_000;

    let tail = block_on_tokio(repo.file("main", "numbers", at, 16)).unwrap();
    let whole = block_on_tokio(repo.file("main", "numbers", 0, 0)).unwrap();

    assert_eq!(tail.size, content.len() as u64);
    assert_eq!(bytes(tail), content.as_bytes()[2_000_000..2_000_016]);
    assert_eq!(bytes(whole), content.as_bytes());
}

#[test]
fn the_diff_of_a_file_over_four_mib_is_truncated_unread() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    let content = numbers(600_000);
    ada.commit(None, "main", "numbers", &content);
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let diff = diff_files(block_on_tokio(repo.diff("main", "", "", "", 0)).unwrap());

    assert_eq!(diff.len(), 1);
    assert!(diff[0].truncated);
    assert!(!diff[0].binary);
}

#[test]
fn a_change_lists_its_first_files_and_a_token_for_the_rest() {
    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    let paths: Vec<_> = (0..150).map(|n| format!("f{n:03}")).collect();
    let adds: Vec<_> = paths.iter().map(|path| (path.as_str(), "x\n")).collect();
    ada.change("main", &[], &adds);
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let change = block_on_tokio(repo.change("main")).unwrap();
    let rest = block_on_tokio(repo.changed_files("main", "", "", &change.files_next_page_token, 0))
        .unwrap();

    let listed: Vec<_> = change
        .files
        .iter()
        .chain(&rest.items)
        .map(|file| file.path.clone())
        .collect();
    assert_eq!(change.files.len(), 100);
    assert_eq!(listed, paths);
    assert_eq!(rest.next, "");
}

#[test]
fn a_page_of_changed_files_stays_within_one_grpc_message() {
    use prost::Message as _;

    let world = World::new();
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    // 1,000 paths of about 4,400 bytes take about 4.4 MB.
    let dir = vec!["d".repeat(1100); 4].join("/");
    let paths: Vec<_> = (0..1000).map(|n| format!("{dir}/f{n:03}")).collect();
    let adds: Vec<_> = paths.iter().map(|path| (path.as_str(), "x\n")).collect();
    ada.change("main", &[], &adds);
    ada.push("main").unwrap();
    let repo = world.repos.get("acme", "forge").unwrap();

    let (mut listed, mut token) = (vec![], String::new());
    loop {
        let page = block_on_tokio(repo.changed_files("main", "", "", &token, 1000)).unwrap();
        let response = jjforge_proto::source::v1::ListChangedFilesResponse {
            files: page.items,
            next_page_token: page.next,
        };
        assert!(response.encoded_len() < GRPC_MESSAGE_LIMIT);
        listed.extend(response.files.into_iter().map(|file| file.path));
        token = response.next_page_token;
        if token.is_empty() {
            break;
        }
    }

    assert_eq!(listed, paths);
}

#[test]
fn watches_of_one_repository_share_a_poll_and_wake_on_a_push() {
    use futures::StreamExt as _;

    let world = World::new();
    let repo = world.repos.get("acme", "forge").unwrap();
    // A poll this long would time the test out, so a push must wake both.
    let poll = std::time::Duration::from_secs(3600);
    let watcher = std::thread::spawn(move || {
        block_on_tokio(async move {
            let one = repo.watch(None, poll);
            let two = repo.watch(None, poll);
            futures::future::join(
                one.take(1).collect::<Vec<_>>(),
                two.take(1).collect::<Vec<_>>(),
            )
            .await
        })
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut ada = Clone::new(world.forge("ada"));
    ada.commit(None, "main", "README", "one\n");
    ada.push("main").unwrap();

    let (one, two) = watcher.join().unwrap();

    assert_eq!(one.len(), 1);
    assert_eq!(two.len(), 1);
}

/// `count` lines of seven digits each.
fn numbers(count: usize) -> String {
    (0..count)
        .map(|n| format!("{n:07}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The files of a page of a diff, each read as the stream reaches it.
fn diff_files(
    page: jjforge_store::server::DiffPage,
) -> Vec<jjforge_proto::source::v1::DiffResponse> {
    use futures::TryStreamExt as _;

    block_on_tokio(page.files.try_collect()).unwrap()
}

/// The bytes of a file read.
fn bytes(read: jjforge_store::server::FileRead) -> Vec<u8> {
    use futures::TryStreamExt as _;

    let parts: Vec<Vec<u8>> = block_on_tokio(read.chunks.try_collect()).unwrap();
    parts.concat()
}
