//! The reads that the server makes through `source/v1`: history, trees,
//! files, and diffs, each at `HEAD` or at the operation a page token names,
//! answered as the messages of shared/proto/source/v1/.
//!
//! A revision is a bookmark, a prefix of a commit ID in hex, or a prefix of
//! a change ID in jj's reverse hex. An empty revision is the trunk bookmark.

mod diff;
mod file;
mod watch;

pub use diff::DiffPage;
pub use file::FileRead;
pub(crate) use watch::HeadSlot;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use futures::StreamExt as _;
use futures::TryStreamExt as _;
use jj_lib::backend::CommitId;
use jj_lib::backend::Signature;
use jj_lib::backend::TreeValue;
use jj_lib::commit::Commit;
use jj_lib::hex_util::encode_hex;
use jj_lib::hex_util::encode_reverse_hex;
use jj_lib::merged_tree::MergedTree;
use jj_lib::object_id::HexPrefix;
use jj_lib::object_id::ObjectId as _;
use jj_lib::object_id::PrefixResolution;
use jj_lib::op_store::OperationId as JjOperationId;
use jj_lib::ref_name::RefName;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo as _;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::revset::ResolvedRevsetExpression;
use jjforge_proto::prost_types::Timestamp;
use jjforge_proto::source::v1 as source;
use jjforge_proto::source::v1::tree_entry::Kind;

use super::Repo;
use super::RepoError;
use super::internal;
use crate::ids::OperationId;

/// The bookmarks that name trunk, in the order `trunk()` tries them.
const TRUNK: [&str; 3] = ["main", "master", "trunk"];

/// The most candidates an ambiguous revision lists.
const CANDIDATES: usize = 100;

/// The most files `GetChange` lists, well within gRPC's default limit of
/// 4 MiB per message. `Diff` pages through any number.
const CHANGED_FILES: usize = 10_000;

/// The page size of a list that names none, and the most it may name.
const PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 1000;

/// One page of a list.
pub struct Page<T> {
    /// The page's items, in the list's order.
    pub items: Vec<T>,
    /// The token of the next page, empty on the last.
    pub next: String,
}

/// Where a page starts. A token is `{operation}.{offset}`: the operation
/// that the first page read at, and the offset of the page's first item.
/// Every later page reads at that operation, so a push between two pages
/// neither skips nor repeats an item.
struct Cursor {
    operation: Option<Vec<u8>>,
    offset: usize,
}

impl Repo {
    /// Runs `read` on jj-lib's repository at `HEAD`.
    async fn at_head<T, F, Fut>(&self, read: F) -> Result<T, RepoError>
    where
        T: Send + 'static,
        F: FnOnce(Arc<ReadonlyRepo>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, RepoError>>,
    {
        self.at(None, read).await
    }

    /// Runs `read` on jj-lib's repository at `operation`, or at `HEAD` without
    /// one. jj-lib's futures aren't Send, so it runs on a thread of its own,
    /// as a publish does.
    async fn at<T, F, Fut>(&self, operation: Option<Vec<u8>>, read: F) -> Result<T, RepoError>
    where
        T: Send + 'static,
        F: FnOnce(Arc<ReadonlyRepo>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, RepoError>>,
    {
        let repo = self.clone();
        let runtime = tokio::runtime::Handle::current();
        let span = tracing::Span::current();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        tokio::task::spawn_blocking(move || {
            tracing::dispatcher::with_default(&dispatch, || {
                span.in_scope(|| {
                    runtime.block_on(async {
                        let head = repo.head().await?;
                        // The catalog holds every operation HEAD reaches, so
                        // it holds the operation of any earlier page too.
                        repo.advance(head.as_bytes()).await?;
                        let loader = repo.loader(HashMap::new())?;
                        let pinned = operation.is_some();
                        let id = operation.unwrap_or_else(|| head.to_vec());
                        let operation = loader
                            .load_operation(&JjOperationId::new(id))
                            .await
                            .map_err(|err| if pinned { stale_token() } else { internal(err) })?;
                        let jj = loader.load_at(&operation).await.map_err(internal)?;
                        read(jj).await
                    })
                })
            })
        })
        .await
        .map_err(internal)?
    }

    /// The change and commit that `rev` names.
    ///
    /// # Errors
    ///
    /// Fails when `rev` names nothing, or more than one commit.
    pub async fn resolve(&self, rev: &str) -> Result<source::ResolveResponse, RepoError> {
        let rev = rev.to_owned();
        self.at_head(move |repo| async move {
            let commit = commit(&repo, &rev).await?;
            Ok(source::ResolveResponse {
                change_id: encode_reverse_hex(commit.change_id().as_bytes()),
                commit_id: commit.id().hex(),
            })
        })
        .await
    }

    /// The change that `rev` names, and the files it touches against its
    /// parents, at most `CHANGED_FILES` of them.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does.
    pub async fn change(&self, rev: &str) -> Result<source::GetChangeResponse, RepoError> {
        let rev = rev.to_owned();
        let this = self.clone();
        self.at_head(move |repo| async move {
            let commit = commit(&repo, &rev).await?;
            let before = parent_tree(&repo, &commit).await?;
            let mut files: Vec<_> = diff::changes(&this, &repo, &before, &commit.tree())
                .await?
                .take(CHANGED_FILES + 1)
                .map_ok(|file| source::ChangedFile {
                    path: file.path.as_internal_file_string().to_owned(),
                    old_path: file
                        .old_path
                        .map(|path| path.as_internal_file_string().to_owned())
                        .unwrap_or_default(),
                    status: file.status.into(),
                    conflicted: !file.after.is_resolved(),
                })
                .try_collect()
                .await?;
            let files_truncated = files.len() > CHANGED_FILES;
            files.truncate(CHANGED_FILES);
            Ok(source::GetChangeResponse {
                change: Some(change(&repo, &commit)),
                files,
                files_truncated,
            })
        })
        .await
    }

    /// One page of the changes that `rev` reaches, newest first.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, or on a page token this call didn't
    /// give.
    pub async fn log(
        &self,
        rev: &str,
        token: &str,
        size: i32,
    ) -> Result<Page<source::Change>, RepoError> {
        let (rev, cursor, size) = (rev.to_owned(), cursor(token)?, page_size(size));
        let offset = cursor.offset;
        self.at(cursor.operation, move |repo| async move {
            let start = commit(&repo, &rev).await?;
            // The root commit is no change anyone made, as `jj log` hides it.
            let revset = ResolvedRevsetExpression::commit(start.id().clone())
                .ancestors()
                .minus(&ResolvedRevsetExpression::root())
                .evaluate(repo.as_ref())
                .map_err(internal)?;
            let ids: Vec<CommitId> = revset
                .stream()
                .skip(offset)
                .take(size + 1)
                .try_collect()
                .await
                .map_err(internal)?;
            let mut changes = Vec::with_capacity(ids.len());
            for id in ids {
                let commit = repo.store().get_commit_async(&id).await.map_err(internal)?;
                changes.push(change(&repo, &commit));
            }
            Ok(page(&repo, changes, offset, size))
        })
        .await
    }

    /// The earlier versions of the change that `rev` names, newest first.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, or on a page token this call didn't
    /// give.
    pub async fn evolution(
        &self,
        rev: &str,
        token: &str,
        size: i32,
    ) -> Result<Page<source::Version>, RepoError> {
        let (rev, cursor, size) = (rev.to_owned(), cursor(token)?, page_size(size));
        let offset = cursor.offset;
        self.at(cursor.operation, move |repo| async move {
            let start = commit(&repo, &rev).await?;
            let entries: Vec<_> =
                jj_lib::evolution::walk_predecessors(&repo, &[start.id().clone()])
                    .skip(offset)
                    .take(size + 1)
                    .try_collect()
                    .await
                    .map_err(internal)?;
            let versions = entries
                .iter()
                .map(|entry| source::Version {
                    commit_id: entry.commit.id().hex(),
                    time: Some(timestamp(&entry.commit.committer().timestamp)),
                    operation_id: entry
                        .operation
                        .as_ref()
                        .map(|operation| operation.id().hex())
                        .unwrap_or_default(),
                })
                .collect();
            Ok(page(&repo, versions, offset, size))
        })
        .await
    }

    /// The operations of the log, newest first.
    ///
    /// # Errors
    ///
    /// Fails on a page token this call didn't give.
    pub async fn operation_page(
        &self,
        token: &str,
        size: i32,
    ) -> Result<Page<source::Operation>, RepoError> {
        let (cursor, size) = (cursor(token)?, page_size(size));
        let offset = cursor.offset;
        self.at(cursor.operation, move |repo| async move {
            let operations: Vec<_> =
                jj_lib::op_walk::walk_ancestors(std::slice::from_ref(repo.operation()))
                    // The root operation is no change anyone made, as a watch
                    // leaves it out too.
                    .try_filter(|operation| std::future::ready(!operation.parent_ids().is_empty()))
                    .skip(offset)
                    .take(size + 1)
                    .try_collect()
                    .await
                    .map_err(internal)?;
            let operations = operations
                .iter()
                .map(|operation| {
                    let metadata = operation.metadata();
                    source::Operation {
                        id: operation.id().hex(),
                        time: Some(timestamp(&metadata.time.end)),
                        principal_id: metadata.username.clone(),
                        description: metadata.description.clone(),
                        parent_ids: operation
                            .parent_ids()
                            .iter()
                            .map(jj_lib::object_id::ObjectId::hex)
                            .collect(),
                    }
                })
                .collect();
            Ok(page(&repo, operations, offset, size))
        })
        .await
    }

    /// The bookmarks, by name, and the changes each points at.
    ///
    /// # Errors
    ///
    /// Fails on a page token this call didn't give.
    pub async fn bookmarks(
        &self,
        token: &str,
        size: i32,
    ) -> Result<Page<source::Bookmark>, RepoError> {
        let (cursor, size) = (cursor(token)?, page_size(size));
        let offset = cursor.offset;
        self.at(cursor.operation, move |repo| async move {
            let mut bookmarks = vec![];
            for (name, target) in repo.view().local_bookmarks().skip(offset).take(size + 1) {
                let mut changes = vec![];
                for id in target.present_adds() {
                    let commit = repo.store().get_commit_async(id).await.map_err(internal)?;
                    changes.push(encode_reverse_hex(commit.change_id().as_bytes()));
                }
                bookmarks.push(source::Bookmark {
                    name: name.as_str().to_owned(),
                    target_change_ids: changes,
                });
            }
            Ok(page(&repo, bookmarks, offset, size))
        })
        .await
    }

    /// The entries of the directory at `path` in the tree of `rev`,
    /// directories first, then by name. A conflicted directory lists the
    /// names of every side.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, when `path` is no directory, or on a
    /// page token this call didn't give.
    pub async fn tree(
        &self,
        rev: &str,
        path: &str,
        token: &str,
        size: i32,
    ) -> Result<Page<source::TreeEntry>, RepoError> {
        let (rev, cursor, size) = (rev.to_owned(), cursor(token)?, page_size(size));
        let offset = cursor.offset;
        let dir = repo_path(path)?;
        let page = self
            .at(cursor.operation, move |repo| async move {
                let commit = commit(&repo, &rev).await?;
                let trees = commit.tree().trees().await.map_err(internal)?;
                let mut names = BTreeMap::new();
                let mut found = false;
                for tree in &trees {
                    let Some(sub) = tree.sub_tree_recursive(&dir).await.map_err(internal)? else {
                        continue;
                    };
                    found = true;
                    for entry in sub.entries_non_recursive() {
                        names
                            .entry(entry.name().as_internal_str().to_owned())
                            .or_insert_with(|| entry.value().clone());
                    }
                }
                if !found {
                    return Err(RepoError::NotFound(format!(
                        "directory {}",
                        dir.as_internal_file_string()
                    )));
                }

                let mut names: Vec<_> = names.into_iter().collect();
                // The sort is stable, so each group keeps the order of names.
                names.sort_by_key(|(_, value)| !is_directory(value));
                let mut entries = vec![];
                for (name, value) in names.into_iter().skip(offset).take(size + 1) {
                    let path = dir
                        .join(jj_lib::repo_path::RepoPathComponent::new(&name).map_err(internal)?);
                    let (kind, file) = match &value {
                        TreeValue::File { id, .. } => (Kind::File, Some(id.to_bytes())),
                        TreeValue::Symlink(_) => (Kind::Symlink, None),
                        TreeValue::Tree(_) | TreeValue::GitSubmodule(_) => (Kind::Directory, None),
                    };
                    let entry = source::TreeEntry {
                        name,
                        path: path.as_internal_file_string().to_owned(),
                        kind: kind.into(),
                        size: 0,
                    };
                    entries.push((entry, file));
                }
                Ok(page(&repo, entries, offset, size))
            })
            .await?;

        // A file's size comes from where its object lies in a segment, so a
        // listing reads no file.
        let mut items = Vec::with_capacity(page.items.len());
        for (mut entry, file) in page.items {
            if let Some(id) = file {
                entry.size = self
                    .file_size(&id)
                    .await
                    .map_or(0, |size| i64::try_from(size).unwrap_or(i64::MAX));
            }
            items.push(entry);
        }
        Ok(Page {
            items,
            next: page.next,
        })
    }
}

impl Repo {
    /// The operations that `HEAD` reaches and `since` doesn't, oldest first,
    /// and the operation at `HEAD`. Without `since`, every operation but the
    /// root.
    ///
    /// # Errors
    ///
    /// Fails when `since` isn't an operation that `HEAD` reaches, or when
    /// the store does.
    pub async fn operations_after(
        &self,
        since: Option<OperationId>,
    ) -> Result<(OperationId, Vec<source::Operation>), RepoError> {
        let root = OperationId::root().to_vec();
        let since = since.map_or_else(|| root.clone(), |id| id.to_vec());
        let head = self.head().await?;
        let mut next = head.to_vec();
        let mut operations = vec![];
        while next != since {
            if next == root {
                return Err(RepoError::NotFound(format!(
                    "operation {} in the log",
                    encode_hex(&since)
                )));
            }
            let (_, operation) = self.read_operation(&next).await?;
            operations.push(source::Operation {
                id: encode_hex(&next),
                time: Some(timestamp(&operation.metadata.time.end)),
                principal_id: operation.metadata.username.clone(),
                description: operation.metadata.description.clone(),
                parent_ids: operation
                    .parents
                    .iter()
                    .map(jj_lib::object_id::ObjectId::hex)
                    .collect(),
            });
            // vcs writes a log without merges.
            next = operation
                .parents
                .first()
                .map_or_else(|| root.clone(), jj_lib::object_id::ObjectId::to_bytes);
        }
        operations.reverse();
        Ok((head, operations))
    }

    /// Each operation after `since`, oldest first, then each new one as it
    /// lands. Every watch of the repository on this replica shares one read
    /// of `HEAD` every `poll`, since another replica may move it, and a push
    /// through this replica wakes them at once. The stream ends after its
    /// first error.
    pub fn watch(
        &self,
        since: Option<OperationId>,
        poll: std::time::Duration,
    ) -> futures::stream::BoxStream<'static, Result<source::Operation, RepoError>> {
        struct Watch {
            repo: Repo,
            since: Option<OperationId>,
            pending: std::collections::VecDeque<source::Operation>,
            moves: watch::Moves,
            first: bool,
            done: bool,
        }
        let watch = Watch {
            repo: self.clone(),
            since,
            pending: std::collections::VecDeque::new(),
            moves: self.head_poll.subscribe(self, poll),
            first: true,
            done: false,
        };
        futures::stream::unfold(watch, move |mut watch| async move {
            loop {
                if watch.done {
                    return None;
                }
                if let Some(operation) = watch.pending.pop_front() {
                    return Some((Ok(operation), watch));
                }
                if !watch.first {
                    watch.moves.next().await;
                }
                watch.first = false;
                match watch.repo.operations_after(watch.since).await {
                    Ok((head, operations)) => {
                        watch.since = Some(head);
                        watch.pending.extend(operations);
                    }
                    Err(err) => {
                        watch.done = true;
                        return Some((Err(err), watch));
                    }
                }
            }
        })
        .boxed()
    }
}

/// The commit that `rev` names at `repo`.
async fn commit(repo: &Arc<ReadonlyRepo>, rev: &str) -> Result<Commit, RepoError> {
    let id = resolve(repo, rev).await?;
    repo.store().get_commit_async(&id).await.map_err(internal)
}

async fn resolve(repo: &Arc<ReadonlyRepo>, rev: &str) -> Result<CommitId, RepoError> {
    let not_found = || RepoError::NotFound(format!("revision {rev}"));
    if rev.is_empty() {
        return TRUNK
            .iter()
            .find_map(|name| {
                repo.view()
                    .get_local_bookmark(RefName::new(name))
                    .as_normal()
                    .cloned()
            })
            .ok_or_else(|| RepoError::NotFound("a trunk bookmark".to_owned()));
    }

    let bookmark = repo.view().get_local_bookmark(RefName::new(rev));
    if bookmark.is_present() {
        return match bookmark.as_normal() {
            Some(id) => Ok(id.clone()),
            None => Err(ambiguous(repo, rev, bookmark.present_adds().cloned().collect()).await),
        };
    }

    let mut candidates = vec![];
    if let Some(prefix) = HexPrefix::try_from_hex(rev) {
        match repo
            .index()
            .resolve_commit_id_prefix(&prefix)
            .await
            .map_err(internal)?
        {
            PrefixResolution::SingleMatch(id) => candidates.push(id),
            PrefixResolution::AmbiguousMatch => {
                return Err(ambiguous(repo, rev, matching(repo, rev).await?).await);
            }
            PrefixResolution::NoMatch => {}
        }
    }
    if let Some(prefix) = HexPrefix::try_from_reverse_hex(rev) {
        match repo
            .resolve_change_id_prefix(&prefix)
            .await
            .map_err(internal)?
        {
            PrefixResolution::SingleMatch(targets) => {
                candidates.extend(targets.visible_with_offsets().map(|(_, id)| id.clone()));
            }
            PrefixResolution::AmbiguousMatch => {
                return Err(ambiguous(repo, rev, matching(repo, rev).await?).await);
            }
            PrefixResolution::NoMatch => {}
        }
    }
    match candidates.len() {
        0 => Err(not_found()),
        1 => Ok(candidates.remove(0)),
        _ => Err(ambiguous(repo, rev, candidates).await),
    }
}

/// The visible commits whose commit ID or change ID starts with `prefix`,
/// at most `CANDIDATES` of them. It reads every visible commit, which only
/// a revision that is already ambiguous pays for.
async fn matching(repo: &ReadonlyRepo, prefix: &str) -> Result<Vec<CommitId>, RepoError> {
    let revset = ResolvedRevsetExpression::visible_heads()
        .ancestors()
        .minus(&ResolvedRevsetExpression::root())
        .evaluate(repo)
        .map_err(internal)?;
    revset
        .commit_change_ids()
        .try_filter(|(commit, change)| {
            std::future::ready(
                commit.hex().starts_with(prefix)
                    || encode_reverse_hex(change.as_bytes()).starts_with(prefix),
            )
        })
        .map_ok(|(commit, _)| commit)
        .take(CANDIDATES)
        .try_collect()
        .await
        .map_err(internal)
}

/// The error for a revision that names each of `ids`, with their changes
/// by commit ID.
async fn ambiguous(repo: &ReadonlyRepo, rev: &str, mut ids: Vec<CommitId>) -> RepoError {
    ids.sort();
    ids.dedup();
    ids.truncate(CANDIDATES);
    let mut candidates = Vec::with_capacity(ids.len());
    for id in &ids {
        match repo.store().get_commit_async(id).await {
            Ok(commit) => candidates.push(change(repo, &commit)),
            Err(err) => return internal(err),
        }
    }
    RepoError::Ambiguous(rev.to_owned(), candidates)
}

/// The tree a change is compared with: its parents', merged without
/// resolving, since resolving writes trees that a read never may.
/// `contents` resolves each path that merges cleanly.
async fn parent_tree(repo: &ReadonlyRepo, commit: &Commit) -> Result<MergedTree, RepoError> {
    commit.parent_tree_no_resolve(repo).await.map_err(internal)
}

fn change(repo: &ReadonlyRepo, commit: &Commit) -> source::Change {
    let bookmarks = repo
        .view()
        .local_bookmarks()
        .filter(|(_, target)| target.present_adds().any(|id| id == commit.id()))
        .map(|(name, _)| name.as_str().to_owned())
        .collect();
    source::Change {
        change_id: encode_reverse_hex(commit.change_id().as_bytes()),
        commit_id: commit.id().hex(),
        description: commit.description().to_owned(),
        author: Some(signature(commit.author())),
        committer: Some(signature(commit.committer())),
        parent_commit_ids: commit
            .parent_ids()
            .iter()
            .map(|id| encode_hex(id.as_bytes()))
            .collect(),
        conflicted: commit.has_conflict(),
        bookmarks,
    }
}

fn signature(signature: &Signature) -> source::Signature {
    source::Signature {
        name: signature.name.clone(),
        email: signature.email.clone(),
        time: Some(timestamp(&signature.timestamp)),
    }
}

fn timestamp(time: &jj_lib::backend::Timestamp) -> Timestamp {
    let millis = time.timestamp.0;
    Timestamp {
        seconds: millis.div_euclid(1000),
        nanos: i32::try_from(millis.rem_euclid(1000) * 1_000_000).unwrap_or(0),
    }
}

fn repo_path(path: &str) -> Result<RepoPathBuf, RepoError> {
    RepoPathBuf::from_internal_string(path.trim_matches('/'))
        .map_err(|err| RepoError::Invalid(format!("path {path}: {err}")))
}

fn is_directory(value: &TreeValue) -> bool {
    matches!(value, TreeValue::Tree(_) | TreeValue::GitSubmodule(_))
}

fn cursor(token: &str) -> Result<Cursor, RepoError> {
    if token.is_empty() {
        return Ok(Cursor {
            operation: None,
            offset: 0,
        });
    }
    let invalid = || RepoError::Invalid(format!("page token {token} is not one this call gave"));
    let (operation, offset) = token.split_once('.').ok_or_else(invalid)?;
    let operation: OperationId = operation.parse().map_err(|_| invalid())?;
    Ok(Cursor {
        operation: Some(operation.to_vec()),
        offset: offset.parse().map_err(|_| invalid())?,
    })
}

/// A token names an operation that the repository doesn't hold, such as one
/// of a repository created again under the same IDs.
fn stale_token() -> RepoError {
    RepoError::Invalid("the page token names an operation the repository lacks".to_owned())
}

/// The token of the page at `offset`, read at the operation of `repo`.
fn token(repo: &ReadonlyRepo, offset: usize) -> String {
    format!("{}.{offset}", repo.operation().id().hex())
}

fn page_size(size: i32) -> usize {
    match usize::try_from(size) {
        Ok(0) | Err(_) => PAGE_SIZE,
        Ok(size) => size.min(MAX_PAGE_SIZE),
    }
}

/// The page of `items`, which holds one more than `size` when another page
/// follows.
fn page<T>(repo: &ReadonlyRepo, mut items: Vec<T>, offset: usize, size: usize) -> Page<T> {
    let next = if items.len() > size {
        items.truncate(size);
        token(repo, offset + size)
    } else {
        String::new()
    };
    Page { items, next }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_names_the_operation_and_the_offset() {
        let operation = "07".repeat(32);

        let cursor = cursor(&format!("{operation}.12")).unwrap();

        assert_eq!(cursor.operation, Some(vec![7; 32]));
        assert_eq!(cursor.offset, 12);
        assert!(super::cursor("12").is_err());
        assert!(super::cursor(&format!("{operation}.x")).is_err());
    }
}
