import type { EditorSemanticTokenLegend } from "@mermanjs/web";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_ABI_VERSION,
  type EditorDocumentSnapshot,
  type EditorWorkerErrorCode,
  type EditorWorkerQuery,
  type EditorWorkerQueryResult,
  type EditorWorkerRequest,
  type EditorWorkerResponse,
} from "./protocol.ts";

export interface EditorCancellationToken {
  readonly isCancellationRequested: boolean;
  onCancellationRequested(listener: () => void): { dispose(): void };
}

export interface EditorWorkerPort {
  addEventListener(type: "error" | "message", listener: (event: any) => void): void;
  removeEventListener(
    type: "error" | "message",
    listener: (event: any) => void
  ): void;
  postMessage(message: EditorWorkerRequest): void;
  terminate(): void;
}

export interface MermanLanguageWorkerClient {
  initialize(): Promise<EditorLanguageIdentity>;
  openDocument(document: EditorDocumentSnapshot): Promise<void>;
  changeDocument(document: EditorDocumentSnapshot): Promise<void>;
  query<Query extends EditorWorkerQuery>(
    document: EditorDocumentSnapshot,
    query: Query,
    cancellation?: EditorCancellationToken
  ): Promise<EditorWorkerQueryResult<Query>>;
  dispose(): void;
}

export interface EditorLanguageIdentity {
  readonly legend: EditorSemanticTokenLegend;
  readonly legendDigest: string;
}

interface EditorSnapshotIdentity {
  readonly uri: string;
  readonly version: number;
  readonly legendDigest: string;
}

interface PendingRequest {
  readonly document?: EditorSnapshotIdentity;
  readonly expected: "queryResult" | "ready" | "result";
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
  cancellation?: { dispose(): void };
}

export class StaleLanguageSnapshotError extends Error {
  readonly code = "STALE_DOCUMENT" as const;

  constructor(
    message = "The editor result belongs to an obsolete document or legend.",
    readonly detail: string | null = null
  ) {
    super(message);
    this.name = "StaleLanguageSnapshotError";
  }
}

export class EditorWorkerProtocolError extends Error {
  constructor(
    message: string,
    readonly code: EditorWorkerErrorCode | "CLIENT_PROTOCOL" = "CLIENT_PROTOCOL",
    readonly detail: string | null = null,
    readonly nativeCode: string | null = null
  ) {
    super(message);
    this.name = "EditorWorkerProtocolError";
  }
}

export function createMermanLanguageWorkerClient(
  worker: EditorWorkerPort,
  expectedLegendDigest: string
): MermanLanguageWorkerClient {
  return new WorkerClient(worker, expectedLegendDigest);
}

class WorkerClient implements MermanLanguageWorkerClient {
  private readonly pending = new Map<number, PendingRequest>();
  private currentDocument: EditorDocumentSnapshot | null = null;
  private disposed = false;
  private failure: Error | null = null;
  private initialized = false;
  private initializePromise: Promise<EditorLanguageIdentity> | null = null;
  private nextRequestId = 1;
  private synchronization: Promise<void> = Promise.resolve();

  private readonly handleError = (event: { message?: unknown }) => {
    const detail = typeof event.message === "string" ? `: ${event.message}` : "";
    this.poison(new Error(`Merman editor worker failed${detail}`));
  };

  private readonly handleMessage = (event: { data?: unknown }) => {
    const response = parseResponse(event.data);
    if (!response) {
      this.poison(
        new EditorWorkerProtocolError("Received a malformed editor worker response.")
      );
      return;
    }
    const pending = this.pending.get(response.requestId);
    if (!pending) return;

    if (response.type !== "error" && response.type !== pending.expected) {
      this.poison(
        new EditorWorkerProtocolError(
          `Editor worker returned ${response.type} while ${pending.expected} was required.`
        )
      );
      return;
    }

    this.pending.delete(response.requestId);
    pending.cancellation?.dispose();

    if (response.type === "error") {
      const failure = workerResponseError(
        response.code,
        response.message,
        response.detail,
        response.nativeCode
      );
      pending.reject(failure);
      if (isFatalWorkerError(response.code)) this.poison(failure);
      return;
    }

    if (response.type === "ready") {
      try {
        if (response.nativeAbi !== MERMAN_ABI_VERSION) {
          throw new EditorWorkerProtocolError(
            `Merman editor worker ABI ${response.nativeAbi} does not match ${MERMAN_ABI_VERSION}.`
          );
        }
        if (response.editorSchema !== EDITOR_SCHEMA_VERSION) {
          throw new EditorWorkerProtocolError(
            `Merman editor schema ${response.editorSchema} does not match ${EDITOR_SCHEMA_VERSION}.`
          );
        }
        if (response.legendDigest !== this.expectedLegendDigest) {
          throw new EditorWorkerProtocolError(
            `Merman editor legend ${response.legendDigest} does not match ${this.expectedLegendDigest}.`
          );
        }
        pending.resolve(
          Object.freeze({
            legend: validateLegend(response.legend),
            legendDigest: response.legendDigest,
          }) satisfies EditorLanguageIdentity
        );
      } catch (error) {
        pending.reject(asError(error));
      }
      return;
    }

    if (response.type === "queryResult") {
      if (
        !pending.document ||
        !sameSnapshotIdentity(pending.document, response) ||
        !this.isCurrentDocument(pending.document)
      ) {
        pending.reject(new StaleLanguageSnapshotError());
        return;
      }
      pending.resolve(response.result);
      return;
    }
    pending.resolve(response.result);
  };

  constructor(
    private readonly worker: EditorWorkerPort,
    private readonly expectedLegendDigest: string
  ) {
    if (!expectedLegendDigest) {
      throw new EditorWorkerProtocolError("A generated editor legend digest is required.");
    }
    worker.addEventListener("error", this.handleError);
    worker.addEventListener("message", this.handleMessage);
  }

  initialize(): Promise<EditorLanguageIdentity> {
    this.assertActive();
    if (this.initializePromise) return this.initializePromise;

    this.initializePromise = this.request<EditorLanguageIdentity>({
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: this.allocateRequestId(),
      type: "initialize",
    })
      .then((legend) => {
        this.initialized = true;
        return legend;
      })
      .catch((error: unknown) => {
        const failure = asError(error);
        this.poison(failure);
        throw failure;
      });
    return this.initializePromise;
  }

  openDocument(document: EditorDocumentSnapshot): Promise<void> {
    this.assertReady();
    validateSnapshot(document);
    if (this.currentDocument) {
      return Promise.reject(
        new EditorWorkerProtocolError("The editor worker already owns a document.")
      );
    }
    this.currentDocument = copySnapshot(document);
    return this.enqueueSynchronization("didOpen", document);
  }

  changeDocument(document: EditorDocumentSnapshot): Promise<void> {
    this.assertReady();
    validateSnapshot(document);
    const current = this.currentDocument;
    if (!current || current.uri !== document.uri) {
      return Promise.reject(
        new EditorWorkerProtocolError("The editor worker does not own this document URI.")
      );
    }
    if (document.version <= current.version) {
      return Promise.reject(
        new EditorWorkerProtocolError(
          `Document version ${document.version} must be newer than ${current.version}.`
        )
      );
    }
    this.currentDocument = copySnapshot(document);
    return this.enqueueSynchronization("didChange", document);
  }

  async query<Query extends EditorWorkerQuery>(
    document: EditorDocumentSnapshot,
    query: Query,
    cancellation?: EditorCancellationToken
  ): Promise<EditorWorkerQueryResult<Query>> {
    this.assertReady();
    if (!this.isCurrentDocument(document)) {
      throw new StaleLanguageSnapshotError();
    }
    if (cancellation?.isCancellationRequested) {
      throw abortError();
    }

    await this.synchronization;
    if (cancellation?.isCancellationRequested) {
      throw abortError();
    }
    if (!this.isCurrentDocument(document)) {
      throw new StaleLanguageSnapshotError();
    }

    const requestId = this.allocateRequestId();
    return this.request<EditorWorkerQueryResult<Query>>(
      {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "query",
        uri: document.uri,
        version: document.version,
        legendDigest: this.expectedLegendDigest,
        query,
      },
      snapshotIdentity(document, this.expectedLegendDigest),
      cancellation,
      requestId
    );
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.worker.removeEventListener("error", this.handleError);
    this.worker.removeEventListener("message", this.handleMessage);
    try {
      this.worker.postMessage({
        protocol: EDITOR_WORKER_PROTOCOL,
        type: "dispose",
      });
    } catch {
      // A failed worker may already reject messages; termination still releases it.
    }
    this.worker.terminate();
    this.failAll(new Error("Merman editor worker was disposed."));
    this.currentDocument = null;
  }

  private allocateRequestId(): number {
    const requestId = this.nextRequestId;
    this.nextRequestId += 1;
    return requestId;
  }

  private enqueueSynchronization(
    type: "didChange" | "didOpen",
    document: EditorDocumentSnapshot
  ): Promise<void> {
    const snapshot = copySnapshot(document);
    const synchronize = this.synchronization.then(() => {
      const requestId = this.allocateRequestId();
      return this.request<void>(
        {
          protocol: EDITOR_WORKER_PROTOCOL,
          requestId,
          type,
          document: snapshot,
        },
        undefined,
        undefined,
        requestId
      );
    });
    const guarded = synchronize.catch((error: unknown) => {
      const failure = asError(error);
      this.poison(failure);
      throw failure;
    });
    this.synchronization = guarded;
    return guarded;
  }

  private request<Result>(
    message: EditorWorkerRequest,
    document?: EditorSnapshotIdentity,
    cancellation?: EditorCancellationToken,
    knownRequestId?: number
  ): Promise<Result> {
    this.assertActive();
    if (!("requestId" in message)) {
      return Promise.reject(new EditorWorkerProtocolError("Request ID is required."));
    }
    const requestId = knownRequestId ?? message.requestId;
    return new Promise<Result>((resolve, reject) => {
      const pending: PendingRequest = {
        document,
        expected:
          message.type === "initialize"
            ? "ready"
            : message.type === "query"
              ? "queryResult"
              : "result",
        resolve: (value) => resolve(value as Result),
        reject,
      };
      if (cancellation) {
        pending.cancellation = cancellation.onCancellationRequested(() => {
          if (!this.pending.delete(requestId)) return;
          pending.cancellation?.dispose();
          // WASM calls are synchronous. Cancellation only prevents publishing their result.
          reject(abortError());
        });
      }
      this.pending.set(requestId, pending);
      try {
        this.worker.postMessage(message);
      } catch (error) {
        this.pending.delete(requestId);
        pending.cancellation?.dispose();
        const failure = asError(error);
        this.poison(failure);
        reject(failure);
      }
    });
  }

  private isCurrentDocument(
    document: Pick<EditorDocumentSnapshot, "uri" | "version"> &
      Partial<Pick<EditorSnapshotIdentity, "legendDigest">>
  ): boolean {
    return (
      this.currentDocument?.uri === document.uri &&
      this.currentDocument.version === document.version &&
      (document.legendDigest === undefined ||
        document.legendDigest === this.expectedLegendDigest)
    );
  }

  private assertActive(): void {
    if (this.failure) throw this.failure;
    if (this.disposed) {
      throw new Error("Merman editor worker was disposed.");
    }
  }

  private assertReady(): void {
    this.assertActive();
    if (!this.initialized) {
      throw new EditorWorkerProtocolError(
        "Initialize the Merman editor worker before opening a document."
      );
    }
  }

  private failAll(error: Error): void {
    for (const pending of this.pending.values()) {
      pending.cancellation?.dispose();
      pending.reject(error);
    }
    this.pending.clear();
  }

  private poison(error: Error): void {
    if (this.failure) return;
    this.failure = error;
    this.failAll(error);
    this.worker.terminate();
  }
}

function validateSnapshot(document: EditorDocumentSnapshot): void {
  if (!document.uri || !Number.isSafeInteger(document.version) || document.version < 1) {
    throw new EditorWorkerProtocolError("Document URI and positive version are required.");
  }
}

function copySnapshot(document: EditorDocumentSnapshot): EditorDocumentSnapshot {
  return {
    uri: document.uri,
    version: document.version,
    source: document.source,
  };
}

function snapshotIdentity(
  document: Pick<EditorDocumentSnapshot, "uri" | "version">,
  legendDigest: string
): EditorSnapshotIdentity {
  return { uri: document.uri, version: document.version, legendDigest };
}

function sameSnapshotIdentity(
  expected: EditorSnapshotIdentity,
  actual: EditorSnapshotIdentity
): boolean {
  return (
    expected.uri === actual.uri &&
    expected.version === actual.version &&
    expected.legendDigest === actual.legendDigest
  );
}

function validateLegend(value: unknown): EditorSemanticTokenLegend {
  if (!value || typeof value !== "object") {
    throw new EditorWorkerProtocolError("Merman returned an invalid semantic token legend.");
  }
  const candidate = value as Partial<EditorSemanticTokenLegend>;
  const tokenTypes = validateLegendNames(candidate.tokenTypes, "token types");
  const tokenModifiers = validateLegendNames(candidate.tokenModifiers, "token modifiers");
  if (tokenTypes.length === 0 || tokenModifiers.length > 31) {
    throw new EditorWorkerProtocolError("Merman returned an unsupported semantic token legend.");
  }
  return Object.freeze({
    tokenTypes: Object.freeze(tokenTypes) as unknown as string[],
    tokenModifiers: Object.freeze(tokenModifiers) as unknown as string[],
  });
}

function validateLegendNames(value: unknown, label: string): string[] {
  if (
    !Array.isArray(value) ||
    value.some((name) => typeof name !== "string" || name.length === 0) ||
    new Set(value).size !== value.length
  ) {
    throw new EditorWorkerProtocolError(`Merman returned invalid semantic ${label}.`);
  }
  return [...value];
}

function parseResponse(value: unknown): EditorWorkerResponse | null {
  if (!value || typeof value !== "object") return null;
  const candidate = value as Partial<EditorWorkerResponse>;
  if (
    candidate.protocol !== EDITOR_WORKER_PROTOCOL ||
    !Number.isSafeInteger(candidate.requestId) ||
    (candidate.type !== "error" &&
      candidate.type !== "queryResult" &&
      candidate.type !== "ready" &&
      candidate.type !== "result")
  ) {
    return null;
  }
  if (candidate.type === "error") {
    const error = candidate as Record<string, unknown>;
    return (
      isWorkerErrorCode(error.code) &&
      typeof error.message === "string" &&
      (error.detail === null || typeof error.detail === "string") &&
      (error.nativeCode === null || typeof error.nativeCode === "string")
    )
      ? (candidate as EditorWorkerResponse)
      : null;
  }
  if (candidate.type === "ready") {
    const ready = candidate as Record<string, unknown>;
    return (
      Number.isSafeInteger(ready.nativeAbi) &&
      Number.isSafeInteger(ready.editorSchema) &&
      typeof ready.legendDigest === "string" &&
      ready.legendDigest.length > 0 &&
      ready.legend !== null &&
      typeof ready.legend === "object"
    )
      ? (candidate as EditorWorkerResponse)
      : null;
  }
  if (candidate.type === "queryResult") {
    const result = candidate as Record<string, unknown>;
    return (
      typeof result.uri === "string" &&
      Number.isSafeInteger(result.version) &&
      typeof result.legendDigest === "string" &&
      result.legendDigest.length > 0
    )
      ? (candidate as EditorWorkerResponse)
      : null;
  }
  return candidate as EditorWorkerResponse;
}

function workerResponseError(
  code: EditorWorkerErrorCode,
  message: string,
  detail: string | null,
  nativeCode: string | null
): Error {
  if (code === "STALE_DOCUMENT") {
    return new StaleLanguageSnapshotError(message, detail);
  }
  return new EditorWorkerProtocolError(message, code, detail, nativeCode);
}

function isWorkerErrorCode(value: unknown): value is EditorWorkerErrorCode {
  return (
    value === "INITIALIZATION_FAILED" ||
    value === "INVALID_STATE" ||
    value === "OPERATION_REJECTED" ||
    value === "PROTOCOL_MISMATCH" ||
    value === "QUERY_FAILED" ||
    value === "STALE_DOCUMENT"
  );
}

function isFatalWorkerError(code: string): boolean {
  return (
    code === "INITIALIZATION_FAILED" ||
    code === "INVALID_STATE" ||
    code === "PROTOCOL_MISMATCH"
  );
}

function abortError(message = "The editor request was canceled."): Error {
  const error = new Error(message);
  error.name = "AbortError";
  return error;
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
