import type { EditorPosition } from "@mermanjs/web";

import type {
  EditorWorkerQuery,
  EditorWorkerQueryResult,
  EditorWorkerQueryResults,
} from "./protocol-query-results.ts";
import { projectEditorWorkerQueryResult } from "./protocol-query-results.ts";
import {
  EditorWorkerProtocolProjectionError,
  assertSchema,
  expectBoolean,
  expectNonEmptyString,
  expectNonNegativeSafeInteger,
  expectNullableString,
  expectPositiveSafeInteger,
  expectRecord,
  expectRequestId,
  expectSetValue,
  expectString,
  fail,
  isPositiveSafeInteger,
  projectStringArray,
  schema,
} from "./protocol-schema.ts";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_WEB_TRANSPORT_API_VERSION,
} from "./protocol-version.ts";

export { projectEditorWorkerQueryResult };
export type {
  EditorWorkerQuery,
  EditorWorkerQueryResult,
  EditorWorkerQueryResults,
};
export {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_WEB_TRANSPORT_API_VERSION,
  EditorWorkerProtocolProjectionError,
};

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
  readonly completionTriggerCharacters: string[];
  readonly transportApiVersion: number;
  readonly editorSchema: typeof EDITOR_SCHEMA_VERSION;
}

export interface EditorWorkerSyncResponse extends EditorWorkerResponseBase {
  readonly type: "result";
  readonly result: null;
}

export interface EditorWorkerRawQueryResponse extends EditorWorkerResponseBase {
  readonly type: "queryResult";
  readonly uri: string;
  readonly version: number;
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
  "completionTriggerCharacters",
  "transportApiVersion",
  "editorSchema",
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
      if (
        response.transportApiVersion !== MERMAN_WEB_TRANSPORT_API_VERSION
      ) {
        fail("Merman Web transport API version is incompatible.");
      }
      return {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "ready",
        completionTriggerCharacters: projectCompletionTriggerCharacters(
          response.completionTriggerCharacters,
        ),
        transportApiVersion: MERMAN_WEB_TRANSPORT_API_VERSION,
        editorSchema: EDITOR_SCHEMA_VERSION,
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

function projectCompletionTriggerCharacters(value: unknown): string[] {
  const triggers = projectStringArray(
    value,
    "completion trigger characters",
  );
  if (
    triggers.length === 0 ||
    triggers.some((trigger) => [...trigger].length !== 1)
  ) {
    fail("Editor completion trigger characters must contain one character each.");
  }
  return triggers;
}

export function requestIdFromEditorWorkerMessage(value: unknown): number | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  const requestId = (value as Record<string, unknown>).requestId;
  return isPositiveSafeInteger(requestId) ? requestId : null;
}

function projectRequestPosition(value: unknown): EditorPosition {
  const position = expectRecord(value, "editor position");
  assertSchema(position, POSITION_SCHEMA, "editor position");
  return {
    line: expectNonNegativeSafeInteger(position.line, "position line"),
    character: expectNonNegativeSafeInteger(
      position.character,
      "position character",
    ),
  };
}
