import assert from "node:assert/strict";
import test from "node:test";
import {
  createMermanLanguageWorkerClient,
  EditorWorkerProtocolError,
  StaleLanguageSnapshotError,
  startMermanLanguageWorkerClient,
  type EditorCancellationToken,
  type EditorWorkerClientOptions,
  type EditorWorkerPort,
} from "./worker-client.ts";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_WEB_TRANSPORT_API_VERSION,
  type EditorDocumentSnapshot,
  type EditorWorkerRequest,
  type EditorWorkerResponse,
} from "./protocol.ts";

const COMPLETION_TRIGGER_CHARACTERS = [" ", "\n", "-", ":"];

class PendingWorkerPort implements EditorWorkerPort {
  readonly messages: EditorWorkerRequest[] = [];
  terminateCalls = 0;
  throwOnType: EditorWorkerRequest["type"] | null = null;
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

  postMessage(message: EditorWorkerRequest): void {
    if (message.type === this.throwOnType) {
      throw new Error("postMessage rejected");
    }
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCalls += 1;
  }

  emitMessage(message: unknown): void {
    this.emit("message", { data: structuredClone(message) });
  }

  emit(type: "error" | "message" | "messageerror", event: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }

  listenerCount(): number {
    return [...this.listeners.values()].reduce(
      (count, listeners) => count + listeners.size,
      0,
    );
  }

  take<Type extends EditorWorkerRequest["type"]>(
    type: Type,
  ): Extract<EditorWorkerRequest, { type: Type }> {
    const index = this.messages.findIndex((message) => message.type === type);
    assert.notEqual(index, -1, `missing worker message of type ${type}`);
    return this.messages.splice(index, 1)[0] as Extract<
      EditorWorkerRequest,
      { type: Type }
    >;
  }

  async takeEventually<Type extends EditorWorkerRequest["type"]>(
    type: Type,
  ): Promise<Extract<EditorWorkerRequest, { type: Type }>> {
    for (let attempt = 0; attempt < 20; attempt += 1) {
      if (this.messages.some((message) => message.type === type)) {
        return this.take(type);
      }
      await Promise.resolve();
    }
    return this.take(type);
  }

  respond(
    request: Exclude<EditorWorkerRequest, { type: "dispose" }>,
    result: unknown,
  ): void {
    const identity =
      request.type === "query"
        ? {
            uri: request.uri,
            version: request.version,
          }
        : {};
    this.emitMessage({
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: request.requestId,
      type: request.type === "query" ? "queryResult" : "result",
      ...identity,
      result,
    });
  }

  fail(
    request: Exclude<EditorWorkerRequest, { type: "dispose" }>,
    code: Extract<EditorWorkerResponse, { type: "error" }>["code"],
    message: string,
    detail: string | null = null,
    nativeCode: string | null = null,
  ): void {
    this.emitMessage({
      protocol: EDITOR_WORKER_PROTOCOL,
      type: "error",
      requestId: request.requestId,
      code,
      message,
      detail,
      nativeCode,
    });
  }
}

test("startup exposes ownership while initialization is still pending", async () => {
  const worker = new PendingWorkerPort();
  const startup = startMermanLanguageWorkerClient(worker);
  let settled = false;
  void startup.ready.then(
    () => {
      settled = true;
    },
    () => {
      settled = true;
    },
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
    ["initialize", "dispose"],
  );
  assert.equal(worker.listenerCount(), 0);
});

test("startup times out and releases transport resources once", async () => {
  const worker = new PendingWorkerPort();
  const startup = startMermanLanguageWorkerClient(worker, 10);

  await assert.rejects(startup.ready, /initialization timed out after 10 ms/);
  assert.equal(worker.terminateCalls, 1);
  assert.deepEqual(
    worker.messages.map((message) => message.type),
    ["initialize", "dispose"],
  );
  assert.equal(worker.listenerCount(), 0);

  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(worker.terminateCalls, 1);
});

test("successful startup clears its initialization deadline", async () => {
  const worker = new PendingWorkerPort();
  const startup = startMermanLanguageWorkerClient(worker, 10);
  ready(worker);

  await startup.ready;
  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(worker.terminateCalls, 0);

  startup.client.dispose();
  assert.equal(worker.terminateCalls, 1);
});

test("synchronization requires a null acknowledgement", async () => {
  const worker = new PendingWorkerPort();
  const client = await initializedClient(worker);
  const opening = client.openDocument(snapshot(1, "flowchart TD"));
  const request = worker.take("didOpen");

  worker.respond(request, { ok: true });

  await assert.rejects(opening, /synchronization result must be null/);
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
});

test("query-specific malformed results poison before reaching Monaco", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const query = client.query(identity(1), {
    kind: "completions",
    position: position(),
  });
  const request = await worker.takeEventually("query");

  worker.respond(request, { items: "not-an-array" });

  await assert.rejects(query, (error: unknown) => {
    assert(error instanceof EditorWorkerProtocolError);
    assert.equal(error.code, "PROTOCOL_MISMATCH");
    assert.match(error.message, /completion/);
    return true;
  });
  assert.equal(worker.terminateCalls, 1);
  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /completion/,
  );
});

test("an acknowledged document does not resubscribe while entering a query", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  assert.equal(cancellation.subscriptionCalls, 1);

  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await query;
  assert.equal(cancellation.listenerCount(), 0);
  client.dispose();
});

test("operation rejection remains request-local and preserves native detail", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const rename = client.query(identity(1), {
    kind: "rename",
    position: position(),
    newName: "bad name",
  });
  const request = await worker.takeEventually("query");
  worker.fail(
    request,
    "OPERATION_REJECTED",
    "Rename target is invalid.",
    "binding detail",
    "MERMAN_INVALID_ARGUMENT",
  );

  await assert.rejects(rename, (error: unknown) => {
    assert(error instanceof EditorWorkerProtocolError);
    assert.equal(error.code, "OPERATION_REJECTED");
    assert.equal(error.detail, "binding detail");
    assert.equal(error.nativeCode, "MERMAN_INVALID_ARGUMENT");
    return true;
  });

  const diagnostics = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  assert.deepEqual((await diagnostics).diagnostics, []);
  assert.equal(worker.terminateCalls, 0);
  client.dispose();
});

test("query failure remains request-local", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const hover = client.query(identity(1), {
    kind: "hover",
    position: position(),
  });
  const request = await worker.takeEventually("query");
  worker.fail(request, "QUERY_FAILED", "Hover failed.");

  await assert.rejects(hover, (error: unknown) => {
    assert(error instanceof EditorWorkerProtocolError);
    assert.equal(error.code, "QUERY_FAILED");
    return true;
  });

  const diagnostics = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await diagnostics;
  assert.equal(worker.terminateCalls, 0);
  client.dispose();
});

test("message decode failure and unknown IDs poison the transport", async (t) => {
  await t.test("worker error", async () => {
    const worker = new PendingWorkerPort();
    const client = await openedClient(worker);
    const failures: Error[] = [];
    client.onDidFail((error) => failures.push(error));

    worker.emit("error", { message: "runtime crashed" });

    assert.equal(failures.length, 1);
    assert.match(failures[0]?.message ?? "", /failed: runtime crashed/);
    assert.equal(worker.terminateCalls, 1);
    assert.equal(worker.listenerCount(), 0);
  });

  await t.test("messageerror", async () => {
    const worker = new PendingWorkerPort();
    const client = await openedClient(worker);
    const failures: Error[] = [];
    client.onDidFail((error) => failures.push(error));
    worker.emit("messageerror", {});

    assert.equal(failures.length, 1);
    assert.match(failures[0]?.message ?? "", /could not decode/);
    await assert.rejects(
      client.query(identity(1), { kind: "diagnostics" }),
      (error: unknown) => {
        assert(error instanceof EditorWorkerProtocolError);
        assert.equal(error.code, "PROTOCOL_MISMATCH");
        assert.match(error.message, /could not decode/);
        return true;
      },
    );
    assert.equal(worker.terminateCalls, 1);
    assert.equal(worker.listenerCount(), 0);
  });

  await t.test("unknown request id", async () => {
    const worker = new PendingWorkerPort();
    const client = await openedClient(worker);
    const failures: Error[] = [];
    client.onDidFail((error) => failures.push(error));
    worker.emitMessage({
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 999,
      type: "result",
      result: null,
    });

    assert.equal(failures.length, 1);
    assert.match(failures[0]?.message ?? "", /unknown request ID 999/);
    await assert.rejects(
      client.query(identity(1), { kind: "diagnostics" }),
      (error: unknown) => {
        assert(error instanceof EditorWorkerProtocolError);
        assert.equal(error.code, "PROTOCOL_MISMATCH");
        assert.match(error.message, /unknown request ID 999/);
        return true;
      },
    );
    assert.equal(worker.terminateCalls, 1);
  });
});

test("valid results from a superseded document reject locally without poisoning", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const query = client.query(identity(1), { kind: "diagnostics" });
  const request = await worker.takeEventually("query");
  const change = client.changeDocument(snapshot(2, "flowchart LR"));
  worker.respond(worker.take("didChange"), null);
  await change;
  worker.respond(request, diagnosticsResult());

  await assert.rejects(query, StaleLanguageSnapshotError);
  const retry = client.query(identity(2), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await retry;
  assert.equal(worker.terminateCalls, 0);
  client.dispose();
});

test("query provenance and payload validation precede cancellation and local staleness", async (t) => {
  for (const state of ["pending", "cancelled", "superseded"] as const) {
    for (const violation of ["URI", "version", "payload"] as const) {
      await t.test(`${state}: invalid ${violation}`, async (t) => {
        const worker = new PendingWorkerPort();
        const client = await openedClient(worker);
        t.after(() => client.dispose());
        const cancellation = new TestCancellationSource();
        const query = client.query(
          identity(1),
          { kind: "diagnostics" },
          cancellation.token,
        );
        void query.catch(() => {});
        const request = await worker.takeEventually("query");

        if (state === "cancelled") {
          cancellation.cancel();
          await assert.rejects(query, { name: "AbortError" });
        } else if (state === "superseded") {
          const change = client.changeDocument(snapshot(2, "flowchart LR"));
          worker.respond(worker.take("didChange"), null);
          await change;
        }

        const response = queryResponse(request, diagnosticsResult());
        worker.emitMessage({
          ...response,
          ...(violation === "URI" ? { uri: "file:///foreign.mmd" } : {}),
          ...(violation === "version" ? { version: 99 } : {}),
          ...(violation === "payload" ? { result: { diagnostics: "invalid" } } : {}),
        });

        if (state !== "cancelled") {
          await assert.rejects(query, {
            name: "EditorWorkerProtocolError",
            code: "PROTOCOL_MISMATCH",
          });
        }
        assert.equal(worker.terminateCalls, 1);
        await assert.rejects(
          client.query(identity(state === "superseded" ? 2 : 1), {
            kind: "diagnostics",
          }),
          { name: "EditorWorkerProtocolError", code: "PROTOCOL_MISMATCH" },
        );
      });
    }
  }
});

test("a duplicate response is a protocol failure", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const failures: Error[] = [];
  client.onDidFail((error) => failures.push(error));
  const query = client.query(identity(1), { kind: "diagnostics" });
  const request = await worker.takeEventually("query");
  const response = queryResponse(request, diagnosticsResult());
  worker.emitMessage(response);
  await query;

  worker.emitMessage(response);

  assert.equal(failures.length, 1);
  assert.match(failures[0]?.message ?? "", /duplicate response/);
  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /duplicate response/,
  );
  assert.equal(worker.terminateCalls, 1);
});

test("terminal failure subscriptions replay once and respect disposal", async () => {
  const worker = new PendingWorkerPort();
  const client = await initializedClient(worker);
  const activeFailures: Error[] = [];
  const removedFailures: Error[] = [];
  const active = client.onDidFail((error) => activeFailures.push(error));
  const removed = client.onDidFail((error) => removedFailures.push(error));
  removed.dispose();

  worker.emit("error", { message: "idle crash" });

  assert.equal(activeFailures.length, 1);
  assert.equal(removedFailures.length, 0);
  const replayedFailures: Error[] = [];
  const replayed = client.onDidFail((error) => replayedFailures.push(error));
  assert.deepEqual(replayedFailures, activeFailures);

  active.dispose();
  replayed.dispose();
  client.dispose();
  assert.equal(activeFailures.length, 1);
  assert.equal(replayedFailures.length, 1);

  const healthyWorker = new PendingWorkerPort();
  const healthyClient = await initializedClient(healthyWorker);
  const disposalFailures: Error[] = [];
  healthyClient.onDidFail((error) => disposalFailures.push(error));
  healthyClient.dispose();
  assert.deepEqual(disposalFailures, []);
});

test("cancelled requests allow one valid late response and reject a duplicate", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  const request = await worker.takeEventually("query");

  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });
  assert.equal(cancellation.listenerCount(), 0);

  const response = queryResponse(request, diagnosticsResult());
  worker.emitMessage(response);
  assert.equal(worker.terminateCalls, 0);
  worker.emitMessage(response);
  assert.equal(worker.terminateCalls, 1);
});

test("cancelled transports retain ownership beyond the completed replay limit", async (t) => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, { tombstoneLimit: 2 });
  t.after(() => client.dispose());
  const requests = [];

  for (let index = 0; index < 3; index += 1) {
    const cancellation = new TestCancellationSource();
    const query = client.query(
      identity(1),
      { kind: "diagnostics" },
      cancellation.token,
    );
    requests.push(await worker.takeEventually("query"));
    cancellation.cancel();
    await assert.rejects(query, { name: "AbortError" });
  }

  for (const request of requests) {
    worker.respond(request, diagnosticsResult());
    assert.equal(worker.terminateCalls, 0);
  }

  const continued = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  assert.deepEqual((await continued).diagnostics, []);
});

test("pending and draining queries consume capacity without poisoning on overload", async (t) => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, {
    queryCapacity: 2,
    tombstoneLimit: 1,
  });
  t.after(() => client.dispose());
  const failures: Error[] = [];
  client.onDidFail((error) => failures.push(error));
  const cancellations = [
    new TestCancellationSource(),
    new TestCancellationSource(),
  ];
  const queries = cancellations.map((cancellation) =>
    client.query(identity(1), { kind: "diagnostics" }, cancellation.token),
  );
  for (const query of queries) void query.catch(() => {});
  const first = await worker.takeEventually("query");
  const second = await worker.takeEventually("query");
  const rejectedCancellation = new TestCancellationSource();
  const overloaded = client.query(
    identity(1),
    { kind: "diagnostics" },
    rejectedCancellation.token,
  );
  void overloaded.catch(() => {});

  assert.equal(worker.messages.some((message) => message.type === "query"), false);
  await assert.rejects(overloaded, (error: unknown) => {
    assert(error instanceof EditorWorkerProtocolError);
    assert.equal(error.code, "OPERATION_REJECTED");
    assert.match(error.message, /query capacity of 2 is exhausted/);
    return true;
  });
  assert.equal(rejectedCancellation.listenerCount(), 0);

  for (const cancellation of cancellations) cancellation.cancel();
  for (const query of queries) {
    await assert.rejects(query, { name: "AbortError" });
  }
  await assert.rejects(client.query(identity(1), { kind: "diagnostics" }), {
    name: "EditorWorkerProtocolError",
    code: "OPERATION_REJECTED",
  });
  assert.equal(worker.messages.some((message) => message.type === "query"), false);
  assert.equal(worker.terminateCalls, 0);
  assert.deepEqual(failures, []);

  worker.respond(first, diagnosticsResult());
  const replacement = client.query(identity(1), { kind: "diagnostics" });
  const replacementRequest = await worker.takeEventually("query");
  await assert.rejects(client.query(identity(1), { kind: "diagnostics" }), {
    name: "EditorWorkerProtocolError",
    code: "OPERATION_REJECTED",
  });
  worker.respond(replacementRequest, diagnosticsResult());
  await replacement;
  worker.respond(second, diagnosticsResult());

  const continued = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await continued;
  assert.deepEqual(failures, []);
});

test("default capacity bounds cancelled transports independently of replay history", async (t) => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, { tombstoneLimit: 1 });
  t.after(() => client.dispose());

  for (let index = 0; index < 256; index += 1) {
    const cancellation = new TestCancellationSource();
    const query = client.query(
      identity(1),
      { kind: "diagnostics" },
      cancellation.token,
    );
    worker.take("query");
    cancellation.cancel();
    await assert.rejects(query, { name: "AbortError" });
  }

  await assert.rejects(client.query(identity(1), { kind: "diagnostics" }), {
    name: "EditorWorkerProtocolError",
    code: "OPERATION_REJECTED",
  });
  assert.equal(worker.messages.some((message) => message.type === "query"), false);
  assert.equal(worker.terminateCalls, 0);
});

test("query capacity must be a positive safe integer before attaching transport", () => {
  for (const queryCapacity of [0, -1, 1.5, Number.NaN, Infinity, 2 ** 53]) {
    const worker = new PendingWorkerPort();
    assert.throws(
      () => createMermanLanguageWorkerClient(worker, { queryCapacity }),
      /positive editor worker query capacity/,
    );
    assert.equal(worker.listenerCount(), 0);
    assert.deepEqual(worker.messages, []);
  }
});

test("full query capacity preserves the independent synchronization mailbox", async (t) => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, { queryCapacity: 1 });
  t.after(() => client.dispose());
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  const request = await worker.takeEventually("query");
  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });

  const firstChange = assert.rejects(
    client.changeDocument(snapshot(2, "source-2")),
    StaleLanguageSnapshotError,
  );
  const firstSynchronization = worker.take("didChange");
  const supersededChange = assert.rejects(
    client.changeDocument(snapshot(3, "source-3")),
    StaleLanguageSnapshotError,
  );
  const latestChange = client.changeDocument(snapshot(4, "source-4"));
  assert.equal(worker.messages.some((message) => message.type === "didChange"), false);

  worker.respond(firstSynchronization, null);
  const latestSynchronization = await worker.takeEventually("didChange");
  assert.equal(latestSynchronization.document.version, 4);
  assert.equal(latestSynchronization.document.source, "source-4");
  worker.respond(latestSynchronization, null);
  await Promise.all([firstChange, supersededChange, latestChange]);
  await assert.rejects(client.query(identity(4), { kind: "diagnostics" }), {
    name: "EditorWorkerProtocolError",
    code: "OPERATION_REJECTED",
  });

  worker.respond(request, diagnosticsResult());
  assert.equal(worker.terminateCalls, 0);
  const continued = client.query(identity(4), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await continued;
});

test("cancelled requests retain their original transport deadline", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, {
    queryCapacity: 2,
    requestTimeoutMs: 100,
  });
  t.after(() => client.dispose());
  const failures: Error[] = [];
  client.onDidFail((error) => failures.push(error));
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  await worker.takeEventually("query");
  t.mock.timers.tick(60);
  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });
  assert.equal(cancellation.listenerCount(), 0);

  const continuedCancellation = new TestCancellationSource();
  const continued = assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }, continuedCancellation.token),
    /diagnostics query timed out after 100 ms/,
  );
  await worker.takeEventually("query");
  t.mock.timers.tick(39);
  assert.equal(failures.length, 0);
  assert.equal(worker.terminateCalls, 0);

  t.mock.timers.tick(1);
  await continued;
  assert.equal(failures.length, 1);
  assert.match(failures[0]?.message ?? "", /diagnostics query timed out after 100 ms/);
  assert.equal(continuedCancellation.listenerCount(), 0);
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
  t.mock.timers.tick(1_000);
  assert.equal(failures.length, 1);
  assert.equal(worker.terminateCalls, 1);
});

test("validated late responses release capacity and clear the draining deadline", async (t) => {
  for (const responseType of ["queryResult", "error"] as const) {
    await t.test(responseType, async (t) => {
      t.mock.timers.enable({ apis: ["setTimeout"] });
      const worker = new PendingWorkerPort();
      const client = await openedClient(worker, {
        queryCapacity: 1,
        requestTimeoutMs: 100,
      });
      t.after(() => client.dispose());
      const cancellation = new TestCancellationSource();
      const query = client.query(
        identity(1),
        { kind: "diagnostics" },
        cancellation.token,
      );
      const request = await worker.takeEventually("query");
      t.mock.timers.tick(60);
      cancellation.cancel();
      await assert.rejects(query, { name: "AbortError" });
      t.mock.timers.tick(10);

      if (responseType === "queryResult") {
        worker.respond(request, diagnosticsResult());
      } else {
        worker.fail(request, "QUERY_FAILED", "The cancelled query failed.");
      }
      const continued = client.query(identity(1), { kind: "diagnostics" });
      const continuedRequest = await worker.takeEventually("query");
      t.mock.timers.tick(30);
      assert.equal(worker.terminateCalls, 0);
      worker.respond(continuedRequest, diagnosticsResult());
      await continued;
      t.mock.timers.tick(100);
      assert.equal(worker.terminateCalls, 0);
    });
  }
});

test("disposal closes draining queries and both synchronization slots once", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, {
    queryCapacity: 2,
    requestTimeoutMs: 100,
  });
  t.after(() => client.dispose());
  const failures: Error[] = [];
  client.onDidFail((error) => failures.push(error));
  const cancelled = new TestCancellationSource();
  const draining = client.query(identity(1), { kind: "diagnostics" }, cancelled.token);
  const drainingRequest = await worker.takeEventually("query");
  cancelled.cancel();
  await assert.rejects(draining, { name: "AbortError" });
  const cancellation = new TestCancellationSource();
  const pending = assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }, cancellation.token),
    /worker was disposed/,
  );
  await worker.takeEventually("query");
  const changing = assert.rejects(
    client.changeDocument(snapshot(2, "source-2")),
    /worker was disposed/,
  );
  const synchronization = worker.take("didChange");
  const queued = assert.rejects(
    client.changeDocument(snapshot(3, "source-3")),
    /worker was disposed/,
  );

  client.dispose();
  client.dispose();
  await Promise.all([pending, changing, queued]);
  worker.respond(drainingRequest, diagnosticsResult());
  worker.respond(synchronization, null);
  t.mock.timers.tick(1_000);
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
  assert.equal(cancellation.listenerCount(), 0);
  assert.equal(cancelled.listenerCount(), 0);
  assert.deepEqual(worker.messages.map((message) => message.type), ["dispose"]);
  assert.deepEqual(failures, []);
  await assert.rejects(
    client.query(identity(3), { kind: "diagnostics" }),
    /worker was disposed/,
  );
});

test("a malformed late result still fails the closed protocol boundary", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  const request = await worker.takeEventually("query");

  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });
  worker.emitMessage(queryResponse(request, { diagnostics: "invalid" }));

  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /diagnostics/,
  );
  assert.equal(worker.terminateCalls, 1);
});

test("draining ownership still rejects invalid envelopes, response types and unknown IDs", async (t) => {
  for (const violation of ["protocol", "extra field", "type", "error", "ID"] as const) {
    await t.test(violation, async (t) => {
      const worker = new PendingWorkerPort();
      const client = await openedClient(worker, { queryCapacity: 2 });
      t.after(() => client.dispose());
      const cancellation = new TestCancellationSource();
      const query = client.query(
        identity(1),
        { kind: "diagnostics" },
        cancellation.token,
      );
      const request = await worker.takeEventually("query");
      cancellation.cancel();
      await assert.rejects(query, { name: "AbortError" });
      const pending = assert.rejects(
        client.query(identity(1), { kind: "diagnostics" }),
        { name: "EditorWorkerProtocolError", code: "PROTOCOL_MISMATCH" },
      );
      await worker.takeEventually("query");
      const response = queryResponse(request, diagnosticsResult());

      switch (violation) {
        case "protocol":
          worker.emitMessage({ ...response, protocol: "foreign protocol" });
          break;
        case "extra field":
          worker.emitMessage({ ...response, unexpected: true });
          break;
        case "type":
          worker.emitMessage({
            protocol: EDITOR_WORKER_PROTOCOL,
            requestId: request.requestId,
            type: "result",
            result: null,
          });
          break;
        case "error":
          worker.emitMessage({
            protocol: EDITOR_WORKER_PROTOCOL,
            requestId: request.requestId,
            type: "error",
            code: "QUERY_FAILED",
            message: "The cancelled query failed.",
            detail: 42,
            nativeCode: null,
          });
          break;
        case "ID":
          worker.emitMessage({ ...response, requestId: 999 });
          break;
      }

      await pending;
      assert.equal(worker.terminateCalls, 1);
      assert.equal(worker.listenerCount(), 0);
    });
  }
});

test("a fatal late error from a cancelled request poisons the transport", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  const request = await worker.takeEventually("query");

  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });
  worker.fail(request, "INVALID_STATE", "The worker session is invalid.");

  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    (error: unknown) => {
      assert(error instanceof EditorWorkerProtocolError);
      assert.equal(error.code, "INVALID_STATE");
      return true;
    },
  );
  assert.equal(worker.terminateCalls, 1);
});

test("a nonfatal late error is ignored once and duplicates still poison", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  const request = await worker.takeEventually("query");

  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });
  worker.fail(request, "QUERY_FAILED", "The cancelled query failed.");
  assert.equal(worker.terminateCalls, 0);

  worker.fail(request, "QUERY_FAILED", "The cancelled query failed twice.");
  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /duplicate response/,
  );
  assert.equal(worker.terminateCalls, 1);
});

test("cancellation while waiting for synchronization sends no query and cleans up", async () => {
  const worker = new PendingWorkerPort();
  const client = await initializedClient(worker);
  const opening = client.openDocument(snapshot(1, "flowchart TD"));
  const openRequest = worker.take("didOpen");
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );

  cancellation.cancel();
  await assert.rejects(query, { name: "AbortError" });
  assert.equal(cancellation.listenerCount(), 0);
  assert.equal(
    worker.messages.some((message) => message.type === "query"),
    false,
  );

  worker.respond(openRequest, null);
  await opening;
  const retry = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await retry;
  client.dispose();
});

test("synchronous cancellation releases unsent capacity and its subscription", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, { queryCapacity: 1 });
  let disposeCalls = 0;
  const token: EditorCancellationToken = {
    isCancellationRequested: false,
    onCancellationRequested(listener) {
      listener();
      return { dispose: () => (disposeCalls += 1) };
    },
  };

  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }, token),
    { name: "AbortError" },
  );
  assert.equal(disposeCalls, 1);
  assert.equal(
    worker.messages.some((message) => message.type === "query"),
    false,
  );
  assert.equal(worker.terminateCalls, 0);
  const continued = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await continued;
  client.dispose();
});

test("subscription errors release unsent capacity without poisoning", async (t) => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, { queryCapacity: 1 });
  t.after(() => client.dispose());
  const token: EditorCancellationToken = {
    isCancellationRequested: false,
    onCancellationRequested() {
      throw new Error("subscription failed");
    },
  };

  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }, token),
    /subscription failed/,
  );
  assert.equal(worker.messages.some((message) => message.type === "query"), false);
  const continued = client.query(identity(1), { kind: "diagnostics" });
  worker.respond(await worker.takeEventually("query"), diagnosticsResult());
  await continued;
  assert.equal(worker.terminateCalls, 0);
});

test("the completed replay history remains bounded", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker, { tombstoneLimit: 2 });
  const responses = [];

  for (let index = 0; index < 3; index += 1) {
    const query = client.query(identity(1), { kind: "diagnostics" });
    const request = await worker.takeEventually("query");
    const response = queryResponse(request, diagnosticsResult());
    responses.push(response);
    worker.emitMessage(response);
    await query;
  }

  worker.emitMessage(responses[0]);
  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /unknown request ID/,
  );
  assert.equal(worker.terminateCalls, 1);
});

test("rapid edits retain one in-flight and only the latest pending source", async () => {
  const worker = new PendingWorkerPort();
  const client = await initializedClient(worker);
  const opening = client.openDocument(snapshot(1, "source-1"));
  const openRequest = worker.take("didOpen");
  const changes: Promise<unknown>[] = [];

  for (let version = 2; version <= 101; version += 1) {
    changes.push(
      client.changeDocument(snapshot(version, `source-${version}`)).then(
        () => "resolved",
        (error: unknown) => error,
      ),
    );
  }

  assert.equal(
    worker.messages.filter((message) => message.type === "didChange").length,
    0,
  );
  worker.respond(openRequest, null);
  await opening;
  await Promise.resolve();

  const changeRequest = worker.take("didChange");
  assert.equal(changeRequest.document.version, 101);
  assert.equal(changeRequest.document.source, "source-101");
  assert.equal(
    worker.messages.some((message) => message.type === "didChange"),
    false,
  );

  const query = client.query(identity(101), { kind: "diagnostics" });
  await Promise.resolve();
  assert.equal(
    worker.messages.some((message) => message.type === "query"),
    false,
  );

  worker.respond(changeRequest, null);
  await Promise.resolve();
  const queryRequest = await worker.takeEventually("query");
  worker.respond(queryRequest, diagnosticsResult());
  await query;

  const outcomes = await Promise.all(changes);
  for (const outcome of outcomes.slice(0, -1)) {
    assert(outcome instanceof StaleLanguageSnapshotError);
  }
  assert.equal(outcomes.at(-1), "resolved");
  client.dispose();
});

test("a hung query reaches Retry state and clears cancellation listeners", async () => {
  const worker = new PendingWorkerPort();
  const client = await initializedClient(worker, 10);
  const opening = client.openDocument(snapshot(1, "flowchart TD"));
  worker.respond(worker.take("didOpen"), null);
  await opening;
  const cancellation = new TestCancellationSource();
  const query = client.query(
    identity(1),
    { kind: "diagnostics" },
    cancellation.token,
  );
  await worker.takeEventually("query");

  await assert.rejects(query, /diagnostics query timed out after 10 ms/);
  assert.equal(cancellation.listenerCount(), 0);
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
});

test("a hung synchronization reaches Retry state", async () => {
  const worker = new PendingWorkerPort();
  const client = await initializedClient(worker, { requestTimeoutMs: 10 });
  const opening = client.openDocument(snapshot(1, "flowchart TD"));
  worker.take("didOpen");

  await assert.rejects(
    opening,
    /didOpen synchronization timed out after 10 ms/,
  );
  assert.equal(worker.terminateCalls, 1);
  assert.equal(worker.listenerCount(), 0);
});

test("a synchronous postMessage failure removes pending state and poisons once", async () => {
  const worker = new PendingWorkerPort();
  const client = await openedClient(worker);
  worker.throwOnType = "query";

  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /postMessage rejected/,
  );
  assert.equal(worker.terminateCalls, 1);
  await assert.rejects(
    client.query(identity(1), { kind: "diagnostics" }),
    /postMessage rejected/,
  );
  client.dispose();
  assert.equal(worker.terminateCalls, 1);
});

for (const trigger of ["timeout", "postMessage"] as const) {
  test(`a reentrant cancellation disposer preserves the ${trigger} failure`, async (t) => {
    t.mock.timers.enable({ apis: ["setTimeout"] });
    const worker = new PendingWorkerPort();
    const client = await openedClient(worker, { requestTimeoutMs: 100 });
    t.after(() => client.dispose());
    if (trigger === "postMessage") worker.throwOnType = "query";
    const cancellation: EditorCancellationToken = {
      isCancellationRequested: false,
      onCancellationRequested(listener) {
        return { dispose: listener };
      },
    };
    const query = assert.rejects(
      client.query(identity(1), { kind: "diagnostics" }, cancellation),
      trigger === "timeout" ? /timed out after 100 ms/ : /postMessage rejected/,
    );
    if (trigger === "timeout") {
      await worker.takeEventually("query");
      t.mock.timers.tick(100);
    }
    await query;
    assert.equal(worker.terminateCalls, 1);
    assert.equal(worker.listenerCount(), 0);
  });
}

class TestCancellationSource {
  private cancelled = false;
  private readonly listeners = new Set<() => void>();
  readonly token: EditorCancellationToken;
  subscriptionCalls = 0;

  constructor() {
    const source = this;
    this.token = {
      get isCancellationRequested() {
        return source.cancelled;
      },
      onCancellationRequested(listener) {
        source.subscriptionCalls += 1;
        source.listeners.add(listener);
        return { dispose: () => source.listeners.delete(listener) };
      },
    };
  }

  cancel(): void {
    this.cancelled = true;
    for (const listener of [...this.listeners]) listener();
  }

  listenerCount(): number {
    return this.listeners.size;
  }
}

async function initializedClient(
  worker: PendingWorkerPort,
  options: EditorWorkerClientOptions | number = {},
) {
  const normalizedOptions =
    typeof options === "number" ? { requestTimeoutMs: options } : options;
  const client = createMermanLanguageWorkerClient(worker, {
    ...normalizedOptions,
    requestTimeoutMs: normalizedOptions.requestTimeoutMs ?? 30_000,
    tombstoneLimit: normalizedOptions.tombstoneLimit ?? 8,
  });
  const initialization = client.initialize();
  ready(worker);
  await initialization;
  return client;
}

async function openedClient(
  worker: PendingWorkerPort,
  options: EditorWorkerClientOptions = {},
) {
  const client = await initializedClient(worker, options);
  const opening = client.openDocument(snapshot(1, "flowchart TD\n  A --> B"));
  worker.respond(worker.take("didOpen"), null);
  await opening;
  return client;
}

function ready(worker: PendingWorkerPort): void {
  const initialize = worker.take("initialize");
  worker.emitMessage({
    protocol: EDITOR_WORKER_PROTOCOL,
    requestId: initialize.requestId,
    type: "ready",
    completionTriggerCharacters: COMPLETION_TRIGGER_CHARACTERS,
    transportApiVersion: MERMAN_WEB_TRANSPORT_API_VERSION,
    editorSchema: EDITOR_SCHEMA_VERSION,
  });
}

function snapshot(version: number, sourceText: string): EditorDocumentSnapshot {
  return {
    uri: "file:///merman/playground.mmd",
    version,
    source: sourceText,
  };
}

function identity(version: number) {
  return { uri: "file:///merman/playground.mmd", version };
}

function position() {
  return { line: 0, character: 0 };
}

function diagnosticsResult() {
  return {
    version: EDITOR_SCHEMA_VERSION,
    valid: true,
    summary: { errors: 0, warnings: 0, infos: 0, hints: 0 },
    source: { kind: "diagram", language: "mermaid" },
    diagnostics: [],
  };
}

function queryResponse(
  request: Extract<EditorWorkerRequest, { type: "query" }>,
  result: unknown,
) {
  return {
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "queryResult",
    requestId: request.requestId,
    uri: request.uri,
    version: request.version,
    result,
  };
}
