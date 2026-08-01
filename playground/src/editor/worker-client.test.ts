import assert from "node:assert/strict";
import test from "node:test";
import {
  startMermanLanguageWorkerClient,
  type EditorWorkerPort,
} from "./worker-client.ts";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  type EditorWorkerRequest,
  type EditorWorkerResponse,
} from "./protocol.ts";

class PendingWorkerPort implements EditorWorkerPort {
  readonly messages: EditorWorkerRequest[] = [];
  terminateCalls = 0;
  private readonly messageListeners = new Set<(event: unknown) => void>();

  addEventListener(
    type: "error" | "message",
    listener: (event: unknown) => void
  ): void {
    if (type === "message") this.messageListeners.add(listener);
  }

  removeEventListener(
    type: "error" | "message",
    listener: (event: unknown) => void
  ): void {
    if (type === "message") this.messageListeners.delete(listener);
  }

  postMessage(message: EditorWorkerRequest): void {
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCalls += 1;
  }

  emitMessage(message: EditorWorkerResponse): void {
    for (const listener of this.messageListeners) listener({ data: message });
  }
}

test("startup exposes ownership while initialization is still pending", async () => {
  const worker = new PendingWorkerPort();
  const startup = startMermanLanguageWorkerClient(worker, "legend-digest");
  let settled = false;
  void startup.ready.then(
    () => {
      settled = true;
    },
    () => {
      settled = true;
    }
  );

  assert.equal(worker.messages[0]?.type, "initialize");
  await Promise.resolve();
  assert.equal(settled, false);

  startup.client.dispose();

  await assert.rejects(startup.ready, /worker was disposed/);
  assert.equal(settled, true);
  assert.equal(worker.terminateCalls, 1);
  assert.deepEqual(
    worker.messages.map((message) => message.type),
    ["initialize", "dispose"]
  );
});

test("startup terminates an owned worker when client construction fails", () => {
  const worker = new PendingWorkerPort();

  assert.throws(
    () => startMermanLanguageWorkerClient(worker, ""),
    /generated editor legend digest is required/
  );
  assert.equal(worker.terminateCalls, 1);
  assert.deepEqual(worker.messages, []);
});

test("startup times out and terminates an unresponsive worker once", async () => {
  const worker = new PendingWorkerPort();
  const startup = startMermanLanguageWorkerClient(worker, "legend-digest", 10);

  await assert.rejects(startup.ready, /initialization timed out after 10 ms/);
  assert.equal(worker.terminateCalls, 1);
  assert.deepEqual(
    worker.messages.map((message) => message.type),
    ["initialize", "dispose"]
  );

  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(worker.terminateCalls, 1);
});

test("successful startup clears its initialization deadline", async () => {
  const worker = new PendingWorkerPort();
  const startup = startMermanLanguageWorkerClient(worker, "legend-digest", 10);
  const initialize = worker.messages[0];
  assert.equal(initialize?.type, "initialize");
  if (!initialize || initialize.type !== "initialize") {
    throw new Error("worker did not receive its initialize request");
  }
  worker.emitMessage({
    protocol: EDITOR_WORKER_PROTOCOL,
    requestId: initialize.requestId,
    type: "ready",
    transportApiVersion: 3,
    editorSchema: EDITOR_SCHEMA_VERSION,
    legendDigest: "legend-digest",
    legend: { tokenTypes: ["keyword"], tokenModifiers: [] },
  });

  await startup.ready;
  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(worker.terminateCalls, 0);
  assert.deepEqual(
    worker.messages.map((message) => message.type),
    ["initialize"]
  );

  startup.client.dispose();
  assert.equal(worker.terminateCalls, 1);
});
