use crate::protocol::WorkspaceEditEncoding;
use merman_editor_core::{PlannedTokenKind, semantic_token_descriptor};
use std::sync::OnceLock;
use tower_lsp::lsp_types::{
    ClientCapabilities, CodeActionKind, DiagnosticTag, MarkupKind, SemanticTokenModifier,
    SemanticTokenType, SemanticTokensClientCapabilities, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, TokenFormat,
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
    token_type_indices: Vec<Option<u32>>,
    token_modifier_bitsets: Vec<u32>,
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
        Self::from_descriptor_support(
            |token_type| capabilities.token_types.contains(token_type),
            |modifier| capabilities.token_modifiers.contains(modifier),
            range,
            full,
        )
    }

    fn all() -> Self {
        Self::from_descriptor_support(
            |_| true,
            |_| true,
            true,
            Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
        )
        .expect("the generated descriptor always contains semantic token kinds")
    }

    fn from_descriptor_support(
        supports_token_type: impl Fn(&SemanticTokenType) -> bool,
        supports_modifier: impl Fn(&SemanticTokenModifier) -> bool,
        range: bool,
        full: Option<SemanticTokensFullOptions>,
    ) -> Option<Self> {
        if !range && full.is_none() {
            return None;
        }

        let descriptor = semantic_token_descriptor();
        let mut token_types = Vec::new();
        let mut token_type_indices = vec![None; descriptor.valid_type_code_max as usize + 1];
        for kind in descriptor.token_kinds {
            let token_type = SemanticTokenType::new(kind.lsp_name);
            if !supports_token_type(&token_type) {
                continue;
            }
            let index = u32::try_from(token_types.len()).ok()?;
            token_type_indices[kind.kind.code() as usize] = Some(index);
            token_types.push(token_type);
        }
        if token_types.is_empty() {
            return None;
        }

        let modifier_slots = descriptor
            .modifiers
            .iter()
            .map(|modifier| modifier.modifier.index() as usize)
            .max()
            .map_or(0, |index| index + 1);
        let mut token_modifiers = Vec::new();
        let mut token_modifier_bitsets = vec![0; modifier_slots];
        for modifier in descriptor.modifiers {
            let token_modifier = SemanticTokenModifier::new(modifier.lsp_name);
            if !supports_modifier(&token_modifier) {
                continue;
            }
            let bit = 1u32.checked_shl(u32::try_from(token_modifiers.len()).ok()?)?;
            token_modifier_bitsets[modifier.modifier.index() as usize] = bit;
            token_modifiers.push(token_modifier);
        }

        Some(Self {
            legend: SemanticTokensLegend {
                token_types,
                token_modifiers,
            },
            token_type_indices,
            token_modifier_bitsets,
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

    pub(crate) fn token_type(&self, kind: PlannedTokenKind) -> Option<u32> {
        self.token_type_indices
            .get(kind.code() as usize)
            .copied()
            .flatten()
    }

    pub(crate) fn token_modifier_bitset(&self, canonical_bits: u32) -> u32 {
        semantic_token_descriptor()
            .modifiers
            .iter()
            .fold(0, |projected_bits, modifier| {
                if canonical_bits & modifier.bit == 0 {
                    return projected_bits;
                }
                projected_bits
                    | self
                        .token_modifier_bitsets
                        .get(modifier.modifier.index() as usize)
                        .copied()
                        .unwrap_or_default()
            })
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
                .and_then(|workspace| workspace.diagnostic.as_ref())
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
    use merman_editor_core::PlannedTokenModifier;

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
    fn semantic_token_projection_uses_descriptor_order_and_reindexes_supported_entries() {
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

        assert_eq!(
            projection.legend().token_types,
            vec![
                SemanticTokenType::new("keyword"),
                SemanticTokenType::new("string"),
                SemanticTokenType::new("variable"),
            ]
        );
        assert_eq!(
            projection.legend().token_modifiers,
            vec![
                SemanticTokenModifier::new("declaration"),
                SemanticTokenModifier::new("defaultLibrary"),
            ]
        );
        assert_eq!(projection.token_type(PlannedTokenKind::Keyword), Some(0));
        assert_eq!(projection.token_type(PlannedTokenKind::String), Some(1));
        assert_eq!(projection.token_type(PlannedTokenKind::Variable), Some(2));
        assert_eq!(projection.token_type(PlannedTokenKind::Comment), None);
        assert_eq!(
            projection.token_modifier_bitset(
                PlannedTokenModifier::Declaration.bit()
                    | PlannedTokenModifier::DefaultLibrary.bit()
            ),
            0b11
        );
        assert_eq!(
            projection.token_modifier_bitset(PlannedTokenModifier::Entity.bit()),
            0
        );
    }

    #[test]
    fn permissive_semantic_token_projection_is_the_full_generated_descriptor() {
        let descriptor = semantic_token_descriptor();
        let projection = ClientProtocolProfile::permissive()
            .semantic_tokens
            .expect("permissive profile enables semantic tokens");

        assert_eq!(
            projection.legend().token_types,
            descriptor
                .token_kinds
                .iter()
                .map(|kind| SemanticTokenType::new(kind.lsp_name))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            projection.legend().token_modifiers,
            descriptor
                .modifiers
                .iter()
                .map(|modifier| SemanticTokenModifier::new(modifier.lsp_name))
                .collect::<Vec<_>>()
        );
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
                "diagnostic": {
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
