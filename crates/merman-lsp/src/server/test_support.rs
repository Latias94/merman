use super::MermanLanguageServer;
use crate::refresh_transport::{MermanClientSocket, RefreshClient};
use crate::session::{LanguageSession, MermanLspService};
use tower_lsp_server::Client;

pub(crate) struct TestService {
    pub(crate) service: MermanLspService,
    pub(crate) socket: MermanClientSocket,
    pub(crate) backend: MermanLanguageServer,
    pub(crate) session: LanguageSession,
    pub(crate) client: Client,
    pub(crate) refresh_client: RefreshClient,
}

pub(crate) fn service() -> TestService {
    let (refresh_client, refresh_requests, refresh_responses) = RefreshClient::channel();
    let refresh_handle = refresh_client.clone();
    let session = LanguageSession::with_refresh_client(refresh_client);
    let (raw_service, client_socket) = MermanLanguageServer::protocol_service(session.clone());
    let backend = raw_service.inner().clone();
    let client = backend.client_effects.client();
    let service = MermanLspService::new(raw_service, session.clone());
    let socket = MermanClientSocket::new(
        client_socket,
        refresh_requests,
        refresh_responses,
        session.endpoint_guard(),
    );

    TestService {
        service,
        socket,
        backend,
        session,
        client,
        refresh_client: refresh_handle,
    }
}
