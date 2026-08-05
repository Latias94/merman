import { isDiagramType } from "@mermanjs/web";
import type {
  AnalysisDiagnosticFix,
  AnalysisDiagnosticFixEdit,
  AnalysisSourceKind,
  AnalysisSpan,
  EditorCodeAction,
  EditorCompletionItem,
  EditorCompletionItemKind,
  EditorCompletionList,
  EditorCompletionResolveData,
  EditorDiagnostic,
  EditorDiagnosticData,
  EditorDiagnosticRelated,
  EditorDiagnosticSeverity,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorRange,
  EditorSemanticFactSource,
  EditorSemanticTokenLegend,
  EditorSymbolKind,
  EditorTextEdit,
  EditorWorkspaceEdit,
  DiagramDetectionFacts,
} from "@mermanjs/web";

export const EDITOR_WORKER_PROTOCOL = 3 as const;
export const EDITOR_SCHEMA_VERSION = 1 as const;

export type EditorWorkerErrorCode =
  | "INITIALIZATION_FAILED"
  | "INVALID_STATE"
  | "OPERATION_REJECTED"
  | "PROTOCOL_MISMATCH"
  | "QUERY_FAILED"
  | "STALE_DOCUMENT";

export interface EditorDocumentIdentity {
  readonly uri: string;
  readonly version: number;
}

export interface EditorDocumentSnapshot extends EditorDocumentIdentity {
  readonly source: string;
}

export type EditorWorkerQuery =
  | { readonly kind: "diagnostics" }
  | { readonly kind: "diagramDetection" }
  | { readonly kind: "codeActions" }
  | {
      readonly kind: "completions";
      readonly position: EditorPosition;
    }
  | {
      readonly kind: "hover";
      readonly position: EditorPosition;
    }
  | { readonly kind: "documentSymbols" }
  | {
      readonly kind: "definition";
      readonly position: EditorPosition;
    }
  | {
      readonly kind: "references";
      readonly position: EditorPosition;
      readonly includeDeclaration: boolean;
    }
  | {
      readonly kind: "prepareRename";
      readonly position: EditorPosition;
    }
  | {
      readonly kind: "rename";
      readonly position: EditorPosition;
      readonly newName: string;
    }
  | { readonly kind: "semanticTokens" };

export interface EditorWorkerQueryResults {
  diagnostics: EditorDiagnosticsResult;
  diagramDetection: DiagramDetectionFacts;
  codeActions: EditorCodeAction[];
  completions: EditorCompletionList;
  hover: EditorHover | null;
  documentSymbols: EditorDocumentSymbol[];
  definition: EditorLocation | null;
  references: EditorLocation[];
  prepareRename: EditorPrepareRename | null;
  rename: EditorWorkspaceEdit | null;
  semanticTokens: Uint32Array;
}

export type EditorWorkerQueryResult<Query extends EditorWorkerQuery> =
  EditorWorkerQueryResults[Query["kind"]];

interface EditorWorkerRequestBase {
  readonly protocol: typeof EDITOR_WORKER_PROTOCOL;
  readonly requestId: number;
}

export type EditorWorkerRequest =
  | (EditorWorkerRequestBase & { readonly type: "initialize" })
  | (EditorWorkerRequestBase & {
      readonly type: "didOpen";
      readonly document: EditorDocumentSnapshot;
    })
  | (EditorWorkerRequestBase & {
      readonly type: "didChange";
      readonly document: EditorDocumentSnapshot;
    })
  | (EditorWorkerRequestBase & {
      readonly type: "query";
      readonly uri: string;
      readonly version: number;
      readonly legendDigest: string;
      readonly query: EditorWorkerQuery;
    })
  | {
      readonly protocol: typeof EDITOR_WORKER_PROTOCOL;
      readonly type: "dispose";
    };

interface EditorWorkerResponseBase {
  readonly protocol: typeof EDITOR_WORKER_PROTOCOL;
  readonly requestId: number;
}

export interface EditorWorkerReadyResponse extends EditorWorkerResponseBase {
  readonly type: "ready";
  readonly transportApiVersion: number;
  readonly editorSchema: typeof EDITOR_SCHEMA_VERSION;
  readonly legendDigest: string;
  readonly legend: EditorSemanticTokenLegend;
}

export interface EditorWorkerSyncResponse extends EditorWorkerResponseBase {
  readonly type: "result";
  readonly result: null;
}

export interface EditorWorkerRawQueryResponse extends EditorWorkerResponseBase {
  readonly type: "queryResult";
  readonly uri: string;
  readonly version: number;
  readonly legendDigest: string;
  readonly result: unknown;
}

export interface EditorWorkerErrorResponse extends EditorWorkerResponseBase {
  readonly type: "error";
  readonly code: EditorWorkerErrorCode;
  readonly message: string;
  readonly detail: string | null;
  readonly nativeCode: string | null;
}

export type EditorWorkerResponse =
  | EditorWorkerErrorResponse
  | EditorWorkerRawQueryResponse
  | EditorWorkerReadyResponse
  | EditorWorkerSyncResponse;

export class EditorWorkerProtocolProjectionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "EditorWorkerProtocolProjectionError";
  }
}

interface ObjectSchema {
  readonly allowed: ReadonlySet<string>;
  readonly required: readonly string[];
}

const INITIALIZE_REQUEST_SCHEMA = schema(["protocol", "requestId", "type"]);
const DOCUMENT_REQUEST_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "document",
]);
const QUERY_REQUEST_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "uri",
  "version",
  "legendDigest",
  "query",
]);
const DISPOSE_REQUEST_SCHEMA = schema(["protocol", "type"]);
const DOCUMENT_IDENTITY_SCHEMA = schema(["uri", "version"]);
const DOCUMENT_SCHEMA = schema(["uri", "version", "source"]);
const POSITION_SCHEMA = schema(["line", "character"]);
const READY_RESPONSE_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "transportApiVersion",
  "editorSchema",
  "legendDigest",
  "legend",
]);
const SYNC_RESPONSE_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "result",
]);
const QUERY_RESPONSE_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "uri",
  "version",
  "legendDigest",
  "result",
]);
const ERROR_RESPONSE_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "code",
  "message",
  "detail",
  "nativeCode",
]);

const NO_ARGUMENT_QUERY_SCHEMA = schema(["kind"]);
const POSITION_QUERY_SCHEMA = schema(["kind", "position"]);
const REFERENCES_QUERY_SCHEMA = schema([
  "kind",
  "position",
  "includeDeclaration",
]);
const RENAME_QUERY_SCHEMA = schema(["kind", "position", "newName"]);

const WORKER_ERROR_CODES = new Set<EditorWorkerErrorCode>([
  "INITIALIZATION_FAILED",
  "INVALID_STATE",
  "OPERATION_REJECTED",
  "PROTOCOL_MISMATCH",
  "QUERY_FAILED",
  "STALE_DOCUMENT",
]);
const SEMANTIC_FACT_SOURCES = new Set<EditorSemanticFactSource>([
  "unavailable",
  "parser_complete",
  "parser_recovered",
]);
const DIAGNOSTIC_SEVERITIES = new Set<EditorDiagnosticSeverity>([
  "error",
  "warning",
  "info",
  "hint",
]);
const COMPLETION_ITEM_KINDS = new Set<EditorCompletionItemKind>([
  "keyword",
  "variable",
  "class",
  "snippet",
]);
const COMPLETION_RESOLVE_KINDS = new Set<EditorCompletionResolveData["kind"]>([
  "diagram_header",
  "operator",
  "direction",
  "directive",
  "shape",
  "class_name",
  "node_identifier",
  "style",
  "interaction",
  "frontmatter",
  "template",
]);
const SYMBOL_KINDS = new Set<EditorSymbolKind>([
  "class",
  "event",
  "function",
  "module",
  "namespace",
  "object",
  "package",
  "property",
  "string",
  "struct",
  "variable",
]);
const ANALYSIS_SOURCE_KINDS = new Set<AnalysisSourceKind>([
  "diagram",
  "markdown",
  "mdx",
]);
const COMPLETION_INSERT_TEXT_FORMATS = new Set<
  NonNullable<EditorCompletionItem["insert_text_format"]>
>(["plain_text", "snippet"]);

export function projectEditorWorkerRequest(
  value: unknown,
): EditorWorkerRequest {
  const request = expectRecord(value, "editor worker request");
  if (request.protocol !== EDITOR_WORKER_PROTOCOL) {
    fail("Editor worker request protocol is invalid.");
  }

  switch (request.type) {
    case "initialize":
      assertSchema(request, INITIALIZE_REQUEST_SCHEMA, "initialize request");
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: expectRequestId(request.requestId),
        type: "initialize",
      };
    case "didOpen":
    case "didChange": {
      assertSchema(request, DOCUMENT_REQUEST_SCHEMA, `${request.type} request`);
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: expectRequestId(request.requestId),
        type: request.type,
        document: projectEditorDocumentSnapshot(request.document),
      };
    }
    case "query":
      assertSchema(request, QUERY_REQUEST_SCHEMA, "query request");
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: expectRequestId(request.requestId),
        type: "query",
        uri: expectNonEmptyString(request.uri, "query URI"),
        version: expectPositiveSafeInteger(request.version, "query version"),
        legendDigest: expectNonEmptyString(
          request.legendDigest,
          "query legend digest",
        ),
        query: projectEditorWorkerQuery(request.query),
      };
    case "dispose":
      assertSchema(request, DISPOSE_REQUEST_SCHEMA, "dispose request");
      return { protocol: EDITOR_WORKER_PROTOCOL, type: "dispose" };
    default:
      fail("Editor worker request type is invalid.");
  }
}

export function projectEditorDocumentSnapshot(
  value: unknown,
): EditorDocumentSnapshot {
  const document = expectRecord(value, "editor document snapshot");
  assertSchema(document, DOCUMENT_SCHEMA, "editor document snapshot");
  return {
    ...projectEditorDocumentIdentityFields(document),
    source: expectString(document.source, "document source"),
  };
}

export function projectEditorDocumentIdentity(
  value: unknown,
): EditorDocumentIdentity {
  const document = expectRecord(value, "editor document identity");
  assertSchema(document, DOCUMENT_IDENTITY_SCHEMA, "editor document identity");
  return projectEditorDocumentIdentityFields(document);
}

function projectEditorDocumentIdentityFields(
  document: Record<string, unknown>,
): EditorDocumentIdentity {
  return {
    uri: expectNonEmptyString(document.uri, "document URI"),
    version: expectPositiveSafeInteger(document.version, "document version"),
  };
}

export function projectEditorWorkerQuery(value: unknown): EditorWorkerQuery {
  const query = expectRecord(value, "editor worker query");
  switch (query.kind) {
    case "codeActions":
    case "diagnostics":
    case "diagramDetection":
    case "documentSymbols":
    case "semanticTokens":
      assertSchema(query, NO_ARGUMENT_QUERY_SCHEMA, `${query.kind} query`);
      return { kind: query.kind };
    case "completions":
    case "definition":
    case "hover":
    case "prepareRename":
      assertSchema(query, POSITION_QUERY_SCHEMA, `${query.kind} query`);
      return {
        kind: query.kind,
        position: projectRequestPosition(query.position),
      };
    case "references":
      assertSchema(query, REFERENCES_QUERY_SCHEMA, "references query");
      return {
        kind: "references",
        position: projectRequestPosition(query.position),
        includeDeclaration: expectBoolean(
          query.includeDeclaration,
          "references includeDeclaration",
        ),
      };
    case "rename":
      assertSchema(query, RENAME_QUERY_SCHEMA, "rename query");
      return {
        kind: "rename",
        position: projectRequestPosition(query.position),
        newName: expectString(query.newName, "rename newName"),
      };
    default:
      fail("Editor worker query kind is invalid.");
  }
}

export function projectEditorWorkerResponse(
  value: unknown,
): EditorWorkerResponse {
  const response = expectRecord(value, "editor worker response");
  if (response.protocol !== EDITOR_WORKER_PROTOCOL) {
    fail("Editor worker response protocol is invalid.");
  }
  const requestId = expectRequestId(response.requestId);

  switch (response.type) {
    case "ready":
      assertSchema(response, READY_RESPONSE_SCHEMA, "ready response");
      if (response.editorSchema !== EDITOR_SCHEMA_VERSION) {
        fail("Editor worker schema version is invalid.");
      }
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "ready",
        transportApiVersion: expectPositiveSafeInteger(
          response.transportApiVersion,
          "transport API version",
        ),
        editorSchema: EDITOR_SCHEMA_VERSION,
        legendDigest: expectNonEmptyString(
          response.legendDigest,
          "legend digest",
        ),
        legend: projectEditorSemanticTokenLegend(response.legend),
      };
    case "result":
      assertSchema(response, SYNC_RESPONSE_SCHEMA, "synchronization response");
      if (response.result !== null) {
        fail("Editor worker synchronization result must be null.");
      }
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "result",
        result: null,
      };
    case "queryResult":
      assertSchema(response, QUERY_RESPONSE_SCHEMA, "query response");
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "queryResult",
        uri: expectNonEmptyString(response.uri, "query response URI"),
        version: expectPositiveSafeInteger(
          response.version,
          "query response version",
        ),
        legendDigest: expectNonEmptyString(
          response.legendDigest,
          "query response legend digest",
        ),
        result: response.result,
      };
    case "error":
      assertSchema(response, ERROR_RESPONSE_SCHEMA, "error response");
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "error",
        code: expectSetValue(
          response.code,
          WORKER_ERROR_CODES,
          "worker error code",
        ),
        message: expectString(response.message, "worker error message"),
        detail: expectNullableString(response.detail, "worker error detail"),
        nativeCode: expectNullableString(
          response.nativeCode,
          "native error code",
        ),
      };
    default:
      fail("Editor worker response type is invalid.");
  }
}

export function projectEditorWorkerQueryResult<Query extends EditorWorkerQuery>(
  query: Query,
  value: unknown,
): EditorWorkerQueryResult<Query> {
  let result: EditorWorkerQueryResults[EditorWorkerQuery["kind"]];
  switch (query.kind) {
    case "diagnostics":
      result = projectDiagnostics(value);
      break;
    case "diagramDetection":
      result = projectDiagramDetection(value);
      break;
    case "codeActions":
      result = projectArray(value, "code actions", projectCodeAction);
      break;
    case "completions":
      result = projectCompletionList(value);
      break;
    case "hover":
      result = value === null ? null : projectHover(value);
      break;
    case "documentSymbols":
      result = projectArray(value, "document symbols", projectDocumentSymbol);
      break;
    case "definition":
      result = value === null ? null : projectLocation(value);
      break;
    case "references":
      result = projectArray(value, "references", projectLocation);
      break;
    case "prepareRename":
      result = value === null ? null : projectPrepareRename(value);
      break;
    case "rename":
      result = value === null ? null : projectWorkspaceEdit(value);
      break;
    case "semanticTokens":
      if (!(value instanceof Uint32Array)) {
        fail("Editor semantic tokens must be a Uint32Array.");
      }
      result = value;
      break;
    default:
      fail("Editor worker query kind is invalid.");
  }
  return result as EditorWorkerQueryResult<Query>;
}

export function projectEditorSemanticTokenLegend(
  value: unknown,
): EditorSemanticTokenLegend {
  const legend = expectRecord(value, "semantic token legend");
  return {
    tokenTypes: projectStringArray(legend.tokenTypes, "semantic token types"),
    tokenModifiers: projectStringArray(
      legend.tokenModifiers,
      "semantic token modifiers",
    ),
  };
}

export function requestIdFromEditorWorkerMessage(value: unknown): number | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  const requestId = (value as Record<string, unknown>).requestId;
  return isPositiveSafeInteger(requestId) ? requestId : null;
}

function projectDiagramDetection(value: unknown): DiagramDetectionFacts {
  const detection = expectRecord(value, "diagram detection result");
  if (detection.status === "unavailable") {
    if (
      detection.validity !== "unknown" ||
      detection.diagramType !== null ||
      detection.syntaxId !== null ||
      detection.effectiveLayoutId !== null
    ) {
      fail("Unavailable diagram detection result is invalid.");
    }
    return {
      status: "unavailable",
      validity: "unknown",
      diagramType: null,
      syntaxId: null,
      effectiveLayoutId: null,
    };
  }
  if (detection.status !== "available") {
    fail("Diagram detection status is invalid.");
  }
  if (
    detection.validity !== "valid" &&
    detection.validity !== "recoverable-invalid"
  ) {
    fail("Diagram detection validity is invalid.");
  }
  const diagramType = expectString(detection.diagramType, "diagram type");
  if (!isDiagramType(diagramType)) {
    fail("Diagram detection type is invalid.");
  }
  return {
    status: "available",
    validity: detection.validity,
    diagramType,
    syntaxId: expectNonBlankString(detection.syntaxId, "syntax ID"),
    effectiveLayoutId: expectNonBlankString(
      detection.effectiveLayoutId,
      "effective layout ID",
    ),
  };
}

function projectDiagnostics(value: unknown): EditorDiagnosticsResult {
  const diagnostics = expectRecord(value, "diagnostics result");
  if (diagnostics.version !== EDITOR_SCHEMA_VERSION) {
    fail("Editor diagnostics schema version is invalid.");
  }
  const summary = expectRecord(diagnostics.summary, "diagnostics summary");
  const source = expectRecord(diagnostics.source, "diagnostics source");
  const kind = expectSetValue(
    source.kind,
    ANALYSIS_SOURCE_KINDS,
    "diagnostics source kind",
  );
  return {
    version: EDITOR_SCHEMA_VERSION,
    valid: expectBoolean(diagnostics.valid, "diagnostics valid"),
    summary: {
      errors: expectNonNegativeSafeInteger(summary.errors, "error count"),
      warnings: expectNonNegativeSafeInteger(summary.warnings, "warning count"),
      infos: expectNonNegativeSafeInteger(summary.infos, "info count"),
      hints: expectNonNegativeSafeInteger(summary.hints, "hint count"),
    },
    source: {
      kind,
      language: expectString(source.language, "diagnostics source language"),
      ...optionalNullableStringProperty(
        source,
        "path",
        "diagnostics source path",
      ),
      ...optionalNullableIntegerProperty(
        source,
        "diagram_index",
        "diagnostics source diagram index",
      ),
    },
    diagnostics: projectArray(
      diagnostics.diagnostics,
      "diagnostics",
      projectDiagnostic,
    ),
  };
}

function projectDiagnostic(value: unknown): EditorDiagnostic {
  const diagnostic = expectRecord(value, "diagnostic");
  const code = diagnostic.code;
  if (typeof code !== "string" && typeof code !== "number") {
    fail("Editor diagnostic code must be a string or number.");
  }
  return {
    range: projectRange(diagnostic.range),
    severity: expectSetValue(
      diagnostic.severity,
      DIAGNOSTIC_SEVERITIES,
      "diagnostic severity",
    ),
    code,
    source: expectString(diagnostic.source, "diagnostic source"),
    message: expectString(diagnostic.message, "diagnostic message"),
    related: projectArray(
      diagnostic.related,
      "diagnostic related information",
      projectDiagnosticRelated,
    ),
    ...optionalNullableProperty(diagnostic, "data", projectDiagnosticData),
  };
}

function projectDiagnosticRelated(value: unknown): EditorDiagnosticRelated {
  const related = expectRecord(value, "diagnostic related information");
  return {
    message: expectString(related.message, "related diagnostic message"),
    range: projectRange(related.range),
  };
}

function projectDiagnosticData(value: unknown): EditorDiagnosticData {
  const data = expectRecord(value, "diagnostic data");
  return {
    id: expectString(data.id, "diagnostic data ID"),
    category: expectString(data.category, "diagnostic category"),
    ...optionalNullableNumberProperty(data, "code", "diagnostic native code"),
    ...optionalNullableStringProperty(data, "codeName", "diagnostic code name"),
    ...optionalNullableStringProperty(
      data,
      "diagramType",
      "diagnostic diagram type",
    ),
    ...optionalNullableStringProperty(data, "help", "diagnostic help"),
    ...optionalArrayProperty(data, "fixes", "diagnostic fixes", projectFix),
  };
}

function projectFix(value: unknown): AnalysisDiagnosticFix {
  const fix = expectRecord(value, "diagnostic fix");
  return {
    title: expectString(fix.title, "diagnostic fix title"),
    edits: projectArray(fix.edits, "diagnostic fix edits", projectFixEdit),
    ...optionalBooleanProperty(fix, "is_preferred", "diagnostic preferred fix"),
  };
}

function projectFixEdit(value: unknown): AnalysisDiagnosticFixEdit {
  const edit = expectRecord(value, "diagnostic fix edit");
  return {
    span: projectSpan(edit.span),
    replacement: expectString(edit.replacement, "diagnostic fix replacement"),
  };
}

function projectSpan(value: unknown): AnalysisSpan {
  const span = expectRecord(value, "analysis span");
  return {
    byte_start: expectNonNegativeSafeInteger(
      span.byte_start,
      "span byte_start",
    ),
    byte_end: expectNonNegativeSafeInteger(span.byte_end, "span byte_end"),
    line: expectNonNegativeSafeInteger(span.line, "span line"),
    column: expectNonNegativeSafeInteger(span.column, "span column"),
    end_line: expectNonNegativeSafeInteger(span.end_line, "span end_line"),
    end_column: expectNonNegativeSafeInteger(
      span.end_column,
      "span end_column",
    ),
    lsp_range: projectRange(span.lsp_range),
  };
}

function projectCodeAction(value: unknown): EditorCodeAction {
  const action = expectRecord(value, "code action");
  if (action.kind !== "quickfix") {
    fail("Editor code action kind is invalid.");
  }
  return {
    title: expectString(action.title, "code action title"),
    kind: "quickfix",
    diagnostics: projectArray(
      action.diagnostics,
      "code action diagnostics",
      projectDiagnostic,
    ),
    edit: projectWorkspaceEdit(action.edit),
    isPreferred: expectBoolean(action.isPreferred, "preferred code action"),
  };
}

function projectCompletionList(value: unknown): EditorCompletionList {
  const list = expectRecord(value, "completion list");
  return {
    is_incomplete: expectBoolean(
      list.is_incomplete,
      "completion list incomplete",
    ),
    items: projectArray(list.items, "completion items", projectCompletionItem),
    ...optionalNullableSetProperty(
      list,
      "fact_source",
      SEMANTIC_FACT_SOURCES,
      "completion fact source",
    ),
  };
}

function projectCompletionItem(value: unknown): EditorCompletionItem {
  const item = expectRecord(value, "completion item");
  const projected: EditorCompletionItem = {
    label: expectString(item.label, "completion label"),
    kind: expectSetValue(item.kind, COMPLETION_ITEM_KINDS, "completion kind"),
  };
  if (hasDefinedOwn(item, "detail")) {
    projected.detail = expectNullableString(item.detail, "completion detail");
  }
  if (hasDefinedOwn(item, "data")) {
    projected.data =
      item.data === null ? null : projectCompletionData(item.data);
  }
  if (hasDefinedOwn(item, "insert_text")) {
    projected.insert_text = expectNullableString(
      item.insert_text,
      "completion insert text",
    );
  }
  if (hasDefinedOwn(item, "insert_text_format")) {
    projected.insert_text_format = expectSetValue(
      item.insert_text_format,
      COMPLETION_INSERT_TEXT_FORMATS,
      "completion insert text format",
    );
  }
  if (hasDefinedOwn(item, "text_edit")) {
    projected.text_edit =
      item.text_edit === null
        ? null
        : projectCompletionTextEdit(item.text_edit);
  }
  if (hasDefinedOwn(item, "label_details")) {
    projected.label_details =
      item.label_details === null
        ? null
        : projectCompletionLabelDetails(item.label_details);
  }
  return projected;
}

function projectCompletionData(
  value: unknown,
): NonNullable<EditorCompletionItem["data"]> {
  const data = expectRecord(value, "completion resolve data");
  return {
    kind: expectSetValue(
      data.kind,
      COMPLETION_RESOLVE_KINDS,
      "completion resolve kind",
    ),
    label: expectString(data.label, "completion resolve label"),
  };
}

function projectCompletionTextEdit(
  value: unknown,
): NonNullable<EditorCompletionItem["text_edit"]> {
  const edit = expectRecord(value, "completion text edit");
  return {
    range: projectRange(edit.range),
    new_text: expectString(edit.new_text, "completion edit text"),
  };
}

function projectCompletionLabelDetails(
  value: unknown,
): NonNullable<EditorCompletionItem["label_details"]> {
  const details = expectRecord(value, "completion label details");
  return {
    ...optionalNullableStringProperty(
      details,
      "description",
      "completion label description",
    ),
    ...optionalNullableStringProperty(
      details,
      "detail",
      "completion label detail",
    ),
  };
}

function projectHover(value: unknown): EditorHover {
  const hover = expectRecord(value, "hover");
  const contents = expectRecord(hover.contents, "hover contents");
  if (contents.kind !== "markdown") {
    fail("Editor hover markup kind is invalid.");
  }
  return {
    contents: {
      kind: "markdown",
      value: expectString(contents.value, "hover contents value"),
    },
    factSource: expectSemanticFactSource(hover.factSource, "hover fact source"),
    ...optionalNullableProperty(hover, "range", projectRange),
  };
}

function projectDocumentSymbol(value: unknown): EditorDocumentSymbol {
  const symbol = expectRecord(value, "document symbol");
  return {
    name: expectString(symbol.name, "document symbol name"),
    kind: expectSetValue(symbol.kind, SYMBOL_KINDS, "document symbol kind"),
    factSource: expectSemanticFactSource(
      symbol.factSource,
      "document symbol fact source",
    ),
    range: projectRange(symbol.range),
    selectionRange: projectRange(symbol.selectionRange),
    children: projectArray(
      symbol.children,
      "document symbol children",
      projectDocumentSymbol,
    ),
    ...optionalNullableStringProperty(
      symbol,
      "detail",
      "document symbol detail",
    ),
  };
}

function projectLocation(value: unknown): EditorLocation {
  const location = expectRecord(value, "editor location");
  return {
    uri: expectString(location.uri, "editor location URI"),
    factSource: expectSemanticFactSource(
      location.factSource,
      "editor location fact source",
    ),
    range: projectRange(location.range),
  };
}

function projectPrepareRename(value: unknown): EditorPrepareRename {
  const rename = expectRecord(value, "prepare rename result");
  return {
    factSource: expectSemanticFactSource(
      rename.factSource,
      "prepare rename fact source",
    ),
    range: projectRange(rename.range),
    placeholder: expectString(rename.placeholder, "prepare rename placeholder"),
  };
}

function projectWorkspaceEdit(value: unknown): EditorWorkspaceEdit {
  const edit = expectRecord(value, "workspace edit");
  const changes = expectRecord(edit.changes, "workspace edit changes");
  const projectedChanges = Object.fromEntries(
    Object.entries(changes).map(([uri, edits]) => [
      uri,
      projectArray(edits, `workspace edits for ${uri}`, projectTextEdit),
    ]),
  );
  return {
    changes: projectedChanges,
    ...optionalNullableSetProperty(
      edit,
      "factSource",
      SEMANTIC_FACT_SOURCES,
      "workspace edit fact source",
    ),
  };
}

function projectTextEdit(value: unknown): EditorTextEdit {
  const edit = expectRecord(value, "text edit");
  return {
    range: projectRange(edit.range),
    newText: expectString(edit.newText, "text edit newText"),
    ...optionalNullableSetProperty(
      edit,
      "factSource",
      SEMANTIC_FACT_SOURCES,
      "text edit fact source",
    ),
  };
}

function projectPosition(value: unknown): EditorPosition {
  const position = expectRecord(value, "editor position");
  return {
    line: expectNonNegativeSafeInteger(position.line, "position line"),
    character: expectNonNegativeSafeInteger(
      position.character,
      "position character",
    ),
  };
}

function projectRange(value: unknown): EditorRange {
  const range = expectRecord(value, "editor range");
  return {
    start: projectPosition(range.start),
    end: projectPosition(range.end),
  };
}

function projectRequestPosition(value: unknown): EditorPosition {
  const position = expectRecord(value, "editor position");
  assertSchema(position, POSITION_SCHEMA, "editor position");
  return projectPosition(position);
}

function projectArray<T>(
  value: unknown,
  label: string,
  project: (item: unknown) => T,
): T[] {
  if (!Array.isArray(value)) {
    fail(`Editor ${label} must be an array.`);
  }
  return value.map(project);
}

function projectStringArray(value: unknown, label: string): string[] {
  return projectArray(value, label, (item) => expectString(item, label));
}

function hasDefinedOwn(record: Record<string, unknown>, key: string): boolean {
  return Object.hasOwn(record, key) && record[key] !== undefined;
}

function optionalNullableProperty<Key extends string, Value>(
  record: Record<string, unknown>,
  key: Key,
  project: (value: unknown) => Value,
): { [Property in Key]?: Value | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  return { [key]: value === null ? null : project(value) } as {
    [Property in Key]?: Value | null;
  };
}

function optionalArrayProperty<Key extends string, Value>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
  project: (value: unknown) => Value,
): { [Property in Key]?: Value[] } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  return { [key]: projectArray(record[key], label, project) } as {
    [Property in Key]?: Value[];
  };
}

function optionalBooleanProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: boolean } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  return { [key]: expectBoolean(record[key], label) } as {
    [Property in Key]?: boolean;
  };
}

function optionalNullableStringProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: string | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  return { [key]: expectNullableString(record[key], label) } as {
    [Property in Key]?: string | null;
  };
}

function optionalNullableNumberProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: number | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  if (value !== null && typeof value !== "number") {
    fail(`Editor ${label} must be a number or null.`);
  }
  return { [key]: value } as { [Property in Key]?: number | null };
}

function optionalNullableIntegerProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: number | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  return {
    [key]: value === null ? null : expectNonNegativeSafeInteger(value, label),
  } as { [Property in Key]?: number | null };
}

function optionalNullableSetProperty<Key extends string, Value extends string>(
  record: Record<string, unknown>,
  key: Key,
  allowed: ReadonlySet<Value>,
  label: string,
): { [Property in Key]?: Value | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  return {
    [key]: value === null ? null : expectSetValue(value, allowed, label),
  } as { [Property in Key]?: Value | null };
}

function expectSemanticFactSource(
  value: unknown,
  label: string,
): EditorSemanticFactSource {
  return expectSetValue(value, SEMANTIC_FACT_SOURCES, label);
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail(`Editor ${label} must be an object.`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(`Editor ${label} must be a plain object.`);
  }
  return value as Record<string, unknown>;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    fail(`Editor ${label} must be a string.`);
  }
  return value;
}

function expectNonEmptyString(value: unknown, label: string): string {
  const text = expectString(value, label);
  if (text.length === 0) {
    fail(`Editor ${label} must not be empty.`);
  }
  return text;
}

function expectNonBlankString(value: unknown, label: string): string {
  const text = expectString(value, label);
  if (text.trim().length === 0) {
    fail(`Editor ${label} must not be blank.`);
  }
  return text;
}

function expectNullableString(value: unknown, label: string): string | null {
  return value === null ? null : expectString(value, label);
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    fail(`Editor ${label} must be a boolean.`);
  }
  return value;
}

function expectRequestId(value: unknown): number {
  return expectPositiveSafeInteger(value, "request ID");
}

function expectPositiveSafeInteger(value: unknown, label: string): number {
  if (!isPositiveSafeInteger(value)) {
    fail(`Editor ${label} must be a positive safe integer.`);
  }
  return value;
}

function expectNonNegativeSafeInteger(value: unknown, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    fail(`Editor ${label} must be a non-negative safe integer.`);
  }
  return value as number;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function expectSetValue<Value extends string>(
  value: unknown,
  allowed: ReadonlySet<Value>,
  label: string,
): Value {
  if (typeof value !== "string" || !allowed.has(value as Value)) {
    fail(`Editor ${label} is invalid.`);
  }
  return value as Value;
}

function schema(required: readonly string[]): ObjectSchema {
  return Object.freeze({
    allowed: new Set(required),
    required,
  });
}

function assertSchema(
  value: Record<string, unknown>,
  expected: ObjectSchema,
  label: string,
): void {
  const keys = Object.keys(value);
  if (
    keys.some((key) => !expected.allowed.has(key)) ||
    expected.required.some((key) => !Object.hasOwn(value, key))
  ) {
    fail(`Editor ${label} contains unexpected or missing fields.`);
  }
}

function fail(message: string): never {
  throw new EditorWorkerProtocolProjectionError(message);
}
