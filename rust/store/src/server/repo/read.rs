//! The reads that the server makes through `source/v1`: history, trees,
//! files, and diffs, each at `HEAD`, answered as the messages of
//! shared/proto/source/v1/.
//!
//! A revision is a bookmark, a prefix of a commit ID in hex, or a prefix of
//! a change ID in jj's reverse hex. An empty revision is the trunk bookmark.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use futures::AsyncReadExt as _;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use jj_lib::backend::CommitId;
use jj_lib::backend::Signature;
use jj_lib::backend::TreeValue;
use jj_lib::commit::Commit;
use jj_lib::diff::ContentDiff;
use jj_lib::diff::DiffHunkKind;
use jj_lib::hex_util::encode_hex;
use jj_lib::hex_util::encode_reverse_hex;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merge::Merge;
use jj_lib::merged_tree::MergedTree;
use jj_lib::object_id::HexPrefix;
use jj_lib::object_id::ObjectId as _;
use jj_lib::object_id::PrefixResolution;
use jj_lib::op_store::OperationId as JjOperationId;
use jj_lib::ref_name::RefName;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo as _;
use jj_lib::repo_path::RepoPath;
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::revset::ResolvedRevsetExpression;
use jjforge_proto::prost_types::Timestamp;
use jjforge_proto::source::v1 as source;
use jjforge_proto::source::v1::diff_response::Status;
use jjforge_proto::source::v1::hunk;
use jjforge_proto::source::v1::tree_entry::Kind;

use super::Repo;
use super::RepoError;
use super::internal;
use crate::ids::OperationId;

/// The bookmarks that name trunk, in the order `trunk()` tries them.
const TRUNK: [&str; 3] = ["main", "master", "trunk"];

/// The most changes one log answers, since `Log` has no pages.
const LOG_LIMIT: usize = 1000;

/// The page size of a list that names none, and the most it may name.
const PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 1000;

/// The lines of context around each change in a hunk, as `git diff` shows.
const CONTEXT: usize = 3;

/// The bytes that a file is checked for NUL in, as git does, to call it
/// binary.
const BINARY_PROBE: usize = 8000;

/// One page of a list.
pub struct Page<T> {
    /// The page's items, in the list's order.
    pub items: Vec<T>,
    /// The token of the next page, empty on the last.
    pub next: String,
}

/// A file's bytes in the range a read asked for.
pub struct FileRead {
    /// The size of the whole file.
    pub size: u64,
    /// Whether the file holds a NUL byte near its start, as git decides.
    pub binary: bool,
    /// The bytes of the range.
    pub bytes: Vec<u8>,
}

impl Repo {
    /// Runs `read` on jj-lib's repository at `HEAD`. jj-lib's futures aren't
    /// Send, so it runs on a thread of its own, as a publish does.
    async fn at_head<T, F, Fut>(&self, read: F) -> Result<T, RepoError>
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
                        repo.advance(head.as_bytes()).await?;
                        let loader = repo.loader(HashMap::new())?;
                        let operation = loader
                            .load_operation(&JjOperationId::new(head.to_vec()))
                            .await
                            .map_err(internal)?;
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

    /// The change that `rev` names.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does.
    pub async fn change(&self, rev: &str) -> Result<source::Change, RepoError> {
        let rev = rev.to_owned();
        self.at_head(move |repo| async move {
            let commit = commit(&repo, &rev).await?;
            Ok(change(&repo, &commit))
        })
        .await
    }

    /// The changes that `rev` reaches, newest first, at most `LOG_LIMIT`.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does.
    pub async fn log(&self, rev: &str) -> Result<Vec<source::Change>, RepoError> {
        let rev = rev.to_owned();
        self.at_head(move |repo| async move {
            let start = commit(&repo, &rev).await?;
            let revset = ResolvedRevsetExpression::commit(start.id().clone())
                .ancestors()
                .evaluate(repo.as_ref())
                .map_err(internal)?;
            let ids: Vec<CommitId> = revset
                .stream()
                .take(LOG_LIMIT)
                .try_collect()
                .await
                .map_err(internal)?;
            let mut changes = Vec::with_capacity(ids.len());
            for id in ids {
                let commit = repo.store().get_commit_async(&id).await.map_err(internal)?;
                changes.push(change(&repo, &commit));
            }
            Ok(changes)
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
        let (rev, offset, size) = (rev.to_owned(), offset(token)?, page_size(size));
        self.at_head(move |repo| async move {
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
            Ok(page(versions, offset, size))
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
        let (offset, size) = (offset(token)?, page_size(size));
        self.at_head(move |repo| async move {
            let operations: Vec<_> =
                jj_lib::op_walk::walk_ancestors(std::slice::from_ref(repo.operation()))
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
            Ok(page(operations, offset, size))
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
        let (offset, size) = (offset(token)?, page_size(size));
        self.at_head(move |repo| async move {
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
            Ok(page(bookmarks, offset, size))
        })
        .await
    }

    /// The entries of the directory at `path` in the tree of `rev`, by name.
    /// A conflicted directory lists the names of every side.
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
        let (rev, offset, size) = (rev.to_owned(), offset(token)?, page_size(size));
        let dir = repo_path(path)?;
        self.at_head(move |repo| async move {
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

            let mut entries = vec![];
            for (name, value) in names.into_iter().skip(offset).take(size + 1) {
                let path =
                    dir.join(jj_lib::repo_path::RepoPathComponent::new(&name).map_err(internal)?);
                let (kind, size) = match &value {
                    TreeValue::File { id, .. } => {
                        (Kind::File, read_all(&repo, &path, id).await?.len())
                    }
                    TreeValue::Symlink(_) => (Kind::Symlink, 0),
                    TreeValue::Tree(_) | TreeValue::GitSubmodule(_) => (Kind::Directory, 0),
                };
                entries.push(source::TreeEntry {
                    name,
                    path: path.as_internal_file_string().to_owned(),
                    kind: kind.into(),
                    size: i64::try_from(size).unwrap_or(i64::MAX),
                });
            }
            Ok(page(entries, offset, size))
        })
        .await
    }

    /// The bytes of the file at `path` in the tree of `rev`, from `offset`,
    /// `length` of them or all to the end when `length` is 0.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, or when `path` is no file or a
    /// conflicted one.
    pub async fn file(
        &self,
        rev: &str,
        path: &str,
        offset: u64,
        length: u64,
    ) -> Result<FileRead, RepoError> {
        let rev = rev.to_owned();
        let path = repo_path(path)?;
        self.at_head(move |repo| async move {
            let commit = commit(&repo, &rev).await?;
            let value = commit.tree().path_value(&path).await.map_err(internal)?;
            let Some(Some(TreeValue::File { id, .. })) = value.as_resolved() else {
                return Err(RepoError::NotFound(format!(
                    "file {}",
                    path.as_internal_file_string()
                )));
            };
            let bytes = read_all(&repo, &path, id).await?;
            let size = bytes.len();
            let start = usize::try_from(offset).unwrap_or(usize::MAX).min(size);
            let end = match usize::try_from(length) {
                Ok(0) | Err(_) => size,
                Ok(length) => start.saturating_add(length).min(size),
            };
            Ok(FileRead {
                size: size as u64,
                binary: is_binary(&bytes),
                bytes: bytes[start..end].to_vec(),
            })
        })
        .await
    }

    /// One `DiffResponse` per changed file: of `rev` against its parents
    /// when `rev` is set, otherwise from `from` to `to`. A conflicted side
    /// shows no hunks.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does.
    pub async fn diff(
        &self,
        rev: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<source::DiffResponse>, RepoError> {
        let (rev, from, to) = (rev.to_owned(), from.to_owned(), to.to_owned());
        self.at_head(move |repo| async move {
            let (before, after) = if rev.is_empty() {
                let from = commit(&repo, &from).await?.tree();
                let to = commit(&repo, &to).await?.tree();
                (from, to)
            } else {
                let commit = commit(&repo, &rev).await?;
                let parent = commit.parent_tree(repo.as_ref()).await.map_err(internal)?;
                (parent, commit.tree())
            };
            diff(&repo, &before, &after).await
        })
        .await
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
    /// lands, read from `HEAD` every `poll`, since another replica may move
    /// it. The stream ends after its first error.
    pub fn watch(
        &self,
        since: Option<OperationId>,
        poll: std::time::Duration,
    ) -> futures::stream::BoxStream<'static, Result<source::Operation, RepoError>> {
        struct Watch {
            repo: Repo,
            since: Option<OperationId>,
            pending: std::collections::VecDeque<source::Operation>,
            first: bool,
            done: bool,
        }
        let watch = Watch {
            repo: self.clone(),
            since,
            pending: std::collections::VecDeque::new(),
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
                    tokio::time::sleep(poll).await;
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
            None => Err(RepoError::Ambiguous(
                rev.to_owned(),
                bookmark
                    .present_adds()
                    .map(jj_lib::object_id::ObjectId::hex)
                    .collect(),
            )),
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
                return Err(RepoError::Ambiguous(rev.to_owned(), vec![]));
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
                return Err(RepoError::Ambiguous(rev.to_owned(), vec![]));
            }
            PrefixResolution::NoMatch => {}
        }
    }
    match candidates.len() {
        0 => Err(not_found()),
        1 => Ok(candidates.remove(0)),
        _ => Err(RepoError::Ambiguous(
            rev.to_owned(),
            candidates
                .iter()
                .map(jj_lib::object_id::ObjectId::hex)
                .collect(),
        )),
    }
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

async fn read_all(
    repo: &ReadonlyRepo,
    path: &RepoPath,
    id: &jj_lib::backend::FileId,
) -> Result<Vec<u8>, RepoError> {
    let mut reader = repo.store().read_file(path, id).await.map_err(internal)?;
    let mut bytes = vec![];
    reader.read_to_end(&mut bytes).await.map_err(internal)?;
    Ok(bytes)
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_PROBE)].contains(&0)
}

/// A page token is the offset of the page's first item, in decimal.
fn offset(token: &str) -> Result<usize, RepoError> {
    if token.is_empty() {
        return Ok(0);
    }
    token
        .parse()
        .map_err(|_| RepoError::Invalid(format!("page token {token} is not one this call gave")))
}

fn page_size(size: i32) -> usize {
    match usize::try_from(size) {
        Ok(0) | Err(_) => PAGE_SIZE,
        Ok(size) => size.min(MAX_PAGE_SIZE),
    }
}

/// The page of `items`, which holds one more than `size` when another page
/// follows.
fn page<T>(mut items: Vec<T>, offset: usize, size: usize) -> Page<T> {
    let next = if items.len() > size {
        items.truncate(size);
        (offset + size).to_string()
    } else {
        String::new()
    };
    Page { items, next }
}

async fn diff(
    repo: &ReadonlyRepo,
    before: &MergedTree,
    after: &MergedTree,
) -> Result<Vec<source::DiffResponse>, RepoError> {
    let entries: Vec<_> = before
        .diff_stream(after, &EverythingMatcher)
        .collect()
        .await;
    let mut files = vec![];
    for entry in entries {
        let values = entry.values.map_err(internal)?;
        let status = match (values.before.is_absent(), values.after.is_absent()) {
            (true, _) => Status::Added,
            (_, true) => Status::Deleted,
            _ => Status::Modified,
        };
        let old = contents(repo, &entry.path, &values.before).await?;
        let new = contents(repo, &entry.path, &values.after).await?;
        let (binary, hunks) = match (old, new) {
            (Some(old), Some(new)) if is_binary(&old) || is_binary(&new) => (true, vec![]),
            (Some(old), Some(new)) => (false, hunks(&old, &new)),
            _ => (false, vec![]),
        };
        files.push(source::DiffResponse {
            path: entry.path.as_internal_file_string().to_owned(),
            old_path: String::new(),
            status: status.into(),
            binary,
            hunks,
        });
    }
    Ok(files)
}

/// The bytes of a side of a diff: empty when absent, `None` when it is no
/// resolved file, such as a conflict or a symbolic link.
async fn contents(
    repo: &ReadonlyRepo,
    path: &RepoPath,
    value: &Merge<Option<TreeValue>>,
) -> Result<Option<Vec<u8>>, RepoError> {
    match value.as_resolved() {
        Some(None) => Ok(Some(vec![])),
        Some(Some(TreeValue::File { id, .. })) => read_all(repo, path, id).await.map(Some),
        _ => Ok(None),
    }
}

/// One line of a diff, before hunks group them.
struct DiffLine {
    kind: hunk::line::Kind,
    text: String,
}

/// The unified hunks between `old` and `new`, with `CONTEXT` lines around
/// each change. Changes closer than twice `CONTEXT` share a hunk.
fn hunks(old: &[u8], new: &[u8]) -> Vec<source::Hunk> {
    let mut lines = vec![];
    for hunk in ContentDiff::by_line([old, new]).hunks() {
        let text = |bytes: &[u8]| -> Vec<String> {
            bytes
                .split_inclusive(|byte| *byte == b'\n')
                .map(|line| String::from_utf8_lossy(line).into_owned())
                .collect()
        };
        match hunk.kind {
            DiffHunkKind::Matching => {
                lines.extend(text(hunk.contents[0]).into_iter().map(|text| DiffLine {
                    kind: hunk::line::Kind::Context,
                    text,
                }));
            }
            DiffHunkKind::Different => {
                lines.extend(text(hunk.contents[0]).into_iter().map(|text| DiffLine {
                    kind: hunk::line::Kind::Removed,
                    text,
                }));
                lines.extend(text(hunk.contents[1]).into_iter().map(|text| DiffLine {
                    kind: hunk::line::Kind::Added,
                    text,
                }));
            }
        }
    }

    let changed: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.kind != hunk::line::Kind::Context)
        .map(|(i, _)| i)
        .collect();
    let mut ranges: Vec<(usize, usize)> = vec![];
    for i in changed {
        let (start, end) = (
            i.saturating_sub(CONTEXT),
            (i + CONTEXT + 1).min(lines.len()),
        );
        match ranges.last_mut() {
            Some(last) if start <= last.1 => last.1 = end,
            _ => ranges.push((start, end)),
        }
    }

    // Line numbers count from 1, and each kind of line moves one side.
    let (mut old_line, mut new_line, mut at) = (1, 1, 0);
    let mut hunks = vec![];
    for (start, end) in ranges {
        for line in &lines[at..start] {
            step(line.kind, &mut old_line, &mut new_line);
        }
        let (old_start, new_start) = (old_line, new_line);
        let mut out = vec![];
        for line in &lines[start..end] {
            step(line.kind, &mut old_line, &mut new_line);
            out.push(source::hunk::Line {
                kind: line.kind.into(),
                text: line.text.clone(),
            });
        }
        at = end;
        hunks.push(source::Hunk {
            old_start,
            old_lines: old_line - old_start,
            new_start,
            new_lines: new_line - new_start,
            lines: out,
        });
    }
    hunks
}

fn step(kind: hunk::line::Kind, old_line: &mut i32, new_line: &mut i32) {
    match kind {
        hunk::line::Kind::Removed => *old_line += 1,
        hunk::line::Kind::Added => *new_line += 1,
        _ => {
            *old_line += 1;
            *new_line += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(hunk: &source::Hunk) -> String {
        hunk.lines
            .iter()
            .map(|line| match line.kind() {
                hunk::line::Kind::Added => '+',
                hunk::line::Kind::Removed => '-',
                _ => ' ',
            })
            .collect()
    }

    #[test]
    fn a_change_shows_with_three_lines_of_context() {
        let old = b"1\n2\n3\n4\n5\n6\n7\n8\n9\n";
        let new = b"1\n2\n3\n4\nfive\n6\n7\n8\n9\n";

        let hunks = hunks(old, new);

        assert_eq!(hunks.len(), 1);
        assert_eq!(kinds(&hunks[0]), "   -+   ");
        assert_eq!((hunks[0].old_start, hunks[0].old_lines), (2, 7));
        assert_eq!((hunks[0].new_start, hunks[0].new_lines), (2, 7));
    }

    #[test]
    fn distant_changes_get_hunks_of_their_own() {
        let old: Vec<u8> = (1..=20)
            .flat_map(|n| format!("{n}\n").into_bytes())
            .collect();
        let new: Vec<u8> = (1..=20)
            .flat_map(|n| match n {
                2 => "two\n".to_owned().into_bytes(),
                19 => "nineteen\n".to_owned().into_bytes(),
                n => format!("{n}\n").into_bytes(),
            })
            .collect();

        let hunks = hunks(&old, &new);

        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[1].old_start, 16);
    }

    #[test]
    fn a_nul_byte_marks_a_file_binary() {
        assert!(is_binary(b"PNG\0"));
        assert!(!is_binary(b"text\n"));
    }

    #[test]
    fn a_page_holds_its_size_and_names_the_next() {
        let page = page(vec![1, 2, 3], 10, 2);

        assert_eq!(page.items, [1, 2]);
        assert_eq!(page.next, "12");
    }
}
