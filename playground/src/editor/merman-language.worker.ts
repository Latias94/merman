/// <reference lib="webworker" />

import {
  abiVersion,
  editorCodeActions,
  editorCompletions,
  editorDefinition,
  editorDiagnostics,
  editorDocumentSymbols,
  editorHover,
  editorPrepareRename,
  editorReferences,
  editorRename,
  editorSemanticTokenLegend,
  editorSemanticTokens,
  initMerman,
} from "@mermanjs/web/editor";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_NATIVE_ABI_VERSION,
  type EditorDocumentSnapshot,
  type EditorWorkerQuery,
  type EditorWorkerRequest,
  type EditorWorkerResponse,
} from "./protocol.ts";

const scope = self;
const canceled = new Set<number>();
let document: EditorDocumentSnapshot | null = null;
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
    document = null;
    canceled.clear();
    scope.close();
    return;
  }
  if (request.type === "cancel") {
    canceled.add(request.requestId);
    scope.setTimeout(() => canceled.delete(request.requestId), 0);
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
        if (canceled.delete(request.requestId)) {
          respondError(request.requestId, "CANCELED", "Editor request was canceled.");
          return;
        }
        respond(
          request.requestId,
          executeQuery(requireDocument(request.uri, request.version), request.query)
        );
        return;
    }
  } catch (error) {
    const code =
      error instanceof WorkerStateError ? error.code : request.type === "initialize"
        ? "INITIALIZATION_FAILED"
        : "QUERY_FAILED";
    respondError(request.requestId, code, errorMessage(error));
  } finally {
    canceled.delete(request.requestId);
  }
}

async function initialize(requestId: number): Promise<void> {
  if (!initialized) {
    await initMerman();
    const nativeAbi = abiVersion();
    if (nativeAbi !== MERMAN_NATIVE_ABI_VERSION) {
      throw new WorkerStateError(
        "PROTOCOL_MISMATCH",
        `Merman native ABI ${nativeAbi} does not match ${MERMAN_NATIVE_ABI_VERSION}.`
      );
    }
    initialized = true;
  }
  post({
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "ready",
    requestId,
    nativeAbi: MERMAN_NATIVE_ABI_VERSION,
    editorSchema: EDITOR_SCHEMA_VERSION,
    legend: editorSemanticTokenLegend(),
  });
}

function openDocument(next: EditorDocumentSnapshot): void {
  validateDocument(next);
  if (document) {
    throw new WorkerStateError("INVALID_STATE", "Editor worker already owns a document.");
  }
  document = { ...next };
}

function changeDocument(next: EditorDocumentSnapshot): void {
  validateDocument(next);
  if (!document || document.uri !== next.uri) {
    throw new WorkerStateError("INVALID_STATE", "Editor worker does not own this URI.");
  }
  if (next.version <= document.version) {
    throw new WorkerStateError(
      "STALE_DOCUMENT",
      `Document version ${next.version} is not newer than ${document.version}.`
    );
  }
  document = { ...next };
}

function requireDocument(uri: string, version: number): EditorDocumentSnapshot {
  if (!document || document.uri !== uri || document.version !== version) {
    throw new WorkerStateError(
      "STALE_DOCUMENT",
      "Editor query does not match the current document URI and version."
    );
  }
  return document;
}

function executeQuery(current: EditorDocumentSnapshot, query: EditorWorkerQuery): unknown {
  const { source, uri } = current;
  switch (query.kind) {
    case "diagnostics": {
      const result = editorDiagnostics(source, undefined, uri);
      if (result.version !== EDITOR_SCHEMA_VERSION) {
        throw new WorkerStateError(
          "PROTOCOL_MISMATCH",
          `Editor diagnostics schema ${result.version} does not match ${EDITOR_SCHEMA_VERSION}.`
        );
      }
      return result;
    }
    case "codeActions":
      return editorCodeActions(source, undefined, uri);
    case "completions":
      return editorCompletions(source, query.position, uri);
    case "hover":
      return editorHover(source, query.position, uri);
    case "documentSymbols":
      return editorDocumentSymbols(source, uri);
    case "definition":
      return editorDefinition(source, query.position, uri);
    case "references":
      return editorReferences(
        source,
        query.position,
        query.includeDeclaration,
        uri
      );
    case "prepareRename":
      return editorPrepareRename(source, query.position, uri);
    case "rename":
      return editorRename(source, query.position, query.newName, uri);
    case "semanticTokens":
      return editorSemanticTokens(source, uri);
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
    (candidate.type !== "cancel" &&
      candidate.type !== "didChange" &&
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
      !queryRequest.query ||
      typeof queryRequest.query !== "object"
    ) {
      return null;
    }
  }
  return candidate as EditorWorkerRequest;
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

function respondError(
  requestId: number,
  code: Extract<EditorWorkerResponse, { type: "error" }>["code"],
  message: string
): void {
  post({
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "error",
    requestId,
    code,
    message,
  });
}

function post(message: EditorWorkerResponse): void {
  scope.postMessage(message);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

class WorkerStateError extends Error {
  constructor(
    readonly code: Extract<EditorWorkerResponse, { type: "error" }>["code"],
    message: string
  ) {
    super(message);
  }
}
