import {
  EDITOR_WORKER_PROTOCOL,
  EditorWorkerProtocolProjectionError,
  projectEditorDocumentIdentity,
  projectEditorDocumentSnapshot,
  projectEditorWorkerQuery,
  projectEditorWorkerQueryResult,
  projectEditorWorkerResponse,
  type EditorDocumentIdentity,
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
  addEventListener(
    type: "error" | "message" | "messageerror",
    listener: (event: any) => void,
  ): void;
  removeEventListener(
    type: "error" | "message" | "messageerror",
    listener: (event: any) => void,
  ): void;
  postMessage(message: EditorWorkerRequest): void;
  terminate(): void;
}

export interface MermanLanguageWorkerClient {
  initialize(): Promise<EditorLanguageIdentity>;
  onDidFail(listener: (error: Error) => void): { dispose(): void };
  openDocument(document: EditorDocumentSnapshot): Promise<void>;
  changeDocument(document: EditorDocumentSnapshot): Promise<void>;
  query<Query extends EditorWorkerQuery>(
    document: EditorDocumentIdentity,
    query: Query,
    cancellation?: EditorCancellationToken,
  ): Promise<EditorWorkerQueryResult<Query>>;
  dispose(): void;
}

export interface MermanLanguageWorkerStartup {
  readonly client: MermanLanguageWorkerClient;
  readonly ready: Promise<EditorLanguageIdentity>;
}

export interface EditorWorkerClientOptions {
  readonly queryCapacity?: number;
  readonly requestTimeoutMs?: number;
  readonly tombstoneLimit?: number;
}

const DEFAULT_EDITOR_WORKER_REQUEST_TIMEOUT_MS = 30_000;
const DEFAULT_EDITOR_WORKER_QUERY_CAPACITY = 256;
const DEFAULT_EDITOR_WORKER_TOMBSTONE_LIMIT = 256;

export interface EditorLanguageIdentity {
  readonly completionTriggerCharacters: readonly string[];
  readonly transportApiVersion: number;
}

type EditorSnapshotIdentity = EditorDocumentIdentity;

type PendingExpected = "queryResult" | "ready" | "result";

interface RequestSettlement {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
}

interface TransportRequest {
  readonly document?: EditorSnapshotIdentity;
  readonly expected: PendingExpected;
  readonly label: string;
  readonly query?: EditorWorkerQuery;
  settlement: RequestSettlement | null;
  cancellation?: { dispose(): void };
  deadline?: ReturnType<typeof setTimeout>;
  sent: boolean;
}

interface Deferred<Value> {
  readonly promise: Promise<Value>;
  readonly resolve: (value: Value) => void;
  readonly reject: (error: Error) => void;
}

interface SynchronizationEntry extends Deferred<void> {
  snapshot: EditorDocumentSnapshot | null;
  superseded: boolean;
  readonly type: "didChange" | "didOpen";
}

export class StaleLanguageSnapshotError extends Error {
  readonly code = "STALE_DOCUMENT" as const;
  readonly detail: string | null;

  constructor(
    message = "The editor result belongs to an obsolete document.",
    detail: string | null = null,
  ) {
    super(message);
    this.name = "StaleLanguageSnapshotError";
    this.detail = detail;
  }
}

export class EditorWorkerProtocolError extends Error {
  readonly code: EditorWorkerErrorCode | "CLIENT_PROTOCOL";
  readonly detail: string | null;
  readonly nativeCode: string | null;

  constructor(
    message: string,
    code: EditorWorkerErrorCode | "CLIENT_PROTOCOL" = "CLIENT_PROTOCOL",
    detail: string | null = null,
    nativeCode: string | null = null,
  ) {
    super(message);
    this.name = "EditorWorkerProtocolError";
    this.code = code;
    this.detail = detail;
    this.nativeCode = nativeCode;
  }
}

export function createMermanLanguageWorkerClient(
  worker: EditorWorkerPort,
  options: EditorWorkerClientOptions = {},
): MermanLanguageWorkerClient {
  return new WorkerClient(worker, options);
}

export function startMermanLanguageWorkerClient(
  worker: EditorWorkerPort,
  timeoutMs = DEFAULT_EDITOR_WORKER_REQUEST_TIMEOUT_MS,
): MermanLanguageWorkerStartup {
  let client: MermanLanguageWorkerClient;
  try {
    client = createMermanLanguageWorkerClient(worker, {
      requestTimeoutMs: timeoutMs,
    });
  } catch (error) {
    worker.terminate();
    throw error;
  }
  const ready = client.initialize().catch((error: unknown) => {
    client.dispose();
    throw error;
  });
  return Object.freeze({ client, ready });
}

class WorkerClient implements MermanLanguageWorkerClient {
  private readonly failureListeners = new Set<(error: Error) => void>();
  private readonly outstanding = new Map<number, TransportRequest>();
  private readonly completed: CompletedRequestLedger;
  private readonly queryCapacity: number;
  private outstandingQueries = 0;
  private currentDocument: EditorDocumentIdentity | null = null;
  private disposed = false;
  private failure: Error | null = null;
  private inFlightSynchronization: SynchronizationEntry | null = null;
  private initialized = false;
  private initializePromise: Promise<EditorLanguageIdentity> | null = null;
  private nextRequestId = 1;
  private queuedSynchronization: SynchronizationEntry | null = null;
  private readonly requestTimeoutMs: number;
  private transportClosed = false;
  private readonly worker: EditorWorkerPort;

  private readonly handleError = (event: { message?: unknown }) => {
    const detail =
      typeof event.message === "string" ? `: ${event.message}` : "";
    this.poison(new Error(`Merman editor worker failed${detail}`));
  };

  private readonly handleMessageError = () => {
    this.poison(
      new EditorWorkerProtocolError(
        "The browser could not decode an editor worker response.",
        "PROTOCOL_MISMATCH",
      ),
    );
  };

  private readonly handleMessage = (event: { data?: unknown }) => {
    let response: EditorWorkerResponse;
    try {
      response = projectEditorWorkerResponse(event.data);
    } catch (error) {
      this.poison(
        protocolProjectionError(
          error,
          "editor worker response",
          "PROTOCOL_MISMATCH",
        ),
      );
      return;
    }

    const pending = this.outstanding.get(response.requestId);
    if (!pending) {
      this.handleResponseWithoutPending(response);
      return;
    }
    if (response.type === "error") {
      const failure = workerResponseError(
        response.code,
        response.message,
        response.detail,
        response.nativeCode,
      );
      const settlement = pending.settlement;
      this.completePending(response.requestId, pending);
      settlement?.reject(failure);
      if (isFatalWorkerError(response.code)) this.poison(failure);
      return;
    }

    try {
      const result = projectRequestResult(pending, response);
      const settlement = pending.settlement;
      if (!settlement) {
        this.completePending(response.requestId, pending);
        return;
      }
      switch (response.type) {
        case "ready":
          this.completePending(response.requestId, pending);
          settlement.resolve(
            Object.freeze({
              completionTriggerCharacters: Object.freeze([
                ...response.completionTriggerCharacters,
              ]),
              transportApiVersion: response.transportApiVersion,
            }) satisfies EditorLanguageIdentity,
          );
          return;
        case "result":
          this.completePending(response.requestId, pending);
          settlement.resolve(undefined);
          return;
        case "queryResult": {
          if (
            !pending.document ||
            !this.isCurrentDocument(pending.document)
          ) {
            this.completePending(response.requestId, pending);
            settlement.reject(new StaleLanguageSnapshotError());
            return;
          }
          this.completePending(response.requestId, pending);
          settlement.resolve(result);
          return;
        }
      }
    } catch (error) {
      this.poison(
        protocolProjectionError(
          error,
          "editor worker result",
          "PROTOCOL_MISMATCH",
        ),
      );
    }
  };

  constructor(
    worker: EditorWorkerPort,
    options: EditorWorkerClientOptions,
  ) {
    this.requestTimeoutMs = positiveOption(
      options.requestTimeoutMs ?? DEFAULT_EDITOR_WORKER_REQUEST_TIMEOUT_MS,
      "editor worker request timeout",
    );
    this.queryCapacity = positiveOption(
      options.queryCapacity ?? DEFAULT_EDITOR_WORKER_QUERY_CAPACITY,
      "editor worker query capacity",
    );
    const tombstoneLimit = positiveOption(
      options.tombstoneLimit ?? DEFAULT_EDITOR_WORKER_TOMBSTONE_LIMIT,
      "editor worker tombstone limit",
    );
    this.worker = worker;
    this.completed = new CompletedRequestLedger(tombstoneLimit);
    worker.addEventListener("error", this.handleError);
    worker.addEventListener("message", this.handleMessage);
    worker.addEventListener("messageerror", this.handleMessageError);
  }

  initialize(): Promise<EditorLanguageIdentity> {
    this.assertActive();
    if (this.initializePromise) return this.initializePromise;

    const requestId = this.allocateRequestId();
    this.initializePromise = this.request<EditorLanguageIdentity>(
      {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "initialize",
      },
      { expected: "ready", label: "initialization", requestId },
    )
      .then((identity) => {
        this.initialized = true;
        return identity;
      })
      .catch((error: unknown) => {
        const failure = asError(error);
        this.poison(failure);
        throw failure;
      });
    return this.initializePromise;
  }

  onDidFail(listener: (error: Error) => void): { dispose(): void } {
    if (this.disposed) return { dispose() {} };
    if (this.failure) {
      listener(this.failure);
      return { dispose() {} };
    }

    this.failureListeners.add(listener);
    let subscribed = true;
    return {
      dispose: () => {
        if (!subscribed) return;
        subscribed = false;
        this.failureListeners.delete(listener);
      },
    };
  }

  openDocument(document: EditorDocumentSnapshot): Promise<void> {
    this.assertReady();
    const snapshot = projectSnapshotForClient(document);
    if (this.currentDocument) {
      return Promise.reject(
        new EditorWorkerProtocolError(
          "The editor worker already owns a document.",
        ),
      );
    }
    this.currentDocument = identityOf(snapshot);
    const entry = createSynchronizationEntry("didOpen", snapshot);
    this.startSynchronization(entry);
    return entry.promise;
  }

  changeDocument(document: EditorDocumentSnapshot): Promise<void> {
    this.assertReady();
    const snapshot = projectSnapshotForClient(document);
    const current = this.currentDocument;
    if (!current || current.uri !== snapshot.uri) {
      return Promise.reject(
        new EditorWorkerProtocolError(
          "The editor worker does not own this document URI.",
        ),
      );
    }
    if (snapshot.version <= current.version) {
      return Promise.reject(
        new EditorWorkerProtocolError(
          `Document version ${snapshot.version} must be newer than ${current.version}.`,
        ),
      );
    }

    this.currentDocument = identityOf(snapshot);
    const entry = createSynchronizationEntry("didChange", snapshot);
    if (this.inFlightSynchronization) {
      if (this.inFlightSynchronization.type === "didChange") {
        this.inFlightSynchronization.superseded = true;
      }
      if (this.queuedSynchronization) {
        rejectSupersededSynchronization(this.queuedSynchronization);
      }
      this.queuedSynchronization = entry;
    } else {
      this.startSynchronization(entry);
    }
    return entry.promise;
  }

  async query<Query extends EditorWorkerQuery>(
    document: EditorDocumentIdentity,
    query: Query,
    cancellation?: EditorCancellationToken,
  ): Promise<EditorWorkerQueryResult<Query>> {
    this.assertReady();
    let identity: EditorDocumentIdentity;
    try {
      identity = projectEditorDocumentIdentity(document);
    } catch (error) {
      throw protocolProjectionError(error, "editor document identity");
    }
    let projectedQuery: Query;
    try {
      projectedQuery = projectEditorWorkerQuery(query) as Query;
    } catch (error) {
      throw protocolProjectionError(error, "editor query");
    }
    if (!this.isCurrentDocument(identity)) {
      throw new StaleLanguageSnapshotError();
    }
    if (cancellation?.isCancellationRequested) throw abortError();

    const synchronization =
      this.queuedSynchronization ?? this.inFlightSynchronization;
    if (synchronization) {
      await waitForCancellation(synchronization.promise, cancellation);
    }
    if (cancellation?.isCancellationRequested) throw abortError();
    if (!this.isCurrentDocument(identity)) {
      throw new StaleLanguageSnapshotError();
    }

    const requestId = this.allocateRequestId();
    const snapshot = identityOf(identity);
    return this.request<EditorWorkerQueryResult<Query>>(
      {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "query",
        uri: identity.uri,
        version: identity.version,
        query: projectedQuery,
      },
      {
        cancellation,
        document: snapshot,
        expected: "queryResult",
        label: `${projectedQuery.kind} query`,
        query: projectedQuery,
        requestId,
      },
    );
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.failureListeners.clear();
    const failure = new Error("Merman editor worker was disposed.");
    this.failAll(failure);
    if (this.queuedSynchronization) {
      rejectSynchronization(this.queuedSynchronization, failure);
      this.queuedSynchronization = null;
    }
    this.closeTransport();
    this.currentDocument = null;
  }

  private allocateRequestId(): number {
    if (!Number.isSafeInteger(this.nextRequestId) || this.nextRequestId < 1) {
      const failure = new EditorWorkerProtocolError(
        "Editor worker request IDs are exhausted.",
      );
      this.poison(failure);
      throw failure;
    }
    const requestId = this.nextRequestId;
    this.nextRequestId += 1;
    return requestId;
  }

  private startSynchronization(entry: SynchronizationEntry): void {
    const snapshot = entry.snapshot;
    if (!snapshot) return;
    this.inFlightSynchronization = entry;
    const requestId = this.allocateRequestId();
    let synchronization: Promise<void>;
    try {
      synchronization = this.request<void>(
        {
          protocol: EDITOR_WORKER_PROTOCOL,
          requestId,
          type: entry.type,
          document: snapshot,
        },
        {
          expected: "result",
          label: `${entry.type} synchronization`,
          requestId,
        },
      );
    } catch (error) {
      this.finishSynchronizationFailure(entry, asError(error));
      return;
    }

    void synchronization.then(
      () => this.finishSynchronizationSuccess(entry),
      (error: unknown) =>
        this.finishSynchronizationFailure(entry, asError(error)),
    );
  }

  private finishSynchronizationSuccess(entry: SynchronizationEntry): void {
    if (this.inFlightSynchronization !== entry) return;
    entry.snapshot = null;
    this.inFlightSynchronization = null;
    if (entry.superseded) {
      entry.reject(new StaleLanguageSnapshotError());
    } else {
      entry.resolve(undefined);
    }
    const next = this.queuedSynchronization;
    this.queuedSynchronization = null;
    if (next) this.startSynchronization(next);
  }

  private finishSynchronizationFailure(
    entry: SynchronizationEntry,
    failure: Error,
  ): void {
    if (this.inFlightSynchronization === entry) {
      entry.snapshot = null;
      this.inFlightSynchronization = null;
    }
    entry.reject(failure);
    if (this.queuedSynchronization) {
      rejectSynchronization(this.queuedSynchronization, failure);
      this.queuedSynchronization = null;
    }
    this.poison(failure);
  }

  private request<Result>(
    message: EditorWorkerRequest,
    options: {
      readonly cancellation?: EditorCancellationToken;
      readonly document?: EditorSnapshotIdentity;
      readonly expected: PendingExpected;
      readonly label: string;
      readonly query?: EditorWorkerQuery;
      readonly requestId: number;
    },
  ): Promise<Result> {
    this.assertActive();
    if (options.query && this.outstandingQueries >= this.queryCapacity) {
      return Promise.reject(
        new EditorWorkerProtocolError(
          `The editor worker query capacity of ${this.queryCapacity} is exhausted.`,
          "OPERATION_REJECTED",
        ),
      );
    }

    return new Promise<Result>((resolve, reject) => {
      const pending: TransportRequest = {
        document: options.document,
        expected: options.expected,
        label: options.label,
        query: options.query,
        settlement: {
          resolve: (value) => resolve(value as Result),
          reject,
        },
        sent: false,
      };
      this.outstanding.set(options.requestId, pending);
      if (pending.query) this.outstandingQueries += 1;

      if (options.cancellation) {
        let cancelledWhileSubscribing = false;
        try {
          pending.cancellation = options.cancellation.onCancellationRequested(
            () => {
              if (!pending.cancellation) {
                cancelledWhileSubscribing = true;
                return;
              }
              this.cancelRequest(options.requestId, pending);
            },
          );
        } catch (error) {
          this.releaseRequest(options.requestId, pending);
          reject(asError(error));
          return;
        }
        if (
          cancelledWhileSubscribing ||
          options.cancellation.isCancellationRequested
        ) {
          this.cancelRequest(options.requestId, pending);
        }
      }
      if (this.outstanding.get(options.requestId) !== pending) return;

      pending.sent = true;
      pending.deadline = setTimeout(() => {
        if (this.outstanding.get(options.requestId) !== pending) return;
        const failure = new EditorWorkerProtocolError(
          `Merman editor worker ${pending.label} timed out after ${this.requestTimeoutMs} ms.`,
        );
        this.poison(failure);
      }, this.requestTimeoutMs);

      try {
        this.worker.postMessage(message);
      } catch (error) {
        const failure = asError(error);
        this.poison(failure);
      }
    });
  }

  private cancelRequest(requestId: number, pending: TransportRequest): void {
    if (this.outstanding.get(requestId) !== pending || !pending.settlement) {
      return;
    }
    const settlement = pending.settlement;
    pending.settlement = null;
    if (pending.sent) {
      pending.cancellation?.dispose();
      pending.cancellation = undefined;
    } else {
      this.releaseRequest(requestId, pending);
    }
    settlement.reject(abortError());
  }

  private releaseRequest(requestId: number, pending: TransportRequest): void {
    if (this.outstanding.get(requestId) !== pending) return;
    this.outstanding.delete(requestId);
    if (pending.query) this.outstandingQueries -= 1;
    disposeRequest(pending);
  }

  private completePending(requestId: number, pending: TransportRequest): void {
    this.releaseRequest(requestId, pending);
    this.completed.add(requestId);
  }

  private handleResponseWithoutPending(response: EditorWorkerResponse): void {
    this.poison(
      new EditorWorkerProtocolError(
        this.completed.has(response.requestId)
          ? `Editor worker returned a duplicate response for request ${response.requestId}.`
          : `Editor worker returned an unknown request ID ${response.requestId}.`,
        "PROTOCOL_MISMATCH",
      ),
    );
  }

  private isCurrentDocument(
    document: EditorDocumentIdentity,
  ): boolean {
    return (
      this.currentDocument?.uri === document.uri &&
      this.currentDocument.version === document.version
    );
  }

  private assertActive(): void {
    if (this.failure) throw this.failure;
    if (this.disposed) throw new Error("Merman editor worker was disposed.");
  }

  private assertReady(): void {
    this.assertActive();
    if (!this.initialized) {
      throw new EditorWorkerProtocolError(
        "Initialize the Merman editor worker before opening a document.",
      );
    }
  }

  private failAll(error: Error): void {
    for (const pending of this.outstanding.values()) {
      disposeRequest(pending);
      pending.settlement?.reject(error);
    }
    this.outstanding.clear();
    this.outstandingQueries = 0;
  }

  private poison(error: Error): void {
    if (this.failure || this.disposed) return;
    this.failure = error;
    const listeners = [...this.failureListeners];
    this.failureListeners.clear();
    this.failAll(error);
    if (this.queuedSynchronization) {
      rejectSynchronization(this.queuedSynchronization, error);
      this.queuedSynchronization = null;
    }
    this.closeTransport();
    for (const listener of listeners) listener(error);
  }

  private closeTransport(): void {
    if (this.transportClosed) return;
    this.transportClosed = true;
    this.worker.removeEventListener("error", this.handleError);
    this.worker.removeEventListener("message", this.handleMessage);
    this.worker.removeEventListener("messageerror", this.handleMessageError);
    try {
      this.worker.postMessage({
        protocol: EDITOR_WORKER_PROTOCOL,
        type: "dispose",
      });
    } catch {
      // Termination remains the final ownership boundary.
    }
    this.worker.terminate();
  }
}

class CompletedRequestLedger {
  private readonly entries = new Set<number>();
  private readonly limit: number;

  constructor(limit: number) {
    this.limit = limit;
  }

  add(requestId: number): void {
    this.entries.add(requestId);
    while (this.entries.size > this.limit) {
      const oldest = this.entries.keys().next().value as number | undefined;
      if (oldest === undefined) return;
      this.entries.delete(oldest);
    }
  }

  has(requestId: number): boolean {
    return this.entries.has(requestId);
  }
}

function positiveOption(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new EditorWorkerProtocolError(`A positive ${label} is required.`);
  }
  return value;
}

function createDeferred<Value>(): Deferred<Value> {
  let resolve!: (value: Value) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<Value>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function createSynchronizationEntry(
  type: SynchronizationEntry["type"],
  snapshot: EditorDocumentSnapshot,
): SynchronizationEntry {
  return {
    ...createDeferred<void>(),
    snapshot,
    superseded: false,
    type,
  };
}

function rejectSynchronization(
  entry: SynchronizationEntry,
  error: Error,
): void {
  entry.snapshot = null;
  entry.reject(error);
}

function rejectSupersededSynchronization(entry: SynchronizationEntry): void {
  rejectSynchronization(entry, new StaleLanguageSnapshotError());
}

function projectSnapshotForClient(
  document: EditorDocumentSnapshot,
): EditorDocumentSnapshot {
  try {
    return projectEditorDocumentSnapshot(document);
  } catch (error) {
    throw protocolProjectionError(error, "editor document snapshot");
  }
}

function identityOf(document: EditorDocumentIdentity): EditorDocumentIdentity {
  return { uri: document.uri, version: document.version };
}

function sameSnapshotIdentity(
  expected: EditorSnapshotIdentity,
  actual: EditorSnapshotIdentity,
): boolean {
  return (
    expected.uri === actual.uri &&
    expected.version === actual.version
  );
}

function disposeRequest(pending: TransportRequest): void {
  if (pending.deadline !== undefined) clearTimeout(pending.deadline);
  pending.cancellation?.dispose();
}

function projectRequestResult(
  pending: TransportRequest,
  response: Exclude<EditorWorkerResponse, { type: "error" }>,
): unknown {
  if (response.type !== pending.expected) {
    throw new EditorWorkerProtocolError(
      `Editor worker returned ${response.type} while ${pending.expected} was required.`,
      "PROTOCOL_MISMATCH",
    );
  }
  if (response.type !== "queryResult") return;
  if (
    !pending.document ||
    !pending.query ||
    !sameSnapshotIdentity(pending.document, response)
  ) {
    throw new EditorWorkerProtocolError(
      "Editor worker query result has the wrong snapshot identity.",
      "PROTOCOL_MISMATCH",
    );
  }
  return projectEditorWorkerQueryResult(pending.query, response.result);
}

async function waitForCancellation<Value>(
  promise: Promise<Value>,
  cancellation?: EditorCancellationToken,
): Promise<Value> {
  if (!cancellation) return promise;
  if (cancellation.isCancellationRequested) throw abortError();
  const cancellationResult = createDeferred<Value>();
  const subscription = cancellation.onCancellationRequested(() => {
    cancellationResult.reject(abortError());
  });
  try {
    if (cancellation.isCancellationRequested) throw abortError();
    return await Promise.race([promise, cancellationResult.promise]);
  } finally {
    subscription.dispose();
  }
}

function workerResponseError(
  code: EditorWorkerErrorCode,
  message: string,
  detail: string | null,
  nativeCode: string | null,
): Error {
  if (code === "STALE_DOCUMENT") {
    return new StaleLanguageSnapshotError(message, detail);
  }
  return new EditorWorkerProtocolError(message, code, detail, nativeCode);
}

function isFatalWorkerError(code: EditorWorkerErrorCode): boolean {
  return (
    code === "INITIALIZATION_FAILED" ||
    code === "INVALID_STATE" ||
    code === "PROTOCOL_MISMATCH"
  );
}

function protocolProjectionError(
  error: unknown,
  label: string,
  code: EditorWorkerErrorCode | "CLIENT_PROTOCOL" = "CLIENT_PROTOCOL",
): Error {
  if (error instanceof EditorWorkerProtocolError) {
    if (code === "CLIENT_PROTOCOL" || error.code !== "CLIENT_PROTOCOL") {
      return error;
    }
    return new EditorWorkerProtocolError(
      error.message,
      code,
      error.detail,
      error.nativeCode,
    );
  }
  if (error instanceof EditorWorkerProtocolProjectionError) {
    return new EditorWorkerProtocolError(
      `Invalid ${label}: ${error.message}`,
      code,
    );
  }
  return asError(error);
}

function abortError(message = "The editor request was canceled."): Error {
  const error = new Error(message);
  error.name = "AbortError";
  return error;
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
