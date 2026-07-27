import assert from "node:assert/strict";
import test from "node:test";
import {
  startMermanLanguageWorkerClient,
  type EditorWorkerPort,
} from "./worker-client.ts";
import type { EditorWorkerRequest } from "./protocol.ts";

class PendingWorkerPort implements EditorWorkerPort {
  readonly messages: EditorWorkerRequest[] = [];
  terminateCalls = 0;

  addEventListener(
    _type: "error" | "message",
    _listener: (event: unknown) => void
  ): void {}

  removeEventListener(
    _type: "error" | "message",
    _listener: (event: unknown) => void
  ): void {}

  postMessage(message: EditorWorkerRequest): void {
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCalls += 1;
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
