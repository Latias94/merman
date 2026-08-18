import {
  EditorWorkerProtocolProjectionError,
  assertSchema,
  expectNonEmptyString,
  expectPositiveSafeInteger,
  expectRecord,
  expectRequestId,
  expectSetValue,
  expectString,
  fail,
  isPositiveSafeInteger,
  schema,
} from "./protocol-schema.ts";

export const MERMAID_SYNTAX_WORKER_PROTOCOL = 1 as const;

export interface SyntaxDocumentIdentity {
  readonly uri: string;
  readonly version: number;
}

export interface SyntaxDocumentSnapshot extends SyntaxDocumentIdentity {
  readonly source: string;
}

interface RequestBase {
  readonly protocol: typeof MERMAID_SYNTAX_WORKER_PROTOCOL;
  readonly requestId: number;
}

export type SyntaxWorkerRequest =
  | (RequestBase & { readonly type: "initialize" })
  | (RequestBase & { readonly type: "didOpen"; readonly document: SyntaxDocumentSnapshot })
  | (RequestBase & { readonly type: "didChange"; readonly document: SyntaxDocumentSnapshot })
  | (RequestBase & { readonly type: "highlights"; readonly uri: string; readonly version: number })
  | { readonly protocol: typeof MERMAID_SYNTAX_WORKER_PROTOCOL; readonly type: "dispose" };

export type SyntaxWorkerErrorCode =
  | "INITIALIZATION_FAILED"
  | "INVALID_STATE"
  | "PROTOCOL_MISMATCH"
  | "QUERY_FAILED"
  | "STALE_DOCUMENT";

export type SyntaxWorkerResponse =
  | (RequestBase & { readonly type: "ready" })
  | (RequestBase & { readonly type: "result"; readonly result: null })
  | (RequestBase & {
      readonly type: "highlights";
      readonly uri: string;
      readonly version: number;
      readonly data: Uint32Array;
    })
  | (RequestBase & {
      readonly type: "error";
      readonly code: SyntaxWorkerErrorCode;
      readonly message: string;
    });

const BASE_SCHEMA = schema(["protocol", "requestId", "type"]);
const DOCUMENT_SCHEMA = schema(["uri", "version", "source"]);
const DOCUMENT_REQUEST_SCHEMA = schema(["protocol", "requestId", "type", "document"]);
const HIGHLIGHT_REQUEST_SCHEMA = schema(["protocol", "requestId", "type", "uri", "version"]);
const DISPOSE_SCHEMA = schema(["protocol", "type"]);
const RESULT_SCHEMA = schema(["protocol", "requestId", "type", "result"]);
const HIGHLIGHT_RESPONSE_SCHEMA = schema([
  "protocol",
  "requestId",
  "type",
  "uri",
  "version",
  "data",
]);
const ERROR_SCHEMA = schema(["protocol", "requestId", "type", "code", "message"]);
const ERROR_CODES = new Set<SyntaxWorkerErrorCode>([
  "INITIALIZATION_FAILED",
  "INVALID_STATE",
  "PROTOCOL_MISMATCH",
  "QUERY_FAILED",
  "STALE_DOCUMENT",
]);

export function projectSyntaxWorkerRequest(value: unknown): SyntaxWorkerRequest {
  const request = expectRecord(value, "syntax worker request");
  expectProtocol(request.protocol);
  switch (request.type) {
    case "initialize":
      assertSchema(request, BASE_SCHEMA, "syntax initialize request");
      return requestBase(request.requestId, "initialize");
    case "didOpen":
    case "didChange":
      assertSchema(request, DOCUMENT_REQUEST_SCHEMA, `syntax ${request.type} request`);
      return {
        ...requestBase(request.requestId, request.type),
        document: projectSyntaxDocumentSnapshot(request.document),
      };
    case "highlights":
      assertSchema(request, HIGHLIGHT_REQUEST_SCHEMA, "syntax highlight request");
      return {
        ...requestBase(request.requestId, "highlights"),
        uri: expectNonEmptyString(request.uri, "syntax document URI"),
        version: expectPositiveSafeInteger(request.version, "syntax document version"),
      };
    case "dispose":
      assertSchema(request, DISPOSE_SCHEMA, "syntax dispose request");
      return { protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, type: "dispose" };
    default:
      fail("Editor syntax worker request type is invalid.");
  }
}

export function projectSyntaxWorkerResponse(value: unknown): SyntaxWorkerResponse {
  const response = expectRecord(value, "syntax worker response");
  expectProtocol(response.protocol);
  const requestId = expectRequestId(response.requestId);
  switch (response.type) {
    case "ready":
      assertSchema(response, BASE_SCHEMA, "syntax ready response");
      return requestBase(requestId, "ready");
    case "result":
      assertSchema(response, RESULT_SCHEMA, "syntax synchronization response");
      if (response.result !== null) fail("Editor syntax synchronization result must be null.");
      return { ...requestBase(requestId, "result"), result: null };
    case "highlights":
      assertSchema(response, HIGHLIGHT_RESPONSE_SCHEMA, "syntax highlight response");
      if (!(response.data instanceof Uint32Array)) {
        fail("Editor syntax highlight data must be a Uint32Array.");
      }
      return {
        ...requestBase(requestId, "highlights"),
        uri: expectNonEmptyString(response.uri, "syntax response URI"),
        version: expectPositiveSafeInteger(response.version, "syntax response version"),
        data: response.data,
      };
    case "error":
      assertSchema(response, ERROR_SCHEMA, "syntax error response");
      return {
        ...requestBase(requestId, "error"),
        code: expectSetValue(response.code, ERROR_CODES, "syntax worker error code"),
        message: expectString(response.message, "syntax worker error message"),
      };
    default:
      fail("Editor syntax worker response type is invalid.");
  }
}

export function projectSyntaxDocumentSnapshot(value: unknown): SyntaxDocumentSnapshot {
  const document = expectRecord(value, "syntax document snapshot");
  assertSchema(document, DOCUMENT_SCHEMA, "syntax document snapshot");
  return {
    uri: expectNonEmptyString(document.uri, "syntax document URI"),
    version: expectPositiveSafeInteger(document.version, "syntax document version"),
    source: expectString(document.source, "syntax document source"),
  };
}

export function requestIdFromSyntaxMessage(value: unknown): number | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const requestId = (value as Record<string, unknown>).requestId;
  return isPositiveSafeInteger(requestId) ? requestId : null;
}

function requestBase<Type extends string>(requestId: unknown, type: Type) {
  return {
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: expectRequestId(requestId),
    type,
  };
}

function expectProtocol(value: unknown): void {
  if (value !== MERMAID_SYNTAX_WORKER_PROTOCOL) {
    throw new EditorWorkerProtocolProjectionError("Editor syntax worker protocol is invalid.");
  }
}
