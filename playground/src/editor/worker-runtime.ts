import type {
  BrowserEditorSession,
  EditorSemanticTokenDescriptor,
  RuntimeCatalog,
} from "@mermanjs/web";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_WEB_TRANSPORT_API_VERSION,
  EditorWorkerProtocolProjectionError,
  projectEditorWorkerQueryResult,
  projectEditorWorkerRequest,
  projectEditorWorkerResponse,
  requestIdFromEditorWorkerMessage,
  type EditorDocumentIdentity,
  type EditorDocumentSnapshot,
  type EditorWorkerErrorCode,
  type EditorWorkerQuery,
  type EditorWorkerRequest,
  type EditorWorkerResponse,
} from "./protocol.ts";
import {
  isBindingErrorPayload,
  projectError,
} from "../runtime/error-projection.ts";

export interface EditorWorkerRuntimeBindings {
  readonly createEditorSession: (
    source: string,
    version: number,
    uri: string,
  ) => BrowserEditorSession;
  readonly editorSemanticTokenDescriptor: () => EditorSemanticTokenDescriptor;
  readonly editorCompletionTriggerCharacters: () => string[];
  readonly initMerman: () => Promise<unknown>;
  readonly legendDigest: string;
  readonly runtimeCatalog: () => RuntimeCatalog;
  readonly transportApiVersion: () => number;
}

export interface EditorWorkerRuntimePort {
  close(): void;
  postMessage(message: EditorWorkerResponse, transfer?: ArrayBuffer[]): void;
}

export interface EditorWorkerRuntime {
  receive(value: unknown): Promise<void>;
  receiveMessageError(): void;
}

interface InitializedContract {
  readonly completionTriggerCharacters: readonly string[];
  readonly descriptor: EditorSemanticTokenDescriptor;
  readonly transportApiVersion: number;
}

export function createEditorWorkerRuntime(
  port: EditorWorkerRuntimePort,
  bindings: EditorWorkerRuntimeBindings,
): EditorWorkerRuntime {
  let document: EditorDocumentIdentity | null = null;
  let editorSession: BrowserEditorSession | null = null;
  let initializedContract: InitializedContract | null = null;
  let disposed = false;

  const disposeQuietly = (session: BrowserEditorSession | null): void => {
    try {
      session?.dispose();
    } catch {
      // Realm teardown remains final even if the native session is already invalid.
    }
  };

  const abandonEditorSession = (): void => {
    const session = editorSession;
    editorSession = null;
    document = null;
    disposeQuietly(session);
  };

  const close = (): void => {
    if (disposed) return;
    disposed = true;
    abandonEditorSession();
    port.close();
  };

  const post = (
    message: EditorWorkerResponse,
    transfer?: ArrayBuffer[],
  ): void => {
    port.postMessage(projectEditorWorkerResponse(message), transfer);
  };

  const respondError = (
    requestId: number,
    code: EditorWorkerErrorCode,
    error: unknown,
  ): void => {
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
  };

  const requireInitialized = (): InitializedContract => {
    if (!initializedContract) {
      throw new WorkerStateError(
        "INVALID_STATE",
        "Editor worker is not initialized.",
      );
    }
    return initializedContract;
  };

  const initialize = async (requestId: number): Promise<void> => {
    if (!initializedContract) {
      await bindings.initMerman();
      const transportVersion = bindings.transportApiVersion();
      if (transportVersion !== MERMAN_WEB_TRANSPORT_API_VERSION) {
        throw new WorkerStateError(
          "PROTOCOL_MISMATCH",
          `Merman Web transport API ${transportVersion} is incompatible with ${MERMAN_WEB_TRANSPORT_API_VERSION}.`,
        );
      }
      const catalog = bindings.runtimeCatalog();
      if (catalog.transport_api_version !== transportVersion) {
        throw new WorkerStateError(
          "PROTOCOL_MISMATCH",
          "Merman Web runtime transport API does not match its runtime catalog.",
        );
      }
      if (!catalog.capabilities.capability_ids.includes("editor")) {
        throw new WorkerStateError(
          "PROTOCOL_MISMATCH",
          "Merman editor worker was loaded without the editor capability.",
        );
      }
      const descriptor = bindings.editorSemanticTokenDescriptor();
      if (descriptor.digest !== bindings.legendDigest) {
        throw new WorkerStateError(
          "PROTOCOL_MISMATCH",
          `Merman editor legend ${descriptor.digest} does not match ${bindings.legendDigest}.`,
        );
      }
      initializedContract = Object.freeze({
        completionTriggerCharacters: Object.freeze([
          ...bindings.editorCompletionTriggerCharacters(),
        ]),
        descriptor,
        transportApiVersion: transportVersion,
      });
    }
    const contract = requireInitialized();
    post({
      protocol: EDITOR_WORKER_PROTOCOL,
      type: "ready",
      requestId,
      transportApiVersion: contract.transportApiVersion,
      editorSchema: EDITOR_SCHEMA_VERSION,
      completionTriggerCharacters: [
        ...contract.completionTriggerCharacters,
      ],
      legendDigest: contract.descriptor.digest,
      legend: {
        tokenTypes: [...contract.descriptor.tokenTypeLspNames],
        tokenModifiers: [...contract.descriptor.modifierLspNames],
      },
    });
  };

  const openDocument = (next: EditorDocumentSnapshot): void => {
    if (document || editorSession) {
      throw new WorkerStateError(
        "INVALID_STATE",
        "Editor worker already owns a document.",
      );
    }
    const session = bindings.createEditorSession(
      next.source,
      next.version,
      next.uri,
    );
    let identityMatches: boolean;
    try {
      identityMatches =
        session.uri === next.uri && session.version === next.version;
    } catch (error) {
      disposeQuietly(session);
      throw error;
    }
    if (!identityMatches) {
      disposeQuietly(session);
      throw new WorkerStateError(
        "PROTOCOL_MISMATCH",
        "Merman editor session identity does not match the opened document.",
      );
    }
    editorSession = session;
    document = { uri: next.uri, version: next.version };
  };

  const changeDocument = (next: EditorDocumentSnapshot): void => {
    if (!document || !editorSession || document.uri !== next.uri) {
      throw new WorkerStateError(
        "INVALID_STATE",
        "Editor worker does not own this URI.",
      );
    }
    if (next.version <= document.version) {
      throw new WorkerStateError(
        "STALE_DOCUMENT",
        `Document version ${next.version} is not newer than ${document.version}.`,
      );
    }
    editorSession.update(next.source, next.version);
    let identityMatches: boolean;
    try {
      identityMatches =
        editorSession.uri === next.uri &&
        editorSession.version === next.version;
    } catch (error) {
      abandonEditorSession();
      throw error;
    }
    if (!identityMatches) {
      abandonEditorSession();
      throw new WorkerStateError(
        "PROTOCOL_MISMATCH",
        "Merman editor session identity drifted after an update.",
      );
    }
    document = { uri: next.uri, version: next.version };
  };

  const requireDocument = (
    uri: string,
    version: number,
    legendDigest: string,
  ): BrowserEditorSession => {
    const contract = requireInitialized();
    if (legendDigest !== contract.descriptor.digest) {
      throw new WorkerStateError(
        "PROTOCOL_MISMATCH",
        `Editor query legend ${legendDigest} does not match ${contract.descriptor.digest}.`,
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
        "Editor query does not match the current document URI and version.",
      );
    }
    return editorSession;
  };

  const executeQuery = (
    current: BrowserEditorSession,
    query: EditorWorkerQuery,
  ): unknown => {
    switch (query.kind) {
      case "diagramDetection":
        return current.diagramDetection();
      case "diagnostics":
        return current.diagnostics();
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
  };

  const respondQuery = (
    request: Extract<EditorWorkerRequest, { type: "query" }>,
  ): void => {
    const projected = projectEditorWorkerQueryResult(
      request.query,
      executeQuery(
        requireDocument(request.uri, request.version, request.legendDigest),
        request.query,
      ),
    );
    const result =
      request.query.kind === "semanticTokens" &&
      projected instanceof Uint32Array
        ? new Uint32Array(projected)
        : projected;
    const message: EditorWorkerResponse = {
      protocol: EDITOR_WORKER_PROTOCOL,
      type: "queryResult",
      requestId: request.requestId,
      uri: request.uri,
      version: request.version,
      legendDigest: request.legendDigest,
      result,
    };
    const transfer =
      request.query.kind === "semanticTokens" &&
      result instanceof Uint32Array &&
      result.buffer instanceof ArrayBuffer
        ? [result.buffer]
        : undefined;
    post(message, transfer);
  };

  const workerErrorCode = (
    request: Exclude<EditorWorkerRequest, { type: "dispose" }>,
    error: unknown,
  ): EditorWorkerErrorCode => {
    if (error instanceof WorkerStateError) return error.code;
    if (error instanceof EditorWorkerProtocolProjectionError) {
      return "PROTOCOL_MISMATCH";
    }
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
  };

  const dispatch = async (request: EditorWorkerRequest): Promise<void> => {
    if (request.type === "dispose") {
      close();
      return;
    }
    if (disposed) return;
    try {
      switch (request.type) {
        case "initialize":
          await initialize(request.requestId);
          return;
        case "didOpen":
          requireInitialized();
          openDocument(request.document);
          post({
            protocol: EDITOR_WORKER_PROTOCOL,
            type: "result",
            requestId: request.requestId,
            result: null,
          });
          return;
        case "didChange":
          requireInitialized();
          changeDocument(request.document);
          post({
            protocol: EDITOR_WORKER_PROTOCOL,
            type: "result",
            requestId: request.requestId,
            result: null,
          });
          return;
        case "query":
          requireInitialized();
          respondQuery(request);
          return;
      }
    } catch (error) {
      respondError(request.requestId, workerErrorCode(request, error), error);
    }
  };

  return Object.freeze({
    async receive(value: unknown): Promise<void> {
      if (disposed) return;
      let request: EditorWorkerRequest;
      try {
        request = projectEditorWorkerRequest(value);
      } catch (error) {
        const requestId = requestIdFromEditorWorkerMessage(value);
        if (requestId !== null) {
          respondError(requestId, "PROTOCOL_MISMATCH", error);
        }
        close();
        return;
      }
      await dispatch(request);
    },
    receiveMessageError(): void {
      close();
    },
  });
}

class WorkerStateError extends Error {
  readonly code: EditorWorkerErrorCode;

  constructor(code: EditorWorkerErrorCode, message: string) {
    super(message);
    this.name = "WorkerStateError";
    this.code = code;
  }
}
