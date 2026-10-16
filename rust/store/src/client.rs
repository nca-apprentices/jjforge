//! `jf`'s side: the store type that jj-cli loads, and the sync of a clone
//! with its [`Forge`], over the [`Transport`] that `jf` implements with
//! `sync/v1`. Both ends name an object by the hash of the same bytes, so a
//! fetch and a push copy bytes unchanged, and an operation keeps its ID in
//! every clone, as #30 asks.
//!
//! A fetch merges the forge's operations into the local log, as #101
//! decides, and the forge's last fetched head lives in `.jj/repo/forge/`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error as StdError;
use std::fs;
use std::hash::BuildHasher as _;
use std::hash::Hasher as _;
use std::hash::RandomState;
use std::io;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::TryStreamExt as _;
use jj_lib::backend::CommitId;
use jj_lib::evolution::walk_predecessors;
use jj_lib::hex_util::encode_hex;
use jj_lib::object_id::ObjectId as _;
use jj_lib::op_store::OperationId as JjOperationId;
use jj_lib::op_store::RefTarget;
use jj_lib::op_store::RemoteRef;
use jj_lib::op_store::RemoteRefState;
use jj_lib::operation::Operation;
use jj_lib::ref_name::RefName;
use jj_lib::ref_name::RefNameBuf;
use jj_lib::ref_name::RemoteName;
use jj_lib::repo::MutableRepo;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo as _;
use jj_lib::revset::ResolvedRevsetExpression;
use jj_lib::workspace::Workspace;
use jjforge_proto::store::v1::CommitIds;
use jjforge_proto::sync::v1::BookmarkUpdate;
use jjforge_proto::sync::v1::PublishRequest;
use prost::Message as _;

pub use crate::lazy::Fetch;
pub use crate::lazy::TransportError;
pub use crate::workspace::FindForge;
pub use crate::workspace::config;
pub use crate::workspace::factories;
pub use crate::workspace::init;

use crate::Published;
use crate::codec;
use crate::codec::Object;
use crate::ids;
use crate::ids::ObjectId;
use crate::ids::OperationId;
use crate::ids::SegmentId;
use crate::shelf::Disk;
use crate::shelf::Shelf as _;

/// The forge among jj's remotes, as in `main@forge`.
const FORGE: &str = "forge";

/// The most bytes one upload carries, below vcs's limit on a request body.
const UPLOAD_BYTES: usize = 16 << 20;

/// The largest object an upload can carry alone, below vcs's limit of 64 MiB
/// on a request body, until #110 measures the limits.
const MAX_OBJECT: usize = 63 << 20;

/// The most IDs one request names.
const BATCH: usize = 1000;

/// How often a push that loses the race to another publishes again, as
/// #89 asks, until #110 measures the limit.
const ATTEMPTS: u32 = 10;

/// The `sync/v1` calls of one repository. `jf` implements them over HTTP.
pub trait Transport: Fetch {
    /// The forge's current operation.
    ///
    /// # Errors
    ///
    /// Fails when the forge can't answer.
    fn head(&self) -> Result<OperationId, TransportError>;

    /// The encoded operations, each followed by its view, that the forge's
    /// head reaches and `since` doesn't. Without `since`, every operation but
    /// the root.
    ///
    /// # Errors
    ///
    /// Fails when the forge can't answer.
    fn operations(&self, since: Option<OperationId>) -> Result<Vec<Vec<u8>>, TransportError>;

    /// Which of `ids` the forge lacks.
    ///
    /// # Errors
    ///
    /// Fails when the forge can't answer.
    fn missing(&self, ids: &[ObjectId]) -> Result<Vec<ObjectId>, TransportError>;

    /// Stores encoded objects as one segment and returns its ID.
    ///
    /// # Errors
    ///
    /// Fails when the forge refuses an object or can't answer.
    fn upload(&self, objects: Vec<Vec<u8>>) -> Result<SegmentId, TransportError>;

    /// Publishes `request`.
    ///
    /// # Errors
    ///
    /// Fails when the forge refuses the request or can't answer. A stale
    /// head is no error but [`Published::Stale`].
    fn publish(&self, request: &PublishRequest) -> Result<Published, TransportError>;
}

/// Why a fetch or a push failed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// The forge couldn't be reached, or refused a call.
    #[error(transparent)]
    Transport(TransportError),
    /// The clone couldn't be read or written, or the forge sent what the
    /// clone can't use.
    #[error(transparent)]
    Local(Box<dyn StdError + Send + Sync>),
    /// After merging the forge's work, these bookmarks are conflicted, so the
    /// push stops, as #89 decides.
    #[error("conflicted bookmarks: {}", .0.join(", "))]
    Conflict(Vec<String>),
    /// Neither the repository nor the forge has these bookmarks.
    #[error("no such bookmarks: {}", .0.join(", "))]
    NoSuchBookmark(Vec<String>),
    /// Another publish won every attempt.
    #[error("the forge moved on during every attempt to push")]
    Contended,
}

fn local(err: impl Into<Box<dyn StdError + Send + Sync>>) -> SyncError {
    SyncError::Local(err.into())
}

/// The clone address of the repository at `repo_path`, which is
/// `.jj/repo`, or `None` for a repository without a forge.
///
/// # Errors
///
/// Fails when the address can't be read.
pub fn forge_url(repo_path: &Path) -> io::Result<Option<String>> {
    ForgeDir::new(repo_path).url()
}

/// Records the clone address of the repository at `repo_path`, which is
/// `.jj/repo`.
///
/// # Errors
///
/// Fails when the address can't be written.
pub fn set_forge_url(repo_path: &Path, url: &str) -> io::Result<()> {
    ForgeDir::new(repo_path).set_url(url)
}

/// Where `jf` keeps the forge of a repository: its clone address and the
/// last head it fetched.
#[derive(Debug)]
struct ForgeDir {
    dir: PathBuf,
}

impl ForgeDir {
    fn new(repo_path: &Path) -> Self {
        Self {
            dir: repo_path.join("forge"),
        }
    }

    fn url(&self) -> io::Result<Option<String>> {
        read_optional(&self.dir.join("url")).map(|url| url.map(|url| url.trim().to_owned()))
    }

    fn set_url(&self, url: &str) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::write(self.dir.join("url"), format!("{url}\n"))
    }

    /// The forge's operation that the last fetch merged, or the root.
    fn head(&self) -> Result<OperationId, SyncError> {
        let path = self.dir.join("head");
        let Some(hex) = read_optional(&path).map_err(local)? else {
            return Ok(OperationId::root());
        };
        hex.trim()
            .parse()
            .map_err(|_| local(format!("{} names no operation", path.display())))
    }

    /// Replaces the head file whole, so an interrupted write leaves the old
    /// one.
    fn set_head(&self, id: OperationId) -> Result<(), SyncError> {
        fs::create_dir_all(&self.dir).map_err(local)?;
        let mut file = tempfile::NamedTempFile::new_in(&self.dir).map_err(local)?;
        writeln!(file, "{id}").map_err(local)?;
        file.persist(self.dir.join("head"))
            .map_err(|err| local(err.error))?;
        Ok(())
    }
}

fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn forge() -> &'static RemoteName {
    RemoteName::new(FORGE)
}

/// The forge of one clone, reached through a [`Transport`].
#[derive(Debug)]
pub struct Forge<'a> {
    repo_path: PathBuf,
    transport: &'a dyn Transport,
}

/// What a fetch or a push did.
#[derive(Debug)]
pub enum Synced {
    /// The clone already matched the forge.
    Unchanged,
    /// The clone took the forge's work, and this is the repository after it.
    Changed(Arc<ReadonlyRepo>),
}

impl<'a> Forge<'a> {
    /// The forge of the repository at `repo_path`, which is `.jj/repo`.
    #[must_use]
    pub fn new(repo_path: &Path, transport: &'a dyn Transport) -> Self {
        Self {
            repo_path: repo_path.to_owned(),
            transport,
        }
    }

    /// Fetches the forge's operations since the last fetch with the commits
    /// their views reach, merges them into the local log, and sets each
    /// `<name>@forge` to the forge's target, as #101 decides. Trees and
    /// files arrive later, when jj reads them, as #90 decides.
    ///
    /// # Errors
    ///
    /// Fails when the forge can't answer, sends a corrupt object, or when
    /// the local repository can't be written.
    pub async fn fetch(&self, repo: &Arc<ReadonlyRepo>) -> Result<Synced, SyncError> {
        let fetched = fetch(repo, &self.repo_path, self.transport).await?;
        Ok(if fetched.changed {
            Synced::Changed(fetched.repo)
        } else {
            Synced::Unchanged
        })
    }

    /// Publishes the bookmarks `names`, as #31 decides. It uploads every
    /// object they reach that the forge lacks, with the earlier versions of
    /// each pushed change as #103 asks. When another publish wins the race,
    /// it fetches, merges, and tries again, as #89 decides.
    ///
    /// # Errors
    ///
    /// Fails when a bookmark is conflicted, before or after a merge, when
    /// the forge refuses the push, or when every attempt loses the race.
    pub async fn push(
        &self,
        repo: &Arc<ReadonlyRepo>,
        names: &[String],
    ) -> Result<Synced, SyncError> {
        push(repo, &self.repo_path, self.transport, names).await
    }
}

/// What a fetch did.
struct Fetched {
    repo: Arc<ReadonlyRepo>,
    changed: bool,
}

async fn fetch(
    repo: &Arc<ReadonlyRepo>,
    repo_path: &Path,
    remote: &dyn Transport,
) -> Result<Fetched, SyncError> {
    let forge_dir = ForgeDir::new(repo_path);
    let head = remote.head().map_err(SyncError::Transport)?;
    let last = forge_dir.head()?;
    let loader = repo.loader();
    let last_operation = loader
        .load_operation(&JjOperationId::new(last.to_vec()))
        .await
        .map_err(local)?;
    let base = known_forge_operation(repo, &last_operation).await?;
    if head == last && base.id() == last_operation.id() {
        return Ok(Fetched {
            repo: repo.clone(),
            changed: false,
        });
    }

    let operations = Disk::new(&repo_path.join("op_store"));
    let mut wanted = vec![];
    // Operations up to `last` are in the op store already, so only newer
    // ones travel.
    let since = (last != OperationId::root()).then_some(last);
    let fetched_operations = if head == last {
        vec![]
    } else {
        remote.operations(since).map_err(SyncError::Transport)?
    };
    for bytes in fetched_operations {
        let object = codec::decode(&bytes).map_err(local)?;
        match &object {
            Object::View(view) => {
                wanted.extend(view.head_ids.iter().map(CommitId::to_bytes));
            }
            Object::Operation(operation) => wanted.extend(
                operation
                    .commit_predecessors
                    .iter()
                    .flatten()
                    .flat_map(|(_, predecessors)| predecessors.iter().map(CommitId::to_bytes)),
            ),
            _ => {
                return Err(local(
                    "the forge sent an object that isn't an operation or a view",
                ));
            }
        }
        operations
            .put(&ids::hash(&bytes), bytes)
            .await
            .map_err(SyncError::Local)?;
    }
    fetch_commits(repo_path, remote, wanted).await?;

    let forge_operation = loader
        .load_operation(&JjOperationId::new(head.to_vec()))
        .await
        .map_err(local)?;
    let forge_view = loader
        .load_at(&forge_operation)
        .await
        .map_err(local)?
        .view()
        .clone();

    let mut tx = repo.start_transaction();
    tx.merge_operation(&base, &forge_operation)
        .await
        .map_err(local)?;
    tx.repo_mut().rebase_descendants().await.map_err(local)?;

    let known: Vec<_> = tx
        .repo()
        .view()
        .remote_bookmarks(forge())
        .map(|(name, _)| name.to_owned())
        .collect();
    for name in known {
        if forge_view.get_local_bookmark(&name).is_absent() {
            tx.repo_mut()
                .set_remote_bookmark(name.to_remote_symbol(forge()), RemoteRef::absent());
        }
    }
    for (name, target) in forge_view.local_bookmarks() {
        let remote_ref = RemoteRef {
            target: target.clone(),
            state: RemoteRefState::Tracked,
        };
        tx.repo_mut()
            .set_remote_bookmark(name.to_remote_symbol(forge()), remote_ref);
    }
    let fetched = tx.write(FETCH).await.map_err(local)?.leave_unpublished();

    // The new operation's parents are the old local head and the forge's
    // head, so the old head goes, and the forge's log stays in the local
    // one.
    loader
        .op_heads_store()
        .update_op_heads(&[repo.op_id().clone()], fetched.op_id())
        .await
        .map_err(local)?;
    forge_dir.set_head(head)?;
    Ok(Fetched {
        repo: fetched,
        changed: true,
    })
}

/// The description of the operation that a fetch records.
const FETCH: &str = "fetch from forge";

/// Restores the clone from before the fetch that `repo`'s base operation
/// records, and returns the description that `jj undo` would give it, or
/// `None` when that operation is no fetch. A fetch records a merge of the
/// forge's operation, as 30.2 decides, and jj undoes no merge, so `jf undo`
/// restores the first parent instead. The description lets `jj undo` and
/// `jj redo` go on from there.
///
/// # Errors
///
/// Fails when an operation or a view can't be read.
pub async fn undo_fetch(repo: &mut MutableRepo) -> Result<Option<String>, SyncError> {
    let head = repo.base_repo().operation().clone();
    if head.metadata().description != FETCH {
        return Ok(None);
    }
    let parents = head.parents().await.map_err(local)?;
    let [before, _] = parents.as_slice() else {
        return Ok(None);
    };
    let view = before.view().await.map_err(local)?;
    repo.set_view(view.store_view().clone());
    Ok(Some(format!(
        "undo: restore to operation {}",
        before.id().hex()
    )))
}

/// The forge operation that the view's `<name>@forge` bookmarks reflect,
/// which is the merge base of a fetch. It is the last fetched one, unless an
/// `op restore` or `undo` took the view back to an earlier forge operation.
/// Then it is the oldest of the latest run of forge operations with those
/// bookmarks, so a merge from it misses none of the forge's later changes.
/// vcs writes a log without merges, so each forge operation has one parent.
async fn known_forge_operation(
    repo: &ReadonlyRepo,
    last: &Operation,
) -> Result<Operation, SyncError> {
    let known: BTreeMap<RefNameBuf, RefTarget> = repo
        .view()
        .remote_bookmarks(forge())
        .map(|(name, remote_ref)| (name.to_owned(), remote_ref.target.clone()))
        .collect();
    let op_store = repo.loader().op_store();
    let matches = async |operation: &Operation| -> Result<bool, SyncError> {
        let view = op_store
            .read_view(operation.view_id())
            .await
            .map_err(local)?;
        Ok(view.local_bookmarks == known)
    };
    if matches(last).await? {
        return Ok(last.clone());
    }

    let mut found: Option<Operation> = None;
    let mut operation = last.clone();
    loop {
        if matches(&operation).await? {
            found = Some(operation.clone());
        } else if found.is_some() {
            break;
        }
        let Some(parent) = operation.parents().await.map_err(local)?.into_iter().next() else {
            break;
        };
        operation = parent;
    }
    found.ok_or_else(|| local("no forge operation has the bookmarks this view knows"))
}

/// Fetches `wanted` commits and the ancestors that the local store lacks,
/// one generation per request. Their trees stay on the forge.
async fn fetch_commits(
    repo_path: &Path,
    remote: &dyn Transport,
    wanted: Vec<Vec<u8>>,
) -> Result<(), SyncError> {
    let store = Disk::new(&repo_path.join("store"));
    let mut seen = HashSet::new();
    let mut next = wanted;
    while !next.is_empty() {
        let mut missing = vec![];
        for id in next.drain(..) {
            if ids::is_virtual(&id) || !seen.insert(id.clone()) {
                continue;
            }
            if store.get(&id).await.map_err(SyncError::Local)?.is_none() {
                missing.push(id);
            }
        }
        for batch in missing.chunks(BATCH) {
            for bytes in remote
                .fetch(&object_ids(batch)?)
                .map_err(SyncError::Transport)?
            {
                let id = ids::hash(&bytes);
                if !batch.contains(&id) {
                    return Err(local("the forge sent an object that wasn't asked for"));
                }
                let Object::Commit(commit) = codec::decode(&bytes).map_err(local)? else {
                    return Err(local("the forge sent another object for a commit"));
                };
                next.extend(commit.parents.iter().map(CommitId::to_bytes));
                store.put(&id, bytes).await.map_err(SyncError::Local)?;
            }
        }
    }
    Ok(())
}

/// The IDs of objects that the clone names, which are hashes, as the
/// forge's calls take them.
fn object_ids(ids: &[Vec<u8>]) -> Result<Vec<ObjectId>, SyncError> {
    ids.iter()
        .map(|id| ObjectId::try_from(id.as_slice()).map_err(local))
        .collect()
}

async fn push(
    repo: &Arc<ReadonlyRepo>,
    repo_path: &Path,
    remote: &dyn Transport,
    names: &[String],
) -> Result<Synced, SyncError> {
    // A bookmark absent locally but present on the forge is a deletion, so
    // only one absent on both sides is unknown.
    let unknown: Vec<_> = names
        .iter()
        .filter(|name| {
            let name = RefName::new(name);
            repo.view().get_local_bookmark(name).is_absent()
                && repo
                    .view()
                    .get_remote_bookmark(name.to_remote_symbol(forge()))
                    .is_absent()
        })
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(SyncError::NoSuchBookmark(unknown));
    }

    // After an `op restore`, the view knows an older forge state than the
    // last fetch, so its bookmarks would move the forge's back. A fetch
    // first merges what the forge holds.
    let mut repo = repo.clone();
    let last = repo
        .loader()
        .load_operation(&JjOperationId::new(
            ForgeDir::new(repo_path).head()?.to_vec(),
        ))
        .await
        .map_err(local)?;
    if known_forge_operation(&repo, &last).await?.id() != last.id() {
        repo = fetch(&repo, repo_path, remote).await?.repo;
    }

    let mut uploads = Uploads::default();
    for attempt in 0..ATTEMPTS {
        let conflicted: Vec<_> = names
            .iter()
            .filter(|name| {
                !repo
                    .view()
                    .get_local_bookmark(RefName::new(name))
                    .is_resolved()
            })
            .cloned()
            .collect();
        if !conflicted.is_empty() {
            return Err(SyncError::Conflict(conflicted));
        }

        let Some(request) = publish_request(&repo, repo_path, remote, names, &mut uploads).await?
        else {
            return Ok(Synced::Unchanged);
        };
        match remote.publish(&request).map_err(SyncError::Transport)? {
            Published::Moved(_) => {
                return Ok(Synced::Changed(fetch(&repo, repo_path, remote).await?.repo));
            }
            Published::Stale(_) => {
                repo = fetch(&repo, repo_path, remote).await?.repo;
                // Racing pushers that retry at once collide again.
                std::thread::sleep(backoff(attempt));
            }
        }
    }
    Err(SyncError::Contended)
}

/// A random wait between half and all of a ceiling that doubles with each
/// attempt, from 50 ms.
fn backoff(attempt: u32) -> Duration {
    let ceiling = 50u64 << attempt.min(6);
    let random = RandomState::new().build_hasher().finish();
    Duration::from_millis(ceiling / 2 + random % (ceiling / 2 + 1))
}

/// Builds the publish of `names` from the forge head the last fetch
/// merged, after uploading what it needs, or `None` when every bookmark is
/// where the forge has it.
async fn publish_request(
    repo: &Arc<ReadonlyRepo>,
    repo_path: &Path,
    remote: &dyn Transport,
    names: &[String],
    uploads: &mut Uploads,
) -> Result<Option<PublishRequest>, SyncError> {
    let expected = ForgeDir::new(repo_path).head()?;
    let loader = repo.loader();
    let forge_view = loader
        .load_at(
            &loader
                .load_operation(&JjOperationId::new(expected.to_vec()))
                .await
                .map_err(local)?,
        )
        .await
        .map_err(local)?
        .view()
        .clone();

    let mut bookmarks = vec![];
    let mut targets = vec![];
    for name in names {
        let new = repo.view().get_local_bookmark(RefName::new(name));
        let old = forge_view.get_local_bookmark(RefName::new(name));
        if new == old {
            continue;
        }
        targets.extend(new.present_adds().cloned());
        bookmarks.push(BookmarkUpdate {
            name: name.clone(),
            old_target: codec::encode_ref_target(old),
            new_target: codec::encode_ref_target(new),
        });
    }
    if bookmarks.is_empty() {
        return Ok(None);
    }

    let forge_heads: Vec<_> = forge_view.heads().iter().cloned().collect();
    let new_commits: Vec<CommitId> = ResolvedRevsetExpression::commits(targets)
        .ancestors()
        .minus(&ResolvedRevsetExpression::commits(forge_heads.clone()).ancestors())
        .evaluate(repo.as_ref())
        .map_err(local)?
        .stream()
        .try_collect()
        .await
        .map_err(local)?;

    // A forge head that this repository no longer shows was abandoned or
    // rewritten here, so the forge hides it too.
    let local_heads: Vec<_> = repo.view().heads().iter().cloned().collect();
    let hidden_ids: Vec<Vec<u8>> = ResolvedRevsetExpression::commits(forge_heads)
        .minus(&ResolvedRevsetExpression::commits(local_heads).ancestors())
        .evaluate(repo.as_ref())
        .map_err(local)?
        .stream()
        .map_ok(|id| id.to_bytes())
        .try_collect()
        .await
        .map_err(local)?;

    let commit_predecessors = predecessors(repo, remote, &new_commits).await?;
    upload(repo_path, remote, new_commits, uploads).await?;

    Ok(Some(PublishRequest {
        expected_operation_id: expected.to_vec(),
        bookmarks,
        head_ids: vec![],
        hidden_ids,
        commit_predecessors,
        description: format!("push {} to forge", names.join(", ")),
        segment_ids: uploads.segments.iter().map(SegmentId::to_vec).collect(),
    }))
}

type Predecessors = HashMap<String, Vec<u8>>;

/// Maps each new commit to the versions of its change that the forge holds
/// and that it replaced, as #103 decides. A version that never left the
/// clone, such as a snapshot of the working copy, stays private, and the
/// walk goes on through it to the versions before. So a push publishes only
/// what the pushed commits reach.
async fn predecessors(
    repo: &Arc<ReadonlyRepo>,
    remote: &dyn Transport,
    new_commits: &[CommitId],
) -> Result<Predecessors, SyncError> {
    let entries: Vec<_> = walk_predecessors(repo, new_commits)
        .try_collect()
        .await
        .map_err(local)?;
    let replaced: HashMap<CommitId, Vec<CommitId>> = entries
        .iter()
        .map(|entry| (entry.commit.id().clone(), entry.predecessor_ids().to_vec()))
        .collect();

    let mut held: HashSet<CommitId> = new_commits.iter().cloned().collect();
    let candidates: Vec<_> = replaced
        .values()
        .flatten()
        .map(CommitId::to_bytes)
        .collect();
    for batch in candidates.chunks(BATCH) {
        let missing: HashSet<_> = remote
            .missing(&object_ids(batch)?)
            .map_err(SyncError::Transport)?
            .iter()
            .map(ObjectId::to_vec)
            .collect();
        held.extend(
            batch
                .iter()
                .filter(|id| !missing.contains(*id))
                .map(|id| CommitId::new(id.clone())),
        );
    }

    let mut map = HashMap::new();
    for commit in new_commits {
        let mut found = vec![];
        let mut seen = HashSet::new();
        let mut next: Vec<_> = replaced.get(commit).cloned().unwrap_or_default();
        while let Some(id) = next.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if held.contains(&id) {
                found.push(id);
            } else {
                next.extend(replaced.get(&id).cloned().unwrap_or_default());
            }
        }
        if !found.is_empty() {
            let ids = CommitIds {
                ids: found.iter().map(CommitId::to_bytes).collect(),
            };
            map.insert(commit.hex(), ids.encode_to_vec());
        }
    }
    Ok(map)
}

/// What the attempts of one push uploaded, so an attempt after a lost race
/// names the same segments instead of uploading again.
#[derive(Default)]
struct Uploads {
    ids: HashSet<Vec<u8>>,
    segments: Vec<SegmentId>,
}

/// Uploads `commits` that the forge lacks, with every object they reach, in
/// bundles below the upload limit, and adds the segments to `uploads`. An
/// object an earlier attempt uploaded counts as held.
async fn upload(
    repo_path: &Path,
    remote: &dyn Transport,
    commits: Vec<CommitId>,
    uploads: &mut Uploads,
) -> Result<(), SyncError> {
    let store = Disk::new(&repo_path.join("store"));
    let mut seen = HashSet::new();
    let mut objects = vec![];
    let mut next: Vec<_> = commits.iter().map(CommitId::to_bytes).collect();
    while !next.is_empty() {
        let candidates: Vec<_> = next
            .drain(..)
            .filter(|id| {
                !ids::is_virtual(id) && !uploads.ids.contains(id) && seen.insert(id.clone())
            })
            .collect();
        for batch in candidates.chunks(BATCH) {
            // An object the forge holds reaches only objects it holds.
            for id in remote
                .missing(&object_ids(batch)?)
                .map_err(SyncError::Transport)?
            {
                let bytes = store
                    .get(id.as_bytes())
                    .await
                    .map_err(SyncError::Local)?
                    .ok_or_else(|| local(format!("object {id} is missing")))?;
                let object = codec::decode(&bytes).map_err(local)?;
                next.extend(codec::references(&object));
                objects.push(bytes);
            }
        }
    }

    let mut bundle = vec![];
    let mut size = 0;
    for bytes in objects {
        if bytes.len() > MAX_OBJECT {
            return Err(local(format!(
                "object {} has {} bytes, and the forge takes at most {MAX_OBJECT} in one object",
                encode_hex(&ids::hash(&bytes)),
                bytes.len()
            )));
        }
        if size + bytes.len() > UPLOAD_BYTES && !bundle.is_empty() {
            send(remote, std::mem::take(&mut bundle), uploads)?;
            size = 0;
        }
        size += bytes.len();
        bundle.push(bytes);
    }
    if !bundle.is_empty() {
        send(remote, bundle, uploads)?;
    }
    Ok(())
}

fn send(
    remote: &dyn Transport,
    bundle: Vec<Vec<u8>>,
    uploads: &mut Uploads,
) -> Result<(), SyncError> {
    let ids: Vec<_> = bundle.iter().map(|bytes| ids::hash(bytes)).collect();
    uploads
        .segments
        .push(remote.upload(bundle).map_err(SyncError::Transport)?);
    uploads.ids.extend(ids);
    Ok(())
}

/// Starts a new change on `main@forge`, or on the root commit when the forge
/// has no `main`, and checks it out, as #30 decides.
///
/// # Errors
///
/// Fails when the change can't be written or checked out.
pub async fn start_on_main(
    workspace: &mut Workspace,
    repo: &Arc<ReadonlyRepo>,
) -> Result<Arc<ReadonlyRepo>, SyncError> {
    let main = repo
        .view()
        .get_remote_bookmark(RefName::new("main").to_remote_symbol(forge()))
        .target
        .as_normal()
        .cloned();
    let (parent, description) = match main {
        Some(id) => (
            repo.store().get_commit_async(&id).await.map_err(local)?,
            "check out main@forge",
        ),
        None => (repo.store().root_commit(), "check out the root commit"),
    };
    let old = repo
        .view()
        .get_wc_commit_id(workspace.workspace_name())
        .cloned();

    let mut tx = repo.start_transaction();
    let commit = tx
        .repo_mut()
        .check_out(workspace.workspace_name().to_owned(), &parent)
        .await
        .map_err(local)?;
    // Checking out abandons the empty change a new workspace starts with.
    tx.repo_mut().rebase_descendants().await.map_err(local)?;
    let repo = tx.commit(description).await.map_err(local)?;

    let old_tree = match old {
        Some(id) => Some(
            repo.store()
                .get_commit_async(&id)
                .await
                .map_err(local)?
                .tree(),
        ),
        None => None,
    };
    workspace
        .check_out(repo.op_id().clone(), old_tree.as_ref(), &commit)
        .await
        .map_err(local)?;
    Ok(repo)
}

/// Checks out the working-copy commit of `new` when a fetch moved it from
/// the one in `old`, which happens when the forge rewrote its parent.
///
/// # Errors
///
/// Fails when the working copy can't be updated.
pub async fn follow_working_copy(
    workspace: &mut Workspace,
    old: &ReadonlyRepo,
    new: &ReadonlyRepo,
) -> Result<bool, SyncError> {
    let name = workspace.workspace_name().to_owned();
    let (Some(old_id), Some(new_id)) = (
        old.view().get_wc_commit_id(&name),
        new.view().get_wc_commit_id(&name),
    ) else {
        return Ok(false);
    };
    if old_id == new_id {
        return Ok(false);
    }
    // The workspace loaded a store of its own, and jj-lib compares trees
    // only within one store, so both commits come from the workspace's.
    let store = workspace.repo_loader().store().clone();
    let old_commit = store.get_commit_async(old_id).await.map_err(local)?;
    let new_commit = store.get_commit_async(new_id).await.map_err(local)?;
    workspace
        .check_out(new.op_id().clone(), Some(&old_commit.tree()), &new_commit)
        .await
        .map_err(local)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_that_isnt_an_operation_id_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let forge = ForgeDir::new(dir.path());
        fs::create_dir_all(dir.path().join("forge")).unwrap();
        fs::write(dir.path().join("forge/head"), "").unwrap();

        assert!(forge.head().is_err());
    }

    #[test]
    fn set_head_replaces_the_file_whole() {
        let dir = tempfile::tempdir().unwrap();
        let forge = ForgeDir::new(dir.path());

        let id = |byte| OperationId::try_from([byte; 32].as_slice()).unwrap();

        forge.set_head(id(1)).unwrap();
        forge.set_head(id(2)).unwrap();

        assert_eq!(forge.head().unwrap(), id(2));
        assert_eq!(fs::read_dir(dir.path().join("forge")).unwrap().count(), 1);
    }
}
