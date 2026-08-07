use crate::binding_error_to_js;
#[cfg(test)]
use merman_analysis::AnalysisPayload;
use merman_analysis::{
    AnalysisOptions, AnalysisRejection, Analyzer, EditorSymbolKind, FenceTextIndexSource,
    SourceDescriptor, Summary,
};
use merman_bindings_core::{BindingError, BindingStatus};
use merman_editor_core::{
    DiagramDetectionValidity, DocumentAnalysisContext, DocumentKind, DocumentSnapshot,
    DocumentWorkspace, EditorDiagnostic, EditorDiagramDetection, EditorDocumentSymbol, EditorHover,
    EditorLocation, EditorPrepareRename, EditorTextEdit, EditorWorkspaceEdit, Position, Range,
    RenameError, SemanticTokenDescriptor, analysis_payload_to_diagnostics, code_actions_from_fixes,
    completion_for_snapshot, document_symbols, goto_definition, hover,
    plan_semantic_tokens_for_snapshot, prepare_rename, references, rename, search_document_symbols,
    semantic_token_descriptor,
};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, OnceLock},
};
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmEditorDiagramDetection {
    status: &'static str,
    validity: &'static str,
    diagram_type: Option<String>,
    syntax_id: Option<String>,
    effective_layout_id: Option<String>,
}

impl From<Option<&EditorDiagramDetection>> for WasmEditorDiagramDetection {
    fn from(value: Option<&EditorDiagramDetection>) -> Self {
        match value {
            Some(value) => Self {
                status: "available",
                validity: match value.validity {
                    DiagramDetectionValidity::Valid => "valid",
                    DiagramDetectionValidity::RecoverableInvalid => "recoverable-invalid",
                },
                diagram_type: Some(value.diagram_type.clone()),
                syntax_id: Some(value.syntax_id.clone()),
                effective_layout_id: Some(value.effective_layout_id.clone()),
            },
            None => Self {
                status: "unavailable",
                validity: "unknown",
                diagram_type: None,
                syntax_id: None,
                effective_layout_id: None,
            },
        }
    }
}

#[derive(Debug)]
struct EditorDocumentContext {
    options_json: String,
    analyzed: DocumentAnalysisContext,
}

impl EditorDocumentContext {
    fn matches(&self, source: &str, uri: &str, options_json: &str) -> bool {
        self.analyzed.snapshot().text() == source
            && self.analyzed.snapshot().uri().as_str() == uri
            && self.options_json == options_json
    }
}

#[wasm_bindgen(js_name = EditorSession)]
pub struct WasmEditorSession {
    version: i32,
    context: Arc<EditorDocumentContext>,
}

#[wasm_bindgen(js_class = EditorSession)]
impl WasmEditorSession {
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        version: i32,
        uri: Option<String>,
        options_json: Option<String>,
    ) -> Result<WasmEditorSession, JsValue> {
        validate_editor_session_version(version, None)?;
        let uri = editor_uri(uri);
        let context =
            build_editor_document_context(source, &uri, version, options_json.as_deref())?;
        Ok(Self { version, context })
    }

    #[wasm_bindgen(getter)]
    pub fn version(&self) -> i32 {
        self.version
    }

    #[wasm_bindgen(getter)]
    pub fn uri(&self) -> String {
        self.context.analyzed.snapshot().uri().as_str().to_string()
    }

    pub fn update(&mut self, source: &str, version: i32) -> Result<(), JsValue> {
        validate_editor_session_version(version, Some(self.version))?;
        let uri = self.context.analyzed.snapshot().uri().as_str().to_string();
        let options_json = self.context.options_json.clone();
        self.context = build_editor_document_context(
            source,
            &uri,
            version,
            (!options_json.is_empty()).then_some(options_json.as_str()),
        )?;
        self.version = version;
        Ok(())
    }

    pub fn diagnostics(&self) -> Result<JsValue, JsValue> {
        diagnostics_for_context(&self.context)
    }

    #[wasm_bindgen(js_name = diagramDetection)]
    pub fn diagram_detection(&self) -> Result<JsValue, JsValue> {
        diagram_detection_for_context(&self.context)
    }

    #[wasm_bindgen(js_name = codeActions)]
    pub fn code_actions(&self) -> Result<JsValue, JsValue> {
        code_actions_for_context(&self.context)
    }

    pub fn completions(&self, line: usize, character: usize) -> Result<JsValue, JsValue> {
        completions_for_context(&self.context, line, character)
    }

    pub fn hover(&self, line: usize, character: usize) -> Result<JsValue, JsValue> {
        hover_for_context(&self.context, line, character)
    }

    #[wasm_bindgen(js_name = documentSymbols)]
    pub fn document_symbols(&self) -> Result<JsValue, JsValue> {
        document_symbols_for_context(&self.context)
    }

    #[wasm_bindgen(js_name = searchDocumentSymbols)]
    pub fn search_document_symbols(&self, query: &str) -> Result<JsValue, JsValue> {
        search_document_symbols_for_context(&self.context, query)
    }

    pub fn definition(&self, line: usize, character: usize) -> Result<JsValue, JsValue> {
        definition_for_context(&self.context, line, character)
    }

    pub fn references(
        &self,
        line: usize,
        character: usize,
        include_declaration: bool,
    ) -> Result<JsValue, JsValue> {
        references_for_context(&self.context, line, character, include_declaration)
    }

    #[wasm_bindgen(js_name = prepareRename)]
    pub fn prepare_rename(&self, line: usize, character: usize) -> Result<JsValue, JsValue> {
        prepare_rename_for_context(&self.context, line, character)
    }

    pub fn rename(
        &self,
        line: usize,
        character: usize,
        new_name: &str,
    ) -> Result<JsValue, JsValue> {
        rename_for_context(&self.context, line, character, new_name)
    }

    #[wasm_bindgen(js_name = semanticTokens)]
    pub fn semantic_tokens(&self) -> Result<Vec<u32>, JsValue> {
        semantic_tokens_for_context(&self.context)
    }
}

thread_local! {
    static EDITOR_DOCUMENT_CONTEXT_CACHE: RefCell<Option<Arc<EditorDocumentContext>>> =
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
struct WasmSemanticTokenKindDescriptor {
    id: &'static str,
    code: u32,
    lsp_name: &'static str,
    lsp_index: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSemanticTokenModifierDescriptor {
    id: &'static str,
    index: u32,
    bit: u32,
    lsp_name: &'static str,
    lsp_index: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSemanticTokenPackedDescriptor {
    encoding: &'static str,
    word_width_bits: u32,
    record_width: usize,
    field_order: &'static [&'static str],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSemanticTokenDescriptor {
    schema_version: u32,
    digest: &'static str,
    token_types: Vec<WasmSemanticTokenKindDescriptor>,
    modifiers: Vec<WasmSemanticTokenModifierDescriptor>,
    packed: WasmSemanticTokenPackedDescriptor,
    valid_type_code_max: u32,
    valid_modifier_mask: u32,
}

impl From<&'static SemanticTokenDescriptor> for WasmSemanticTokenDescriptor {
    fn from(value: &'static SemanticTokenDescriptor) -> Self {
        Self {
            schema_version: value.schema_version,
            digest: value.digest,
            token_types: value
                .token_kinds
                .iter()
                .map(|token| WasmSemanticTokenKindDescriptor {
                    id: token.id,
                    code: token.kind.code(),
                    lsp_name: token.lsp_name,
                    lsp_index: token.lsp_index,
                })
                .collect(),
            modifiers: value
                .modifiers
                .iter()
                .map(|modifier| WasmSemanticTokenModifierDescriptor {
                    id: modifier.id,
                    index: modifier.modifier.index(),
                    bit: modifier.bit,
                    lsp_name: modifier.lsp_name,
                    lsp_index: modifier.lsp_index,
                })
                .collect(),
            packed: WasmSemanticTokenPackedDescriptor {
                encoding: value.packed.encoding,
                word_width_bits: value.packed.word_width_bits,
                record_width: value.packed.words_per_token,
                field_order: value.packed.field_order,
            },
            valid_type_code_max: value.valid_type_code_max,
            valid_modifier_mask: value.valid_modifier_mask,
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
    let context = editor_document_context(source, Some(uri), options_json.as_deref())?;
    diagnostics_for_context(&context)
}

fn diagnostics_for_context(context: &EditorDocumentContext) -> Result<JsValue, JsValue> {
    let payload = context.analyzed.payload();
    let response = WasmEditorDiagnostics {
        version: payload.version,
        valid: payload.valid,
        summary: payload.summary,
        source: payload.source.clone(),
        diagnostics: analysis_payload_to_diagnostics(payload),
    };
    js_value(&response)
}

#[wasm_bindgen(js_name = editorDiagramDetection)]
pub fn editor_diagram_detection(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    diagram_detection_for_context(&context)
}

fn diagram_detection_for_context(context: &EditorDocumentContext) -> Result<JsValue, JsValue> {
    js_value(&WasmEditorDiagramDetection::from(
        context.analyzed.detection(),
    ))
}

#[wasm_bindgen(js_name = editorCodeActions)]
pub fn editor_code_actions(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let uri = editor_uri(uri);
    let context = editor_document_context(source, Some(uri.clone()), options_json.as_deref())?;
    code_actions_for_context(&context)
}

fn code_actions_for_context(context: &EditorDocumentContext) -> Result<JsValue, JsValue> {
    let uri = context.analyzed.snapshot().uri().as_str();
    let diagnostics = analysis_payload_to_diagnostics(context.analyzed.payload());
    js_value(&code_actions_for_diagnostics(&diagnostics, uri))
}

#[wasm_bindgen(js_name = editorCompletions)]
pub fn editor_completions(
    source: &str,
    line: usize,
    character: usize,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    completions_for_context(&context, line, character)
}

fn completions_for_context(
    context: &EditorDocumentContext,
    line: usize,
    character: usize,
) -> Result<JsValue, JsValue> {
    js_value(&completion_for_snapshot(
        context.analyzed.snapshot(),
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
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    hover_for_context(&context, line, character)
}

fn hover_for_context(
    context: &EditorDocumentContext,
    line: usize,
    character: usize,
) -> Result<JsValue, JsValue> {
    js_value(
        &hover(context.analyzed.snapshot(), Position::new(line, character)).map(WasmHover::from),
    )
}

#[wasm_bindgen(js_name = editorDocumentSymbols)]
pub fn editor_document_symbols(
    source: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    document_symbols_for_context(&context)
}

fn document_symbols_for_context(context: &EditorDocumentContext) -> Result<JsValue, JsValue> {
    let symbols = document_symbols(context.analyzed.snapshot())
        .into_iter()
        .map(WasmDocumentSymbol::from)
        .collect::<Vec<_>>();
    js_value(&symbols)
}

#[wasm_bindgen(js_name = editorSearchDocumentSymbols)]
pub fn editor_search_document_symbols(
    source: &str,
    query: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    search_document_symbols_for_context(&context, query)
}

fn search_document_symbols_for_context(
    context: &EditorDocumentContext,
    query: &str,
) -> Result<JsValue, JsValue> {
    let symbols = search_document_symbols(context.analyzed.snapshot(), query)
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
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    definition_for_context(&context, line, character)
}

fn definition_for_context(
    context: &EditorDocumentContext,
    line: usize,
    character: usize,
) -> Result<JsValue, JsValue> {
    js_value(
        &goto_definition(context.analyzed.snapshot(), Position::new(line, character))
            .map(WasmLocation::from),
    )
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
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    references_for_context(&context, line, character, include_declaration)
}

fn references_for_context(
    context: &EditorDocumentContext,
    line: usize,
    character: usize,
    include_declaration: bool,
) -> Result<JsValue, JsValue> {
    let locations = references(
        context.analyzed.snapshot(),
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
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    prepare_rename_for_context(&context, line, character)
}

fn prepare_rename_for_context(
    context: &EditorDocumentContext,
    line: usize,
    character: usize,
) -> Result<JsValue, JsValue> {
    js_value(
        &prepare_rename(context.analyzed.snapshot(), Position::new(line, character))
            .map(WasmPrepareRename::from),
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
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    rename_for_context(&context, line, character, new_name)
}

fn rename_for_context(
    context: &EditorDocumentContext,
    line: usize,
    character: usize,
    new_name: &str,
) -> Result<JsValue, JsValue> {
    match rename(
        context.analyzed.snapshot(),
        Position::new(line, character),
        new_name,
    ) {
        Ok(edit) => js_value(&edit.map(WasmWorkspaceEdit::from)),
        Err(err) => Err(rename_error_to_js(err)),
    }
}

#[wasm_bindgen(js_name = editorSemanticTokenDescriptor)]
pub fn editor_semantic_token_descriptor() -> Result<JsValue, JsValue> {
    js_value(&WasmSemanticTokenDescriptor::from(
        semantic_token_descriptor(),
    ))
}

#[wasm_bindgen(js_name = editorSemanticTokens)]
pub fn editor_semantic_tokens(
    source: &str,
    uri: Option<String>,
    options_json: Option<String>,
) -> Result<Vec<u32>, JsValue> {
    let context = editor_document_context(source, uri, options_json.as_deref())?;
    semantic_tokens_for_context(&context)
}

fn semantic_tokens_for_context(context: &EditorDocumentContext) -> Result<Vec<u32>, JsValue> {
    packed_semantic_tokens_for_snapshot(context.analyzed.snapshot())
}

fn packed_semantic_tokens_for_snapshot(snapshot: &DocumentSnapshot) -> Result<Vec<u32>, JsValue> {
    plan_semantic_tokens_for_snapshot(snapshot)
        .map(|plan| plan.packed().to_vec())
        .map_err(|error| {
            binding_error_to_js(BindingError::new(
                BindingStatus::InternalError,
                error.to_string(),
            ))
        })
}

fn js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn editor_uri(uri: Option<String>) -> String {
    uri.filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "file:///merman/document.mmd".to_string())
}

fn validate_editor_session_version(version: i32, previous: Option<i32>) -> Result<(), JsValue> {
    if version <= 0 {
        return Err(binding_error_to_js(BindingError::new(
            BindingStatus::InvalidArgument,
            "editor document version must be positive",
        )));
    }
    if previous.is_some_and(|previous| version <= previous) {
        return Err(binding_error_to_js(BindingError::new(
            BindingStatus::InvalidArgument,
            "editor document version must increase",
        )));
    }
    Ok(())
}

fn document_kind_for_uri(uri: &str) -> DocumentKind {
    DocumentKind::from_path(uri.split(['?', '#']).next().unwrap_or(uri))
}

#[cfg(test)]
fn editor_analysis_payload(
    source: &str,
    options_json: Option<&str>,
    uri: &str,
) -> Result<AnalysisPayload, JsValue> {
    Ok(
        editor_document_context(source, Some(uri.to_string()), options_json)?
            .analyzed
            .payload()
            .clone(),
    )
}

fn editor_document_context(
    source: &str,
    uri: Option<String>,
    options_json: Option<&str>,
) -> Result<Arc<EditorDocumentContext>, JsValue> {
    let uri = editor_uri(uri);
    let options_json_key = editor_options_cache_key(options_json);
    EDITOR_DOCUMENT_CONTEXT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(context) = cache
            .as_ref()
            .filter(|context| context.matches(source, &uri, options_json_key))
        {
            return Ok(Arc::clone(context));
        }

        let context = build_editor_document_context(source, &uri, 1, options_json)?;
        *cache = Some(Arc::clone(&context));
        Ok(context)
    })
}

fn build_editor_document_context(
    source: &str,
    uri: &str,
    version: i32,
    options_json: Option<&str>,
) -> Result<Arc<EditorDocumentContext>, JsValue> {
    let analyzed = build_editor_document_analysis(source, uri, version, options_json)
        .map_err(binding_error_to_js)?;
    Ok(Arc::new(EditorDocumentContext {
        options_json: editor_options_cache_key(options_json).to_string(),
        analyzed,
    }))
}

fn build_editor_document_analysis(
    source: &str,
    uri: &str,
    version: i32,
    options_json: Option<&str>,
) -> Result<DocumentAnalysisContext, BindingError> {
    let options = parse_analysis_options(options_json)?;
    let analyzer = Analyzer::with_options(options);
    let kind = document_kind_for_uri(uri);
    let text = Arc::<str>::from(source);
    record_editor_document_context_build();
    DocumentWorkspace::build_analysis_context_with_shared_text(
        &analyzer,
        uri.to_string(),
        version,
        text,
        kind,
    )
    .into_ready()
    .map_err(editor_rejection_to_binding_error)
}

fn editor_rejection_to_binding_error(rejection: AnalysisRejection) -> BindingError {
    let message = rejection
        .payload()
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| rejection.resource_limit().to_string());
    BindingError::new(BindingStatus::ResourceLimitExceeded, message)
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
    let options_json = options_json
        .filter(|options_json| !options_json.trim().is_empty())
        .unwrap_or_default();
    let ceiling = editor_resource_ceiling()?;
    let normalized = merman_bindings_core::apply_resource_ceiling_json(
        options_json.as_bytes(),
        ceiling.profile_id,
        &[],
    )?;
    let normalized = serde_json::from_slice::<serde_json::Value>(&normalized).map_err(|err| {
        BindingError::new(
            BindingStatus::InternalError,
            format!("failed to decode normalized editor options_json: {err}"),
        )
    })?;
    let max_source_bytes = normalized_editor_max_source_bytes(&normalized, ceiling)?;

    let mut analysis_value = if options_json.is_empty() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str::<serde_json::Value>(options_json).map_err(|err| {
            BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("invalid options_json: {err}"),
            )
        })?
    };
    remove_resource_profile_for_analysis(&mut analysis_value);
    let options = merman_analysis::analysis_options_from_json_value(&analysis_value)
        .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))?;
    Ok(options.with_max_source_bytes(Some(max_source_bytes)))
}

#[derive(Debug, Clone, Copy)]
struct EditorResourceCeiling {
    profile_id: &'static str,
    max_source_bytes: usize,
}

fn editor_resource_ceiling() -> Result<EditorResourceCeiling, BindingError> {
    static CEILING: OnceLock<Option<EditorResourceCeiling>> = OnceLock::new();

    CEILING
        .get_or_init(|| {
            let resources = crate::wasm_runtime_catalog().resources;
            let profile_id = resources.general_binding_default_profile;
            let max_source_bytes = resources
                .profiles
                .iter()
                .find(|profile| profile.id == profile_id)?
                .limits
                .get("max_source_bytes")
                .copied()
                .flatten()?;
            Some(EditorResourceCeiling {
                profile_id,
                max_source_bytes,
            })
        })
        .as_ref()
        .copied()
        .ok_or_else(|| {
            BindingError::new(
                BindingStatus::InternalError,
                "WASM runtime catalog default resource profile must define max_source_bytes",
            )
        })
}

fn normalized_editor_max_source_bytes(
    normalized: &serde_json::Value,
    ceiling: EditorResourceCeiling,
) -> Result<usize, BindingError> {
    let root = normalized.as_object().ok_or_else(|| {
        BindingError::new(
            BindingStatus::InternalError,
            "normalized editor options_json root must be an object",
        )
    })?;
    let options = ["analysis", "merman"]
        .into_iter()
        .find_map(|key| root.get(key).and_then(serde_json::Value::as_object))
        .unwrap_or(root);
    let resources = options
        .get("resources")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            BindingError::new(
                BindingStatus::InternalError,
                "normalized editor options_json omitted resources",
            )
        })?;

    match resources
        .get("limits")
        .and_then(serde_json::Value::as_object)
        .and_then(|limits| limits.get("max_source_bytes"))
    {
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InternalError,
                    "normalized max_source_bytes must fit usize",
                )
            }),
        None => Ok(ceiling.max_source_bytes),
    }
}

fn remove_resource_profile_for_analysis(value: &mut serde_json::Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    remove_resource_profile(root);
    for wrapper in ["analysis", "merman"] {
        if let Some(options) = root
            .get_mut(wrapper)
            .and_then(serde_json::Value::as_object_mut)
        {
            remove_resource_profile(options);
        }
    }
}

fn remove_resource_profile(options: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(resources) = options
        .get_mut("resources")
        .and_then(serde_json::Value::as_object_mut)
    {
        resources.remove("profile");
    }
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
        FenceTextIndexSource::Unavailable => "unavailable",
        FenceTextIndexSource::ParserComplete => "parser_complete",
        FenceTextIndexSource::ParserRecovered => "parser_recovered",
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

    #[test]
    fn fact_source_names_match_parser_contract() {
        assert_eq!(
            fact_source_name(FenceTextIndexSource::Unavailable),
            "unavailable"
        );
        assert_eq!(
            fact_source_name(FenceTextIndexSource::ParserComplete),
            "parser_complete"
        );
        assert_eq!(
            fact_source_name(FenceTextIndexSource::ParserRecovered),
            "parser_recovered"
        );
    }

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
    fn editor_language_enforces_the_web_interactive_source_ceiling_by_default() {
        let ceiling = editor_resource_ceiling().expect("WASM editor resource ceiling");
        assert_eq!(ceiling.profile_id, "interactive");
        assert_eq!(ceiling.max_source_bytes, 2 * 1024 * 1024);

        let options = parse_analysis_options(None).expect("default editor analysis options");
        assert_eq!(options.max_source_bytes(), Some(ceiling.max_source_bytes));

        let source = "x".repeat(ceiling.max_source_bytes + 1);
        let error = build_editor_document_analysis(&source, "file:///tmp/oversized.mmd", 1, None)
            .expect_err("oversized editor source must not create a snapshot");
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(
            error
                .message()
                .contains("exceeding max_source_bytes 2097152")
        );
    }

    #[test]
    fn editor_language_accepts_resource_profiles_and_projects_the_source_limit() {
        let constrained =
            parse_analysis_options(Some(r#"{"resources":{"profile":"constrained"}}"#))
                .expect("constrained editor profile");
        assert_eq!(constrained.max_source_bytes(), Some(1024 * 1024));

        let wrapped = parse_analysis_options(Some(
            r#"{"analysis":{"resources":{"profile":"interactive"}}}"#,
        ))
        .expect("wrapped interactive editor profile");
        assert_eq!(wrapped.max_source_bytes(), Some(2 * 1024 * 1024));
    }

    #[test]
    fn editor_language_resource_limits_can_only_tighten_the_transport_ceiling() {
        let options =
            parse_analysis_options(Some(r#"{"resources":{"limits":{"max_source_bytes":32}}}"#))
                .expect("stricter editor source limit");
        assert_eq!(options.max_source_bytes(), Some(32));

        let error = build_editor_document_analysis(
            "flowchart TD\nA-->B\nA-->C\n",
            "file:///tmp/tightened.mmd",
            1,
            Some(r#"{"resources":{"limits":{"max_source_bytes":16}}}"#),
        )
        .expect_err("tightened source limit must reject the editor generation");
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains("exceeding max_source_bytes 16"));

        let error = parse_analysis_options(Some(
            r#"{"resources":{"limits":{"max_source_bytes":2097153}}}"#,
        ))
        .expect_err("editor options must not loosen the Web ceiling");
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("loosen the transport ceiling"));
    }

    #[test]
    fn editor_language_rejects_unknown_or_looser_resource_profiles() {
        let unknown = parse_analysis_options(Some(r#"{"resources":{"profile":"future-profile"}}"#))
            .expect_err("unknown editor resource profile");
        assert_eq!(unknown.status(), BindingStatus::InvalidArgument);
        assert!(unknown.message().contains("unsupported resources.profile"));

        for profile in ["trusted-native", "unbounded-for-trusted-input"] {
            let options = format!(r#"{{"resources":{{"profile":"{profile}"}}}}"#);
            let error =
                parse_analysis_options(Some(&options)).expect_err("looser editor resource profile");
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("loosen the transport ceiling"));
        }
    }

    #[test]
    fn editor_language_helpers_cover_browser_editor_surface() {
        reset_editor_document_context_cache_for_tests();

        let completion_context = editor_document_context(
            "flowchart TD\nA-->B\nC-->\n",
            Some("file:///tmp/example.mmd".to_string()),
            None,
        )
        .unwrap();
        let completions =
            completion_for_snapshot(completion_context.analyzed.snapshot(), Position::new(2, 4));
        assert!(completions.items.iter().any(|item| item.label == "B"));

        let reference_context = editor_document_context(
            "flowchart TD\nA-->B\nA-->C\n",
            Some("file:///tmp/example.mmd".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(
            references(
                reference_context.analyzed.snapshot(),
                Position::new(1, 0),
                true,
            )
            .unwrap()
            .len(),
            2
        );
        assert!(
            !plan_semantic_tokens_for_snapshot(reference_context.analyzed.snapshot())
                .unwrap()
                .packed()
                .is_empty()
        );

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

        let context = editor_document_context(source, Some(uri.to_string()), None).unwrap();
        assert_eq!(editor_document_context_builds_for_tests(), 1);
        assert!(
            !plan_semantic_tokens_for_snapshot(context.analyzed.snapshot())
                .unwrap()
                .packed()
                .is_empty()
        );

        let repeated_payload = editor_analysis_payload(source, Some(" \n "), uri).unwrap();
        assert_eq!(repeated_payload, payload);
        assert_eq!(editor_document_context_builds_for_tests(), 1);
    }

    #[test]
    fn one_entry_cache_shares_and_releases_analysis_generations() {
        reset_editor_document_context_cache_for_tests();
        let source = "flowchart TD\nA-->B\n";
        let uri = "file:///tmp/cache.mmd";

        let first = editor_document_context(source, Some(uri.to_string()), None).unwrap();
        let repeated = editor_document_context(source, Some(uri.to_string()), None).unwrap();
        let first_generation = first.analyzed.shared_analysis_generation();
        let repeated_generation = repeated.analyzed.shared_analysis_generation();
        let displaced = Arc::downgrade(&first_generation);

        assert!(Arc::ptr_eq(&first_generation, &repeated_generation));
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        let replacement = editor_document_context(source, Some(uri.to_string()), Some("{}"))
            .expect("options mismatch replaces the one-entry cache");
        let replacement_generation = replacement.analyzed.shared_analysis_generation();
        assert!(!Arc::ptr_eq(&replacement_generation, &first_generation));
        assert_eq!(editor_document_context_builds_for_tests(), 2);

        drop(first_generation);
        drop(repeated_generation);
        drop(first);
        drop(repeated);
        assert!(displaced.upgrade().is_none());
    }

    #[test]
    fn editor_language_context_invalidates_on_source_or_uri_change() {
        reset_editor_document_context_cache_for_tests();

        let uri = "file:///tmp/example.mmd";
        let source = "flowchart TD\nA-->B\n";
        let updated_source = "flowchart TD\nA-->C\n";

        let first = editor_document_context(source, Some(uri.to_string()), None).unwrap();
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        let repeated = editor_document_context(source, Some(uri.to_string()), None).unwrap();
        assert!(Arc::ptr_eq(&repeated, &first));
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        let updated = editor_document_context(updated_source, Some(uri.to_string()), None).unwrap();
        assert!(!Arc::ptr_eq(&updated, &first));
        assert_eq!(updated.analyzed.snapshot().text(), updated_source);
        assert_eq!(editor_document_context_builds_for_tests(), 2);

        let other_uri = "file:///tmp/other.mmd";
        let other_document =
            editor_document_context(updated_source, Some(other_uri.to_string()), None)
                .expect("uri change rebuilds cached context");
        assert_eq!(other_document.analyzed.snapshot().uri().as_str(), other_uri);
        assert!(!Arc::ptr_eq(&other_document, &updated));
        assert_eq!(editor_document_context_builds_for_tests(), 3);
    }

    #[test]
    fn editor_session_moves_source_transfer_to_open_and_change() {
        reset_editor_document_context_cache_for_tests();
        let uri = "file:///tmp/session.mmd";
        let mut session =
            WasmEditorSession::new("flowchart TD\nA-->B\n", 1, Some(uri.to_string()), None)
                .expect("open editor session");

        assert_eq!(session.version(), 1);
        assert_eq!(session.uri(), uri);
        assert_eq!(editor_document_context_builds_for_tests(), 1);
        assert!(
            !plan_semantic_tokens_for_snapshot(session.context.analyzed.snapshot())
                .expect("session semantic tokens")
                .packed()
                .is_empty()
        );
        assert_eq!(editor_document_context_builds_for_tests(), 1);

        session
            .update("flowchart TD\nA-->C\n", 2)
            .expect("change editor session");
        assert_eq!(session.version(), 2);
        assert_eq!(
            session.context.analyzed.snapshot().text(),
            "flowchart TD\nA-->C\n"
        );
        assert_eq!(editor_document_context_builds_for_tests(), 2);
    }

    #[test]
    fn wasm_semantic_tokens_are_the_exact_planner_packed_sequence() {
        let context = editor_document_context(
            "flowchart TD\nAlpha-->Beta\nAlpha-->Gamma\n",
            Some("file:///tmp/example.mmd".to_string()),
            None,
        )
        .expect("editor snapshot");
        let snapshot = context.analyzed.snapshot();
        let expected = merman_editor_core::plan_semantic_tokens_for_snapshot(snapshot)
            .expect("validated token plan");

        let actual =
            packed_semantic_tokens_for_snapshot(snapshot).expect("WASM semantic token transport");

        assert_eq!(actual, expected.packed());
        assert_eq!(
            actual.len() % merman_editor_core::SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
            0
        );
    }

    #[test]
    fn incomplete_flowchart_detection_comes_from_the_shared_analyzed_snapshot() {
        reset_editor_document_context_cache_for_tests();
        let context = editor_document_context(
            "flowchart TD\nA[unterminated\n",
            Some("file:///tmp/incomplete.mmd".to_string()),
            None,
        )
        .expect("shared editor analysis");
        let detection = context
            .analyzed
            .detection()
            .expect("recoverable diagram detection");

        assert_eq!(
            detection.validity,
            DiagramDetectionValidity::RecoverableInvalid
        );
        assert_eq!(detection.diagram_type, "flowchart");
        assert_eq!(detection.syntax_id, "flowchart-v2");
        assert_eq!(detection.effective_layout_id, "dagre");
        assert_eq!(editor_document_context_builds_for_tests(), 1);
    }

    #[test]
    fn wasm_detection_projection_is_independent_from_diagnostic_severity() {
        let cases = [
            (
                concat!(
                    "cynefin-beta\n",
                    "  complex\n",
                    "  complicated\n",
                    "  complicated --> complicated : \"Self-loop\"\n",
                ),
                "merman.parse.recovered_editor_facts",
                "error",
                "valid",
            ),
            (
                "flowchart TD\nA[unterminated\n",
                "merman.parse.diagram_parse",
                "hint",
                "recoverable-invalid",
            ),
        ];

        for (source, rule_id, severity, expected_validity) in cases {
            reset_editor_document_context_cache_for_tests();
            let options = serde_json::json!({
                "lint": {
                    "rule_severities": [{
                        "rule_id": rule_id,
                        "severity": severity
                    }]
                }
            })
            .to_string();
            let context = editor_document_context(
                source,
                Some(format!("file:///tmp/{severity}.mmd")),
                Some(&options),
            )
            .expect("shared editor analysis");

            let projected = WasmEditorDiagramDetection::from(context.analyzed.detection());
            assert_eq!(projected.status, "available");
            assert_eq!(projected.validity, expected_validity);
        }
    }

    #[test]
    fn all_family_editor_queries_share_one_analyzed_snapshot() {
        let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../playground/examples/manifest.json"))
                .expect("playground example manifest");
        let baselines = manifest["examples"]
            .as_array()
            .expect("example list")
            .iter()
            .filter(|example| example["evidence"]["role"] == "family-baseline")
            .collect::<Vec<_>>();
        assert_eq!(baselines.len(), 35);

        for example in baselines {
            let family = example["diagramType"].as_str().expect("diagram family");
            let fixture = example["fixture"].as_str().expect("fixture path");
            let source = std::fs::read_to_string(repository_root.join(fixture))
                .unwrap_or_else(|error| panic!("read {family} fixture {fixture}: {error}"));
            let uri = format!("file:///tmp/{family}.mmd");
            reset_editor_document_context_cache_for_tests();

            let context = editor_document_context(&source, Some(uri.clone()), None)
                .unwrap_or_else(|_| panic!("analyze {family}"));
            let detection = context
                .analyzed
                .detection()
                .unwrap_or_else(|| panic!("detect {family}"));
            assert_eq!(detection.diagram_type, family, "{family} detection");
            let diagnostics = analysis_payload_to_diagnostics(context.analyzed.payload());
            let _ = code_actions_for_diagnostics(&diagnostics, &uri);
            let snapshot = context.analyzed.snapshot();
            let header_character = source
                .lines()
                .next()
                .unwrap_or_default()
                .encode_utf16()
                .count();
            let _ = completion_for_snapshot(snapshot, Position::new(0, header_character));
            let _ = hover(snapshot, Position::new(0, 0));
            let _ = document_symbols(snapshot);
            let _ = prepare_rename(snapshot, Position::new(0, 0));
            let _ = rename(snapshot, Position::new(0, 0), "RenamedNode");
            let plan = plan_semantic_tokens_for_snapshot(snapshot)
                .unwrap_or_else(|error| panic!("plan {family} tokens: {error}"));
            assert!(!plan.packed().is_empty(), "{family} packed tokens");
            assert_eq!(
                editor_document_context_builds_for_tests(),
                1,
                "{family} editor capabilities rebuilt the analyzed snapshot"
            );
        }
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

    #[test]
    fn wasm_code_actions_include_frontmatter_config_migration_quickfix() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let uri = "file:///tmp/example.mmd";
        let payload =
            editor_analysis_payload(source, Some(r#"{"lint":{"profile":"recommended"}}"#), uri)
                .expect("analysis payload");
        let diagnostics = analysis_payload_to_diagnostics(&payload);
        let actions = code_actions_for_diagnostics(&diagnostics, uri);

        let action = actions
            .iter()
            .find(|action| action.title == "Move init directive config into frontmatter")
            .expect("frontmatter migration action");
        assert!(action.is_preferred);
        let edits = action.edit.changes.get(uri).expect("uri edits");
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.starts_with("---\nconfig:\n"));
        assert!(edits[0].new_text.contains("theme: dark\n"));
        assert_eq!(edits[0].range.start, Position::new(0, 0));
        assert_eq!(edits[0].range.end, Position::new(1, 0));
    }
}
