import type { MermaidSyntaxEngine } from "./syntax-engine.ts";
import {
  MERMAID_SYNTAX_WORKER_PROTOCOL,
  projectSyntaxWorkerRequest,
  projectSyntaxWorkerResponse,
  requestIdFromSyntaxMessage,
  type SyntaxDocumentIdentity,
  type SyntaxDocumentSnapshot,
  type SyntaxWorkerErrorCode,
  type SyntaxWorkerRequest,
  type SyntaxWorkerResponse,
} from "./syntax-protocol.ts";

export interface SyntaxWorkerRuntimePort {
  close(): void;
  postMessage(message: SyntaxWorkerResponse, transfer?: ArrayBuffer[]): void;
}

export interface SyntaxWorkerRuntime {
  receive(value: unknown): Promise<void>;
  receiveMessageError(): void;
}

export function createSyntaxWorkerRuntime(
  port: SyntaxWorkerRuntimePort,
  createEngine: () => Promise<MermaidSyntaxEngine>,
): SyntaxWorkerRuntime {
  let document: SyntaxDocumentIdentity | null = null;
  let engine: MermaidSyntaxEngine | null = null;
  let disposed = false;

  const close = (): void => {
    if (disposed) return;
    disposed = true;
    engine?.dispose();
    engine = null;
    document = null;
    port.close();
  };
  const post = (message: SyntaxWorkerResponse, transfer?: ArrayBuffer[]): void =>
    port.postMessage(projectSyntaxWorkerResponse(message), transfer);
  const error = (requestId: number, code: SyntaxWorkerErrorCode, cause: unknown): void =>
    post({
      protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
      requestId,
      type: "error",
      code,
      message: cause instanceof Error ? cause.message : String(cause),
    });
  const requireEngine = (): MermaidSyntaxEngine => {
    if (!engine) throw new SyntaxWorkerStateError("INVALID_STATE", "Syntax worker is not initialized.");
    return engine;
  };
  const requireDocument = (uri: string, version: number): MermaidSyntaxEngine => {
    const current = requireEngine();
    if (!document || document.uri !== uri || document.version !== version) {
      throw new SyntaxWorkerStateError("STALE_DOCUMENT", "Syntax query does not match the current document.");
    }
    return current;
  };
  const synchronize = (next: SyntaxDocumentSnapshot, open: boolean): void => {
    const current = requireEngine();
    if (open) {
      if (document) throw new SyntaxWorkerStateError("INVALID_STATE", "Syntax worker already owns a document.");
      current.open(next.source);
    } else {
      if (!document || document.uri !== next.uri) {
        throw new SyntaxWorkerStateError("INVALID_STATE", "Syntax worker does not own this URI.");
      }
      if (next.version <= document.version) {
        throw new SyntaxWorkerStateError("STALE_DOCUMENT", "Syntax document version must increase.");
      }
      current.update(next.source);
    }
    document = { uri: next.uri, version: next.version };
  };

  const dispatch = async (request: SyntaxWorkerRequest): Promise<void> => {
    if (request.type === "dispose") return close();
    if (disposed) return;
    try {
      switch (request.type) {
        case "initialize":
          engine ??= await createEngine();
          post({ ...request, type: "ready" });
          return;
        case "didOpen":
        case "didChange":
          synchronize(request.document, request.type === "didOpen");
          post({
            protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
            requestId: request.requestId,
            type: "result",
            result: null,
          });
          return;
        case "highlights": {
          const data = requireDocument(request.uri, request.version).highlight();
          const transfer = data.buffer instanceof ArrayBuffer ? [data.buffer] : undefined;
          post({ ...request, type: "highlights", data }, transfer);
          return;
        }
      }
    } catch (cause) {
      const code =
        cause instanceof SyntaxWorkerStateError
          ? cause.code
          : request.type === "initialize"
            ? "INITIALIZATION_FAILED"
            : "QUERY_FAILED";
      error(request.requestId, code, cause);
    }
  };

  return Object.freeze({
    async receive(value: unknown): Promise<void> {
      if (disposed) return;
      let request: SyntaxWorkerRequest;
      try {
        request = projectSyntaxWorkerRequest(value);
      } catch (cause) {
        const requestId = requestIdFromSyntaxMessage(value);
        if (requestId !== null) error(requestId, "PROTOCOL_MISMATCH", cause);
        close();
        return;
      }
      await dispatch(request);
    },
    receiveMessageError: close,
  });
}

class SyntaxWorkerStateError extends Error {
  readonly code: SyntaxWorkerErrorCode;

  constructor(
    code: SyntaxWorkerErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "SyntaxWorkerStateError";
    this.code = code;
  }
}
