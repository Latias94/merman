import assert from "node:assert/strict";
import test from "node:test";
import type { MermaidSyntaxEngine } from "./syntax-engine.ts";
import {
  MERMAID_SYNTAX_WORKER_PROTOCOL,
  type SyntaxWorkerResponse,
} from "./syntax-protocol.ts";
import {
  createSyntaxWorkerRuntime,
  type SyntaxWorkerRuntimePort,
} from "./syntax-worker-runtime.ts";

test("syntax runtime owns versioned document state and transfers highlights", async () => {
  const calls: string[] = [];
  const port = new RuntimePort();
  const runtime = createSyntaxWorkerRuntime(port, async () => engine(calls));

  await runtime.receive(request(1, "initialize"));
  assert.equal(port.take(1).type, "ready");
  await runtime.receive({
    ...request(2, "didOpen"),
    document: snapshot(1, "flowchart TD"),
  });
  assert.equal(port.take(2).type, "result");
  await runtime.receive({
    ...request(3, "didChange"),
    document: snapshot(2, "flowchart TD\nA --> B"),
  });
  port.take(3);
  await runtime.receive({
    ...request(4, "highlights"),
    uri: snapshot(2, "").uri,
    version: 2,
  });
  const response = port.take(4);
  assert.equal(response.type, "highlights");
  assert(response.type === "highlights");
  assert.deepEqual([...response.data], [0, 0, 9, 3, 0]);
  assert.deepEqual(port.lastTransfer, [response.data.buffer]);
  assert.deepEqual(calls, ["open:flowchart TD", "update:flowchart TD\nA --> B", "highlight"]);
});

test("syntax runtime rejects stale versions without discarding the current tree", async () => {
  const port = new RuntimePort();
  const runtime = createSyntaxWorkerRuntime(port, async () => engine([]));
  await runtime.receive(request(1, "initialize"));
  port.take(1);
  await runtime.receive({ ...request(2, "didOpen"), document: snapshot(2, "flowchart TD") });
  port.take(2);
  await runtime.receive({ ...request(3, "highlights"), uri: snapshot(2, "").uri, version: 1 });
  const stale = port.take(3);
  assert.equal(stale.type, "error");
  assert(stale.type === "error");
  assert.equal(stale.code, "STALE_DOCUMENT");

  await runtime.receive({ ...request(4, "highlights"), uri: snapshot(2, "").uri, version: 2 });
  assert.equal(port.take(4).type, "highlights");
});

class RuntimePort implements SyntaxWorkerRuntimePort {
  private readonly messages: SyntaxWorkerResponse[] = [];
  lastTransfer: ArrayBuffer[] | undefined;
  close(): void {}
  postMessage(message: SyntaxWorkerResponse, transfer?: ArrayBuffer[]): void {
    this.messages.push(message);
    this.lastTransfer = transfer;
  }
  take(requestId: number): SyntaxWorkerResponse {
    const index = this.messages.findIndex((message) => message.requestId === requestId);
    assert.notEqual(index, -1);
    return this.messages.splice(index, 1)[0];
  }
}

function engine(calls: string[]): MermaidSyntaxEngine {
  return {
    open(source) {
      calls.push(`open:${source}`);
    },
    update(source) {
      calls.push(`update:${source}`);
    },
    highlight() {
      calls.push("highlight");
      return new Uint32Array([0, 0, 9, 3, 0]);
    },
    dispose() {},
  };
}

function request(requestId: number, type: "didChange" | "didOpen" | "highlights" | "initialize") {
  return { protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, requestId, type };
}

function snapshot(version: number, source: string) {
  return { uri: "file:///merman/playground.mmd", version, source };
}
