use crate::client_profile::ClientProtocolProfile;
use crate::code_actions::{allows_quickfix, append_action_diagnostic, code_action_for_server_fix};
use crate::diagnostics::editor_diagnostic_to_lsp_with_data;
use crate::protocol::{DiagnosticIdentityData, range_to_lsp};
use crate::snapshot::{
    AnalysisResultIdentity, DiagnosticGeneration, DocumentEpoch, DocumentSnapshot,
};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisPayload, DiagnosticFix,
};
use merman_editor_core::{EditorDiagnostic, analysis_diagnostic_to_editor};
use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic, NumberOrString, Uri,
};

const DIAGNOSTIC_ID_PREFIX: &str = "m1";

/// One immutable server-owned diagnostic projection and its code-action provenance.
#[derive(Debug)]
pub(crate) struct DiagnosticRoundTrip {
    scope: DiagnosticResultScope,
    diagnostics: Box<[EditorDiagnostic]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticResultScope {
    uri: Uri,
    document_epoch: DocumentEpoch,
    document_version: i32,
    analysis_result_identity: AnalysisResultIdentity,
    diagnostic_generation: DiagnosticGeneration,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct ServerDiagnosticId {
    analysis_result_identity: AnalysisResultIdentity,
    document_epoch: DocumentEpoch,
    diagnostic_generation: DiagnosticGeneration,
    ordinal: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SharedFixIdentity {
    edits_ptr: usize,
    edits_len: usize,
    title: String,
    is_preferred: bool,
}

impl DiagnosticRoundTrip {
    pub(crate) fn build(
        snapshot: &DocumentSnapshot,
        document_epoch: DocumentEpoch,
        diagnostic_generation: DiagnosticGeneration,
        payload: &AnalysisPayload,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let mut diagnostics = Vec::with_capacity(payload.diagnostics.len());
        for (index, diagnostic) in payload.diagnostics.iter().enumerate() {
            if index.is_multiple_of(128) {
                cancellation.checkpoint()?;
            }
            diagnostics.push(analysis_diagnostic_to_editor(diagnostic));
        }
        cancellation.checkpoint()?;
        Ok(Self {
            scope: DiagnosticResultScope {
                uri: snapshot.uri().clone(),
                document_epoch,
                document_version: snapshot.version(),
                analysis_result_identity: snapshot.analysis_result_identity(),
                diagnostic_generation,
            },
            diagnostics: diagnostics.into_boxed_slice(),
        })
    }

    pub(crate) fn diagnostics_with_profile(
        &self,
        profile: &ClientProtocolProfile,
    ) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .enumerate()
            .map(|(ordinal, diagnostic)| {
                let data = profile.diagnostics.data.then(|| {
                    serde_json::to_value(DiagnosticIdentityData {
                        id: self.server_id(ordinal).to_wire(),
                        document_version: Some(self.scope.document_version),
                    })
                    .ok()
                });
                editor_diagnostic_to_lsp_with_data(
                    diagnostic,
                    &self.scope.uri,
                    data.flatten(),
                    profile.diagnostics,
                )
            })
            .collect()
    }

    pub(crate) fn code_actions_with_profile(
        &self,
        params: &CodeActionParams,
        profile: &ClientProtocolProfile,
    ) -> Option<CodeActionResponse> {
        let projection = profile.code_actions.as_ref()?;
        if params.text_document.uri != self.scope.uri || !allows_quickfix(&params.context) {
            return None;
        }

        let mut seen = HashSet::with_capacity(params.context.diagnostics.len());
        let mut requested = Vec::with_capacity(params.context.diagnostics.len());
        for diagnostic in &params.context.diagnostics {
            let (identity, server_diagnostic) = self.resolve(diagnostic)?;
            if !seen.insert(identity) {
                return None;
            }
            requested.push((diagnostic, server_diagnostic));
        }

        let mut actions = Vec::<CodeActionOrCommand>::new();
        let mut materialized_fixes = HashMap::<SharedFixIdentity, usize>::new();
        for (lsp_diagnostic, server_diagnostic) in requested {
            let Some(data) = server_diagnostic.data.as_ref() else {
                continue;
            };
            for fix in &data.fixes {
                let identity = SharedFixIdentity::new(fix);
                if let Some(index) = materialized_fixes.get(&identity).copied() {
                    append_action_diagnostic(&mut actions[index], lsp_diagnostic);
                    continue;
                }
                let Some(action) = code_action_for_server_fix(
                    fix,
                    lsp_diagnostic,
                    &self.scope.uri,
                    self.scope.document_version,
                    profile.workspace_edit_encoding,
                    projection.is_preferred,
                ) else {
                    continue;
                };
                materialized_fixes.insert(identity, actions.len());
                actions.push(action);
            }
        }

        (!actions.is_empty()).then_some(actions)
    }

    pub(crate) fn result_id(&self) -> String {
        format!(
            "r1:{:016x}:{:016x}:{:016x}",
            self.scope.analysis_result_identity.get(),
            self.scope.document_epoch.0,
            self.scope.diagnostic_generation.0,
        )
    }

    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        let mut total = size_of::<Self>()
            .saturating_add(self.scope.uri.as_str().len())
            .saturating_add(
                self.diagnostics
                    .len()
                    .saturating_mul(size_of::<EditorDiagnostic>()),
            );
        for diagnostic in &self.diagnostics {
            total = total
                .saturating_add(diagnostic.code.capacity())
                .saturating_add(diagnostic.source.capacity())
                .saturating_add(diagnostic.message.capacity())
                .saturating_add(
                    diagnostic
                        .tags
                        .capacity()
                        .saturating_mul(size_of::<merman_analysis::AnalysisDiagnosticTag>()),
                )
                .saturating_add(
                    diagnostic
                        .related
                        .capacity()
                        .saturating_mul(size_of::<merman_editor_core::EditorDiagnosticRelated>()),
                );
            for related in &diagnostic.related {
                total = total.saturating_add(related.message.capacity());
            }
            if let Some(data) = &diagnostic.data {
                total = total
                    .saturating_add(data.id.capacity())
                    .saturating_add(data.code_name.as_ref().map_or(0, String::capacity))
                    .saturating_add(data.diagram_type.as_ref().map_or(0, String::capacity))
                    .saturating_add(data.help.as_ref().map_or(0, String::capacity))
                    .saturating_add(
                        data.fixes
                            .capacity()
                            .saturating_mul(size_of::<DiagnosticFix>()),
                    );
                for fix in &data.fixes {
                    total = total.saturating_add(fix.title.capacity());
                }
            }
        }
        total
    }

    fn resolve(&self, diagnostic: &Diagnostic) -> Option<(ServerDiagnosticId, &EditorDiagnostic)> {
        let identity =
            serde_json::from_value::<DiagnosticIdentityData>(diagnostic.data.as_ref()?.clone())
                .ok()?;
        if identity.document_version != Some(self.scope.document_version) {
            return None;
        }
        let server_id = ServerDiagnosticId::from_wire(&identity.id)?;
        if server_id.analysis_result_identity != self.scope.analysis_result_identity
            || server_id.document_epoch != self.scope.document_epoch
            || server_id.diagnostic_generation != self.scope.diagnostic_generation
        {
            return None;
        }
        let ordinal = usize::try_from(server_id.ordinal).ok()?;
        let server_diagnostic = self.diagnostics.get(ordinal)?;
        let NumberOrString::String(code) = diagnostic.code.as_ref()? else {
            return None;
        };
        if diagnostic.source.as_deref() != Some(server_diagnostic.source.as_str())
            || code != &server_diagnostic.code
            || diagnostic.message != server_diagnostic.message
            || diagnostic.range != range_to_lsp(server_diagnostic.range)
        {
            return None;
        }
        Some((server_id, server_diagnostic))
    }

    fn server_id(&self, ordinal: usize) -> ServerDiagnosticId {
        ServerDiagnosticId {
            analysis_result_identity: self.scope.analysis_result_identity,
            document_epoch: self.scope.document_epoch,
            diagnostic_generation: self.scope.diagnostic_generation,
            ordinal: u64::try_from(ordinal).expect("diagnostic ordinal must fit u64"),
        }
    }
}

impl ServerDiagnosticId {
    fn to_wire(self) -> String {
        format!(
            "{DIAGNOSTIC_ID_PREFIX}:{:016x}:{:016x}:{:016x}:{:016x}",
            self.analysis_result_identity.get(),
            self.document_epoch.0,
            self.diagnostic_generation.0,
            self.ordinal,
        )
    }

    fn from_wire(value: &str) -> Option<Self> {
        let mut fields = value.split(':');
        if fields.next()? != DIAGNOSTIC_ID_PREFIX {
            return None;
        }
        let analysis_result_identity = u64::from_str_radix(fields.next()?, 16).ok()?;
        let document_epoch = u64::from_str_radix(fields.next()?, 16).ok()?;
        let diagnostic_generation = u64::from_str_radix(fields.next()?, 16).ok()?;
        let ordinal = u64::from_str_radix(fields.next()?, 16).ok()?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            analysis_result_identity: AnalysisResultIdentity::from_wire_value(
                analysis_result_identity,
            ),
            document_epoch: DocumentEpoch(document_epoch),
            diagnostic_generation: DiagnosticGeneration(diagnostic_generation),
            ordinal,
        })
    }
}

impl SharedFixIdentity {
    fn new(fix: &DiagnosticFix) -> Self {
        Self {
            edits_ptr: fix.edits.as_ptr() as usize,
            edits_len: fix.edits.len(),
            title: fix.title.clone(),
            is_preferred: fix.is_preferred,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiagnosticRoundTrip;
    use crate::client_profile::ClientProtocolProfile;
    use crate::snapshot::{DiagnosticGeneration, DocumentEpoch, snapshot_for_test};
    use merman_analysis::{
        AnalysisCancellationToken, AnalysisDiagnostic, AnalysisOptions, AnalysisPayload,
        AnalysisRuleConfig, AnalysisRuleProfile, Analyzer, DiagnosticCategory, DiagnosticFix,
        DiagnosticFixEdit, DiagnosticSeverity, DiagnosticSpan, LspRange, SourceDescriptor,
        SourcePosition, Utf16Position,
    };
    use serde_json::json;
    use std::str::FromStr;
    use tower_lsp_server::ls_types::{
        CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, NumberOrString,
        Range, TextDocumentIdentifier, Uri,
    };

    const DOCUMENT_VERSION: i32 = 7;

    #[test]
    fn round_trip_accepts_only_the_exact_returned_diagnostic() {
        let (round_trip, profile, uri) = direction_round_trip();
        let diagnostic = direction_diagnostic(&round_trip, &profile);
        let data = diagnostic
            .data
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .expect("server diagnostic identity");
        assert_eq!(data.len(), 2);
        assert_eq!(data["documentVersion"], DOCUMENT_VERSION);
        assert!(data["id"].as_str().is_some_and(|id| id.starts_with("m1:")));

        let actions = round_trip
            .code_actions_with_profile(&params(uri.clone(), vec![diagnostic.clone()]), &profile)
            .expect("exact diagnostic must resolve its server-owned fix");
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action");
        };
        assert_eq!(action.title, "Insert `TB` into the flowchart header");

        let mut forged_fix = diagnostic.clone();
        forged_fix
            .data
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .expect("diagnostic identity")
            .insert(
                "fixes".to_string(),
                json!([{"title": "Replace everything", "replacement": "forged"}]),
            );
        let forged_actions = round_trip
            .code_actions_with_profile(&params(uri.clone(), vec![forged_fix]), &profile)
            .expect("unknown client data must not replace server-owned fix provenance");
        let CodeActionOrCommand::CodeAction(forged_action) = &forged_actions[0] else {
            panic!("expected code action");
        };
        assert_eq!(forged_action.title, "Insert `TB` into the flowchart header");
        assert_eq!(first_edit_text(forged_action), " TB");

        let mut mutations = Vec::new();
        let mut changed = diagnostic.clone();
        changed.source = Some("forged".to_string());
        mutations.push(changed);
        let mut changed = diagnostic.clone();
        changed.data.as_mut().expect("identity")["id"] = json!("m1:forged");
        mutations.push(changed);
        let mut changed = diagnostic.clone();
        changed.data.as_mut().expect("identity")["documentVersion"] = json!(DOCUMENT_VERSION - 1);
        mutations.push(changed);
        let mut changed = diagnostic.clone();
        changed.code = Some(NumberOrString::String("forged".to_string()));
        mutations.push(changed);
        let mut changed = diagnostic.clone();
        changed.message.push_str(" forged");
        mutations.push(changed);
        let mut changed = diagnostic.clone();
        changed.range.end.character += 1;
        mutations.push(changed);

        for changed in mutations {
            assert!(
                round_trip
                    .code_actions_with_profile(&params(uri.clone(), vec![changed]), &profile)
                    .is_none()
            );
        }
        assert!(
            round_trip
                .code_actions_with_profile(
                    &params(uri, vec![diagnostic.clone(), diagnostic.clone()]),
                    &profile,
                )
                .is_none(),
            "a returned diagnostic id may be consumed at most once per request"
        );
        let wrong_uri = Uri::from_str("file:///tmp/other.mmd").unwrap();
        assert!(
            round_trip
                .code_actions_with_profile(&params(wrong_uri, vec![diagnostic]), &profile)
                .is_none(),
            "the request URI is part of the diagnostic result scope"
        );
    }

    #[test]
    fn identity_changes_only_when_its_analysis_or_diagnostic_scope_changes() {
        let uri = Uri::from_str("file:///tmp/identity.mmd").unwrap();
        let source = "flowchart\nA-->B\n";
        let payload = recommended_analyzer().analyze(source);
        let snapshot = snapshot_for_test(uri.clone(), DOCUMENT_VERSION, source);

        let original = build_round_trip(
            &snapshot,
            DocumentEpoch(1),
            DiagnosticGeneration(1),
            &payload,
        );
        let repeated = build_round_trip(
            &snapshot,
            DocumentEpoch(1),
            DiagnosticGeneration(1),
            &payload,
        );
        let reprojected = build_round_trip(
            &snapshot,
            DocumentEpoch(1),
            DiagnosticGeneration(2),
            &payload,
        );
        let reopened = build_round_trip(
            &snapshot,
            DocumentEpoch(2),
            DiagnosticGeneration(1),
            &payload,
        );
        let rebuilt_snapshot = snapshot_for_test(uri, DOCUMENT_VERSION, source);
        let rebuilt = build_round_trip(
            &rebuilt_snapshot,
            DocumentEpoch(1),
            DiagnosticGeneration(1),
            &payload,
        );

        assert_eq!(first_id(&original), first_id(&repeated));
        assert_ne!(first_id(&original), first_id(&reprojected));
        assert_ne!(first_id(&original), first_id(&reopened));
        assert_ne!(first_id(&original), first_id(&rebuilt));
    }

    #[test]
    fn fix_aggregation_uses_shared_arc_provenance_not_equal_contents() {
        let uri = Uri::from_str("file:///tmp/provenance.mmd").unwrap();
        let snapshot = snapshot_for_test(uri.clone(), DOCUMENT_VERSION, "flowchart TD\nA-->B\n");
        let span = test_span();
        let fix = DiagnosticFix::new("Apply fix", vec![DiagnosticFixEdit::new(span, "X")]);

        let shared_payload = duplicate_payload(fix.clone(), fix);
        let shared = build_round_trip(
            &snapshot,
            DocumentEpoch(1),
            DiagnosticGeneration(1),
            &shared_payload,
        );
        let profile = ClientProtocolProfile::permissive();
        let shared_diagnostics = shared.diagnostics_with_profile(&profile);
        assert_ne!(
            shared_diagnostics[0].data.as_ref().unwrap()["id"],
            shared_diagnostics[1].data.as_ref().unwrap()["id"]
        );
        let shared_actions = shared
            .code_actions_with_profile(&params(uri.clone(), shared_diagnostics), &profile)
            .expect("shared fix");
        assert_eq!(shared_actions.len(), 1);
        let CodeActionOrCommand::CodeAction(shared_action) = &shared_actions[0] else {
            panic!("expected code action");
        };
        assert_eq!(
            shared_action
                .diagnostics
                .as_ref()
                .expect("shared fix must retain both diagnostics")
                .len(),
            2
        );

        let equal_payload = duplicate_payload(
            DiagnosticFix::new("Apply fix", vec![DiagnosticFixEdit::new(span, "X")]),
            DiagnosticFix::new("Apply fix", vec![DiagnosticFixEdit::new(span, "X")]),
        );
        let equal = build_round_trip(
            &snapshot,
            DocumentEpoch(1),
            DiagnosticGeneration(2),
            &equal_payload,
        );
        let equal_diagnostics = equal.diagnostics_with_profile(&profile);
        let equal_actions = equal
            .code_actions_with_profile(&params(uri, equal_diagnostics), &profile)
            .expect("equal but independently owned fixes");
        assert_eq!(equal_actions.len(), 2);
    }

    fn direction_round_trip() -> (DiagnosticRoundTrip, ClientProtocolProfile, Uri) {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let source = "flowchart\nA-->B\n";
        let snapshot = snapshot_for_test(uri.clone(), DOCUMENT_VERSION, source);
        let payload = recommended_analyzer().analyze(source);
        (
            build_round_trip(
                &snapshot,
                DocumentEpoch(1),
                DiagnosticGeneration(1),
                &payload,
            ),
            ClientProtocolProfile::permissive(),
            uri,
        )
    }

    fn recommended_analyzer() -> Analyzer {
        Analyzer::with_options(AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
        ))
    }

    fn build_round_trip(
        snapshot: &crate::snapshot::DocumentSnapshot,
        epoch: DocumentEpoch,
        generation: DiagnosticGeneration,
        payload: &AnalysisPayload,
    ) -> DiagnosticRoundTrip {
        DiagnosticRoundTrip::build(
            snapshot,
            epoch,
            generation,
            payload,
            &AnalysisCancellationToken::new(),
        )
        .expect("test projection must not be cancelled")
    }

    fn direction_diagnostic(
        round_trip: &DiagnosticRoundTrip,
        profile: &ClientProtocolProfile,
    ) -> tower_lsp_server::ls_types::Diagnostic {
        round_trip
            .diagnostics_with_profile(profile)
            .into_iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "merman.authoring.flowchart.explicit_direction".to_string(),
                    ))
            })
            .expect("flowchart direction diagnostic")
    }

    fn params(
        uri: Uri,
        diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
    ) -> CodeActionParams {
        CodeActionParams {
            text_document: TextDocumentIdentifier { uri },
            range: diagnostics
                .first()
                .map_or_else(Range::default, |diagnostic| diagnostic.range),
            context: CodeActionContext {
                diagnostics,
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }

    fn first_id(round_trip: &DiagnosticRoundTrip) -> String {
        round_trip.diagnostics_with_profile(&ClientProtocolProfile::permissive())[0]
            .data
            .as_ref()
            .and_then(|data| data.get("id"))
            .and_then(serde_json::Value::as_str)
            .expect("opaque diagnostic id")
            .to_string()
    }

    fn first_edit_text(action: &tower_lsp_server::ls_types::CodeAction) -> &str {
        let Some(tower_lsp_server::ls_types::DocumentChanges::Edits(document_edits)) = action
            .edit
            .as_ref()
            .and_then(|edit| edit.document_changes.as_ref())
        else {
            panic!("expected versioned text edits");
        };
        let Some(tower_lsp_server::ls_types::OneOf::Left(edit)) = document_edits
            .first()
            .and_then(|document| document.edits.first())
        else {
            panic!("expected a plain text edit");
        };
        edit.new_text.as_str()
    }

    fn duplicate_payload(first_fix: DiagnosticFix, second_fix: DiagnosticFix) -> AnalysisPayload {
        let diagnostic = |fix| {
            AnalysisDiagnostic::new(
                "merman.test.same_visible_diagnostic",
                DiagnosticSeverity::Warning,
                DiagnosticCategory::Semantic,
                "same visible diagnostic",
            )
            .with_span(test_span())
            .with_fix(fix)
        };
        AnalysisPayload::new(
            SourceDescriptor::diagram(),
            vec![diagnostic(first_fix), diagnostic(second_fix)],
        )
    }

    const fn test_span() -> DiagnosticSpan {
        DiagnosticSpan::new(
            0..1,
            SourcePosition::new(0, 0),
            SourcePosition::new(0, 1),
            LspRange::new(
                Utf16Position {
                    line: 0,
                    character: 0,
                },
                Utf16Position {
                    line: 0,
                    character: 1,
                },
            ),
        )
    }
}
