/// <reference lib="webworker" />

import {
  abiVersion,
  createEditorSession,
  editorSemanticTokenDescriptor,
  initMerman,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
  SEMANTIC_TOKEN_TYPE_LSP_NAMES,
  type BrowserEditorSession,
} from "@mermanjs/web/editor";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_ABI_VERSION,
  type EditorDocumentSnapshot,
  type EditorWorkerQuery,
  type EditorWorkerRequest,
  type EditorWorkerResponse,
} from "./protocol.ts";
import {
  isBindingErrorPayload,
  projectError,
} from "../runtime/error-projection.ts";

const scope = self;
type EditorDocumentIdentity = Pick<EditorDocumentSnapshot, "uri" | "version">;

let document: EditorDocumentIdentity | null = null;
let editorSession: BrowserEditorSession | null = null;
let initialized = false;
let disposed = false;

scope.addEventListener("message", (event: MessageEvent<unknown>) => {
  const request = parseRequest(event.data);
  if (!request) {
    const requestId = requestIdFromMalformedMessage(event.data);
    if (requestId !== null) {
      respondError(
        requestId,
        "PROTOCOL_MISMATCH",
        "Received a malformed editor worker request."
      );
    }
    return;
  }
  void dispatch(request);
});

async function dispatch(request: EditorWorkerRequest): Promise<void> {
  if (request.type === "dispose") {
    disposed = true;
    disposeEditorSession();
    return;
  }
  if (disposed) {
    respondError(request.requestId, "INVALID_STATE", "Editor worker is disposed.");
    return;
  }

  try {
    switch (request.type) {
      case "initialize":
        await initialize(request.requestId);
        return;
      case "didOpen":
        requireInitialized();
        openDocument(request.document);
        respond(request.requestId, null);
        return;
      case "didChange":
        requireInitialized();
        changeDocument(request.document);
        respond(request.requestId, null);
        return;
      case "query":
        requireInitialized();
        respondQuery(
          request.requestId,
          request.uri,
          request.version,
          request.legendDigest,
          executeQuery(
            requireDocument(
              request.uri,
              request.version,
              request.legendDigest
            ),
            request.query
          )
        );
        return;
    }
  } catch (error) {
    const code = workerErrorCode(request, error);
    respondError(request.requestId, code, error);
  }
}

async function initialize(requestId: number): Promise<void> {
  if (!initialized) {
    await initMerman();
    const nativeAbi = abiVersion();
    if (nativeAbi !== MERMAN_ABI_VERSION) {
      throw new WorkerStateError(
        "PROTOCOL_MISMATCH",
        `Merman native ABI ${nativeAbi} does not match ${MERMAN_ABI_VERSION}.`
      );
    }
    const descriptor = editorSemanticTokenDescriptor();
    if (descriptor.digest !== SEMANTIC_TOKEN_DESCRIPTOR_DIGEST) {
      throw new WorkerStateError(
        "PROTOCOL_MISMATCH",
        `Merman editor legend ${descriptor.digest} does not match ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}.`
      );
    }
    initialized = true;
  }
  post({
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "ready",
    requestId,
    nativeAbi: MERMAN_ABI_VERSION,
    editorSchema: EDITOR_SCHEMA_VERSION,
    legendDigest: SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
    legend: {
      tokenTypes: [...SEMANTIC_TOKEN_TYPE_LSP_NAMES],
      tokenModifiers: [...SEMANTIC_TOKEN_MODIFIER_LSP_NAMES],
    },
  });
}

function openDocument(next: EditorDocumentSnapshot): void {
  validateDocument(next);
  if (document || editorSession) {
    throw new WorkerStateError("INVALID_STATE", "Editor worker already owns a document.");
  }
  const session = createEditorSession(next.source, next.version, next.uri);
  let identityMatches: boolean;
  try {
    identityMatches = session.uri === next.uri && session.version === next.version;
  } catch (error) {
    disposeQuietly(session);
    throw error;
  }
  if (!identityMatches) {
    disposeQuietly(session);
    throw new WorkerStateError(
      "PROTOCOL_MISMATCH",
      "Merman editor session identity does not match the opened document."
    );
  }
  editorSession = session;
  document = { uri: next.uri, version: next.version };
}

function changeDocument(next: EditorDocumentSnapshot): void {
  validateDocument(next);
  if (!document || !editorSession || document.uri !== next.uri) {
    throw new WorkerStateError("INVALID_STATE", "Editor worker does not own this URI.");
  }
  if (next.version <= document.version) {
    throw new WorkerStateError(
      "STALE_DOCUMENT",
      `Document version ${next.version} is not newer than ${document.version}.`
    );
  }
  editorSession.update(next.source, next.version);
  let identityMatches: boolean;
  try {
    identityMatches =
      editorSession.uri === next.uri && editorSession.version === next.version;
  } catch (error) {
    abandonEditorSession();
    throw error;
  }
  if (!identityMatches) {
    abandonEditorSession();
    throw new WorkerStateError(
      "PROTOCOL_MISMATCH",
      "Merman editor session identity drifted after an update."
    );
  }
  document = { uri: next.uri, version: next.version };
}

function requireDocument(
  uri: string,
  version: number,
  legendDigest: string
): BrowserEditorSession {
  if (legendDigest !== SEMANTIC_TOKEN_DESCRIPTOR_DIGEST) {
    throw new WorkerStateError(
      "PROTOCOL_MISMATCH",
      `Editor query legend ${legendDigest} does not match ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}.`
    );
  }
  if (
    !document ||
    !editorSession ||
    document.uri !== uri ||
    document.version !== version ||
    editorSession.uri !== uri ||
    editorSession.version !== version
  ) {
    throw new WorkerStateError(
      "STALE_DOCUMENT",
      "Editor query does not match the current document URI and version."
    );
  }
  return editorSession;
}

function executeQuery(current: BrowserEditorSession, query: EditorWorkerQuery): unknown {
  switch (query.kind) {
    case "diagramDetection":
      return current.diagramDetection();
    case "diagnostics": {
      const result = current.diagnostics();
      if (result.version !== EDITOR_SCHEMA_VERSION) {
        throw new WorkerStateError(
          "PROTOCOL_MISMATCH",
          `Editor diagnostics schema ${result.version} does not match ${EDITOR_SCHEMA_VERSION}.`
        );
      }
      return result;
    }
    case "codeActions":
      return current.codeActions();
    case "completions":
      return current.completions(query.position);
    case "hover":
      return current.hover(query.position);
    case "documentSymbols":
      return current.documentSymbols();
    case "definition":
      return current.definition(query.position);
    case "references":
      return current.references(query.position, query.includeDeclaration);
    case "prepareRename":
      return current.prepareRename(query.position);
    case "rename":
      return current.rename(query.position, query.newName);
    case "semanticTokens":
      return current.semanticTokens();
  }
}

function disposeEditorSession(): void {
  const session = editorSession;
  editorSession = null;
  document = null;
  try {
    session?.dispose();
  } catch {
    // Worker teardown remains final even if the realm is already invalid.
  } finally {
    scope.close();
  }
}

function abandonEditorSession(): void {
  const session = editorSession;
  editorSession = null;
  document = null;
  disposeQuietly(session);
}

function disposeQuietly(session: BrowserEditorSession | null): void {
  try {
    session?.dispose();
  } catch {
    // The client will poison the worker after the protocol failure.
  }
}

function requireInitialized(): void {
  if (!initialized) {
    throw new WorkerStateError("INVALID_STATE", "Editor worker is not initialized.");
  }
}

function validateDocument(value: EditorDocumentSnapshot): void {
  if (!value.uri || !Number.isSafeInteger(value.version) || value.version < 1) {
    throw new WorkerStateError("INVALID_STATE", "Invalid editor document snapshot.");
  }
}

function parseRequest(value: unknown): EditorWorkerRequest | null {
  if (!value || typeof value !== "object") return null;
  const candidate = value as Partial<EditorWorkerRequest>;
  if (candidate.protocol !== EDITOR_WORKER_PROTOCOL) return null;
  if (candidate.type === "dispose") return candidate as EditorWorkerRequest;
  const requestId = (candidate as { requestId?: unknown }).requestId;
  if (
    !Number.isSafeInteger(requestId) ||
    (candidate.type !== "didChange" &&
      candidate.type !== "didOpen" &&
      candidate.type !== "initialize" &&
      candidate.type !== "query")
  ) {
    return null;
  }
  if (
    (candidate.type === "didChange" || candidate.type === "didOpen") &&
    (!candidate.document || typeof candidate.document !== "object")
  ) {
    return null;
  }
  if (candidate.type === "query") {
    const queryRequest = candidate as Record<string, unknown>;
    if (
      typeof queryRequest.uri !== "string" ||
      !Number.isSafeInteger(queryRequest.version) ||
      typeof queryRequest.legendDigest !== "string" ||
      queryRequest.legendDigest.length === 0 ||
      !isEditorWorkerQuery(queryRequest.query)
    ) {
      return null;
    }
  }
  return candidate as EditorWorkerRequest;
}

function isEditorWorkerQuery(value: unknown): value is EditorWorkerQuery {
  if (!value || typeof value !== "object") return false;
  const query = value as Record<string, unknown>;
  switch (query.kind) {
    case "codeActions":
    case "diagnostics":
    case "diagramDetection":
    case "documentSymbols":
    case "semanticTokens":
      return true;
    case "completions":
    case "definition":
    case "hover":
    case "prepareRename":
      return isEditorPosition(query.position);
    case "references":
      return (
        isEditorPosition(query.position) &&
        typeof query.includeDeclaration === "boolean"
      );
    case "rename":
      return (
        isEditorPosition(query.position) && typeof query.newName === "string"
      );
    default:
      return false;
  }
}

function isEditorPosition(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const position = value as Record<string, unknown>;
  return (
    Number.isSafeInteger(position.line) &&
    (position.line as number) >= 0 &&
    Number.isSafeInteger(position.character) &&
    (position.character as number) >= 0
  );
}

function requestIdFromMalformedMessage(value: unknown): number | null {
  if (!value || typeof value !== "object") return null;
  const requestId = (value as { requestId?: unknown }).requestId;
  return Number.isSafeInteger(requestId) ? (requestId as number) : null;
}

function respond(requestId: number, result: unknown): void {
  post({
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "result",
    requestId,
    result,
  });
}

function respondQuery(
  requestId: number,
  uri: string,
  version: number,
  legendDigest: string,
  result: unknown
): void {
  post({
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "queryResult",
    requestId,
    uri,
    version,
    legendDigest,
    result,
  });
}

function respondError(
  requestId: number,
  code: Extract<EditorWorkerResponse, { type: "error" }>["code"],
  error: unknown
): void {
  const projection = projectError(error);
  post({
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "error",
    requestId,
    code,
    message: projection.summary,
    detail: projection.detail,
    nativeCode: isBindingErrorPayload(error) ? error.code_name : null,
  });
}

function workerErrorCode(
  request: Exclude<EditorWorkerRequest, { type: "dispose" }>,
  error: unknown
): Extract<EditorWorkerResponse, { type: "error" }>["code"] {
  if (error instanceof WorkerStateError) return error.code;
  if (request.type === "initialize") return "INITIALIZATION_FAILED";
  if (
    request.type === "query" &&
    request.query.kind === "rename" &&
    isBindingErrorPayload(error) &&
    error.code_name === "MERMAN_INVALID_ARGUMENT"
  ) {
    return "OPERATION_REJECTED";
  }
  return "QUERY_FAILED";
}

function post(message: EditorWorkerResponse): void {
  scope.postMessage(message);
}

class WorkerStateError extends Error {
  constructor(
    readonly code: Extract<EditorWorkerResponse, { type: "error" }>["code"],
    message: string
  ) {
    super(message);
    this.name = "WorkerStateError";
  }
}
