#![forbid(unsafe_code)]

//! WebAssembly bindings for browser integrations.
//!
//! The crate intentionally stays thin: all parsing, rendering, options parsing, and error
//! classification are delegated to `merman-bindings-core`.

#[cfg(feature = "editor-language")]
use merman_analysis::{
    AnalysisOptions, AnalysisPayload, Analyzer, EditorSymbolKind, FenceTextIndexSource,
    SourceDescriptor, Summary,
};
use merman_bindings_core::BindingError;
#[cfg(feature = "editor-language")]
use merman_bindings_core::BindingStatus;
#[cfg(feature = "editor-language")]
use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentWorkspace, EditorDiagnostic, EditorDocumentSymbol,
    EditorHover, EditorLocation, EditorPrepareRename, EditorTextEdit, EditorWorkspaceEdit,
    Position, Range, SemanticToken, SemanticTokenKind, SemanticTokenLegend, SemanticTokenModifier,
    analysis_payload_to_diagnostics, completion_for_snapshot, document_symbols, goto_definition,
    hover, prepare_rename, references, rename, semantic_token_legend, semantic_tokens_for_snapshot,
    workspace_symbols,
};
use serde::Serialize;
#[cfg(feature = "editor-language")]
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "render", target_arch = "wasm32"))]
use merman_bindings_core::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
#[cfg(all(feature = "render", target_arch = "wasm32"))]
use serde::Deserialize;
#[cfg(all(feature = "render", target_arch = "wasm32"))]
use std::{cell::RefCell, sync::Arc};

const WASM_ABI_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
struct WasmErrorPayload<'a> {
    version: u32,
    ok: bool,
    code: i32,
    code_name: &'a str,
    message: &'a str,
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmEditorDiagnostics {
    version: u32,
    valid: bool,
    summary: Summary,
    source: SourceDescriptor,
    diagnostics: Vec<EditorDiagnostic>,
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmHover {
    contents: WasmMarkupContent,
    fact_source: &'static str,
    range: Option<Range>,
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmMarkupContent {
    kind: &'static str,
    value: String,
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSymbolInformation {
    name: String,
    kind: &'static str,
    fact_source: &'static str,
    location: WasmLocation,
    container_name: Option<String>,
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmLocation {
    uri: String,
    fact_source: &'static str,
    range: Range,
}

#[cfg(feature = "editor-language")]
impl From<EditorLocation> for WasmLocation {
    fn from(value: EditorLocation) -> Self {
        Self {
            uri: value.uri.as_str().to_string(),
            fact_source: fact_source_name(value.fact_source),
            range: value.range,
        }
    }
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmPrepareRename {
    fact_source: &'static str,
    range: Range,
    placeholder: String,
}

#[cfg(feature = "editor-language")]
impl From<EditorPrepareRename> for WasmPrepareRename {
    fn from(value: EditorPrepareRename) -> Self {
        Self {
            fact_source: fact_source_name(value.fact_source),
            range: value.range,
            placeholder: value.placeholder,
        }
    }
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmWorkspaceEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    fact_source: Option<&'static str>,
    changes: HashMap<String, Vec<WasmTextEdit>>,
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmTextEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    fact_source: Option<&'static str>,
    range: Range,
    new_text: String,
}

#[cfg(feature = "editor-language")]
impl From<EditorTextEdit> for WasmTextEdit {
    fn from(value: EditorTextEdit) -> Self {
        Self {
            fact_source: Some(fact_source_name(value.fact_source)),
            range: value.range,
            new_text: value.new_text,
        }
    }
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmCodeAction {
    title: String,
    kind: &'static str,
    diagnostics: Vec<EditorDiagnostic>,
    edit: WasmWorkspaceEdit,
    is_preferred: bool,
}

#[cfg(feature = "editor-language")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSemanticTokenLegend {
    token_types: Vec<&'static str>,
    token_modifiers: Vec<&'static str>,
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
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

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = abiVersion)]
pub fn abi_version() -> u32 {
    WASM_ABI_VERSION
}

#[wasm_bindgen(js_name = packageVersion)]
pub fn package_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen(js_name = renderSvg)]
pub fn render_svg(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::render_svg(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = renderSvgWithTextMeasurer)]
pub fn render_svg_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let engine =
            merman_bindings_core::BindingEngine::new(options_bytes(options_json.as_deref()))
                .map_err(binding_error_to_js)?;
        let engine = engine.with_text_measurer(Arc::new(WasmHostTextMeasurer::default()));
        string_result(engine.render_svg(source.as_bytes()))
    })
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = layoutJsonWithTextMeasurer)]
pub fn layout_json_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let engine =
            merman_bindings_core::BindingEngine::new(options_bytes(options_json.as_deref()))
                .map_err(binding_error_to_js)?;
        let engine = engine.with_text_measurer(Arc::new(WasmHostTextMeasurer::default()));
        string_result(engine.layout_json(source.as_bytes()))
    })
}

#[wasm_bindgen(js_name = parseJson)]
pub fn parse_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::parse_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = layoutJson)]
pub fn layout_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::layout_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = renderAscii)]
pub fn render_ascii(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::render_ascii(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen]
pub fn analyze(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(merman_bindings_core::analyze_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = analyzeJson)]
pub fn analyze_json(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    analyze(source, options_json)
}

#[wasm_bindgen(js_name = analyzeDocument)]
pub fn analyze_document(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let uri = document_uri(uri);
    json_value_result(merman_bindings_core::analyze_document_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        uri.as_bytes(),
    ))
}

#[wasm_bindgen]
pub fn validate(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(merman_bindings_core::validate_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = supportedDiagrams)]
pub fn supported_diagrams() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_diagrams())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = bindingCapabilities)]
pub fn binding_capabilities() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::binding_capabilities())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = selectedRegistryProfile)]
pub fn selected_registry_profile() -> String {
    merman_bindings_core::selected_registry_profile().to_string()
}

#[wasm_bindgen(js_name = diagramFamilyCapabilities)]
pub fn diagram_family_capabilities() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::diagram_family_capabilities())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = lintRuleCatalog)]
pub fn lint_rule_catalog() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::lint_rule_catalog())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = supportedThemes)]
pub fn supported_themes() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_themes())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = supportedHostThemePresets)]
pub fn supported_host_theme_presets() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_host_theme_presets())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = asciiSupportedDiagrams)]
pub fn ascii_supported_diagrams() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::ascii_supported_diagrams())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = asciiCapabilities)]
pub fn ascii_capabilities() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::ascii_capabilities())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorCompletions)]
pub fn editor_completions(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    js_value(&completion_for_snapshot(
        &snapshot,
        Position::new(line, character),
    ))
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorHover)]
pub fn editor_hover(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    js_value(&hover(&snapshot, Position::new(line, character)).map(WasmHover::from))
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorDocumentSymbols)]
pub fn editor_document_symbols(source: &str, uri: Option<String>) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    let symbols = document_symbols(&snapshot)
        .into_iter()
        .map(WasmDocumentSymbol::from)
        .collect::<Vec<_>>();
    js_value(&symbols)
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorWorkspaceSymbols)]
pub fn editor_workspace_symbols(
    source: &str,
    query: &str,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    let symbols = workspace_symbols(&snapshot, query)
        .into_iter()
        .map(WasmSymbolInformation::from)
        .collect::<Vec<_>>();
    js_value(&symbols)
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorDefinition)]
pub fn editor_definition(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    js_value(&goto_definition(&snapshot, Position::new(line, character)).map(WasmLocation::from))
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorReferences)]
pub fn editor_references(
    source: &str,
    line: usize,
    character: usize,
    include_declaration: bool,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
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

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorPrepareRename)]
pub fn editor_prepare_rename(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    js_value(
        &prepare_rename(&snapshot, Position::new(line, character)).map(WasmPrepareRename::from),
    )
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorRename)]
pub fn editor_rename(
    source: &str,
    line: usize,
    character: usize,
    new_name: &str,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    match rename(&snapshot, Position::new(line, character), new_name) {
        Ok(edit) => js_value(&edit.map(WasmWorkspaceEdit::from)),
        Err(err) => Err(JsValue::from_str(&err.to_string())),
    }
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorSemanticTokenLegend)]
pub fn editor_semantic_token_legend() -> Result<JsValue, JsValue> {
    js_value(&WasmSemanticTokenLegend::from(semantic_token_legend()))
}

#[cfg(feature = "editor-language")]
#[wasm_bindgen(js_name = editorSemanticTokens)]
pub fn editor_semantic_tokens(source: &str, uri: Option<String>) -> Result<JsValue, JsValue> {
    let snapshot = editor_snapshot(source, uri);
    let tokens = semantic_tokens_for_snapshot(&snapshot)
        .into_iter()
        .map(WasmSemanticToken::from)
        .collect::<Vec<_>>();
    js_value(&tokens)
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

#[cfg(feature = "editor-language")]
fn js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|err| JsValue::from_str(&err.to_string()))
}

fn document_uri(uri: Option<String>) -> String {
    uri.filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "file:///merman/document.mmd".to_string())
}

#[cfg(feature = "editor-language")]
fn editor_uri(uri: Option<String>) -> String {
    document_uri(uri)
}

#[cfg(feature = "editor-language")]
fn editor_snapshot(source: &str, uri: Option<String>) -> DocumentSnapshot {
    let uri = editor_uri(uri);
    let kind = document_kind_for_uri(&uri);
    let mut workspace = DocumentWorkspace::new();
    workspace.upsert(uri, 1, source.to_string(), kind)
}

#[cfg(feature = "editor-language")]
fn document_kind_for_uri(uri: &str) -> DocumentKind {
    DocumentKind::from_path(uri.split(['?', '#']).next().unwrap_or(uri))
}

#[cfg(feature = "editor-language")]
fn editor_analysis_payload(
    source: &str,
    options_json: Option<&str>,
    uri: &str,
) -> Result<AnalysisPayload, JsValue> {
    let options = parse_analysis_options(options_json).map_err(binding_error_to_js)?;
    let analyzer = Analyzer::with_options(options);
    let kind = document_kind_for_uri(uri);
    let descriptor = source_descriptor_for_kind(kind, uri);
    Ok(merman_analysis::document::analyze_document(
        source, &analyzer, descriptor,
    ))
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
fn source_descriptor_for_kind(kind: DocumentKind, uri: &str) -> SourceDescriptor {
    let source_kind = match kind {
        DocumentKind::Diagram => merman_analysis::SourceKind::Diagram,
        DocumentKind::Markdown => merman_analysis::SourceKind::Markdown,
        DocumentKind::Mdx => merman_analysis::SourceKind::Mdx,
    };
    merman_analysis::source_descriptor_for_kind(Some(uri), source_kind)
}

#[cfg(feature = "editor-language")]
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
            data.fixes
                .iter()
                .filter_map(|fix| {
                    let edits = fix
                        .edits
                        .iter()
                        .map(|edit| WasmTextEdit {
                            fact_source: None,
                            range: Range::new(
                                Position::new(
                                    edit.span.lsp_range.start.line,
                                    edit.span.lsp_range.start.character,
                                ),
                                Position::new(
                                    edit.span.lsp_range.end.line,
                                    edit.span.lsp_range.end.character,
                                ),
                            ),
                            new_text: edit.replacement.clone(),
                        })
                        .collect::<Vec<_>>();
                    if edits.is_empty() {
                        return None;
                    }

                    Some(WasmCodeAction {
                        title: fix.title.clone(),
                        kind: "quickfix",
                        diagnostics: vec![diagnostic.clone()],
                        edit: WasmWorkspaceEdit {
                            fact_source: None,
                            changes: HashMap::from([(uri.to_string(), edits)]),
                        },
                        is_preferred: fix.is_preferred,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
fn fact_source_name(source: FenceTextIndexSource) -> &'static str {
    match source {
        FenceTextIndexSource::TextScan => "text_scan",
        FenceTextIndexSource::ParserComplete => "parser_complete",
        FenceTextIndexSource::ParserRecovered => "parser_recovered",
    }
}

#[cfg(feature = "editor-language")]
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

#[cfg(feature = "editor-language")]
fn semantic_token_modifier_name(modifier: SemanticTokenModifier) -> &'static str {
    match modifier {
        SemanticTokenModifier::Entity => "entity",
        SemanticTokenModifier::Outline => "outline",
        SemanticTokenModifier::Payload => "payload",
    }
}

fn string_result(result: Result<Vec<u8>, BindingError>) -> Result<String, JsValue> {
    let bytes = result.map_err(binding_error_to_js)?;
    String::from_utf8(bytes).map_err(|err| JsValue::from_str(&err.to_string()))
}

fn json_value_result(result: Result<Vec<u8>, BindingError>) -> Result<JsValue, JsValue> {
    let bytes = result.map_err(binding_error_to_js)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|err| JsValue::from_str(&err.to_string()))?;
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn binding_error_to_js(err: BindingError) -> JsValue {
    let payload = wasm_error_payload(&err);
    payload
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or_else(|_| {
            JsValue::from_str(&format!("{}: {}", payload.code_name, payload.message))
        })
}

fn wasm_error_payload(err: &BindingError) -> WasmErrorPayload<'_> {
    WasmErrorPayload {
        version: 1,
        ok: false,
        code: err.status().code(),
        code_name: err.status().code_name(),
        message: err.message(),
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
thread_local! {
    static HOST_TEXT_MEASURE_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[derive(Debug, Serialize)]
struct WasmHostTextMeasureRequest<'a> {
    text: &'a str,
    font_family: Option<&'a str>,
    font_size: f64,
    font_weight: Option<&'a str>,
    font_style: &'static str,
    max_width: Option<f64>,
    has_max_width: bool,
    line_height: f64,
    letter_spacing: f64,
    word_spacing: f64,
    wrap_mode: &'static str,
    direction: &'static str,
    white_space: &'static str,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct WasmHostTextMeasureResult {
    handled: Option<bool>,
    width: f64,
    height: f64,
    line_count: Option<usize>,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[derive(Default)]
struct WasmHostTextMeasurer {
    fallback: merman_bindings_core::VendoredFontMetricsTextMeasurer,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl WasmHostTextMeasurer {
    fn call_host(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        let request = WasmHostTextMeasureRequest {
            text,
            font_family: style.font_family.as_deref(),
            font_size: style.font_size,
            font_weight: style.font_weight.as_deref(),
            font_style: "normal",
            max_width,
            has_max_width: max_width.is_some(),
            line_height: wasm_line_height(style, wrap_mode),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            wrap_mode: wasm_wrap_mode(wrap_mode),
            direction: "auto",
            white_space: wasm_white_space(max_width, wrap_mode),
        };
        let request = serde_wasm_bindgen::to_value(&request).ok()?;

        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            let callback = slot.borrow().clone()?;
            let value = callback.call1(&JsValue::NULL, &request).ok()?;
            if value.is_null() || value.is_undefined() {
                return None;
            }

            let result: WasmHostTextMeasureResult = serde_wasm_bindgen::from_value(value).ok()?;
            if result.handled == Some(false)
                || !result.width.is_finite()
                || !result.height.is_finite()
                || result.width < 0.0
                || result.height < 0.0
            {
                return None;
            }

            let line_count = result.line_count.unwrap_or(1);
            if line_count == 0 {
                return None;
            }

            Some(TextMetrics {
                width: result.width,
                height: result.height,
                line_count,
            })
        })
    }

    fn measure_with_fallback(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.call_host(text, style, max_width, wrap_mode)
            .unwrap_or_else(|| {
                self.fallback
                    .measure_wrapped(text, style, max_width, wrap_mode)
            })
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl TextMeasurer for WasmHostTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.call_host(text, style, None, WrapMode::SvgLike)
            .unwrap_or_else(|| self.fallback.measure(text, style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.measure_with_fallback(text, style, max_width, wrap_mode)
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        if let Some(metrics) = self.call_host(text, style, max_width, wrap_mode) {
            let raw_width = max_width
                .and_then(|_| self.call_host(text, style, None, wrap_mode))
                .map(|raw| raw.width);
            return (metrics, raw_width);
        }
        self.fallback
            .measure_wrapped_with_raw_width(text, style, max_width, wrap_mode)
    }

    fn measure_wrapped_raw(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.call_host(text, style, max_width, wrap_mode)
            .unwrap_or_else(|| {
                self.fallback
                    .measure_wrapped_raw(text, style, max_width, wrap_mode)
            })
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
struct HostTextMeasureCallbackGuard(Option<js_sys::Function>);

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl Drop for HostTextMeasureCallbackGuard {
    fn drop(&mut self) {
        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn with_host_text_measure_callback<R>(callback: js_sys::Function, f: impl FnOnce() -> R) -> R {
    let previous = HOST_TEXT_MEASURE_CALLBACK.with(|slot| slot.replace(Some(callback)));
    let _guard = HostTextMeasureCallbackGuard(previous);
    f()
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn wasm_wrap_mode(wrap_mode: WrapMode) -> &'static str {
    match wrap_mode {
        WrapMode::SvgLike => "svg-like",
        WrapMode::SvgLikeSingleRun => "svg-like-single-run",
        WrapMode::HtmlLike => "html-like",
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn wasm_line_height(style: &TextStyle, wrap_mode: WrapMode) -> f64 {
    let factor = match wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        WrapMode::HtmlLike => 1.5,
    };
    style.font_size.max(1.0) * factor
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn wasm_white_space(max_width: Option<f64>, wrap_mode: WrapMode) -> &'static str {
    match wrap_mode {
        WrapMode::HtmlLike if max_width.is_some() => "break-spaces",
        WrapMode::HtmlLike => "nowrap",
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => "normal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn render_svg_impl_returns_svg() {
        let result = merman_bindings_core::render_svg(b"flowchart TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "render") {
            let svg = string_result(result).unwrap();
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Hello"));
        } else {
            let error = result.unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::UnsupportedFormat
            );
        }
    }

    #[test]
    fn validation_error_uses_binding_status() {
        let json: Value =
            serde_json::from_slice(&merman_bindings_core::validate_json(b"", b"").unwrap())
                .unwrap();

        assert_eq!(json["valid"], false);
        if cfg!(feature = "render") {
            assert_eq!(json["code_name"], "MERMAN_NO_DIAGRAM");
            assert!(
                json["error"]
                    .as_str()
                    .unwrap()
                    .contains("no Mermaid diagram")
            );
        } else {
            assert_eq!(json["code_name"], "MERMAN_UNSUPPORTED_FORMAT");
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn analyze_json_exposes_diagnostics_payload() {
        let value: Value = serde_wasm_bindgen::from_value(analyze_json("", None).unwrap()).unwrap();
        assert_no_diagram_analysis_payload(&value);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn analyze_json_exposes_diagnostics_payload() {
        let value: Value =
            serde_json::from_slice(&merman_bindings_core::analyze_json(b"", b"").unwrap()).unwrap();
        assert_no_diagram_analysis_payload(&value);
    }

    fn assert_no_diagram_analysis_payload(value: &Value) {
        assert_eq!(value["version"], 1);
        assert_eq!(value["valid"], false);
        assert_eq!(value["diagnostics"][0]["code_name"], "MERMAN_NO_DIAGRAM");
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn analyze_document_exposes_markdown_diagnostics_payload() {
        let value: Value = serde_wasm_bindgen::from_value(
            analyze_document(
                "before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
                None,
                Some("file:///tmp/example.md".to_string()),
            )
            .unwrap(),
        )
        .unwrap();
        assert_markdown_document_analysis_payload(&value);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn analyze_document_exposes_markdown_diagnostics_payload() {
        let value: Value = serde_json::from_slice(
            &merman_bindings_core::analyze_document_json(
                b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
                b"",
                b"file:///tmp/example.md",
            )
            .unwrap(),
        )
        .unwrap();
        assert_markdown_document_analysis_payload(&value);
    }

    fn assert_markdown_document_analysis_payload(value: &Value) {
        assert_eq!(value["valid"], false);
        assert_eq!(value["source"]["kind"], "markdown");
        assert_eq!(value["diagnostics"][0]["span"]["line"], 4);
        assert!(
            value["diagnostics"][0]["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|related| related["message"] == "Mermaid fence 1")
        );
    }

    #[test]
    fn wasm_error_payload_is_structured() {
        let err = merman_bindings_core::render_svg(b"flowchart TD\nA", b"{").unwrap_err();
        let json = serde_json::to_value(wasm_error_payload(&err)).unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], false);
        if cfg!(feature = "render") {
            assert_eq!(json["code_name"], "MERMAN_OPTIONS_JSON_ERROR");
            assert!(json["message"].as_str().unwrap().contains("options_json"));
        } else {
            assert_eq!(json["code_name"], "MERMAN_UNSUPPORTED_FORMAT");
        }
    }

    #[test]
    fn binding_capabilities_follow_features() {
        let capabilities = merman_bindings_core::binding_capabilities();

        assert_eq!(capabilities.render, cfg!(feature = "render"));
        assert_eq!(capabilities.ascii, cfg!(feature = "ascii"));
        assert_eq!(
            capabilities.core_full,
            cfg!(feature = "core-full") || cfg!(feature = "ascii")
        );
        assert_eq!(
            capabilities.core_host,
            cfg!(feature = "core-host") || cfg!(feature = "ascii")
        );
        assert_eq!(capabilities.elk_layout, cfg!(feature = "elk-layout"));
        assert_eq!(capabilities.ratex_math, cfg!(feature = "ratex-math"));
        assert_eq!(
            capabilities.editor_language,
            cfg!(feature = "editor-language")
        );
    }

    #[cfg(feature = "editor-language")]
    #[test]
    fn editor_language_helpers_cover_browser_editor_surface() {
        let completion_snapshot = editor_snapshot(
            "flowchart TD\nA-->B\nC-->\n",
            Some("file:///tmp/example.mmd".to_string()),
        );
        let completions = completion_for_snapshot(&completion_snapshot, Position::new(2, 4));
        assert!(completions.items.iter().any(|item| item.label == "B"));

        let reference_snapshot = editor_snapshot(
            "flowchart TD\nA-->B\nA-->C\n",
            Some("file:///tmp/example.mmd".to_string()),
        );
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
    fn registry_profile_and_family_capabilities_are_exposed() {
        let expected_profile = if cfg!(feature = "core-full") || cfg!(feature = "ascii") {
            "full"
        } else {
            "tiny"
        };
        assert_eq!(selected_registry_profile(), expected_profile);

        let capabilities = merman_bindings_core::diagram_family_capabilities();
        assert!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "flowchart"
                    && capability.has_semantic_parser
                    && capability.has_render_parser)
        );
        assert_eq!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "mindmap"),
            expected_profile == "full"
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn render_ascii_impl_returns_text() {
        let text = string_result(merman_bindings_core::render_ascii(
            b"flowchart TD\nA[Hello] --> B[World]",
            b"",
        ))
        .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}
