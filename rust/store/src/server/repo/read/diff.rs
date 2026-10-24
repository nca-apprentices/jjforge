//! Diffs between two trees, file by file: which paths changed, which moved,
//! and the hunks of each. A page lists its paths without reading a file, and
//! reads each file only when its turn in the stream comes, so memory holds
//! one file's sides at a time.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::Hash as _;
use std::hash::Hasher as _;
use std::sync::Arc;

use futures::StreamExt as _;
use futures::TryStreamExt as _;
use jj_lib::backend::TreeValue;
use jj_lib::diff::ContentDiff;
use jj_lib::diff::DiffHunkKind;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merge::Merge;
use jj_lib::merged_tree::MergedTree;
use jj_lib::object_id::ObjectId as _;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo_path::RepoPathBuf;
use jjforge_proto::source::v1 as source;
use jjforge_proto::source::v1::diff_response::Status;
use jjforge_proto::source::v1::hunk;
use prost::Message as _;

use super::super::Repo;
use super::super::RepoError;
use super::super::internal;
use super::Page;
use super::commit;
use super::cursor;
use super::file::BINARY_PROBE;
use super::file::is_binary;
use super::page;
use super::page_size;
use super::parent_tree;
use super::token;

/// The lines of context around each change in a hunk, as `git diff` shows.
const CONTEXT: usize = 3;

/// The most bytes one file's diff takes before vcs drops its hunks, well
/// below gRPC's default limit of 4 MiB per message.
const DIFF_LIMIT: usize = 1 << 20;

/// The largest side of a file that vcs compares. A larger file's diff is
/// truncated without reading it, and a client reads both sides instead.
const DIFF_INPUT: u64 = 4 << 20;

/// The most bytes of files one page of changed files holds, well below
/// gRPC's default limit of 4 MiB per message.
const FILES_SIZE: usize = 3 << 20;

/// The most added or deleted files a diff looks for renames among, as git's
/// `diff.renameLimit` bounds its search.
const RENAME_FILES: usize = 1000;

/// The most added and deleted files, together, that vcs compares by content
/// to find a rename with edits, and the largest of them.
const SIMILAR_FILES: usize = 100;
const SIMILAR_SIZE: u64 = 1 << 20;

/// One page of a diff. `files` reads each file when the stream reaches it.
pub struct DiffPage {
    /// One `DiffResponse` per file, by path.
    pub files: futures::stream::BoxStream<'static, Result<source::DiffResponse, RepoError>>,
    /// The number of files `files` yields.
    pub len: usize,
    /// The token of the next page, empty on the last.
    pub next: String,
}

/// A path whose sides differ once trivial conflicts are resolved. A renamed
/// path names its old path, and `before` is the old path's value.
pub(super) struct ChangedPath {
    pub(super) path: RepoPathBuf,
    pub(super) old_path: Option<RepoPathBuf>,
    pub(super) status: Status,
    pub(super) before: Merge<Option<TreeValue>>,
    pub(super) after: Merge<Option<TreeValue>>,
}

/// One side of a file, as a diff reads it.
enum Side {
    Absent,
    /// A resolved file, by the ID of its object.
    File(Vec<u8>),
    /// A conflict, a symbolic link, or a directory, which shows no hunks.
    Other,
}

fn side(value: &Merge<Option<TreeValue>>) -> Side {
    match value.as_resolved() {
        Some(None) => Side::Absent,
        Some(Some(TreeValue::File { id, .. })) => Side::File(id.to_bytes()),
        _ => Side::Other,
    }
}

/// A file of a page, before its sides are read.
struct Pending {
    path: String,
    old_path: String,
    status: Status,
    before: Side,
    after: Side,
    conflicted: bool,
}

/// What a diff knows of one side once it looked at the file.
enum Content {
    Bytes(Vec<u8>),
    /// Larger than `DIFF_INPUT`, with whether its start is binary.
    Large(bool),
    None,
}

impl Repo {
    /// One page of the diff, one file at a time, by path: of `rev` against
    /// its parents when `rev` is set, otherwise from `from` to `to`. A
    /// conflicted side shows no hunks, and a moved file shows as a rename.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, or on a page token this call didn't
    /// give. A file that the store can't read fails its item in `files`.
    pub async fn diff(
        &self,
        rev: &str,
        from: &str,
        to: &str,
        token: &str,
        size: i32,
    ) -> Result<DiffPage, RepoError> {
        let (rev, from, to) = (rev.to_owned(), from.to_owned(), to.to_owned());
        let (cursor, size) = (cursor(token)?, page_size(size));
        let offset = cursor.offset;
        let this = self.clone();
        let Page {
            items: pending,
            next,
        } = self
            .at(cursor.operation, move |repo| async move {
                let (before, after) = trees(&repo, &rev, &from, &to).await?;
                let paths: Vec<_> = changes(&this, &repo, &before, &after)
                    .await?
                    .skip(offset)
                    .take(size + 1)
                    .try_collect()
                    .await?;
                let pending = paths
                    .into_iter()
                    .map(|changed| Pending {
                        path: changed.path.as_internal_file_string().to_owned(),
                        old_path: changed
                            .old_path
                            .map(|path| path.as_internal_file_string().to_owned())
                            .unwrap_or_default(),
                        status: changed.status,
                        before: side(&changed.before),
                        after: side(&changed.after),
                        conflicted: !changed.before.is_resolved() || !changed.after.is_resolved(),
                    })
                    .collect();
                Ok(page(&repo, pending, offset, size))
            })
            .await?;
        let len = pending.len();
        let this = self.clone();
        let files = futures::stream::iter(pending)
            .then(move |pending| {
                let this = this.clone();
                async move { this.file_diff(pending).await }
            })
            .boxed();
        Ok(DiffPage { files, len, next })
    }

    /// One page of the paths that the diff of [`Repo::diff`] shows, by path,
    /// without reading a file.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, or on a page token this call didn't
    /// give.
    pub async fn changed_files(
        &self,
        rev: &str,
        from: &str,
        to: &str,
        token: &str,
        size: i32,
    ) -> Result<Page<source::ChangedFile>, RepoError> {
        let (rev, from, to) = (rev.to_owned(), from.to_owned(), to.to_owned());
        let (cursor, size) = (cursor(token)?, page_size(size));
        let offset = cursor.offset;
        let this = self.clone();
        self.at(cursor.operation, move |repo| async move {
            let (before, after) = trees(&repo, &rev, &from, &to).await?;
            files_page(&this, &repo, &before, &after, offset, size).await
        })
        .await
    }

    async fn file_diff(&self, pending: Pending) -> Result<source::DiffResponse, RepoError> {
        let old = self.content(&pending.before).await?;
        let new = self.content(&pending.after).await?;
        let (binary, hunks, truncated) = match (old, new) {
            (Content::Bytes(old), Content::Bytes(new)) if is_binary(&old) || is_binary(&new) => {
                (true, vec![], false)
            }
            (Content::Bytes(old), Content::Bytes(new)) => (false, hunks(&old, &new), false),
            (Content::Large(binary), Content::Bytes(other))
            | (Content::Bytes(other), Content::Large(binary)) => {
                (binary || is_binary(&other), vec![], true)
            }
            (Content::Large(old), Content::Large(new)) => (old || new, vec![], true),
            _ => (false, vec![], false),
        };
        let mut file = source::DiffResponse {
            path: pending.path,
            old_path: pending.old_path,
            status: pending.status.into(),
            binary,
            hunks,
            truncated,
            conflicted: pending.conflicted,
            next_page_token: String::new(),
        };
        if file.encoded_len() > DIFF_LIMIT {
            file.hunks.clear();
            file.truncated = true;
        }
        Ok(file)
    }

    /// A side's bytes, or only whether it is binary when it is too large to
    /// compare.
    async fn content(&self, side: &Side) -> Result<Content, RepoError> {
        let id = match side {
            Side::Absent => return Ok(Content::Bytes(vec![])),
            Side::Other => return Ok(Content::None),
            Side::File(id) => id,
        };
        let window = self.window(id).await.ok_or_else(|| {
            RepoError::Internal(
                format!("file {} is in no segment", jj_lib::hex_util::encode_hex(id)).into(),
            )
        })?;
        if window.size() > DIFF_INPUT {
            let probe = self
                .read_range(&window, 0..BINARY_PROBE.min(window.size()))
                .await?;
            return Ok(Content::Large(is_binary(&probe)));
        }
        self.read_whole(&window).await.map(Content::Bytes)
    }
}

/// The trees a diff compares: of `rev` against its parents when `rev` is
/// set, otherwise of `from` and `to`.
async fn trees(
    repo: &Arc<ReadonlyRepo>,
    rev: &str,
    from: &str,
    to: &str,
) -> Result<(MergedTree, MergedTree), RepoError> {
    if rev.is_empty() {
        let from = commit(repo, from).await?.tree();
        let to = commit(repo, to).await?.tree();
        return Ok((from, to));
    }
    let commit = commit(repo, rev).await?;
    Ok((parent_tree(repo, &commit).await?, commit.tree()))
}

/// The page of changed files at `offset`, at most `size` of them and at most
/// `FILES_SIZE` bytes, since a path's length has no limit of its own. A
/// file larger than `FILES_SIZE` alone still makes a page.
pub(super) async fn files_page(
    this: &Repo,
    repo: &ReadonlyRepo,
    before: &MergedTree,
    after: &MergedTree,
    offset: usize,
    size: usize,
) -> Result<Page<source::ChangedFile>, RepoError> {
    let changes = changes(this, repo, before, after).await?.skip(offset);
    let mut changes = std::pin::pin!(changes);
    let (mut files, mut bytes) = (vec![], 0);
    while let Some(file) = changes.try_next().await? {
        let file = source::ChangedFile {
            path: file.path.as_internal_file_string().to_owned(),
            old_path: file
                .old_path
                .map(|path| path.as_internal_file_string().to_owned())
                .unwrap_or_default(),
            status: file.status.into(),
            conflicted: !file.after.is_resolved(),
        };
        // Each file takes a tag and a length besides its own bytes.
        let len = file.encoded_len();
        bytes += 1 + prost::length_delimiter_len(len) + len;
        if files.len() == size || (bytes > FILES_SIZE && !files.is_empty()) {
            let next = token(repo, offset + files.len());
            return Ok(Page { items: files, next });
        }
        files.push(file);
    }
    Ok(Page {
        items: files,
        next: String::new(),
    })
}

/// The paths that differ between `before` and `after`, by path, with each
/// rename in place of its deletion and its addition. It reads only the files
/// that rename detection compares.
pub(super) async fn changes(
    this: &Repo,
    repo: &ReadonlyRepo,
    before: &MergedTree,
    after: &MergedTree,
) -> Result<impl futures::Stream<Item = Result<ChangedPath, RepoError>>, RepoError> {
    let renames = renames(this, repo, before, after).await?;
    Ok(changed(before, after).try_filter_map(move |mut path| {
        let path = match path.status {
            Status::Deleted if renames.old.contains(&path.path) => None,
            Status::Added => {
                if let Some((old, value)) = renames.new.get(&path.path) {
                    path.status = Status::Renamed;
                    path.old_path = Some(old.clone());
                    path.before = value.clone();
                }
                Some(path)
            }
            _ => Some(path),
        };
        std::future::ready(Ok(path))
    }))
}

/// The paths that differ between `before` and `after`, by path, without
/// reading a file.
fn changed(
    before: &MergedTree,
    after: &MergedTree,
) -> impl futures::Stream<Item = Result<ChangedPath, RepoError>> {
    before
        .diff_stream(after, &EverythingMatcher)
        .map(|entry| {
            let values = entry.values.map_err(internal)?;
            let before = resolved(&values.before);
            let after = resolved(&values.after);
            let status = match (before.is_absent(), after.is_absent()) {
                (true, _) => Status::Added,
                (_, true) => Status::Deleted,
                _ => Status::Modified,
            };
            Ok((before != after).then_some(ChangedPath {
                path: entry.path,
                old_path: None,
                status,
                before,
                after,
            }))
        })
        .try_filter_map(|path| std::future::ready(Ok(path)))
}

/// The renames of a diff: each new path with its old path and that path's
/// value, and the old paths, which the diff leaves out.
#[derive(Default)]
struct Renames {
    new: HashMap<RepoPathBuf, (RepoPathBuf, Merge<Option<TreeValue>>)>,
    old: HashSet<RepoPathBuf>,
}

/// An added or a deleted file that may be one side of a rename.
struct Candidate {
    path: RepoPathBuf,
    id: Vec<u8>,
    value: Merge<Option<TreeValue>>,
}

/// Pairs deleted files with added ones, as git does: first those with the
/// same content, then, among few enough small text files, those whose lines
/// are at least half the same. Empty files never pair, and a diff with more
/// than `RENAME_FILES` added or deleted files looks for no rename.
async fn renames(
    this: &Repo,
    repo: &ReadonlyRepo,
    before: &MergedTree,
    after: &MergedTree,
) -> Result<Renames, RepoError> {
    let mut added = vec![];
    let mut deleted = vec![];
    let mut paths = std::pin::pin!(changed(before, after));
    while let Some(path) = paths.try_next().await? {
        let (list, value) = match path.status {
            Status::Added => (&mut added, path.after),
            Status::Deleted => (&mut deleted, path.before),
            _ => continue,
        };
        let Side::File(id) = side(&value) else {
            continue;
        };
        if this.file_size(&id).await == Some(0) {
            continue;
        }
        list.push(Candidate {
            path: path.path,
            id,
            value,
        });
        if added.len() > RENAME_FILES || deleted.len() > RENAME_FILES {
            return Ok(Renames::default());
        }
    }
    if added.is_empty() || deleted.is_empty() {
        return Ok(Renames::default());
    }

    let mut pairs = vec![];
    let mut by_id: HashMap<&[u8], VecDeque<usize>> = HashMap::new();
    for (index, file) in deleted.iter().enumerate() {
        by_id.entry(&file.id).or_default().push_back(index);
    }
    let mut unpaired = vec![];
    for (index, file) in added.iter().enumerate() {
        match by_id
            .get_mut(file.id.as_slice())
            .and_then(VecDeque::pop_front)
        {
            Some(old) => pairs.push((index, old)),
            None => unpaired.push(index),
        }
    }
    let paired: HashSet<usize> = pairs.iter().map(|(_, old)| *old).collect();
    let left: Vec<usize> = (0..deleted.len()).filter(|i| !paired.contains(i)).collect();
    if !unpaired.is_empty() && !left.is_empty() && unpaired.len() + left.len() <= SIMILAR_FILES {
        pairs.extend(similar(this, repo, &added, &unpaired, &deleted, &left).await?);
    }

    let mut renames = Renames::default();
    for (new, old) in pairs {
        let old = &deleted[old];
        renames.old.insert(old.path.clone());
        renames.new.insert(
            added[new].path.clone(),
            (old.path.clone(), old.value.clone()),
        );
    }
    Ok(renames)
}

/// The pairs of added and deleted files whose lines are at least half the
/// same, best first, each file in one pair at most.
async fn similar(
    this: &Repo,
    repo: &ReadonlyRepo,
    added: &[Candidate],
    new: &[usize],
    deleted: &[Candidate],
    old: &[usize],
) -> Result<Vec<(usize, usize)>, RepoError> {
    let mut lines = HashMap::new();
    for (files, indexes) in [(added, new), (deleted, old)] {
        for &index in indexes {
            let file = &files[index];
            if this
                .file_size(&file.id)
                .await
                .is_none_or(|size| size > SIMILAR_SIZE)
            {
                continue;
            }
            let id = jj_lib::backend::FileId::new(file.id.clone());
            let bytes = super::file::read_all(repo, &file.path, &id).await?;
            if !is_binary(&bytes) {
                lines.insert(file.id.clone(), signature(&bytes));
            }
        }
    }

    let mut scores = vec![];
    for &new_index in new {
        for &old_index in old {
            let (Some(a), Some(b)) = (
                lines.get(&added[new_index].id),
                lines.get(&deleted[old_index].id),
            ) else {
                continue;
            };
            let (common, total) = (common_bytes(a, b), weight(a) + weight(b));
            // At least half of the bytes are the same: 2 * common >= total / 2.
            if total > 0 && 4 * common >= total {
                scores.push((common * 1000 / total, new_index, old_index));
            }
        }
    }
    scores.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (mut used_new, mut used_old) = (HashSet::new(), HashSet::new());
    Ok(scores
        .into_iter()
        .filter(|(_, new, old)| used_new.insert(*new) && used_old.insert(*old))
        .map(|(_, new, old)| (new, old))
        .collect())
}

/// Each line of `bytes` as its hash and its length, sorted by hash.
fn signature(bytes: &[u8]) -> Vec<(u64, u64)> {
    let mut lines: Vec<_> = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            line.hash(&mut hasher);
            (hasher.finish(), line.len() as u64)
        })
        .collect();
    lines.sort_unstable();
    lines
}

fn weight(lines: &[(u64, u64)]) -> u64 {
    lines.iter().map(|(_, len)| len).sum()
}

/// The bytes of the lines that two signatures share, each line as often as
/// both hold it.
fn common_bytes(a: &[(u64, u64)], b: &[(u64, u64)]) -> u64 {
    let (mut i, mut j, mut common) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += a[i].1;
                i += 1;
                j += 1;
            }
        }
    }
    common
}

/// A side of a diff with its trivial conflicts resolved, as a clean merge
/// of the parents of a merge commit leaves them.
fn resolved(value: &Merge<Option<TreeValue>>) -> Merge<Option<TreeValue>> {
    value
        .resolve_trivial(jj_lib::merge::SameChange::Accept)
        .map_or_else(
            || value.clone(),
            |resolved| Merge::resolved(resolved.clone()),
        )
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
        // A side without lines starts at the line before the hunk, as git
        // shows an added or a deleted file as `-0,0` or `+0,0`.
        let start = |start: i32, lines: i32| if lines == 0 { start - 1 } else { start };
        hunks.push(source::Hunk {
            old_start: start(old_start, old_line - old_start),
            old_lines: old_line - old_start,
            new_start: start(new_start, new_line - new_start),
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
    fn lines_moved_with_an_edit_are_mostly_common() {
        let old = signature(b"one\ntwo\nthree\nfour\n");
        let new = signature(b"one\ntwo\nthree\nFOUR\n");

        let common = common_bytes(&old, &new);

        assert_eq!(common, 14);
        assert!(4 * common >= weight(&old) + weight(&new));
    }

    #[test]
    fn an_added_file_starts_its_old_side_at_zero() {
        let hunks = hunks(b"", b"one\n");

        assert_eq!((hunks[0].old_start, hunks[0].old_lines), (0, 0));
        assert_eq!((hunks[0].new_start, hunks[0].new_lines), (1, 1));
    }
}
