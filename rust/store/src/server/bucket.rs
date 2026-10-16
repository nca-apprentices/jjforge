//! The object store under a URL, as ADR 0002 names it: `s3://` with the
//! twins and in the cluster, `file://` for vcs on its own, and `memory:///`
//! for tests. Each repository keeps everything under `t/{org}/r/{repo}/`.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use futures::StreamExt as _;
use futures::TryStreamExt as _;
use object_store::ObjectStore;
use object_store::ObjectStoreExt as _;
use object_store::PutMode;
use object_store::PutPayload;
use object_store::UpdateVersion;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::memory::InMemory;
use object_store::path::Path;
use tokio::sync::Mutex;
use tracing::field::Empty;
use url::Url;

/// What a conditional write found.
pub(crate) enum Swap {
    Done,
    /// Another writer changed the object first.
    Lost,
}

impl Swap {
    /// How a span names the outcome.
    fn name(&self) -> &'static str {
        match self {
            Self::Done => "won",
            Self::Lost => "lost",
        }
    }
}

/// The current bytes of a mutable object and the tag that names them.
pub(crate) struct Tagged {
    pub(crate) bytes: Vec<u8>,
    pub(crate) tag: Option<String>,
}

pub(crate) struct Bucket {
    store: Arc<dyn ObjectStore>,
    /// A local disk has no conditional replace, so the one vcs that uses it
    /// replaces objects under this lock.
    swap_lock: Option<Mutex<()>>,
}

impl fmt::Debug for Bucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bucket({})", self.store)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum OpenError {
    #[error("{0} is not an object store URL that vcs reads")]
    Url(Url),
    #[error(transparent)]
    Store(#[from] object_store::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Bucket {
    /// Opens `s3://{bucket}`, with the endpoint and keys of the `AWS_*`
    /// environment, `file://{dir}`, or `memory:///`.
    pub(crate) fn open(url: &Url) -> Result<Self, OpenError> {
        let (store, swap_lock): (Arc<dyn ObjectStore>, _) = match url.scheme() {
            "s3" => (
                Arc::new(AmazonS3Builder::from_env().with_url(url.as_str()).build()?),
                None,
            ),
            "file" => {
                let dir = url
                    .to_file_path()
                    .map_err(|()| OpenError::Url(url.clone()))?;
                std::fs::create_dir_all(&dir)?;
                (
                    Arc::new(LocalFileSystem::new_with_prefix(dir)?),
                    Some(Mutex::new(())),
                )
            }
            "memory" => (Arc::new(InMemory::new()), None),
            _ => return Err(OpenError::Url(url.clone())),
        };
        Ok(Self { store, swap_lock })
    }

    /// The bytes under `key`, or `None` when nothing is there.
    #[tracing::instrument(name = "object_store get", skip_all, fields(
        otel.kind = "client",
        jjforge.object_store.operation = "get",
        jjforge.object_store.key = key,
    ))]
    pub(crate) async fn get(&self, key: &str) -> Result<Option<Tagged>, object_store::Error> {
        match self.store.get(&Path::from(key)).await {
            Ok(result) => {
                let tag = result.meta.e_tag.clone();
                let bytes = result.bytes().await?.to_vec();
                Ok(Some(Tagged { bytes, tag }))
            }
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Reads `ranges` of the object under `key`.
    #[tracing::instrument(name = "object_store get_ranges", skip_all, fields(
        otel.kind = "client",
        jjforge.object_store.operation = "get_ranges",
        jjforge.object_store.key = key,
        jjforge.object_store.ranges = ranges.len(),
    ))]
    pub(crate) async fn get_ranges(
        &self,
        key: &str,
        ranges: &[Range<u64>],
    ) -> Result<Vec<Vec<u8>>, object_store::Error> {
        let parts = self.store.get_ranges(&Path::from(key), ranges).await?;
        Ok(parts.into_iter().map(|part| part.to_vec()).collect())
    }

    /// Writes an immutable object with `If-None-Match: *`. An object already
    /// there has the same bytes, because its key names their hash.
    #[tracing::instrument(name = "object_store put", skip_all, fields(
        otel.kind = "client",
        jjforge.object_store.operation = "put",
        jjforge.object_store.key = key,
        jjforge.object_store.bytes = bytes.len(),
    ))]
    pub(crate) async fn put_immutable(
        &self,
        key: &str,
        bytes: Vec<u8>,
    ) -> Result<(), object_store::Error> {
        match self
            .store
            .put_opts(
                &Path::from(key),
                PutPayload::from(bytes),
                PutMode::Create.into(),
            )
            .await
        {
            Ok(_) | Err(object_store::Error::AlreadyExists { .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// Writes `bytes` under `key` only if the object is still the one that
    /// `expected` read, or absent when `expected` is `None`: `If-Match` or
    /// `If-None-Match: *`, as ADR 0002 decides.
    #[tracing::instrument(name = "object_store swap", skip_all, fields(
        otel.kind = "client",
        jjforge.object_store.operation = "swap",
        jjforge.object_store.key = key,
        jjforge.object_store.bytes = bytes.len(),
        jjforge.object_store.swap = Empty,
    ))]
    pub(crate) async fn swap(
        &self,
        key: &str,
        expected: Option<&Tagged>,
        bytes: Vec<u8>,
    ) -> Result<Swap, object_store::Error> {
        let path = Path::from(key);
        let payload = PutPayload::from(bytes);

        let result = if let Some(lock) = &self.swap_lock {
            let _guard = lock.lock().await;
            let current = self.get(key).await?.map(|tagged| tagged.bytes);
            if current != expected.map(|tagged| tagged.bytes.clone()) {
                tracing::Span::current().record("jjforge.object_store.swap", Swap::Lost.name());
                return Ok(Swap::Lost);
            }
            self.store.put(&path, payload).await
        } else {
            let mode = match expected {
                None => PutMode::Create,
                Some(tagged) => PutMode::Update(UpdateVersion {
                    e_tag: tagged.tag.clone(),
                    version: None,
                }),
            };
            self.store.put_opts(&path, payload, mode.into()).await
        };

        let swap = match result {
            Ok(_) => Swap::Done,
            Err(
                object_store::Error::Precondition { .. }
                | object_store::Error::AlreadyExists { .. },
            ) => Swap::Lost,
            Err(err) => return Err(err),
        };
        tracing::Span::current().record("jjforge.object_store.swap", swap.name());
        Ok(swap)
    }

    /// The keys under `prefix`.
    #[tracing::instrument(name = "object_store list", skip_all, fields(
        otel.kind = "client",
        jjforge.object_store.operation = "list",
        jjforge.object_store.key = prefix,
    ))]
    pub(crate) async fn list(&self, prefix: &str) -> Result<Vec<String>, object_store::Error> {
        self.store
            .list(Some(&Path::from(prefix)))
            .map_ok(|meta| meta.location.to_string())
            .try_collect()
            .await
    }

    /// Deletes every object under `prefix`. Deleting nothing succeeds.
    #[tracing::instrument(name = "object_store delete", skip_all, fields(
        otel.kind = "client",
        jjforge.object_store.operation = "delete",
        jjforge.object_store.key = prefix,
    ))]
    pub(crate) async fn delete_all(&self, prefix: &str) -> Result<(), object_store::Error> {
        let locations = self
            .store
            .list(Some(&Path::from(prefix)))
            .map_ok(|meta| meta.location)
            .boxed();
        self.store
            .delete_stream(locations)
            .try_collect::<Vec<_>>()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Bucket {
        Bucket::open(&Url::parse("memory:///").unwrap()).unwrap()
    }

    #[tokio::test]
    async fn swap_from_a_stale_read_is_lost() {
        for bucket in [memory(), {
            let dir = tempfile::tempdir().unwrap();
            Bucket::open(&Url::from_directory_path(dir.keep()).unwrap()).unwrap()
        }] {
            assert!(matches!(
                bucket.swap("HEAD", None, b"1".to_vec()).await.unwrap(),
                Swap::Done
            ));
            assert!(matches!(
                bucket.swap("HEAD", None, b"2".to_vec()).await.unwrap(),
                Swap::Lost
            ));

            let first = bucket.get("HEAD").await.unwrap().unwrap();
            assert!(matches!(
                bucket
                    .swap("HEAD", Some(&first), b"2".to_vec())
                    .await
                    .unwrap(),
                Swap::Done
            ));
            assert!(matches!(
                bucket
                    .swap("HEAD", Some(&first), b"3".to_vec())
                    .await
                    .unwrap(),
                Swap::Lost
            ));
            assert_eq!(bucket.get("HEAD").await.unwrap().unwrap().bytes, b"2");
        }
    }

    #[tokio::test]
    async fn put_immutable_keeps_the_first_bytes() {
        let bucket = memory();

        bucket.put_immutable("a", b"first".to_vec()).await.unwrap();
        bucket.put_immutable("a", b"second".to_vec()).await.unwrap();

        assert_eq!(bucket.get("a").await.unwrap().unwrap().bytes, b"first");
    }

    #[tokio::test]
    async fn delete_all_removes_only_the_prefix() {
        let bucket = memory();
        for key in ["t/a/r/1/HEAD", "t/a/r/1/op/x", "t/a/r/2/HEAD"] {
            bucket.put_immutable(key, vec![]).await.unwrap();
        }

        bucket.delete_all("t/a/r/1").await.unwrap();

        assert_eq!(bucket.list("t/a").await.unwrap(), ["t/a/r/2/HEAD"]);
        bucket.delete_all("t/a/r/1").await.unwrap();
    }

    #[test]
    fn open_refuses_an_unknown_scheme() {
        assert!(matches!(
            Bucket::open(&Url::parse("ftp://example.com/").unwrap()),
            Err(OpenError::Url(_))
        ));
    }
}
