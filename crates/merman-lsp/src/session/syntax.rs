use super::LanguageSession;
use super::documents::SemanticTokensState;
use crate::syntax_highlighting::SyntaxDocumentState;
use std::sync::Arc;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Uri;

impl LanguageSession {
    pub(crate) async fn query_semantic_tokens<T>(
        &self,
        uri: &Uri,
        previous_result_id: Option<&str>,
        compute: impl FnOnce(
            &SyntaxDocumentState,
            Option<Arc<SemanticTokensState>>,
            &merman_analysis::AnalysisCancellationToken,
        ) -> Result<Option<(T, Option<SemanticTokensState>)>>,
    ) -> Result<Option<T>> {
        let captured = {
            let mut state = self.inner.state.lock().await;
            self.commit_state_if_active(&mut state, |state| {
                let snapshot = state.syntax_document_snapshot(uri);
                let previous = previous_result_id
                    .and_then(|result_id| state.semantic_tokens_state_for_delta(uri, result_id));
                (snapshot, previous)
            })
        };
        let Some((Some(snapshot), previous)) = captured else {
            return Ok(None);
        };
        let computed = compute(&snapshot.document, previous, snapshot.cancellation());

        let mut state = self.inner.state.lock().await;
        self.commit_state_if_active(&mut state, |state| {
            if !state.is_syntax_document_current(&snapshot) {
                return Err(semantic_tokens_stale_error());
            }
            match computed {
                Ok(Some((result, Some(next_state)))) => {
                    if state.set_semantic_tokens_state_if_syntax_current(&snapshot, next_state) {
                        Ok(Some(result))
                    } else {
                        Err(semantic_tokens_stale_error())
                    }
                }
                Ok(Some((result, None))) => Ok(Some(result)),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            }
        })
        .unwrap_or(Ok(None))
    }
}

fn semantic_tokens_stale_error() -> tower_lsp_server::jsonrpc::Error {
    let mut error = tower_lsp_server::jsonrpc::Error::content_modified();
    error.message = "semantic tokens document changed while computing".into();
    error
}
