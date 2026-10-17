//! `SessionService` of `source/v1`, which opens a session for the server as
//! `POST /sync/v1/session` does for `jf`.

use jjforge_proto::source::v1::GetChallengeRequest;
use jjforge_proto::source::v1::GetChallengeResponse;
use jjforge_proto::source::v1::OpenSessionRequest;
use jjforge_proto::source::v1::OpenSessionResponse;
use jjforge_proto::source::v1::session_service_server::SessionService;
use tonic::Request;
use tonic::Response;
use tonic::Status;

use crate::gate::Gate;

pub(crate) struct Service(pub(crate) Gate);

#[tonic::async_trait]
impl SessionService for Service {
    async fn get_challenge(
        &self,
        _: Request<GetChallengeRequest>,
    ) -> Result<Response<GetChallengeResponse>, Status> {
        let nonce = self.0.verifier().challenge().map_err(|err| {
            tracing::error!(error = %err, "unavailable");
            Status::unavailable(err.to_string())
        })?;
        Ok(Response::new(GetChallengeResponse { nonce }))
    }

    async fn open_session(
        &self,
        request: Request<OpenSessionRequest>,
    ) -> Result<Response<OpenSessionResponse>, Status> {
        let request = request.into_inner();
        let (token, expires_at) = self
            .0
            .verifier()
            .session(
                &request.certificate,
                crate::auth::Audience::Server,
                &request.nonce,
                &request.signature,
            )
            .map_err(|err| Status::unauthenticated(err.to_string()))?;
        Ok(Response::new(OpenSessionResponse {
            token,
            expires_at: i64::try_from(expires_at).unwrap_or(i64::MAX),
        }))
    }
}

#[cfg(test)]
mod tests {
    use jjforge_store::server::Repos;
    use url::Url;

    use super::*;
    use crate::auth::GRPC_AUDIENCE;
    use crate::auth::NAMESPACE;
    use crate::auth::SessionKey;
    use crate::auth::TestAuthority;
    use crate::auth::Verifier;
    use crate::auth::signed;

    #[tokio::test]
    async fn the_server_opens_a_session_with_its_certificate() {
        let authority = TestAuthority::new();
        let cache = tempfile::tempdir().unwrap();
        let repos = Repos::open(&Url::parse("memory:///").unwrap(), cache.path()).unwrap();
        let key = SessionKey::random().unwrap();
        let verifier = Verifier::new(vec![authority.authority("acme")], key);
        let service = Service(Gate::new(repos, verifier));
        let server = authority.person("server", "sync:admin:acme", "");

        let nonce = service
            .get_challenge(Request::new(GetChallengeRequest {}))
            .await
            .unwrap()
            .into_inner()
            .nonce;
        let opened = service
            .open_session(Request::new(OpenSessionRequest {
                certificate: server.certificate(),
                signature: server.sign(&signed(GRPC_AUDIENCE, &nonce), NAMESPACE),
                nonce: nonce.clone(),
            }))
            .await;
        let forged = service
            .open_session(Request::new(OpenSessionRequest {
                certificate: server.certificate(),
                signature: server.sign(&signed(GRPC_AUDIENCE, &nonce), "git"),
                nonce,
            }))
            .await;

        assert_ne!(opened.unwrap().into_inner().token, "");
        assert_eq!(forged.unwrap_err().code(), tonic::Code::Unauthenticated);
    }
}
