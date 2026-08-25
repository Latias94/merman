import assert from "node:assert/strict";
import test from "node:test";
import {
  StaleSyntaxSnapshotError,
  startMermaidSyntaxWorkerClient,
  type SyntaxWorkerPort,
} from "./syntax-worker-client.ts";
import {
  MERMAID_SYNTAX_WORKER_PROTOCOL,
  type SyntaxWorkerRequest,
} from "./syntax-protocol.ts";

class PendingSyntaxPort implements SyntaxWorkerPort {
  readonly messages: SyntaxWorkerRequest[] = [];
  terminateCalls = 0;
  private readonly listeners = new Map<
    "error" | "message" | "messageerror",
    Set<(event: unknown) => void>
  >();

  addEventListener(
    type: "error" | "message" | "messageerror",
    listener: (event: unknown) => void,
  ): void {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(
    type: "error" | "message" | "messageerror",
    listener: (event: unknown) => void,
  ): void {
    this.listeners.get(type)?.delete(listener);
  }

  postMessage(message: SyntaxWorkerRequest): void {
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCalls += 1;
  }

  emitMessage(message: unknown): void {
    for (const listener of this.listeners.get("message") ?? []) {
      listener({ data: structuredClone(message) });
    }
  }

  listenerCount(): number {
    return [...this.listeners.values()].reduce(
      (count, listeners) => count + listeners.size,
      0,
    );
  }

  take<Type extends SyntaxWorkerRequest["type"]>(
    type: Type,
  ): Extract<SyntaxWorkerRequest, { type: Type }> {
    const index = this.messages.findIndex((message) => message.type === type);
    assert.notEqual(index, -1, `missing syntax worker message of type ${type}`);
    return this.messages.splice(index, 1)[0] as Extract<
      SyntaxWorkerRequest,
      { type: Type }
    >;
  }

  async takeEventually<Type extends SyntaxWorkerRequest["type"]>(
    type: Type,
  ): Promise<Extract<SyntaxWorkerRequest, { type: Type }>> {
    for (let attempt = 0; attempt < 20; attempt += 1) {
      if (this.messages.some((message) => message.type === type)) {
        return this.take(type);
      }
      await Promise.resolve();
    }
    return this.take(type);
  }
}

test("syntax client rejects stale highlights without poisoning the worker", async () => {
  const worker = new PendingSyntaxPort();
  const startup = startMermaidSyntaxWorkerClient(worker);
  const initialize = worker.take("initialize");
  worker.emitMessage({
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: initialize.requestId,
    type: "ready",
  });
  await startup.ready;

  const opening = startup.client.openDocument(snapshot(1));
  acknowledge(worker, await worker.takeEventually("didOpen"));
  await opening;

  const stale = startup.client.highlights(identity(1));
  const staleRequest = await worker.takeEventually("highlights");
  worker.emitMessage({
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: staleRequest.requestId,
    type: "highlights",
    uri: staleRequest.uri,
    version: staleRequest.version + 1,
    data: new Uint32Array([0, 0, 9, 3, 0]),
  });
  await assert.rejects(stale, StaleSyntaxSnapshotError);

  const retry = startup.client.highlights(identity(1));
  const retryRequest = await worker.takeEventually("highlights");
  worker.emitMessage({
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: retryRequest.requestId,
    type: "highlights",
    uri: retryRequest.uri,
    version: retryRequest.version,
    data: new Uint32Array([0, 0, 9, 3, 0]),
  });
  assert.deepEqual([...(await retry)], [0, 0, 9, 3, 0]);

  startup.client.dispose();
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
});

test("syntax client poisons unknown responses and releases listeners", async () => {
  const worker = new PendingSyntaxPort();
  const startup = startMermaidSyntaxWorkerClient(worker);
  const initialize = worker.take("initialize");
  worker.emitMessage({
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: initialize.requestId,
    type: "ready",
  });
  await startup.ready;
  const failures: Error[] = [];
  startup.client.onDidFail((error) => failures.push(error));

  worker.emitMessage({
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: 999,
    type: "result",
    result: null,
  });

  assert.match(failures[0]?.message ?? "", /unknown request ID/);
  await assert.rejects(
    async () => startup.client.openDocument(snapshot(1)),
    /unknown request ID/,
  );
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
});

function acknowledge(
  worker: PendingSyntaxPort,
  request: Extract<SyntaxWorkerRequest, { type: "didChange" | "didOpen" }>,
): void {
  worker.emitMessage({
    protocol: MERMAID_SYNTAX_WORKER_PROTOCOL,
    requestId: request.requestId,
    type: "result",
    result: null,
  });
}

function identity(version: number) {
  return { uri: "file:///merman/playground.mmd", version };
}

function snapshot(version: number) {
  return { ...identity(version), source: "flowchart TD" };
}
