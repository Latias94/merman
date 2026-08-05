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
  readonly requestTimeoutMs?: number;
  readonly tombstoneLimit?: number;
}

const DEFAULT_EDITOR_WORKER_REQUEST_TIMEOUT_MS = 30_000;
const DEFAULT_EDITOR_WORKER_TOMBSTONE_LIMIT = 256;

export interface EditorLanguageIdentity {
  readonly legend: ReadonlyEditorSemanticTokenLegend;
  readonly legendDigest: string;
  readonly transportApiVersion: number;
}

export interface ReadonlyEditorSemanticTokenLegend {
  readonly tokenTypes: readonly string[];
  readonly tokenModifiers: readonly string[];
}

interface EditorSnapshotIdentity extends EditorDocumentIdentity {
  readonly legendDigest: string;
}

type PendingExpected = "queryResult" | "ready" | "result";

interface PendingRequest {
  readonly document?: EditorSnapshotIdentity;
  readonly expected: PendingExpected;
  readonly label: string;
  readonly query?: EditorWorkerQuery;
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
  cancellation?: { dispose(): void };
  deadline?: ReturnType<typeof setTimeout>;
  sent: boolean;
}

interface RequestTombstone {
  readonly document?: EditorSnapshotIdentity;
  readonly expected: PendingExpected;
  readonly query?: EditorWorkerQuery;
  readonly reason: "cancelled" | "completed";
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
    message = "The editor result belongs to an obsolete document or legend.",
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
  expectedLegendDigest: string,
  options: EditorWorkerClientOptions = {},
): MermanLanguageWorkerClient {
  return new WorkerClient(worker, expectedLegendDigest, options);
}

export function startMermanLanguageWorkerClient(
  worker: EditorWorkerPort,
  expectedLegendDigest: string,
  timeoutMs = DEFAULT_EDITOR_WORKER_REQUEST_TIMEOUT_MS,
): MermanLanguageWorkerStartup {
  let client: MermanLanguageWorkerClient;
  try {
    client = createMermanLanguageWorkerClient(worker, expectedLegendDigest, {
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
  private readonly pending = new Map<number, PendingRequest>();
  private readonly tombstones: RequestTombstoneLedger;
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
  private readonly expectedLegendDigest: string;

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

    const pending = this.pending.get(response.requestId);
    if (!pending) {
      this.handleResponseWithoutPending(response);
      return;
    }
    if (response.type !== "error" && response.type !== pending.expected) {
      this.poison(
        new EditorWorkerProtocolError(
          `Editor worker returned ${response.type} while ${pending.expected} was required.`,
          "PROTOCOL_MISMATCH",
        ),
      );
      return;
    }

    if (response.type === "error") {
      const failure = workerResponseError(
        response.code,
        response.message,
        response.detail,
        response.nativeCode,
      );
      this.completePending(response.requestId, pending);
      pending.reject(failure);
      if (isFatalWorkerError(response.code)) this.poison(failure);
      return;
    }

    try {
      switch (response.type) {
        case "ready":
          if (response.legendDigest !== this.expectedLegendDigest) {
            throw new EditorWorkerProtocolError(
              `Merman editor legend ${response.legendDigest} does not match ${this.expectedLegendDigest}.`,
            );
          }
          this.completePending(response.requestId, pending);
          pending.resolve(
            Object.freeze({
              legend: Object.freeze({
                tokenTypes: Object.freeze([...response.legend.tokenTypes]),
                tokenModifiers: Object.freeze([
                  ...response.legend.tokenModifiers,
                ]),
              }),
              legendDigest: response.legendDigest,
              transportApiVersion: response.transportApiVersion,
            }) satisfies EditorLanguageIdentity,
          );
          return;
        case "result":
          this.completePending(response.requestId, pending);
          pending.resolve(undefined);
          return;
        case "queryResult": {
          if (
            !pending.document ||
            !pending.query ||
            !sameSnapshotIdentity(pending.document, response) ||
            !this.isCurrentDocument(pending.document)
          ) {
            this.completePending(response.requestId, pending);
            pending.reject(new StaleLanguageSnapshotError());
            return;
          }
          const result = projectEditorWorkerQueryResult(
            pending.query,
            response.result,
          );
          this.completePending(response.requestId, pending);
          pending.resolve(result);
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
    expectedLegendDigest: string,
    options: EditorWorkerClientOptions,
  ) {
    if (!expectedLegendDigest) {
      throw new EditorWorkerProtocolError(
        "A generated editor legend digest is required.",
      );
    }
    this.requestTimeoutMs = positiveOption(
      options.requestTimeoutMs ?? DEFAULT_EDITOR_WORKER_REQUEST_TIMEOUT_MS,
      "editor worker request timeout",
    );
    const tombstoneLimit = positiveOption(
      options.tombstoneLimit ?? DEFAULT_EDITOR_WORKER_TOMBSTONE_LIMIT,
      "editor worker tombstone limit",
    );
    this.worker = worker;
    this.expectedLegendDigest = expectedLegendDigest;
    this.tombstones = new RequestTombstoneLedger(tombstoneLimit);
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
    const snapshot = snapshotIdentity(identity, this.expectedLegendDigest);
    return this.request<EditorWorkerQueryResult<Query>>(
      {
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId,
        type: "query",
        uri: identity.uri,
        version: identity.version,
        legendDigest: this.expectedLegendDigest,
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

    return new Promise<Result>((resolve, reject) => {
      const pending: PendingRequest = {
        document: options.document,
        expected: options.expected,
        label: options.label,
        query: options.query,
        resolve: (value) => resolve(value as Result),
        reject,
        sent: false,
      };
      this.pending.set(options.requestId, pending);

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
          this.pending.delete(options.requestId);
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
      if (this.pending.get(options.requestId) !== pending) return;

      pending.sent = true;
      pending.deadline = setTimeout(() => {
        if (this.pending.get(options.requestId) !== pending) return;
        this.pending.delete(options.requestId);
        disposePending(pending);
        const failure = new EditorWorkerProtocolError(
          `Merman editor worker ${pending.label} timed out after ${this.requestTimeoutMs} ms.`,
        );
        pending.reject(failure);
        this.poison(failure);
      }, this.requestTimeoutMs);

      try {
        this.worker.postMessage(message);
      } catch (error) {
        this.pending.delete(options.requestId);
        disposePending(pending);
        const failure = asError(error);
        pending.reject(failure);
        this.poison(failure);
      }
    });
  }

  private cancelRequest(requestId: number, pending: PendingRequest): void {
    if (this.pending.get(requestId) !== pending) return;
    this.pending.delete(requestId);
    disposePending(pending);
    if (pending.sent) {
      this.tombstones.add(
        requestId,
        tombstoneFromPending(pending, "cancelled"),
      );
    }
    pending.reject(abortError());
  }

  private completePending(requestId: number, pending: PendingRequest): void {
    this.pending.delete(requestId);
    disposePending(pending);
    this.tombstones.add(requestId, tombstoneFromPending(pending, "completed"));
  }

  private handleResponseWithoutPending(response: EditorWorkerResponse): void {
    const tombstone = this.tombstones.get(response.requestId);
    if (!tombstone) {
      this.poison(
        new EditorWorkerProtocolError(
          `Editor worker returned an unknown request ID ${response.requestId}.`,
          "PROTOCOL_MISMATCH",
        ),
      );
      return;
    }
    if (tombstone.reason !== "cancelled") {
      this.poison(
        new EditorWorkerProtocolError(
          `Editor worker returned a duplicate response for request ${response.requestId}.`,
          "PROTOCOL_MISMATCH",
        ),
      );
      return;
    }
    if (response.type === "error" && isFatalWorkerError(response.code)) {
      this.poison(
        workerResponseError(
          response.code,
          response.message,
          response.detail,
          response.nativeCode,
        ),
      );
      return;
    }
    try {
      assertTombstoneResponse(tombstone, response);
    } catch (error) {
      this.poison(
        protocolProjectionError(
          error,
          "late editor worker response",
          "PROTOCOL_MISMATCH",
        ),
      );
      return;
    }
    this.tombstones.add(response.requestId, {
      ...tombstone,
      reason: "completed",
    });
  }

  private isCurrentDocument(
    document: EditorDocumentIdentity &
      Partial<Pick<EditorSnapshotIdentity, "legendDigest">>,
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
    for (const pending of this.pending.values()) {
      disposePending(pending);
      pending.reject(error);
    }
    this.pending.clear();
  }

  private poison(error: Error): void {
    if (this.failure || this.disposed) return;
    this.failure = error;
    this.failAll(error);
    if (this.queuedSynchronization) {
      rejectSynchronization(this.queuedSynchronization, error);
      this.queuedSynchronization = null;
    }
    this.closeTransport();
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

class RequestTombstoneLedger {
  private readonly entries = new Map<number, RequestTombstone>();
  private readonly limit: number;

  constructor(limit: number) {
    this.limit = limit;
  }

  add(requestId: number, tombstone: RequestTombstone): void {
    this.entries.delete(requestId);
    this.entries.set(requestId, tombstone);
    while (this.entries.size > this.limit) {
      const oldest = this.entries.keys().next().value as number | undefined;
      if (oldest === undefined) return;
      this.entries.delete(oldest);
    }
  }

  get(requestId: number): RequestTombstone | undefined {
    return this.entries.get(requestId);
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

function identityOf(document: EditorDocumentSnapshot): EditorDocumentIdentity {
  return { uri: document.uri, version: document.version };
}

function snapshotIdentity(
  document: EditorDocumentIdentity,
  legendDigest: string,
): EditorSnapshotIdentity {
  return { uri: document.uri, version: document.version, legendDigest };
}

function sameSnapshotIdentity(
  expected: EditorSnapshotIdentity,
  actual: EditorSnapshotIdentity,
): boolean {
  return (
    expected.uri === actual.uri &&
    expected.version === actual.version &&
    expected.legendDigest === actual.legendDigest
  );
}

function disposePending(pending: PendingRequest): void {
  if (pending.deadline !== undefined) clearTimeout(pending.deadline);
  pending.cancellation?.dispose();
}

function tombstoneFromPending(
  pending: PendingRequest,
  reason: RequestTombstone["reason"],
): RequestTombstone {
  return {
    document: pending.document,
    expected: pending.expected,
    query: pending.query,
    reason,
  };
}

function assertTombstoneResponse(
  tombstone: RequestTombstone,
  response: EditorWorkerResponse,
): void {
  if (response.type === "error") return;
  if (response.type !== tombstone.expected) {
    throw new EditorWorkerProtocolError(
      `Late editor worker response ${response.type} does not match ${tombstone.expected}.`,
      "PROTOCOL_MISMATCH",
    );
  }
  if (response.type !== "queryResult") return;
  if (
    !tombstone.document ||
    !tombstone.query ||
    !sameSnapshotIdentity(tombstone.document, response)
  ) {
    throw new EditorWorkerProtocolError(
      "Late editor worker query result has the wrong snapshot identity.",
      "PROTOCOL_MISMATCH",
    );
  }
  projectEditorWorkerQueryResult(tombstone.query, response.result);
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
