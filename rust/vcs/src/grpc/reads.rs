//! `HistoryService`, `FileService`, and `DiffService` of `source/v1`, which
//! the server reads repositories through for the REST routes of #34, #41,
//! and #47, and follows every push through, as #98 needs. Each call needs
//! read access, as `sync/v1` gives it.

use std::pin::Pin;

use futures::Stream;
use futures::StreamExt as _;
use jjforge_proto::source::v1::DiffRequest;
use jjforge_proto::source::v1::DiffResponse;
use jjforge_proto::source::v1::GetChangeRequest;
use jjforge_proto::source::v1::GetChangeResponse;
use jjforge_proto::source::v1::GetEvolutionRequest;
use jjforge_proto::source::v1::GetEvolutionResponse;
use jjforge_proto::source::v1::ListBookmarksRequest;
use jjforge_proto::source::v1::ListBookmarksResponse;
use jjforge_proto::source::v1::ListOperationsRequest;
use jjforge_proto::source::v1::ListOperationsResponse;
use jjforge_proto::source::v1::ListTreeRequest;
use jjforge_proto::source::v1::ListTreeResponse;
use jjforge_proto::source::v1::LogRequest;
use jjforge_proto::source::v1::LogResponse;
use jjforge_proto::source::v1::ReadFileRequest;
use jjforge_proto::source::v1::ReadFileResponse;
use jjforge_proto::source::v1::ResolveRequest;
use jjforge_proto::source::v1::ResolveResponse;
use jjforge_proto::source::v1::WatchOperationsRequest;
use jjforge_proto::source::v1::WatchOperationsResponse;
use jjforge_proto::source::v1::diff_service_server::DiffService;
use jjforge_proto::source::v1::file_service_server::FileService;
use jjforge_proto::source::v1::history_service_server::HistoryService;
use jjforge_store::OperationId;
use tonic::Request;
use tonic::Response;
use tonic::Status;

use super::open;
use super::status;
use crate::auth::Access;
use crate::gate::Gate;

/// The bytes of one `ReadFileResponse`, so a large file arrives in parts.
const CHUNK: usize = 64 << 10;

/// How often a watch reads `HEAD` again. Another replica may have moved it,
/// so vcs reads the object store rather than waiting for its own pushes.
const POLL: std::time::Duration = std::time::Duration::from_secs(1);

type Answers<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

fn answers<T: Send + 'static>(items: Vec<T>) -> Answers<T> {
    futures::stream::iter(items.into_iter().map(Ok)).boxed()
}

pub(crate) struct Service(pub(crate) Gate);

#[tonic::async_trait]
impl HistoryService for Service {
    type LogStream = Answers<LogResponse>;
    type WatchOperationsStream = Answers<WatchOperationsResponse>;

    async fn watch_operations(
        &self,
        request: Request<WatchOperationsRequest>,
    ) -> Result<Response<Self::WatchOperationsStream>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let since = match request.get_ref().since.as_str() {
            "" => None,
            since => Some(
                since
                    .parse::<OperationId>()
                    .map_err(|err| Status::invalid_argument(format!("since: {err}")))?,
            ),
        };
        let operations = repo.watch(since, POLL).map(|operation| {
            operation
                .map(|operation| WatchOperationsResponse {
                    operation: Some(operation),
                })
                .map_err(status)
        });
        Ok(Response::new(operations.boxed()))
    }

    async fn resolve(
        &self,
        request: Request<ResolveRequest>,
    ) -> Result<Response<ResolveResponse>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let resolved = repo.resolve(&request.get_ref().rev).await.map_err(status)?;
        Ok(Response::new(resolved))
    }

    async fn log(&self, request: Request<LogRequest>) -> Result<Response<Self::LogStream>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        let page = repo
            .log(&request.rev, &request.page_token, request.page_size)
            .await
            .map_err(status)?;
        let mut changes: Vec<_> = page
            .items
            .into_iter()
            .map(|change| LogResponse {
                change: Some(change),
                next_page_token: String::new(),
            })
            .collect();
        if let Some(last) = changes.last_mut() {
            last.next_page_token = page.next;
        }
        Ok(Response::new(answers(changes)))
    }

    async fn get_change(
        &self,
        request: Request<GetChangeRequest>,
    ) -> Result<Response<GetChangeResponse>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let change = repo.change(&request.get_ref().rev).await.map_err(status)?;
        Ok(Response::new(change))
    }

    async fn get_evolution(
        &self,
        request: Request<GetEvolutionRequest>,
    ) -> Result<Response<GetEvolutionResponse>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        let page = repo
            .evolution(&request.rev, &request.page_token, request.page_size)
            .await
            .map_err(status)?;
        Ok(Response::new(GetEvolutionResponse {
            versions: page.items,
            next_page_token: page.next,
        }))
    }

    async fn list_operations(
        &self,
        request: Request<ListOperationsRequest>,
    ) -> Result<Response<ListOperationsResponse>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        let page = repo
            .operation_page(&request.page_token, request.page_size)
            .await
            .map_err(status)?;
        Ok(Response::new(ListOperationsResponse {
            operations: page.items,
            next_page_token: page.next,
        }))
    }

    async fn list_bookmarks(
        &self,
        request: Request<ListBookmarksRequest>,
    ) -> Result<Response<ListBookmarksResponse>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        let page = repo
            .bookmarks(&request.page_token, request.page_size)
            .await
            .map_err(status)?;
        Ok(Response::new(ListBookmarksResponse {
            bookmarks: page.items,
            next_page_token: page.next,
        }))
    }
}

#[tonic::async_trait]
impl FileService for Service {
    type ReadFileStream = Answers<ReadFileResponse>;

    async fn list_tree(
        &self,
        request: Request<ListTreeRequest>,
    ) -> Result<Response<ListTreeResponse>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        let page = repo
            .tree(
                &request.rev,
                &request.path,
                &request.page_token,
                request.page_size,
            )
            .await
            .map_err(status)?;
        Ok(Response::new(ListTreeResponse {
            entries: page.items,
            next_page_token: page.next,
        }))
    }

    async fn read_file(
        &self,
        request: Request<ReadFileRequest>,
    ) -> Result<Response<Self::ReadFileStream>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        let range = |name: &str, value: i64| {
            u64::try_from(value)
                .map_err(|_| Status::invalid_argument(format!("{name} is negative")))
        };
        let file = repo
            .file(
                &request.rev,
                &request.path,
                range("offset", request.offset)?,
                range("length", request.length)?,
            )
            .await
            .map_err(status)?;
        let size = i64::try_from(file.size).unwrap_or(i64::MAX);
        // The first chunk carries the size and whether the file is binary,
        // even when the range is empty. Each chunk is copied as it is sent,
        // so the range sits in memory once.
        let chunks = futures::stream::unfold(Some(0), move |at| {
            let response = at.map(|at| {
                let end = (at + CHUNK).min(file.bytes.len());
                let first = at == 0;
                let response = ReadFileResponse {
                    size: if first { size } else { 0 },
                    binary: first && file.binary,
                    chunk: file.bytes[at..end].to_vec(),
                };
                (Ok(response), (end < file.bytes.len()).then_some(end))
            });
            async move { response }
        });
        Ok(Response::new(chunks.boxed()))
    }
}

#[tonic::async_trait]
impl DiffService for Service {
    type DiffStream = Answers<DiffResponse>;

    async fn diff(
        &self,
        request: Request<DiffRequest>,
    ) -> Result<Response<Self::DiffStream>, Status> {
        let repo = open(
            &self.0,
            &request,
            request.get_ref().repo.as_ref(),
            Access::Read,
        )?;
        let request = request.get_ref();
        if request.rev.is_empty() == (request.from.is_empty() || request.to.is_empty()) {
            return Err(Status::invalid_argument(
                "a diff names either rev, or both from and to",
            ));
        }
        let page = repo
            .diff(
                &request.rev,
                &request.from,
                &request.to,
                &request.page_token,
                request.page_size,
            )
            .await
            .map_err(status)?;
        let mut files = page.items;
        if let Some(last) = files.last_mut() {
            last.next_page_token = page.next;
        }
        Ok(Response::new(answers(files)))
    }
}

#[cfg(test)]
mod tests {
    use jjforge_proto::source::v1::RepoRef;
    use jjforge_store::server::Repos;
    use url::Url;

    use super::*;
    use crate::auth::test_principals;

    async fn service() -> (tempfile::TempDir, Service) {
        let cache = tempfile::tempdir().unwrap();
        let repos = Repos::open(&Url::parse("memory:///").unwrap(), cache.path()).unwrap();
        repos.get("acme", "demo").unwrap().create().await.unwrap();
        (cache, Service(Gate::new(repos, test_principals())))
    }

    fn request<T>(message: T, principal: Option<&str>) -> Request<T> {
        let mut request = Request::new(message);
        if let Some(principal) = principal {
            request.metadata_mut().insert(
                "authorization",
                format!("Bearer {principal}").parse().unwrap(),
            );
        }
        request
    }

    fn bookmarks() -> ListBookmarksRequest {
        ListBookmarksRequest {
            repo: Some(RepoRef {
                org_id: "acme".to_owned(),
                repo_id: "demo".to_owned(),
            }),
            ..ListBookmarksRequest::default()
        }
    }

    #[tokio::test]
    async fn a_reader_lists_the_bookmarks_of_a_new_repository() {
        let (_cache, service) = service().await;

        let answer = service
            .list_bookmarks(request(bookmarks(), Some("reader")))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(answer.bookmarks, []);
    }

    #[tokio::test]
    async fn a_read_without_a_principal_is_unauthenticated() {
        let (_cache, service) = service().await;

        let refused = service.list_bookmarks(request(bookmarks(), None)).await;

        assert_eq!(refused.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn a_log_without_trunk_is_not_found() {
        let (_cache, service) = service().await;
        let log = LogRequest {
            repo: bookmarks().repo,
            ..LogRequest::default()
        };

        let refused = service.log(request(log, Some("reader"))).await;

        assert_eq!(refused.err().unwrap().code(), tonic::Code::NotFound);
    }
}
