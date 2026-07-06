use crate::{binding_error_to_js, document_uri};
use merman_analysis::{
    AnalysisOptions, AnalysisPayload, AnalyzedDiagram, Analyzer, EditorSymbolKind,
    FenceTextIndexSource, SourceDescriptor, Summary,
};
use merman_bindings_core::{BindingError, BindingStatus};
use merman_editor_core::{
    DocumentKind, DocumentSnapshot, EditorDiagnostic, EditorDocumentSymbol, EditorHover,
    EditorLocation, EditorPrepareRename, EditorTextEdit, EditorWorkspaceEdit, FenceSnapshot,
    Position, Range, RenameError, SemanticToken, SemanticTokenKind, SemanticTokenLegend,
    SemanticTokenModifier, analysis_payload_to_diagnostics, code_actions_from_fixes,
    completion_for_snapshot, document_symbols, goto_definition, hover, prepare_rename, references,
    rename, semantic_token_legend, semantic_tokens_for_snapshot, workspace_symbols,
};
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmEditorDiagnostics {
    version: u32,
    valid: bool,
    summary: Summary,
    source: SourceDescriptor,
    diagnostics: Vec<EditorDiagnostic>,
}

#[derive(Debug, Clone)]
struct EditorDocumentContext {
    options_json: String,
    payload: AnalysisPayload,
    snapshot: DocumentSnapshot,
}

impl EditorDocumentContext {
    fn matches(&self, source: &str, uri: &str, options_json: &str) -> bool {
        self.snapshot.text.as_ref() == source
            && self.snapshot.uri.as_str() == uri
            && self.options_json == options_json
    }
}

thread_local! {
    static EDITOR_DOCUMENT_CONTEXT_CACHE: RefCell<Option<EditorDocumentContext>> =
        const { RefCell::new(None) };
}

#[cfg(test)]
static EDITOR_DOCUMENT_CONTEXT_BUILDS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmHover {
    contents: WasmMarkupContent,
    fact_source: &'static str,
    range: Option<Range>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmMarkupContent {
    kind: &'static str,
    value: String,
}

impl From<EditorHover> for WasmHover {
    fn from(value: EditorHover) -> Self {
        Self {
            contents: WasmMarkupContent {
                kind: "markdown",
                value: value.contents.value,
            },
            fact_source: fact_source_name(value.fact_source),
            range: value.range,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmDocumentSymbol {
    name: String,
    detail: Option<String>,
    kind: &'static str,
    fact_source: &'static str,
    range: Range,
    selection_range: Range,
    children: Vec<WasmDocumentSymbol>,
}

impl From<EditorDocumentSymbol> for WasmDocumentSymbol {
    fn from(value: EditorDocumentSymbol) -> Self {
        Self {
            name: value.name,
            detail: value.detail,
            kind: symbol_kind_name(value.kind),
            fact_source: fact_source_name(value.fact_source),
            range: value.range,
            selection_range: value.selection_range,
            children: value.children.into_iter().map(Self::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSymbolInformation {
    name: String,
    kind: &'static str,
    fact_source: &'static str,
    location: WasmLocation,
    container_name: Option<String>,
}

impl From<merman_editor_core::EditorSymbolInformation> for WasmSymbolInformation {
    fn from(value: merman_editor_core::EditorSymbolInformation) -> Self {
        Self {
            name: value.name,
            kind: symbol_kind_name(value.kind),
            fact_source: fact_source_name(value.fact_source),
            location: WasmLocation::from(value.location),
            container_name: value.container_name,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmLocation {
    uri: String,
    fact_source: &'static str,
    range: Range,
}

impl From<EditorLocation> for WasmLocation {
    fn from(value: EditorLocation) -> Self {
        Self {
            uri: value.uri.as_str().to_string(),
            fact_source: fact_source_name(value.fact_source),
            range: value.range,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmPrepareRename {
    fact_source: &'static str,
    range: Range,
    placeholder: String,
}

impl From<EditorPrepareRename> for WasmPrepareRename {
    fn from(value: EditorPrepareRename) -> Self {
        Self {
            fact_source: fact_source_name(value.fact_source),
            range: value.range,
            placeholder: value.placeholder,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmWorkspaceEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    fact_source: Option<&'static str>,
    changes: HashMap<String, Vec<WasmTextEdit>>,
}

impl From<EditorWorkspaceEdit> for WasmWorkspaceEdit {
    fn from(value: EditorWorkspaceEdit) -> Self {
        Self {
            fact_source: Some(fact_source_name(value.fact_source)),
            changes: value
                .changes
                .into_iter()
                .map(|(uri, edits)| {
                    (
                        uri.as_str().to_string(),
                        edits.into_iter().map(WasmTextEdit::from).collect(),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmTextEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    fact_source: Option<&'static str>,
    range: Range,
    new_text: String,
}

impl From<EditorTextEdit> for WasmTextEdit {
    fn from(value: EditorTextEdit) -> Self {
        Self {
            fact_source: Some(fact_source_name(value.fact_source)),
            range: value.range,
            new_text: value.new_text,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmCodeAction {
    title: String,
    kind: &'static str,
    diagnostics: Vec<EditorDiagnostic>,
    edit: WasmWorkspaceEdit,
    is_preferred: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSemanticTokenLegend {
    token_types: Vec<&'static str>,
    token_modifiers: Vec<&'static str>,
}

impl From<SemanticTokenLegend> for WasmSemanticTokenLegend {
    fn from(value: SemanticTokenLegend) -> Self {
        Self {
            token_types: value
                .token_types
                .into_iter()
                .map(semantic_token_kind_name)
                .collect(),
            token_modifiers: value
                .token_modifiers
                .into_iter()
                .map(semantic_token_modifier_name)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: &'static str,
    token_modifier: &'static str,
    fact_source: &'static str,
}

impl From<SemanticToken> for WasmSemanticToken {
    fn from(value: SemanticToken) -> Self {
        Self {
            line: value.line,
            start: value.start,
            length: value.length,
            token_type: semantic_token_kind_name(value.kind),
            token_modifier: semantic_token_modifier_name(value.modifier),
            fact_source: fact_source_name(value.fact_source),
        }
    }
}

#[wasm_bindgen(js_name = editorDiagnostics)]
pub fn editor_diagnostics(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let uri = editor_uri(uri);
    let payload = editor_analysis_payload(source, options_json.as_deref(), &uri)?;
    let diagnostics = analysis_payload_to_diagnostics(&payload);
    let response = WasmEditorDiagnostics {
        version: payload.version,
        valid: payload.valid,
        summary: payload.summary,
        source: payload.source,
        diagnostics,
    };
    js_value(&response)
}

#[wasm_bindgen(js_name = editorCodeActions)]
pub fn editor_code_actions(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let uri = editor_uri(uri);
    let payload = editor_analysis_payload(source, options_json.as_deref(), &uri)?;
    let diagnostics = analysis_payload_to_diagnostics(&payload);
    js_value(&code_actions_for_diagnostics(&diagnostics, &uri))
}

#[wasm_bindgen(js_name = editorCompletions)]
pub fn editor_completions(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    js_value(&completion_for_snapshot(
        &snapshot,
        Position::new(line, character),
    ))
}

#[wasm_bindgen(js_name = editorHover)]
pub fn editor_hover(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    js_value(&hover(&snapshot, Position::new(line, character)).map(WasmHover::from))
}

#[wasm_bindgen(js_name = editorDocumentSymbols)]
pub fn editor_document_symbols(
    source: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    let symbols = document_symbols(&snapshot)
        .into_iter()
        .map(WasmDocumentSymbol::from)
        .collect::<Vec<_>>();
    js_value(&symbols)
}

#[wasm_bindgen(js_name = editorWorkspaceSymbols)]
pub fn editor_workspace_symbols(
    source: &str,
    query: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    let symbols = workspace_symbols(&snapshot, query)
        .into_iter()
        .map(WasmSymbolInformation::from)
        .collect::<Vec<_>>();
    js_value(&symbols)
}

#[wasm_bindgen(js_name = editorDefinition)]
pub fn editor_definition(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    js_value(&goto_definition(&snapshot, Position::new(line, character)).map(WasmLocation::from))
}

#[wasm_bindgen(js_name = editorReferences)]
pub fn editor_references(
    source: &str,
    line: usize,
    character: usize,
    include_declaration: bool,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    let locations = references(
        &snapshot,
        Position::new(line, character),
        include_declaration,
    )
    .unwrap_or_default()
    .into_iter()
    .map(WasmLocation::from)
    .collect::<Vec<_>>();
    js_value(&locations)
}

#[wasm_bindgen(js_name = editorPrepareRename)]
pub fn editor_prepare_rename(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    js_value(
        &prepare_rename(&snapshot, Position::new(line, character)).map(WasmPrepareRename::from),
    )
}

#[wasm_bindgen(js_name = editorRename)]
pub fn editor_rename(
    source: &str,
    line: usize,
    character: usize,
    new_name: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    match rename(&snapshot, Position::new(line, character), new_name) {
        Ok(edit) => js_value(&edit.map(WasmWorkspaceEdit::from)),
        Err(err) => Err(rename_error_to_js(err)),
    }
}

#[wasm_bindgen(js_name = editorSemanticTokenLegend)]
pub fn editor_semantic_token_legend() -> Result<JsValue, JsValue> {
    js_value(&WasmSemanticTokenLegend::from(semantic_token_legend()))
}

#[wasm_bindgen(js_name = editorSemanticTokens)]
pub fn editor_semantic_tokens(
    source: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri, options_json.as_deref())?;
    let tokens = semantic_tokens_for_snapshot(&snapshot)
        .into_iter()
        .map(WasmSemanticToken::from)
        .collect::<Vec<_>>();
    js_value(&tokens)
}

fn js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn editor_uri(uri: Option<String>) -> String {
    document_uri(uri)
}

fn editor_snapshot(
    source: &str,
    uri: Option<String>,
    options_json: Option<&str>,
) -> Result<DocumentSnapshot, JsValue> {
    Ok(editor_document_context(source, uri, options_json)?.snapshot)
}

fn document_kind_for_uri(uri: &str) -> DocumentKind {
    DocumentKind::from_path(uri.split(['?', '#']).next().unwrap_or(uri))
}

fn editor_analysis_payload(
    source: &str,
    options_json: Option<&str>,
    uri: &str,
) -> Result<AnalysisPayload, JsValue> {
    Ok(editor_document_context(source, Some(uri.to_string()), options_json)?.payload)
}

fn editor_document_context(
    source: &str,
    uri: Option<String>,
    options_json: Option<&str>,
) -> Result<EditorDocumentContext, JsValue> {
    let uri = editor_uri(uri);
    let options_json_key = editor_options_cache_key(options_json);
    EDITOR_DOCUMENT_CONTEXT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(context) = cache
            .as_ref()
            .filter(|context| context.matches(source, &uri, options_json_key))
        {
            return Ok(context.clone());
        }

        let context = build_editor_document_context(source, &uri, options_json)?;
        *cache = Some(context.clone());
        Ok(context)
    })
}

fn build_editor_document_context(
    source: &str,
    uri: &str,
    options_json: Option<&str>,
) -> Result<EditorDocumentContext, JsValue> {
    let options = parse_analysis_options(options_json).map_err(binding_error_to_js)?;
    let analyzer = Analyzer::with_options(options);
    let kind = document_kind_for_uri(uri);
    let descriptor = source_descriptor_for_kind(kind, uri);
    let text = Arc::<str>::from(source);
    record_editor_document_context_build();
    let analysis =
        merman_analysis::analyze_document_result_shared(Arc::clone(&text), &analyzer, descriptor);
    let payload = analysis.payload().clone();
    let snapshot = DocumentSnapshot {
        uri: uri.to_string().into(),
        version: 1,
        kind,
        source: payload.source.clone(),
        text,
        source_map: analysis.source_map().clone(),
        fences: analysis
            .diagrams()
            .iter()
            .map(editor_fence_snapshot)
            .collect(),
    };
    Ok(EditorDocumentContext {
        options_json: editor_options_cache_key(options_json).to_string(),
        payload,
        snapshot,
    })
}

fn editor_fence_snapshot(diagram: &AnalyzedDiagram) -> FenceSnapshot {
    FenceSnapshot {
        source_id: diagram.source_id.clone(),
        index: diagram.index,
        source: diagram.source.clone(),
        start: diagram.start,
        body_start: diagram.body_start,
        body_end: diagram.body_end,
        end: diagram.end,
        text: diagram.text.clone(),
        fence_delimiter: diagram.fence_delimiter,
        diagram_type: diagram.syntax.diagram_type.clone(),
        text_index: diagram.syntax.text_index.clone(),
    }
}

fn editor_options_cache_key(options_json: Option<&str>) -> &str {
    match options_json {
        Some(options_json) if !options_json.trim().is_empty() => options_json,
        _ => "",
    }
}

fn record_editor_document_context_build() {
    #[cfg(test)]
    {
        EDITOR_DOCUMENT_CONTEXT_BUILDS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

fn parse_analysis_options(options_json: Option<&str>) -> Result<AnalysisOptions, BindingError> {
    let Some(options_json) = options_json else {
        return Ok(AnalysisOptions::default());
    };
    if options_json.trim().is_empty() {
        return Ok(AnalysisOptions::default());
    }
    let value = serde_json::from_str::<serde_json::Value>(options_json).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })?;
    merman_analysis::analysis_options_from_json_value(&value)
        .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))
}

fn source_descriptor_for_kind(kind: DocumentKind, uri: &str) -> SourceDescriptor {
    let source_kind = match kind {
        DocumentKind::Diagram => merman_analysis::SourceKind::Diagram,
        DocumentKind::Markdown => merman_analysis::SourceKind::Markdown,
        DocumentKind::Mdx => merman_analysis::SourceKind::Mdx,
    };
    merman_analysis::source_descriptor_for_kind(Some(uri), source_kind)
}

fn code_actions_for_diagnostics(
    diagnostics: &[EditorDiagnostic],
    uri: &str,
) -> Vec<WasmCodeAction> {
    diagnostics
        .iter()
        .flat_map(|diagnostic| {
            let Some(data) = diagnostic.data.as_ref() else {
                return Vec::new();
            };
            code_actions_from_fixes(&data.fixes)
                .into_iter()
                .map(|action| {
                    let edits = action
                        .edits
                        .into_iter()
                        .map(|edit| WasmTextEdit {
                            fact_source: None,
                            range: edit.range,
                            new_text: edit.new_text,
                        })
                        .collect::<Vec<_>>();

                    WasmCodeAction {
                        title: action.title,
                        kind: "quickfix",
                        diagnostics: vec![diagnostic.clone()],
                        edit: WasmWorkspaceEdit {
                            fact_source: None,
                            changes: HashMap::from([(uri.to_string(), edits)]),
                        },
                        is_preferred: action.is_preferred,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn symbol_kind_name(kind: EditorSymbolKind) -> &'static str {
    match kind {
        EditorSymbolKind::Class => "class",
        EditorSymbolKind::Event => "event",
        EditorSymbolKind::Function => "function",
        EditorSymbolKind::Module => "module",
        EditorSymbolKind::Namespace => "namespace",
        EditorSymbolKind::Object => "object",
        EditorSymbolKind::Package => "package",
        EditorSymbolKind::Property => "property",
        EditorSymbolKind::String => "string",
        EditorSymbolKind::Struct => "struct",
        EditorSymbolKind::Variable => "variable",
    }
}

fn fact_source_name(source: FenceTextIndexSource) -> &'static str {
    match source {
        FenceTextIndexSource::TextScan => "text_scan",
        FenceTextIndexSource::ParserComplete => "parser_complete",
        FenceTextIndexSource::ParserCompleteDegradedSpans => "parser_complete_degraded_spans",
        FenceTextIndexSource::ParserRecovered => "parser_recovered",
        FenceTextIndexSource::ParserRecoveredDegradedSpans => "parser_recovered_degraded_spans",
    }
}

fn semantic_token_kind_name(kind: SemanticTokenKind) -> &'static str {
    match kind {
        SemanticTokenKind::Namespace => "namespace",
        SemanticTokenKind::Class => "class",
        SemanticTokenKind::Struct => "struct",
        SemanticTokenKind::Variable => "variable",
        SemanticTokenKind::Property => "property",
        SemanticTokenKind::Event => "event",
        SemanticTokenKind::Function => "function",
        SemanticTokenKind::String => "string",
    }
}

fn semantic_token_modifier_name(modifier: SemanticTokenModifier) -> &'static str {
    match modifier {
        SemanticTokenModifier::Entity => "entity",
        SemanticTokenModifier::Outline => "outline",
        SemanticTokenModifier::Payload => "payload",
    }
}

fn rename_error_to_js(err: RenameError) -> JsValue {
    binding_error_to_js(BindingError::new(
        BindingStatus::InvalidArgument,
        err.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_editor_document_context_cache_for_tests() {
        EDITOR_DOCUMENT_CONTEXT_CACHE.with(|cache| {
            cache.replace(None);
        });
        EDITOR_DOCUMENT_CONTEXT_BUILDS.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn editor_document_context_builds_for_tests() -> usize {
        EDITOR_DOCUMENT_CONTEXT_BUILDS.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn editor_language_helpers_cover_browser_editor_surface() {
        reset_editor_document_context_cache_for_tests();

        let completion_snapshot = editor_snapshot(
            "flowchart TD\nA-->B\nC-->\n",
            Some("file:///tmp/example.mmd".to_string()),
            None,
        )
        .unwrap();
        let completions = completion_for_snapshot(&completion_snapshot, Position::new(2, 4));
        assert!(completions.items.iter().any(|item| item.label == "B"));

        let reference_snapshot = editor_snapshot(
            "flowchart TD\nA-->B\nA-->C\n",
            Some("file:///tmp/example.mmd".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(
            references(&reference_snapshot, Position::new(1, 0), true)
                .unwrap()
                .len(),
            2
        );
        assert!(!semantic_tokens_for_snapshot(&reference_snapshot).is_empty());

        let payload =
            editor_analysis_payload("flowchart TD\nA-->\n", None, "file:///tmp/example.mmd")
                .unwrap();
        let diagnostics = analysis_payload_to_diagnostics(&payload);
        assert!(!diagnostics.is_empty());

        let actions = code_actions_for_diagnostics(&diagnostics, "file:///tmp/example.mmd");
        assert!(actions.iter().all(|action| action.kind == "quickfix"));
    }

    #[test]
    fn editor_language_context_reuses_same_source_across_browser_calls() {
        reset_editor_document_context_cache_for_tests();

        let source = "flowchart TD\nA-->B\n";
        let uri = "file:///tmp/example.mmd";

        let payload = editor_analysis_payload(source, None, uri).unwrap();
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        let snapshot = editor_snapshot(source, Some(uri.to_string()), None).unwrap();
        assert_eq!(editor_document_context_builds_for_tests(), 1);
        assert!(!semantic_tokens_for_snapshot(&snapshot).is_empty());

        let repeated_payload = editor_analysis_payload(source, Some(" \n "), uri).unwrap();
        assert_eq!(repeated_payload, payload);
        assert_eq!(editor_document_context_builds_for_tests(), 1);
    }

    #[test]
    fn editor_language_context_invalidates_on_source_or_uri_change() {
        reset_editor_document_context_cache_for_tests();

        let uri = "file:///tmp/example.mmd";
        let source = "flowchart TD\nA-->B\n";
        let updated_source = "flowchart TD\nA-->C\n";

        let first = editor_snapshot(source, Some(uri.to_string()), None).unwrap();
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        let repeated = editor_snapshot(source, Some(uri.to_string()), None).unwrap();
        assert_eq!(repeated.text, first.text);
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        let updated = editor_snapshot(updated_source, Some(uri.to_string()), None).unwrap();
        assert_eq!(updated.text.as_ref(), updated_source);
        assert_eq!(editor_document_context_builds_for_tests(), 2);

        let other_uri = "file:///tmp/other.mmd";
        let other_document = editor_snapshot(updated_source, Some(other_uri.to_string()), None)
            .expect("uri change rebuilds cached context");
        assert_eq!(other_document.uri.as_str(), other_uri);
        assert_eq!(editor_document_context_builds_for_tests(), 3);
    }

    #[test]
    fn wasm_code_actions_share_sorted_overlap_policy() {
        let map = merman_analysis::SourceMap::new("0123456789");
        let valid_later = map.span(5, 6).unwrap();
        let valid_earlier = map.span(1, 2).unwrap();
        let overlap_left = map.span(0, 4).unwrap();
        let overlap_right = map.span(2, 5).unwrap();
        let diagnostic = EditorDiagnostic {
            range: Range::default(),
            severity: merman_analysis::DiagnosticSeverity::Warning,
            code: "merman.test".to_string(),
            source: "merman".to_string(),
            message: "test".to_string(),
            related: Vec::new(),
            data: Some(merman_editor_core::DiagnosticCodeActionData {
                id: "merman.test".to_string(),
                code: None,
                code_name: None,
                category: merman_analysis::DiagnosticCategory::Semantic,
                diagram_type: None,
                help: None,
                fixes: vec![
                    merman_analysis::DiagnosticFix::new(
                        "Sort edits",
                        vec![
                            merman_analysis::DiagnosticFixEdit::new(valid_later, "late"),
                            merman_analysis::DiagnosticFixEdit::new(valid_earlier, "early"),
                        ],
                    ),
                    merman_analysis::DiagnosticFix::new(
                        "Reject overlaps",
                        vec![
                            merman_analysis::DiagnosticFixEdit::new(overlap_right, "right"),
                            merman_analysis::DiagnosticFixEdit::new(overlap_left, "left"),
                        ],
                    ),
                ],
            }),
        };

        let actions = code_actions_for_diagnostics(&[diagnostic], "file:///tmp/example.mmd");

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Sort edits");
        let edits = actions[0]
            .edit
            .changes
            .get("file:///tmp/example.mmd")
            .expect("uri edits");
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].range.start, Position::new(0, 1));
        assert_eq!(edits[0].new_text, "early");
        assert_eq!(edits[1].range.start, Position::new(0, 5));
        assert_eq!(edits[1].new_text, "late");
    }
}
