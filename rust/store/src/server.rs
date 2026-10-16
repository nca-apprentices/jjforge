//! The repositories that vcs serves, each under `t/{org}/r/{repo}/` in the
//! object store, as ADR 0002 lays it out:
//!
//! ```text
//! pool/seg/{segment}   the objects of one upload
//! pool/idx/{segment}   where each object of the segment starts and ends
//! op/{hash}            operations and views
//! HEAD                 the current operation's ID, moved by compare-and-swap
//! ```
//!
//! An operation names the segments it made visible, so the log alone finds
//! every object. vcs builds an operation with jj-lib over its own index,
//! which it keeps in a local cache, and moves `HEAD` itself.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use futures::stream;
use jj_lib::backend::CommitId;
use jj_lib::backend::MillisSinceEpoch;
use jj_lib::backend::Timestamp;
use jj_lib::config::StackedConfig;
use jj_lib::default_index::DefaultIndexStore;
use jj_lib::default_submodule_store::DefaultSubmoduleStore;
use jj_lib::hex_util::encode_hex;
use jj_lib::object_id::ObjectId as _;
use jj_lib::op_heads_store::OpHeadsStore;
use jj_lib::op_heads_store::OpHeadsStoreError;
use jj_lib::op_heads_store::OpHeadsStoreLock;
use jj_lib::op_store::OpStore;
use jj_lib::op_store::Operation;
use jj_lib::op_store::OperationId as JjOperationId;
use jj_lib::op_store::OperationMetadata;
use jj_lib::op_store::RefTarget;
use jj_lib::op_store::TimestampRange;
use jj_lib::op_store::ViewId;
use jj_lib::ref_name::RefNameBuf;
use jj_lib::repo::MutableRepo;
use jj_lib::repo::Repo as _;
use jj_lib::repo::RepoLoader;
use jj_lib::settings::UserSettings;
use jj_lib::signing_factory::signer_from_settings;
use jj_lib::simple_workspace_store::SimpleWorkspaceStore;
use jj_lib::tree_merge::MergeOptions;
use jjforge_proto::store::v1::CommitIds;
use jjforge_proto::sync::v1::PublishRequest;
use prost::Message as _;
use tokio::sync::RwLock;
use url::Url;

use crate::Published;
use crate::backend;
use crate::bucket::Bucket;
use crate::bucket::Swap;
use crate::bucket::Tagged;
use crate::codec;
use crate::codec::Object;
use crate::ids;
use crate::ids::ObjectId;
use crate::ids::OperationId;
use crate::ids::SegmentId;
use crate::op_store;
use crate::segment;
use crate::shelf::Shelf;
use crate::shelf::Source;

/// How many objects vcs reads from the object store at once.
const CONCURRENCY: usize = 16;

/// The most bytes of commits and trees a publish's check holds at once, so a
/// publish of many segments can't exhaust vcs.
const CLOSURE_BATCH: u64 = 16 << 20;

/// Why a call on a repository failed.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    /// The repository, an operation, or an object doesn't exist.
    #[error("{0} doesn't exist")]
    NotFound(String),
    /// The request breaks a rule of the store.
    #[error("{0}")]
    Invalid(String),
    /// The object store or the local cache failed.
    #[error(transparent)]
    Internal(Source),
}

/// An ID that the store read from itself, so a wrong length means a corrupt
/// store.
fn operation_id(bytes: &[u8]) -> Result<OperationId, RepoError> {
    OperationId::try_from(bytes).map_err(internal)
}

fn internal(err: impl Into<Source>) -> RepoError {
    RepoError::Internal(err.into())
}

/// Who made an operation, from the verified token, as #102 decides.
#[derive(Clone, Debug)]
pub struct Author {
    /// The principal the token names.
    pub subject: String,
    /// The principals that delegated to `subject`, outermost first.
    pub delegation: Vec<String>,
}

/// Every repository under one object store URL.
pub struct Repos {
    bucket: Arc<Bucket>,
    cache: PathBuf,
    settings: UserSettings,
    catalogs: std::sync::Mutex<HashMap<String, Arc<RwLock<Catalog>>>>,
}

impl fmt::Debug for Repos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Repos({:?}, {})", self.bucket, self.cache.display())
    }
}

impl Repos {
    /// Opens the repositories under `url`, with jj's index of each in a
    /// directory under `cache`, which vcs can lose at any time.
    ///
    /// # Errors
    ///
    /// Fails on a URL that isn't `s3://`, `file://`, or `memory:///`, or
    /// when the store can't be configured.
    pub fn open(url: &Url, cache: &Path) -> Result<Self, RepoError> {
        let bucket = Bucket::open(url).map_err(internal)?;
        let settings = UserSettings::from_config_and_home_dir(StackedConfig::with_defaults(), None)
            .map_err(internal)?;
        Ok(Self {
            bucket: Arc::new(bucket),
            cache: cache.to_owned(),
            settings,
            catalogs: std::sync::Mutex::default(),
        })
    }

    /// The repository with these IDs. vcs addresses repositories by ID, so a
    /// rename never changes a clone address.
    ///
    /// # Errors
    ///
    /// Fails on an ID that isn't letters, digits, and hyphens.
    pub fn get(&self, org: &str, repo: &str) -> Result<Repo, RepoError> {
        for id in [org, repo] {
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(RepoError::NotFound(format!("repository {org}/{repo}")));
            }
        }
        let prefix = format!("t/{org}/r/{repo}");
        let catalog = self
            .catalogs
            .lock()
            .map_err(|_| RepoError::Internal("a catalog lock is poisoned".into()))?
            .entry(prefix.clone())
            .or_default()
            .clone();
        Ok(Repo {
            bucket: self.bucket.clone(),
            cache: self.cache.join(org).join(repo),
            settings: self.settings.clone(),
            catalog,
            prefix,
            name: format!("{org}/{repo}"),
        })
    }
}

/// One repository in the object store. A clone shares the repository's
/// catalog, so it costs no read.
#[derive(Clone)]
pub struct Repo {
    bucket: Arc<Bucket>,
    cache: PathBuf,
    settings: UserSettings,
    catalog: Arc<RwLock<Catalog>>,
    prefix: String,
    /// `{org}/{repo}`, as errors name the repository to a client.
    name: String,
}

impl fmt::Debug for Repo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Repo({})", self.prefix)
    }
}

/// Where an object lives.
#[derive(Clone, Debug)]
struct Location {
    segment: Arc<str>,
    range: Range<u64>,
    /// Whether the object names other objects, as a commit or a tree does.
    links: bool,
}

/// The objects that a repository's log makes visible, found from its
/// operations. Operations and segments never change, so the catalog only
/// grows, and each replica keeps its own.
#[derive(Debug, Default)]
struct Catalog {
    operations: HashSet<Vec<u8>>,
    locations: HashMap<Vec<u8>, Location>,
}

impl Repo {
    fn key(&self, rest: &str) -> String {
        format!("{}/{rest}", self.prefix)
    }

    fn op_key(&self, id: &[u8]) -> String {
        self.key(&format!("op/{}", encode_hex(id)))
    }

    fn segment_key(&self, id: &str) -> String {
        self.key(&format!("pool/seg/{id}"))
    }

    fn index_key(&self, id: &str) -> String {
        self.key(&format!("pool/idx/{id}"))
    }

    /// Creates the repository at the root operation, before the server
    /// records it, so a recorded repository can always be cloned, as #25
    /// decides. Creating it again changes nothing.
    ///
    /// # Errors
    ///
    /// Fails when the store does.
    pub async fn create(&self) -> Result<(), RepoError> {
        self.bucket
            .swap(&self.key("HEAD"), None, ids::root_operation_id().to_bytes())
            .await
            .map_err(internal)?;
        Ok(())
    }

    /// Removes everything under the repository's prefix, as #28 decides.
    /// Deleting it again changes nothing.
    ///
    /// # Errors
    ///
    /// Fails when the store does.
    pub async fn delete(&self) -> Result<(), RepoError> {
        self.bucket
            .delete_all(&self.prefix)
            .await
            .map_err(internal)?;
        *self.catalog.write().await = Catalog::default();
        Ok(())
    }

    async fn tagged_head(&self) -> Result<Tagged, RepoError> {
        self.bucket
            .get(&self.key("HEAD"))
            .await
            .map_err(internal)?
            .ok_or_else(|| RepoError::NotFound(format!("repository {}", self.name)))
    }

    /// Reads `HEAD` from the object store on every call, as #32 decides.
    ///
    /// # Errors
    ///
    /// Fails when the repository doesn't exist, or when the store does.
    pub async fn head(&self) -> Result<OperationId, RepoError> {
        operation_id(&self.tagged_head().await?.bytes)
    }

    async fn read_operation(&self, id: &[u8]) -> Result<(Vec<u8>, Operation), RepoError> {
        let not_found = || RepoError::NotFound(format!("operation {}", encode_hex(id)));
        let bytes = self
            .bucket
            .get(&self.op_key(id))
            .await
            .map_err(internal)?
            .ok_or_else(not_found)?
            .bytes;
        match codec::decode(&bytes) {
            Ok(Object::Operation(operation)) => Ok((bytes, *operation)),
            _ => Err(not_found()),
        }
    }

    /// Adds the operations that `head` reaches to the catalog, with the
    /// segments they name. A log that reaches the root without meeting an
    /// operation the catalog knows belongs to a repository created again
    /// under the same IDs, so the catalog starts over.
    async fn advance(&self, head: &[u8]) -> Result<(), RepoError> {
        let root = ids::root_operation_id().to_bytes();
        let known = |catalog: &Catalog, id: &[u8]| catalog.operations.contains(id);
        if known(&*self.catalog.read().await, head) {
            return Ok(());
        }

        let mut queue = VecDeque::from([head.to_vec()]);
        let mut segments = vec![];
        let mut seen = HashSet::new();
        let mut met_known = false;
        while let Some(id) = queue.pop_front() {
            if id == root || seen.contains(&id) {
                continue;
            }
            if known(&*self.catalog.read().await, &id) {
                met_known = true;
                continue;
            }
            let (bytes, operation) = self.read_operation(&id).await?;
            segments.extend(
                codec::segment_ids(&bytes)
                    .map_err(internal)?
                    .iter()
                    .map(|segment| encode_hex(segment)),
            );
            queue.extend(operation.parents.iter().map(JjOperationId::to_bytes));
            seen.insert(id);
        }
        let mut locations = vec![];
        for segment in segments {
            locations.extend(self.read_index(&segment).await?);
        }

        let mut catalog = self.catalog.write().await;
        if !met_known {
            *catalog = Catalog::default();
        }
        catalog.locations.extend(locations);
        catalog.operations.extend(seen);
        Ok(())
    }

    async fn read_index(&self, segment: &str) -> Result<Vec<(Vec<u8>, Location)>, RepoError> {
        let index = self
            .bucket
            .get(&self.index_key(segment))
            .await
            .map_err(internal)?
            .ok_or_else(|| RepoError::NotFound(format!("segment {segment}")))?
            .bytes;
        let segment: Arc<str> = Arc::from(segment);
        Ok(segment::parse(&index)
            .map_err(internal)?
            .into_iter()
            .map(|entry| {
                (
                    entry.id,
                    Location {
                        segment: segment.clone(),
                        range: entry.range,
                        links: entry.links,
                    },
                )
            })
            .collect())
    }

    /// Answers which of `ids` the repository's log doesn't reach, as #31
    /// asks before an upload.
    ///
    /// # Errors
    ///
    /// Fails when the repository doesn't exist, or when the store does.
    pub async fn missing(&self, ids: &[ObjectId]) -> Result<Vec<ObjectId>, RepoError> {
        self.advance(self.head().await?.as_bytes()).await?;
        let catalog = self.catalog.read().await;
        Ok(ids
            .iter()
            .filter(|id| {
                !ids::is_virtual(id.as_bytes()) && !catalog.locations.contains_key(id.as_bytes())
            })
            .copied()
            .collect())
    }

    /// Stores encoded objects as one segment with its index and returns the
    /// segment's ID, which a publish names.
    ///
    /// # Errors
    ///
    /// Fails on bytes that aren't a `store.v1` object as `encode` writes it,
    /// on an operation or a view, which only vcs writes, or when the store
    /// does.
    pub async fn upload(&self, objects: Vec<Vec<u8>>) -> Result<SegmentId, RepoError> {
        self.tagged_head().await?;
        if objects.is_empty() {
            return Err(RepoError::Invalid("an upload needs an object".to_owned()));
        }
        let mut marked = Vec::with_capacity(objects.len());
        for (index, bytes) in objects.into_iter().enumerate() {
            let invalid = |reason: String| RepoError::Invalid(format!("object {index} {reason}"));
            let links = match codec::decode(&bytes) {
                Ok(Object::Operation(_) | Object::View(_)) => {
                    return Err(invalid(
                        "is an operation or a view, which vcs writes".to_owned(),
                    ));
                }
                Ok(Object::Commit(_) | Object::Tree(_)) => true,
                Ok(_) => false,
                Err(err) => return Err(invalid(format!("is invalid: {err}"))),
            };
            marked.push((bytes, links));
        }

        let packed = segment::pack(marked).map_err(|err| RepoError::Invalid(err.to_string()))?;
        let id = encode_hex(&packed.id);
        // A reader finds a segment through its index, so the index comes
        // last.
        self.bucket
            .put_immutable(&self.segment_key(&id), packed.segment)
            .await
            .map_err(internal)?;
        self.bucket
            .put_immutable(&self.index_key(&id), packed.index)
            .await
            .map_err(internal)?;
        SegmentId::try_from(packed.id.as_slice()).map_err(internal)
    }

    /// Reads objects by ID, in the order of `ids`, with one range read per
    /// segment.
    ///
    /// # Errors
    ///
    /// Fails when an object isn't in the repository, or when the store
    /// does.
    pub async fn fetch(&self, ids: &[ObjectId]) -> Result<Vec<Vec<u8>>, RepoError> {
        self.advance(self.head().await?.as_bytes()).await?;
        let ids: Vec<_> = ids.iter().map(ObjectId::to_vec).collect();
        let catalog = self.catalog.read().await;
        read_objects(
            &self.bucket,
            &self.prefix,
            &catalog.locations,
            &HashMap::new(),
            &ids,
        )
        .await
    }

    /// Reads the operations that `HEAD` reaches and `since` doesn't, each
    /// followed by its view, newest first. Without `since`, every operation
    /// but the root.
    ///
    /// # Errors
    ///
    /// Fails when `since` isn't an operation that `HEAD` reaches, or when
    /// the store does.
    pub async fn operations_since(
        &self,
        since: Option<OperationId>,
    ) -> Result<Vec<Vec<u8>>, RepoError> {
        let root = OperationId::root().to_vec();
        let since = since.map_or_else(|| root.clone(), |id| id.to_vec());
        let mut bundle = vec![];
        let mut next = self.head().await?.to_vec();
        while next != since {
            if next == root {
                return Err(RepoError::NotFound(format!(
                    "operation {} in the log",
                    encode_hex(&since)
                )));
            }
            let (bytes, operation) = self.read_operation(&next).await?;
            let view = self
                .bucket
                .get(&self.op_key(operation.view_id.as_bytes()))
                .await
                .map_err(internal)?
                .ok_or_else(|| RepoError::NotFound(format!("view {}", operation.view_id.hex())))?
                .bytes;
            bundle.push(bytes);
            bundle.push(view);
            // vcs writes a log without merges.
            next = operation
                .parents
                .first()
                .map_or_else(|| root.clone(), JjOperationId::to_bytes);
        }
        Ok(bundle)
    }

    fn loader(&self, overlay: HashMap<Vec<u8>, Location>) -> Result<RepoLoader, RepoError> {
        let pool = Pool {
            bucket: self.bucket.clone(),
            prefix: self.prefix.clone(),
            catalog: self.catalog.clone(),
            overlay,
        };
        let store = jj_lib::store::Store::new(
            Box::new(backend::Store::new(Box::new(pool), CONCURRENCY)),
            signer_from_settings(&self.settings).map_err(internal)?,
            MergeOptions::from_settings(&self.settings).map_err(internal)?,
        );
        let op_store: Arc<dyn OpStore> = Arc::new(op_store::Store::new(Box::new(Operations {
            bucket: self.bucket.clone(),
            prefix: self.prefix.clone(),
        })));
        let op_heads_store: Arc<dyn OpHeadsStore> = Arc::new(Heads);

        // jj's index and the stores a loader needs but vcs never uses live
        // in the cache, which jj-lib rebuilds from the log when it's gone.
        let index_dir = self.cache.join("index");
        std::fs::create_dir_all(&index_dir).map_err(internal)?;
        let index = DefaultIndexStore::init(&index_dir).map_err(internal)?;
        let workspaces = self.cache.join("workspace_store");
        let workspace_store = if workspaces.exists() {
            SimpleWorkspaceStore::load(&workspaces).map_err(|err| internal(err.0))?
        } else {
            std::fs::create_dir_all(&workspaces).map_err(internal)?;
            SimpleWorkspaceStore::init(&workspaces).map_err(|err| internal(err.0))?
        };
        Ok(RepoLoader::new(
            self.settings.clone(),
            store,
            Arc::new(workspace_store),
            op_store,
            op_heads_store,
            Arc::new(index),
            Arc::new(DefaultSubmoduleStore::load(&self.cache.join("submodules"))),
        ))
    }

    /// Applies `request` to the view at `HEAD` with jj-lib, writes the view
    /// and an operation by `author`, and moves `HEAD` to it by
    /// compare-and-swap, as #31, #33, and #102 decide. Every check runs
    /// before anything is written.
    ///
    /// # Errors
    ///
    /// Fails when the repository doesn't exist, when a segment or a commit is
    /// absent, when a new object names one the repository lacks, when a
    /// bookmark name is empty, or when the store does.
    pub async fn publish(
        &self,
        request: PublishRequest,
        author: Author,
    ) -> Result<Published, RepoError> {
        // jj-lib's futures aren't Send, so the publish runs on a thread of
        // its own, in the caller's span, and this future stays Send.
        let repo = self.clone();
        let runtime = tokio::runtime::Handle::current();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            span.in_scope(|| runtime.block_on(repo.publish_here(&request, &author)))
        })
        .await
        .map_err(internal)?
    }

    async fn publish_here(
        &self,
        request: &PublishRequest,
        author: &Author,
    ) -> Result<Published, RepoError> {
        let head = self.tagged_head().await?;
        if request.expected_operation_id != head.bytes {
            return Ok(Published::Stale(operation_id(&head.bytes)?));
        }
        self.advance(&head.bytes).await?;

        let overlay = self.read_segments(&request.segment_ids).await?;
        let predecessors = commit_predecessors(request)?;
        let loader = self.loader(overlay.clone())?;
        let operation = loader
            .load_operation(&JjOperationId::new(head.bytes.clone()))
            .await
            .map_err(internal)?;
        let base = loader.load_at(&operation).await.map_err(internal)?;
        let mut repo = MutableRepo::new(base.clone(), base.readonly_index(), base.view());

        let bookmarks = bookmark_updates(request)?;
        if bookmarks
            .iter()
            .any(|(name, old, _)| repo.get_local_bookmark(name) != old)
        {
            return Ok(Published::Stale(operation_id(&head.bytes)?));
        }
        let mut visible = request.head_ids.clone();
        for (_, _, new) in &bookmarks {
            visible.extend(new.present_adds().map(CommitId::to_bytes));
        }

        let mut roots = visible.clone();
        roots.extend(overlay.keys().cloned());
        for (id, replaced) in &predecessors {
            roots.push(id.to_bytes());
            roots.extend(replaced.iter().map(CommitId::to_bytes));
        }
        self.check_closure(&overlay, roots).await?;

        let mut commits = vec![];
        for id in visible {
            let id = CommitId::new(id);
            let commit = base
                .store()
                .get_commit_async(&id)
                .await
                .map_err(|_| RepoError::Invalid(format!("commit {} is missing", id.hex())))?;
            commits.push(commit);
        }
        repo.add_heads(&commits).await.map_err(internal)?;
        for (name, _, target) in bookmarks {
            repo.set_local_bookmark_target(&name, target);
        }
        for replaced in predecessors.values().flatten() {
            repo.remove_head(replaced);
        }
        for hidden in &request.hidden_ids {
            repo.remove_head(&CommitId::new(hidden.clone()));
        }
        let (index, view, _) = repo.consume().await.map_err(internal)?;

        // A publish that changes no view leaves the log as it is.
        if view.store_view() == base.view().store_view() {
            return Ok(Published::Moved(operation_id(&head.bytes)?));
        }

        let view_id = self.write_view(view.store_view()).await?;
        let new = Operation {
            view_id,
            parents: vec![JjOperationId::new(head.bytes.clone())],
            metadata: metadata(request, author),
            commit_predecessors: Some(predecessors),
        };
        let bytes = codec::encode_operation(&new, request.segment_ids.clone());
        let id = ids::hash(&bytes);
        self.bucket
            .put_immutable(&self.op_key(&id), bytes)
            .await
            .map_err(internal)?;

        // The index is a cache. A replica that misses it rebuilds it.
        let written = jj_lib::operation::Operation::new(
            loader.op_store().clone(),
            JjOperationId::new(id.clone()),
            new,
        );
        loader.index_store().write_index(index, &written).ok();

        match self
            .bucket
            .swap(&self.key("HEAD"), Some(&head), id.clone())
            .await
            .map_err(internal)?
        {
            Swap::Done => Ok(Published::Moved(operation_id(&id)?)),
            Swap::Lost => Ok(Published::Stale(self.head().await?)),
        }
    }

    async fn write_view(&self, view: &jj_lib::op_store::View) -> Result<ViewId, RepoError> {
        let bytes = codec::encode(&Object::View(Box::new(view.clone())))
            .map_err(|err| RepoError::Invalid(err.to_string()))?;
        let id = ids::hash(&bytes);
        self.bucket
            .put_immutable(&self.op_key(&id), bytes)
            .await
            .map_err(internal)?;
        Ok(ViewId::new(id))
    }

    /// Reads the indexes of the segments a publish names.
    async fn read_segments(
        &self,
        segment_ids: &[Vec<u8>],
    ) -> Result<HashMap<Vec<u8>, Location>, RepoError> {
        let mut overlay = HashMap::new();
        for segment in segment_ids {
            let entries = self
                .read_index(&encode_hex(segment))
                .await
                .map_err(|err| match err {
                    RepoError::NotFound(what) => RepoError::Invalid(format!("{what} is missing")),
                    err => err,
                })?;
            overlay.extend(entries);
        }
        Ok(overlay)
    }

    /// Checks that everything `roots` reach is in the repository or in the
    /// publish's segments. The roots are the commits the publish makes
    /// visible, every object of its segments, and every commit it names as
    /// an earlier version, so an object in the catalog reaches only objects
    /// in it, and the walk stops there.
    async fn check_closure(
        &self,
        overlay: &HashMap<Vec<u8>, Location>,
        roots: Vec<Vec<u8>>,
    ) -> Result<(), RepoError> {
        let catalog = self.catalog.read().await;
        let mut seen = HashSet::new();
        let mut next = roots;
        while !next.is_empty() {
            let wanted: Vec<_> = next
                .drain(..)
                .filter(|id| {
                    !ids::is_virtual(id)
                        && !catalog.locations.contains_key(id)
                        && seen.insert(id.clone())
                })
                .collect();
            if let Some(absent) = wanted.iter().find(|id| !overlay.contains_key(*id)) {
                return Err(RepoError::Invalid(format!(
                    "object {} is missing",
                    encode_hex(absent)
                )));
            }
            // Only commits and trees name objects, so the walk reads no file,
            // and it holds at most one batch of them at once.
            let mut batch = vec![];
            let mut size = 0;
            for id in wanted {
                let location = &overlay[&id];
                if !location.links {
                    continue;
                }
                size += location.range.end - location.range.start;
                batch.push(id);
                if size >= CLOSURE_BATCH {
                    next.extend(self.references(&catalog, overlay, &batch).await?);
                    batch.clear();
                    size = 0;
                }
            }
            next.extend(self.references(&catalog, overlay, &batch).await?);
        }
        Ok(())
    }

    /// The IDs that the objects `ids` name.
    async fn references(
        &self,
        catalog: &Catalog,
        overlay: &HashMap<Vec<u8>, Location>,
        ids: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, RepoError> {
        let objects =
            read_objects(&self.bucket, &self.prefix, &catalog.locations, overlay, ids).await?;
        let mut references = vec![];
        for bytes in objects {
            let object = codec::decode(&bytes).map_err(internal)?;
            references.extend(codec::references(&object));
        }
        Ok(references)
    }
}

/// Reads objects by ID from their segments, one range read per segment.
async fn read_objects(
    bucket: &Bucket,
    prefix: &str,
    locations: &HashMap<Vec<u8>, Location>,
    overlay: &HashMap<Vec<u8>, Location>,
    ids: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, RepoError> {
    let mut by_segment: BTreeMap<Arc<str>, Vec<(usize, Range<u64>)>> = BTreeMap::new();
    for (position, id) in ids.iter().enumerate() {
        let location = overlay
            .get(id)
            .or_else(|| locations.get(id))
            .ok_or_else(|| RepoError::NotFound(format!("object {}", encode_hex(id))))?;
        by_segment
            .entry(location.segment.clone())
            .or_default()
            .push((position, location.range.clone()));
    }

    let mut objects = vec![vec![]; ids.len()];
    let reads = stream::iter(by_segment)
        .map(|(segment, wanted)| async move {
            let ranges: Vec<_> = wanted.iter().map(|(_, range)| range.clone()).collect();
            let parts = bucket
                .get_ranges(&format!("{prefix}/pool/seg/{segment}"), &ranges)
                .await
                .map_err(internal)?;
            Ok::<_, RepoError>(wanted.into_iter().map(|(position, _)| position).zip(parts))
        })
        .buffer_unordered(CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;
    for (position, bytes) in reads.into_iter().flatten() {
        if ids::hash(&bytes) != ids[position] {
            return Err(RepoError::Internal(
                format!(
                    "object {} doesn't match its bytes",
                    encode_hex(&ids[position])
                )
                .into(),
            ));
        }
        objects[position] = bytes;
    }
    Ok(objects)
}

type BookmarkUpdate = (RefNameBuf, RefTarget, RefTarget);

/// Each bookmark a publish moves, with the target the client saw and the
/// new one.
fn bookmark_updates(request: &PublishRequest) -> Result<Vec<BookmarkUpdate>, RepoError> {
    request
        .bookmarks
        .iter()
        .map(|update| {
            let invalid =
                |reason: &str| RepoError::Invalid(format!("bookmark {:?} {reason}", update.name));
            if update.name.is_empty() {
                return Err(invalid("has no name"));
            }
            let old = codec::decode_ref_target(&update.old_target)
                .map_err(|_| invalid("has an invalid old target"))?;
            let new = codec::decode_ref_target(&update.new_target)
                .map_err(|_| invalid("has an invalid new target"))?;
            Ok((RefNameBuf::from(update.name.clone()), old, new))
        })
        .collect()
}

fn commit_predecessors(
    request: &PublishRequest,
) -> Result<BTreeMap<CommitId, Vec<CommitId>>, RepoError> {
    request
        .commit_predecessors
        .iter()
        .map(|(hex, bytes)| {
            let invalid = || RepoError::Invalid(format!("the predecessors of {hex:?} are invalid"));
            let id = CommitId::try_from_hex(hex).ok_or_else(invalid)?;
            let ids = CommitIds::decode(bytes.as_slice()).map_err(|_| invalid())?;
            Ok((id, ids.ids.into_iter().map(CommitId::new).collect()))
        })
        .collect()
}

/// The operation's metadata. The author comes from the token and the time
/// from vcs's clock, so a client can claim neither, as #102 decides.
fn metadata(request: &PublishRequest, author: &Author) -> OperationMetadata {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        });
    let now = Timestamp {
        timestamp: MillisSinceEpoch(millis),
        tz_offset: 0,
    };
    let mut attributes = BTreeMap::from([("jjforge.principal".to_owned(), author.subject.clone())]);
    if !author.delegation.is_empty() {
        attributes.insert("jjforge.delegation".to_owned(), author.delegation.join(","));
    }
    OperationMetadata {
        time: TimestampRange {
            start: now,
            end: now,
        },
        description: request.description.clone(),
        hostname: "jjforge".to_owned(),
        username: author.subject.clone(),
        is_snapshot: false,
        workspace_name: None,
        attributes,
    }
}

/// The objects of a repository on vcs: those its log reaches, and those of
/// the segments a publish names.
#[derive(Debug)]
struct Pool {
    bucket: Arc<Bucket>,
    prefix: String,
    catalog: Arc<RwLock<Catalog>>,
    overlay: HashMap<Vec<u8>, Location>,
}

#[async_trait]
impl Shelf for Pool {
    async fn get(&self, id: &[u8]) -> Result<Option<Vec<u8>>, Source> {
        let catalog = self.catalog.read().await;
        let ids = [id.to_vec()];
        match read_objects(
            &self.bucket,
            &self.prefix,
            &catalog.locations,
            &self.overlay,
            &ids,
        )
        .await
        {
            Ok(mut objects) => Ok(objects.pop()),
            Err(RepoError::NotFound(_)) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    async fn put(&self, _id: &[u8], _bytes: Vec<u8>) -> Result<(), Source> {
        Err("vcs stores objects only from an upload".into())
    }

    async fn ids_with_prefix(&self, _prefix: &str) -> Result<Vec<Vec<u8>>, Source> {
        Err("vcs resolves no object ID prefix".into())
    }
}

/// The operations and views of a repository on vcs, under `op/{hash}`.
#[derive(Debug)]
struct Operations {
    bucket: Arc<Bucket>,
    prefix: String,
}

#[async_trait]
impl Shelf for Operations {
    async fn get(&self, id: &[u8]) -> Result<Option<Vec<u8>>, Source> {
        let key = format!("{}/op/{}", self.prefix, encode_hex(id));
        Ok(self.bucket.get(&key).await?.map(|tagged| tagged.bytes))
    }

    async fn put(&self, id: &[u8], bytes: Vec<u8>) -> Result<(), Source> {
        let key = format!("{}/op/{}", self.prefix, encode_hex(id));
        Ok(self.bucket.put_immutable(&key, bytes).await?)
    }

    async fn ids_with_prefix(&self, prefix: &str) -> Result<Vec<Vec<u8>>, Source> {
        let dir = format!("{}/op/", self.prefix);
        Ok(self
            .bucket
            .list(&dir)
            .await?
            .into_iter()
            .filter_map(|key| {
                let name = key.strip_prefix(&dir)?;
                name.starts_with(prefix)
                    .then(|| jj_lib::hex_util::decode_hex(name))
                    .flatten()
            })
            .collect())
    }
}

/// vcs loads a repository at the operation it read from `HEAD` and moves
/// `HEAD` itself, so jj-lib's op heads are never asked for.
#[derive(Debug)]
struct Heads;

struct NoLock;

impl OpHeadsStoreLock for NoLock {}

#[async_trait]
impl OpHeadsStore for Heads {
    fn name(&self) -> &str {
        crate::NAME
    }

    async fn update_op_heads(
        &self,
        _old_ids: &[JjOperationId],
        new_id: &JjOperationId,
    ) -> Result<(), OpHeadsStoreError> {
        Err(OpHeadsStoreError::Write {
            new_op_id: new_id.clone(),
            source: "vcs moves HEAD itself".into(),
        })
    }

    async fn get_op_heads(&self) -> Result<Vec<JjOperationId>, OpHeadsStoreError> {
        Err(OpHeadsStoreError::Read("vcs reads HEAD itself".into()))
    }

    async fn lock(&self) -> Result<Box<dyn OpHeadsStoreLock + '_>, OpHeadsStoreError> {
        Ok(Box::new(NoLock))
    }
}
