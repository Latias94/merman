import {
  MERMAID_SYNTAX_WORKER_PROTOCOL,
  projectSyntaxDocumentSnapshot,
  projectSyntaxWorkerResponse,
  type SyntaxDocumentIdentity,
  type SyntaxDocumentSnapshot,
  type SyntaxWorkerErrorCode,
  type SyntaxWorkerRequest,
} from "./syntax-protocol.ts";

export interface SyntaxWorkerPort {
  addEventListener(type: "error" | "message" | "messageerror", listener: (event: unknown) => void): void;
  removeEventListener(type: "error" | "message" | "messageerror", listener: (event: unknown) => void): void;
  postMessage(message: SyntaxWorkerRequest): void;
  terminate(): void;
}

export interface MermaidSyntaxWorkerClient {
  changeDocument(document: SyntaxDocumentSnapshot): Promise<void>;
  dispose(): void;
  highlights(document: SyntaxDocumentIdentity): Promise<Uint32Array>;
  initialize(): Promise<void>;
  onDidFail(listener: (error: Error) => void): { dispose(): void };
  openDocument(document: SyntaxDocumentSnapshot): Promise<void>;
}

export interface MermaidSyntaxWorkerStartup {
  readonly client: MermaidSyntaxWorkerClient;
  readonly ready: Promise<void>;
}

interface PendingRequest {
  readonly expected: "highlights" | "ready" | "result";
  readonly identity?: SyntaxDocumentIdentity;
  readonly reject: (error: Error) => void;
  readonly resolve: (value: unknown) => void;
  timeout: ReturnType<typeof setTimeout>;
}

const REQUEST_TIMEOUT_MS = 30_000;

export class StaleSyntaxSnapshotError extends Error {
  constructor(message = "The syntax result belongs to an obsolete document.") {
    super(message);
    this.name = "StaleSyntaxSnapshotError";
  }
}

export class SyntaxWorkerProtocolError extends Error {
  readonly code: SyntaxWorkerErrorCode | "CLIENT_PROTOCOL";

  constructor(
    message: string,
    code: SyntaxWorkerErrorCode | "CLIENT_PROTOCOL" = "CLIENT_PROTOCOL",
  ) {
    super(message);
    this.name = "SyntaxWorkerProtocolError";
    this.code = code;
  }
}

export function startMermaidSyntaxWorkerClient(worker: SyntaxWorkerPort): MermaidSyntaxWorkerStartup {
  const client = new WorkerClient(worker);
  const ready = client.initialize().catch((error: unknown) => {
    client.dispose();
    throw asError(error);
  });
  return Object.freeze({ client, ready });
}

class WorkerClient implements MermaidSyntaxWorkerClient {
  private currentDocument: SyntaxDocumentIdentity | null = null;
  private disposed = false;
  private failure: Error | null = null;
  private readonly failureListeners = new Set<(error: Error) => void>();
  private initialized = false;
  private initializePromise: Promise<void> | null = null;
  private nextRequestId = 1;
  private readonly pending = new Map<number, PendingRequest>();
  private synchronization: Promise<void> = Promise.resolve();
  private transportClosed = false;
  private readonly worker: SyntaxWorkerPort;

  private readonly handleError = (event: unknown): void => {
    const message = eventMessage(event);
    this.poison(new Error(`Mermaid syntax worker failed${message ? `: ${message}` : ""}`));
  };
  private readonly handleMessageError = (): void => {
    this.poison(new SyntaxWorkerProtocolError("The browser could not decode a syntax worker response."));
  };
  private readonly handleMessage = (event: unknown): void => {
    try {
      const response = projectSyntaxWorkerResponse(eventData(event));
      const pending = this.pending.get(response.requestId);
      if (!pending) throw new SyntaxWorkerProtocolError("Syntax worker returned an unknown request ID.");
      if (response.type === "error") {
        this.complete(response.requestId, pending);
        const error =
          response.code === "STALE_DOCUMENT"
            ? new StaleSyntaxSnapshotError(response.message)
            : new SyntaxWorkerProtocolError(response.message, response.code);
        pending.reject(error);
        if (response.code !== "STALE_DOCUMENT") this.poison(error);
        return;
      }
      if (response.type !== pending.expected) {
        throw new SyntaxWorkerProtocolError(`Syntax worker returned ${response.type}; expected ${pending.expected}.`);
      }
      if (response.type === "highlights") {
        if (!pending.identity || !sameIdentity(pending.identity, response) || !this.isCurrent(response)) {
          this.complete(response.requestId, pending);
          pending.reject(new StaleSyntaxSnapshotError());
          return;
        }
        this.complete(response.requestId, pending);
        pending.resolve(response.data);
        return;
      }
      this.complete(response.requestId, pending);
      pending.resolve(undefined);
    } catch (error) {
      this.poison(asError(error));
    }
  };

  constructor(worker: SyntaxWorkerPort) {
    this.worker = worker;
    worker.addEventListener("error", this.handleError);
    worker.addEventListener("message", this.handleMessage);
    worker.addEventListener("messageerror", this.handleMessageError);
  }

  initialize(): Promise<void> {
    this.assertActive();
    if (this.initializePromise) return this.initializePromise;
    const requestId = this.allocateRequestId();
    this.initializePromise = this.request(
      { protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, requestId, type: "initialize" },
      "ready",
    ).then(() => {
      this.initialized = true;
    });
    return this.initializePromise;
  }

  onDidFail(listener: (error: Error) => void): { dispose(): void } {
    if (this.failure) {
      listener(this.failure);
      return { dispose() {} };
    }
    this.failureListeners.add(listener);
    return { dispose: () => this.failureListeners.delete(listener) };
  }

  openDocument(document: SyntaxDocumentSnapshot): Promise<void> {
    this.assertReady();
    const snapshot = projectSyntaxDocumentSnapshot(document);
    if (this.currentDocument) return Promise.reject(new SyntaxWorkerProtocolError("Syntax worker already owns a document."));
    this.currentDocument = identityOf(snapshot);
    return this.enqueueSynchronization("didOpen", snapshot);
  }

  changeDocument(document: SyntaxDocumentSnapshot): Promise<void> {
    this.assertReady();
    const snapshot = projectSyntaxDocumentSnapshot(document);
    if (!this.currentDocument || this.currentDocument.uri !== snapshot.uri) {
      return Promise.reject(new SyntaxWorkerProtocolError("Syntax worker does not own this document URI."));
    }
    if (snapshot.version <= this.currentDocument.version) {
      return Promise.reject(new StaleSyntaxSnapshotError());
    }
    this.currentDocument = identityOf(snapshot);
    return this.enqueueSynchronization("didChange", snapshot);
  }

  async highlights(document: SyntaxDocumentIdentity): Promise<Uint32Array> {
    this.assertReady();
    if (!this.isCurrent(document)) throw new StaleSyntaxSnapshotError();
    await this.synchronization;
    if (!this.isCurrent(document)) throw new StaleSyntaxSnapshotError();
    const requestId = this.allocateRequestId();
    return this.request(
      { protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, requestId, type: "highlights", ...document },
      "highlights",
      document,
    );
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.failAll(new Error("Mermaid syntax worker was disposed."));
    this.closeTransport();
  }

  private enqueueSynchronization(
    type: "didChange" | "didOpen",
    document: SyntaxDocumentSnapshot,
  ): Promise<void> {
    const operation = this.synchronization.then(() => {
      const requestId = this.allocateRequestId();
      return this.request<void>(
        { protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, requestId, type, document },
        "result",
      );
    });
    this.synchronization = operation;
    return operation;
  }

  private request<Result>(
    message: Exclude<SyntaxWorkerRequest, { readonly type: "dispose" }>,
    expected: PendingRequest["expected"],
    identity?: SyntaxDocumentIdentity,
  ): Promise<Result> {
    this.assertActive();
    return new Promise<Result>((resolve, reject) => {
      const timeout = setTimeout(() => {
        const pending = this.pending.get(message.requestId);
        if (!pending) return;
        this.pending.delete(message.requestId);
        const error = new SyntaxWorkerProtocolError("Mermaid syntax worker request timed out.");
        pending.reject(error);
        this.poison(error);
      }, REQUEST_TIMEOUT_MS);
      this.pending.set(message.requestId, {
        expected,
        identity,
        reject,
        resolve: (value) => resolve(value as Result),
        timeout,
      });
      try {
        this.worker.postMessage(message);
      } catch (error) {
        clearTimeout(timeout);
        this.pending.delete(message.requestId);
        reject(asError(error));
      }
    });
  }

  private complete(requestId: number, pending: PendingRequest): void {
    clearTimeout(pending.timeout);
    this.pending.delete(requestId);
  }

  private allocateRequestId(): number {
    const requestId = this.nextRequestId;
    if (!Number.isSafeInteger(requestId)) throw new SyntaxWorkerProtocolError("Syntax request IDs are exhausted.");
    this.nextRequestId += 1;
    return requestId;
  }

  private isCurrent(identity: SyntaxDocumentIdentity): boolean {
    return Boolean(this.currentDocument && sameIdentity(this.currentDocument, identity));
  }

  private assertReady(): void {
    this.assertActive();
    if (!this.initialized) throw new SyntaxWorkerProtocolError("Initialize syntax before opening a document.");
  }

  private assertActive(): void {
    if (this.failure) throw this.failure;
    if (this.disposed) throw new Error("Mermaid syntax worker was disposed.");
  }

  private poison(error: Error): void {
    if (this.failure || this.disposed) return;
    this.failure = error;
    this.failAll(error);
    this.closeTransport();
    for (const listener of this.failureListeners) listener(error);
    this.failureListeners.clear();
  }

  private failAll(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }

  private closeTransport(): void {
    if (this.transportClosed) return;
    this.transportClosed = true;
    this.worker.removeEventListener("error", this.handleError);
    this.worker.removeEventListener("message", this.handleMessage);
    this.worker.removeEventListener("messageerror", this.handleMessageError);
    try {
      this.worker.postMessage({ protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, type: "dispose" });
    } catch {
      // Worker termination is the final ownership boundary.
    }
    this.worker.terminate();
  }
}

function identityOf(document: SyntaxDocumentIdentity): SyntaxDocumentIdentity {
  return { uri: document.uri, version: document.version };
}

function sameIdentity(left: SyntaxDocumentIdentity, right: SyntaxDocumentIdentity): boolean {
  return left.uri === right.uri && left.version === right.version;
}

function eventData(event: unknown): unknown {
  return typeof event === "object" && event !== null && "data" in event
    ? (event as { data: unknown }).data
    : undefined;
}

function eventMessage(event: unknown): string {
  return typeof event === "object" && event !== null && "message" in event
    ? String((event as { message: unknown }).message ?? "")
    : "";
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
