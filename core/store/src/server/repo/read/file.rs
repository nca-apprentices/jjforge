//! Reads of a file's bytes. A stored file is one object in a segment, so vcs
//! reads any range of it with one ranged read of that segment, and a read of
//! part of a large file costs only that part.

use std::ops::Range;

use futures::AsyncReadExt as _;
use futures::StreamExt as _;
use jj_lib::backend::FileId;
use jj_lib::backend::TreeValue;
use jj_lib::conflicts::ConflictMarkerStyle;
use jj_lib::conflicts::ConflictMaterializeOptions;
use jj_lib::conflicts::materialize_merge_result_to_bytes;
use jj_lib::conflicts::try_materialize_file_conflict_value;
use jj_lib::object_id::ObjectId as _;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo as _;
use jj_lib::repo_path::RepoPath;

use super::super::Repo;
use super::super::RepoError;
use super::super::internal;
use super::commit;
use super::repo_path;
use crate::ids;

/// The bytes that a file is checked for NUL in, as git does, to call it
/// binary.
pub(super) const BINARY_PROBE: u64 = 8000;

/// The most bytes one read of a segment asks for, so a long range streams
/// in parts.
const READ_CHUNK: u64 = 1 << 20;

/// A file's size and the range of it that a read asked for.
pub struct FileRead {
    /// The size of the whole file.
    pub size: u64,
    /// Whether the file holds a NUL byte near its start, as git decides.
    pub binary: bool,
    /// Whether the file is conflicted, so its bytes hold jj's conflict
    /// markers around each side.
    pub conflicted: bool,
    /// The bytes of the range, in order, in parts of at most 1 MiB.
    pub chunks: futures::stream::BoxStream<'static, Result<Vec<u8>, RepoError>>,
}

/// Where a stored file's object lies: the segment, the object's range in
/// it, and the start of the file's content after the object's header.
pub(super) struct Window {
    id: Vec<u8>,
    key: String,
    object: Range<u64>,
    content: u64,
}

impl Window {
    pub(super) fn size(&self) -> u64 {
        self.object.end - self.content
    }
}

/// What a read of a path found.
enum Source {
    Stored(Window),
    /// A conflict, materialized, or a file that no segment holds.
    Inline(Vec<u8>),
}

impl Repo {
    /// The size of the file with the object `id`, from where its object lies
    /// in a segment, so no file is read.
    pub(super) async fn file_size(&self, id: &[u8]) -> Option<u64> {
        self.window(id).await.map(|window| window.size())
    }

    pub(super) async fn window(&self, id: &[u8]) -> Option<Window> {
        let catalog = self.catalog.read().await;
        let location = catalog.locations.get(id)?;
        let size = content_len(location.range.end - location.range.start)?;
        Some(Window {
            id: id.to_vec(),
            key: self.segment_key(&location.segment),
            object: location.range.clone(),
            content: location.range.end - size,
        })
    }

    /// The bytes of `range` of the file, which must lie within it.
    pub(super) async fn read_range(
        &self,
        window: &Window,
        range: Range<u64>,
    ) -> Result<Vec<u8>, RepoError> {
        let range = window.content + range.start..window.content + range.end;
        let mut parts = self
            .bucket
            .get_ranges(&window.key, &[range])
            .await
            .map_err(internal)?;
        Ok(parts.pop().unwrap_or_default())
    }

    /// The whole file, checked against the hash that names its object.
    pub(super) async fn read_whole(&self, window: &Window) -> Result<Vec<u8>, RepoError> {
        let mut parts = self
            .bucket
            .get_ranges(&window.key, std::slice::from_ref(&window.object))
            .await
            .map_err(internal)?;
        let object = parts.pop().unwrap_or_default();
        verify(window, &object)?;
        let header = usize::try_from(window.content - window.object.start).map_err(internal)?;
        Ok(object[header.min(object.len())..].to_vec())
    }

    /// The file at `path` in the tree of `rev`, from `offset`, `length` bytes
    /// of it or all to the end when `length` is 0. A conflicted file reads as
    /// jj writes it into a working copy, with conflict markers.
    ///
    /// # Errors
    ///
    /// Fails as [`Repo::resolve`] does, or when `path` is no file. A part
    /// that the store can't read fails its item in `chunks`.
    pub async fn file(
        &self,
        rev: &str,
        path: &str,
        offset: u64,
        length: u64,
    ) -> Result<FileRead, RepoError> {
        let rev = rev.to_owned();
        let path = repo_path(path)?;
        let this = self.clone();
        let (source, conflicted) = self
            .at_head(move |repo| async move {
                let not_file =
                    || RepoError::NotFound(format!("file {}", path.as_internal_file_string()));
                let tree = commit(&repo, &rev).await?.tree();
                let value = tree.path_value(&path).await.map_err(internal)?;
                match value.as_resolved() {
                    Some(Some(TreeValue::File { id, .. })) => {
                        let source = match this.window(id.as_bytes()).await {
                            Some(window) => Source::Stored(window),
                            None => Source::Inline(read_all(&repo, &path, id).await?),
                        };
                        return Ok((source, false));
                    }
                    Some(_) => return Err(not_file()),
                    None => {}
                }
                let conflict =
                    try_materialize_file_conflict_value(repo.store(), &path, &value, tree.labels())
                        .await
                        .map_err(internal)?
                        .ok_or_else(not_file)?;
                let options = ConflictMaterializeOptions {
                    marker_style: ConflictMarkerStyle::Diff,
                    marker_len: None,
                    merge: repo.store().merge_options().clone(),
                };
                let bytes = materialize_merge_result_to_bytes(
                    &conflict.contents,
                    &conflict.labels,
                    &options,
                );
                Ok((Source::Inline(bytes.into()), true))
            })
            .await?;

        let end = match length {
            0 => u64::MAX,
            length => offset.saturating_add(length),
        };
        match source {
            Source::Inline(bytes) => {
                let size = bytes.len() as u64;
                let binary = is_binary(&bytes);
                let clamp = |at: u64| usize::try_from(at.min(size)).unwrap_or(usize::MAX);
                let range = clamp(offset).min(clamp(end))..clamp(end);
                let chunks: Vec<_> = bytes[range]
                    .chunks(usize::try_from(READ_CHUNK).unwrap_or(usize::MAX))
                    .map(|chunk| Ok(chunk.to_vec()))
                    .collect();
                Ok(FileRead {
                    size,
                    binary,
                    conflicted,
                    chunks: futures::stream::iter(chunks).boxed(),
                })
            }
            Source::Stored(window) => {
                let size = window.size();
                let probe = self.read_range(&window, 0..BINARY_PROBE.min(size)).await?;
                let (start, end) = (offset.min(size), end.min(size));
                Ok(FileRead {
                    size,
                    binary: is_binary(&probe),
                    conflicted,
                    chunks: self.chunks(window, start.min(end)..end),
                })
            }
        }
    }

    /// The bytes of `range` of a stored file, one ranged read per part. A
    /// read of the whole file is checked against its hash, which fails the
    /// last part on a mismatch. A read of part of it can't be checked.
    fn chunks(
        &self,
        window: Window,
        range: Range<u64>,
    ) -> futures::stream::BoxStream<'static, Result<Vec<u8>, RepoError>> {
        struct Read {
            repo: Repo,
            window: Window,
            at: u64,
            end: u64,
            hasher: Option<blake3::Hasher>,
        }
        let whole = range.start == 0 && range.end == window.size() && range.end > 0;
        let read = Read {
            repo: self.clone(),
            hasher: whole.then(blake3::Hasher::new),
            window,
            at: range.start,
            end: range.end,
        };
        futures::stream::unfold(Some(read), |read| async move {
            let mut read = read?;
            if read.at >= read.end {
                return None;
            }
            let next = (read.at + READ_CHUNK).min(read.end);
            // A checked read starts with the object's header, which the hash
            // covers too.
            let from = match (&read.hasher, read.at) {
                (Some(_), 0) => read.window.object.start,
                _ => read.window.content + read.at,
            };
            let range = from..read.window.content + next;
            let part = read
                .repo
                .bucket
                .get_ranges(&read.window.key, &[range])
                .await
                .map_err(internal)
                .map(|mut parts| parts.pop().unwrap_or_default());
            let mut part = match part {
                Ok(part) => part,
                Err(err) => return Some((Err(err), None)),
            };
            if let Some(hasher) = &mut read.hasher {
                hasher.update(&part);
                if read.at == 0 {
                    let header = usize::try_from(read.window.content - read.window.object.start)
                        .unwrap_or(0);
                    part.drain(..header.min(part.len()));
                }
            }
            read.at = next;
            if read.at == read.end
                && let Some(hasher) = &read.hasher
                && hasher.finalize().as_bytes().as_slice() != read.window.id.as_slice()
            {
                return Some((Err(mismatch(&read.window)), None));
            }
            Some((Ok(part), Some(read)))
        })
        .boxed()
    }
}

fn verify(window: &Window, object: &[u8]) -> Result<(), RepoError> {
    if ids::hash(object) == window.id {
        Ok(())
    } else {
        Err(mismatch(window))
    }
}

fn mismatch(window: &Window) -> RepoError {
    RepoError::Internal(
        format!(
            "object {} doesn't match its bytes",
            jj_lib::hex_util::encode_hex(&window.id)
        )
        .into(),
    )
}

pub(super) async fn read_all(
    repo: &ReadonlyRepo,
    path: &RepoPath,
    id: &FileId,
) -> Result<Vec<u8>, RepoError> {
    let mut reader = repo.store().read_file(path, id).await.map_err(internal)?;
    let mut bytes = vec![];
    reader.read_to_end(&mut bytes).await.map_err(internal)?;
    Ok(bytes)
}

/// The length of the content of a `StoredObject` of `encoded` bytes that
/// holds one `File`: a tag and a length around the `File`, which is a tag and
/// a length around the content, or nothing for an empty file.
fn content_len(encoded: u64) -> Option<u64> {
    let varint = |n: u64| u64::from(64 - n.max(1).leading_zeros()).div_ceil(7);
    (encoded.saturating_sub(22)..=encoded).find(|&n| {
        let file = if n == 0 { 0 } else { 1 + varint(n) + n };
        1 + varint(file) + file == encoded
    })
}

pub(super) fn is_binary(bytes: &[u8]) -> bool {
    let probe = usize::try_from(BINARY_PROBE).unwrap_or(usize::MAX);
    bytes[..bytes.len().min(probe)].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_object_names_the_length_of_its_content() {
        use jjforge_proto::store::v1::File;
        use jjforge_proto::store::v1::StoredObject;
        use jjforge_proto::store::v1::stored_object::Kind;
        use prost::Message as _;

        for size in [0, 1, 127, 128, 16_383, 16_384, 70_000_000] {
            let object = StoredObject {
                kind: Some(Kind::File(File {
                    content: vec![0; size],
                })),
            };

            let encoded = object.encoded_len() as u64;

            assert_eq!(content_len(encoded), Some(size as u64), "{size}");
        }
    }

    #[test]
    fn a_nul_byte_marks_a_file_binary() {
        assert!(is_binary(b"PNG\0"));
        assert!(!is_binary(b"text\n"));
    }
}
