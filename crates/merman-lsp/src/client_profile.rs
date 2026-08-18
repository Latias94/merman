use crate::syntax_highlighting::SyntaxTokenKind;
use crate::workspace_edit::WorkspaceEditEncoding;
use std::sync::OnceLock;
use tower_lsp_server::ls_types::{
    ClientCapabilities, CodeActionKind, DiagnosticTag, MarkupKind, SemanticTokenType,
    SemanticTokensClientCapabilities, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, TokenFormat,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkupPreference {
    Markdown,
    PlainText,
    String,
}

impl MarkupPreference {
    fn negotiate(supported: Option<&Vec<MarkupKind>>) -> Self {
        let Some(supported) = supported else {
            return Self::String;
        };
        supported
            .first()
            .map(|kind| match kind {
                MarkupKind::Markdown => Self::Markdown,
                MarkupKind::PlainText => Self::PlainText,
            })
            .unwrap_or(Self::String)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticProtocolProfile {
    pub(crate) related_information: bool,
    pub(crate) deprecated_tag: bool,
    pub(crate) version: bool,
    pub(crate) code_description: bool,
    pub(crate) data: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CodeActionProjection {
    pub(crate) is_preferred: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticTokenProjection {
    legend: SemanticTokensLegend,
    token_types: [Option<u32>; SyntaxTokenKind::COUNT],
    range: bool,
    full: Option<SemanticTokensFullOptions>,
}

impl SemanticTokenProjection {
    fn negotiate(capabilities: &SemanticTokensClientCapabilities) -> Option<Self> {
        if !capabilities.formats.contains(&TokenFormat::RELATIVE) {
            return None;
        }

        let range = capabilities.requests.range.unwrap_or(false);
        let full = match capabilities.requests.full.as_ref() {
            Some(SemanticTokensFullOptions::Bool(true)) => {
                Some(SemanticTokensFullOptions::Bool(true))
            }
            Some(SemanticTokensFullOptions::Delta { delta: Some(true) }) => {
                Some(SemanticTokensFullOptions::Delta { delta: Some(true) })
            }
            Some(SemanticTokensFullOptions::Delta { .. }) => {
                Some(SemanticTokensFullOptions::Bool(true))
            }
            _ => None,
        };
        Self::from_supported_types(
            |kind| {
                capabilities
                    .token_types
                    .contains(&SemanticTokenType::new(kind.lsp_name()))
            },
            range,
            full,
        )
    }

    fn all() -> Self {
        Self::from_supported_types(
            |_| true,
            true,
            Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
        )
        .expect("the standard Tree-sitter projection always contains token kinds")
    }

    fn from_supported_types(
        supports: impl Fn(SyntaxTokenKind) -> bool,
        range: bool,
        full: Option<SemanticTokensFullOptions>,
    ) -> Option<Self> {
        if !range && full.is_none() {
            return None;
        }
        let mut token_types = [None; SyntaxTokenKind::COUNT];
        let mut legend_types = Vec::new();
        for kind in SyntaxTokenKind::ALL {
            if supports(kind) {
                token_types[kind.index()] = u32::try_from(legend_types.len()).ok();
                legend_types.push(SemanticTokenType::new(kind.lsp_name()));
            }
        }
        if legend_types.is_empty() {
            return None;
        }

        Some(Self {
            legend: SemanticTokensLegend {
                token_types: legend_types,
                token_modifiers: Vec::new(),
            },
            token_types,
            range,
            full,
        })
    }

    pub(crate) fn options(&self) -> SemanticTokensOptions {
        SemanticTokensOptions {
            work_done_progress_options: Default::default(),
            legend: self.legend.clone(),
            range: self.range.then_some(true),
            full: self.full.clone(),
        }
    }

    pub(crate) const fn token_type(&self, kind: SyntaxTokenKind) -> Option<u32> {
        self.token_types[kind.index()]
    }

    pub(crate) const fn supports_range(&self) -> bool {
        self.range
    }

    pub(crate) const fn supports_full(&self) -> bool {
        self.full.is_some()
    }

    pub(crate) fn supports_delta(&self) -> bool {
        matches!(
            self.full,
            Some(SemanticTokensFullOptions::Delta { delta: Some(true) })
        )
    }

    #[cfg(test)]
    pub(crate) fn legend(&self) -> SemanticTokensLegend {
        self.legend.clone()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClientProtocolProfile {
    pub(crate) completion_snippets: bool,
    pub(crate) completion_label_details: bool,
    pub(crate) completion_documentation: MarkupPreference,
    pub(crate) hover: MarkupPreference,
    pub(crate) diagnostics: DiagnosticProtocolProfile,
    pub(crate) code_actions: Option<CodeActionProjection>,
    pub(crate) semantic_tokens: Option<SemanticTokenProjection>,
    pub(crate) semantic_tokens_refresh: bool,
    pub(crate) diagnostic_pull: bool,
    pub(crate) diagnostic_refresh: bool,
    pub(crate) workspace_edit_encoding: WorkspaceEditEncoding,
    pub(crate) hierarchical_document_symbols: bool,
}

impl ClientProtocolProfile {
    pub(crate) fn negotiate(capabilities: &ClientCapabilities) -> Self {
        let text_document = capabilities.text_document.as_ref();
        let completion_item = text_document
            .and_then(|capabilities| capabilities.completion.as_ref())
            .and_then(|completion| completion.completion_item.as_ref());
        let hover = text_document.and_then(|capabilities| capabilities.hover.as_ref());
        let diagnostics =
            text_document.and_then(|capabilities| capabilities.publish_diagnostics.as_ref());
        let code_actions = text_document.and_then(|capabilities| capabilities.code_action.as_ref());

        let diagnostic_data = diagnostics
            .and_then(|diagnostics| diagnostics.data_support)
            .unwrap_or(false);
        let code_action_literals = code_actions
            .and_then(|code_actions| code_actions.code_action_literal_support.as_ref())
            .is_some_and(|support| {
                support
                    .code_action_kind
                    .value_set
                    .iter()
                    .any(|kind| kind == CodeActionKind::QUICKFIX.as_str())
            });
        let deprecated_tag = diagnostics
            .and_then(|diagnostics| diagnostics.tag_support.as_ref())
            .is_some_and(|support| support.value_set.contains(&DiagnosticTag::DEPRECATED));

        Self {
            completion_snippets: completion_item
                .and_then(|completion| completion.snippet_support)
                .unwrap_or(false),
            completion_label_details: completion_item
                .and_then(|completion| completion.label_details_support)
                .unwrap_or(false),
            completion_documentation: MarkupPreference::negotiate(
                completion_item.and_then(|completion| completion.documentation_format.as_ref()),
            ),
            hover: MarkupPreference::negotiate(
                hover.and_then(|hover| hover.content_format.as_ref()),
            ),
            diagnostics: DiagnosticProtocolProfile {
                related_information: diagnostics
                    .and_then(|diagnostics| diagnostics.related_information)
                    .unwrap_or(false),
                deprecated_tag,
                version: diagnostics
                    .and_then(|diagnostics| diagnostics.version_support)
                    .unwrap_or(false),
                code_description: diagnostics
                    .and_then(|diagnostics| diagnostics.code_description_support)
                    .unwrap_or(false),
                data: diagnostic_data,
            },
            code_actions: (diagnostic_data && code_action_literals).then(|| CodeActionProjection {
                is_preferred: code_actions
                    .and_then(|code_actions| code_actions.is_preferred_support)
                    .unwrap_or(false),
            }),
            semantic_tokens: text_document
                .and_then(|capabilities| capabilities.semantic_tokens.as_ref())
                .and_then(SemanticTokenProjection::negotiate),
            semantic_tokens_refresh: capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.semantic_tokens.as_ref())
                .and_then(|semantic_tokens| semantic_tokens.refresh_support)
                .unwrap_or(false),
            diagnostic_pull: text_document
                .and_then(|capabilities| capabilities.diagnostic.as_ref())
                .is_some(),
            diagnostic_refresh: capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.diagnostics.as_ref())
                .and_then(|diagnostic| diagnostic.refresh_support)
                .unwrap_or(false),
            workspace_edit_encoding: WorkspaceEditEncoding::from_document_changes_support(
                capabilities
                    .workspace
                    .as_ref()
                    .and_then(|workspace| workspace.workspace_edit.as_ref())
                    .and_then(|workspace_edit| workspace_edit.document_changes)
                    .unwrap_or(false),
            ),
            hierarchical_document_symbols: text_document
                .and_then(|capabilities| capabilities.document_symbol.as_ref())
                .and_then(|document_symbol| document_symbol.hierarchical_document_symbol_support)
                .unwrap_or(false),
        }
    }

    pub(crate) fn permissive() -> Self {
        Self {
            completion_snippets: true,
            completion_label_details: true,
            completion_documentation: MarkupPreference::Markdown,
            hover: MarkupPreference::Markdown,
            diagnostics: DiagnosticProtocolProfile {
                related_information: true,
                deprecated_tag: true,
                version: true,
                code_description: true,
                data: true,
            },
            code_actions: Some(CodeActionProjection { is_preferred: true }),
            semantic_tokens: Some(SemanticTokenProjection::all()),
            semantic_tokens_refresh: false,
            diagnostic_pull: false,
            diagnostic_refresh: false,
            workspace_edit_encoding: WorkspaceEditEncoding::DocumentChanges,
            hierarchical_document_symbols: true,
        }
    }

    pub(crate) fn conservative() -> Self {
        Self::negotiate(&ClientCapabilities::default())
    }

    pub(crate) fn conservative_ref() -> &'static Self {
        static PROFILE: OnceLock<ClientProtocolProfile> = OnceLock::new();
        PROFILE.get_or_init(Self::conservative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_negotiation_respects_client_preference_order() {
        assert_eq!(
            MarkupPreference::negotiate(Some(&vec![MarkupKind::PlainText, MarkupKind::Markdown,])),
            MarkupPreference::PlainText
        );
    }

    #[test]
    fn empty_diagnostic_tag_set_does_not_enable_deprecated_tags() {
        let capabilities: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "textDocument": {
                "publishDiagnostics": {
                    "tagSupport": { "valueSet": [] }
                }
            }
        }))
        .unwrap();

        let profile = ClientProtocolProfile::negotiate(&capabilities);

        assert!(!profile.diagnostics.deprecated_tag);
    }

    #[test]
    fn empty_client_capabilities_negotiate_a_conservative_profile() {
        let profile = ClientProtocolProfile::conservative();

        assert!(!profile.completion_snippets);
        assert!(!profile.completion_label_details);
        assert!(profile.code_actions.is_none());
        assert!(!profile.diagnostics.data);
        assert!(profile.semantic_tokens.is_none());
    }

    #[test]
    fn semantic_token_projection_uses_adapter_order_and_reindexes_supported_entries() {
        let capabilities: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "textDocument": {
                "semanticTokens": {
                    "requests": { "range": true, "full": { "delta": true } },
                    "tokenTypes": ["variable", "string", "keyword"],
                    "tokenModifiers": ["defaultLibrary", "declaration"],
                    "formats": ["relative"]
                }
            }
        }))
        .unwrap();

        let profile = ClientProtocolProfile::negotiate(&capabilities);
        let projection = profile
            .semantic_tokens
            .expect("supported semantic tokens should negotiate a projection");

        assert!(projection.supports_range());
        assert!(projection.supports_full());
        assert!(projection.supports_delta());

        assert_eq!(
            projection.legend().token_types,
            vec![
                SemanticTokenType::new("string"),
                SemanticTokenType::new("keyword"),
                SemanticTokenType::new("variable"),
            ]
        );
        assert!(projection.legend().token_modifiers.is_empty());
        assert!(projection.token_type(SyntaxTokenKind::Keyword).is_some());
        assert!(projection.token_type(SyntaxTokenKind::String).is_some());
        assert!(projection.token_type(SyntaxTokenKind::Variable).is_some());
        assert!(projection.token_type(SyntaxTokenKind::Comment).is_none());
    }

    #[test]
    fn permissive_semantic_token_projection_uses_the_full_standard_adapter_legend() {
        let projection = ClientProtocolProfile::permissive()
            .semantic_tokens
            .expect("permissive profile enables semantic tokens");

        assert_eq!(
            projection.legend().token_types,
            SyntaxTokenKind::ALL
                .into_iter()
                .map(|kind| SemanticTokenType::new(kind.lsp_name()))
                .collect::<Vec<_>>()
        );
        assert!(projection.legend().token_modifiers.is_empty());
    }

    #[test]
    fn semantic_token_request_modes_are_negotiated_independently() {
        let cases = [
            (serde_json::json!({ "range": true }), true, false, false),
            (serde_json::json!({ "full": true }), false, true, false),
            (
                serde_json::json!({ "full": { "delta": false } }),
                false,
                true,
                false,
            ),
            (
                serde_json::json!({ "full": { "delta": true } }),
                false,
                true,
                true,
            ),
        ];

        for (requests, expected_range, expected_full, expected_delta) in cases {
            let capabilities: ClientCapabilities = serde_json::from_value(serde_json::json!({
                "textDocument": {
                    "semanticTokens": {
                        "requests": requests,
                        "tokenTypes": ["keyword"],
                        "tokenModifiers": [],
                        "formats": ["relative"]
                    }
                }
            }))
            .unwrap();
            let projection = ClientProtocolProfile::negotiate(&capabilities)
                .semantic_tokens
                .expect("at least one semantic token request mode is enabled");

            assert_eq!(projection.supports_range(), expected_range);
            assert_eq!(projection.supports_full(), expected_full);
            assert_eq!(projection.supports_delta(), expected_delta);
        }
    }

    #[test]
    fn code_actions_require_diagnostic_data_and_quickfix_literals() {
        let cases = [
            (false, false, false, None),
            (true, false, false, None),
            (false, true, false, None),
            (true, true, false, Some(false)),
            (true, true, true, Some(true)),
        ];

        for (diagnostic_data, quickfix_literal, preferred, expected) in cases {
            let code_action = if quickfix_literal {
                serde_json::json!({
                    "codeActionLiteralSupport": {
                        "codeActionKind": { "valueSet": ["quickfix"] }
                    },
                    "isPreferredSupport": preferred
                })
            } else {
                serde_json::json!({ "isPreferredSupport": preferred })
            };
            let capabilities: ClientCapabilities = serde_json::from_value(serde_json::json!({
                "textDocument": {
                    "publishDiagnostics": { "dataSupport": diagnostic_data },
                    "codeAction": code_action
                }
            }))
            .unwrap();

            let profile = ClientProtocolProfile::negotiate(&capabilities);
            assert_eq!(
                profile
                    .code_actions
                    .map(|projection| projection.is_preferred),
                expected,
                "diagnostic_data={diagnostic_data}, quickfix_literal={quickfix_literal}, preferred={preferred}"
            );
        }
    }

    #[test]
    fn protocol_extensions_are_derived_from_one_negotiated_profile() {
        let capabilities: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "textDocument": {
                "diagnostic": {},
                "documentSymbol": {
                    "hierarchicalDocumentSymbolSupport": true
                }
            },
            "workspace": {
                "diagnostics": {
                    "refreshSupport": true
                },
                "semanticTokens": {
                    "refreshSupport": true
                },
                "workspaceEdit": {
                    "documentChanges": true
                }
            }
        }))
        .unwrap();

        let profile = ClientProtocolProfile::negotiate(&capabilities);

        assert!(profile.diagnostic_pull);
        assert!(profile.diagnostic_refresh);
        assert!(profile.semantic_tokens_refresh);
        assert_eq!(
            profile.workspace_edit_encoding,
            WorkspaceEditEncoding::DocumentChanges
        );
        assert!(profile.hierarchical_document_symbols);
    }
}
