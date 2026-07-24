import assert from "node:assert/strict";
import test from "node:test";

import {
  createNodeEngine,
  normalizeBindingOptions,
} from "../src/engine.mjs";
import {
  MermanDisposedError,
  MermanOperationError,
  MermanQueueSaturatedError,
} from "../src/errors.mjs";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function success(svg = "<svg />") {
  return JSON.stringify({
    version: 1,
    ok: true,
    result: {
      operation_id: "svg",
      media_type: "image/svg+xml",
      data: svg,
      metadata_json: JSON.stringify({
        version: 1,
        operation_id: "svg",
        media_type: "image/svg+xml",
        runtime_policy: "deterministic",
        byte_length: Buffer.byteLength(svg),
      }),
    },
  });
}

function failure({ kind, capabilityId = null }) {
  return JSON.stringify({
    version: 1,
    ok: false,
    error: {
      code: 7,
      code_name: "MERMAN_UNSUPPORTED_OPERATION",
      kind,
      capability_id: capabilityId,
      message:
        kind === "unknown-operation"
          ? "unknown operation `bitmap`"
          : `operation requires missing capability \`${capabilityId}\``,
    },
  });
}

function transportFactory(overrides = {}) {
  const calls = [];
  const createdWith = [];
  const transport = {
    async execute(requestJson) {
      calls.push(JSON.parse(requestJson));
      return success();
    },
    executeSync(requestJson) {
      calls.push(JSON.parse(requestJson));
      return success();
    },
    async dispose() {},
    ...overrides,
  };
  return {
    calls,
    createdWith,
    loadTransport: async (optionsJson) => {
      createdWith.push(JSON.parse(optionsJson));
      return transport;
    },
    transport,
  };
}

test("default construction is explicit deterministic interactive policy", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  assert.deepEqual(factory.createdWith, [
    {
      version: 1,
      runtime_policy: "deterministic",
      resources: { profile: "interactive" },
    },
  ]);

  const rendered = engine.renderSvg("flowchart TD\nA --> B");
  assert.ok(rendered instanceof Promise);
  assert.equal(await rendered, "<svg />");
  assert.deepEqual(factory.calls, [
    {
      operation_id: "svg",
      source: "flowchart TD\nA --> B",
      uri: null,
    },
  ]);
  await engine.dispose();
});

test("binding options preserve the shared profile vocabulary and reject host measurement", () => {
  assert.deepEqual(
    normalizeBindingOptions({
      resources: { profile: "trusted-native" },
      fixed_today: "2026-07-23",
    }),
    {
      version: 1,
      runtime_policy: "deterministic",
      resources: { profile: "trusted-native" },
      fixed_today: "2026-07-23",
    },
  );

  assert.throws(
    () => normalizeBindingOptions({ textMeasurer: () => ({ width: 1 }) }),
    /text measurement callbacks are not supported/i,
  );
  assert.throws(
    () => normalizeBindingOptions({ resources: { profile: "default" } }),
    /unknown resource profile `default`/i,
  );
});

test("typed unknown-operation and missing-capability errors survive the JS boundary", async () => {
  for (const expected of [
    { operationId: "bitmap", kind: "unknown-operation", capabilityId: null },
    { operationId: "png", kind: "missing-capability", capabilityId: "png" },
  ]) {
    const factory = transportFactory({
      async execute() {
        return failure({ kind: expected.kind, capabilityId: expected.capabilityId });
      },
    });
    const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
    await assert.rejects(
      engine.executeOperation({
        operationId: expected.operationId,
        source: "flowchart TD\nA",
      }),
      (error) => {
        assert.ok(error instanceof MermanOperationError);
        assert.equal(error.kind, expected.kind);
        assert.equal(error.capabilityId, expected.capabilityId);
        assert.equal(error.codeName, "MERMAN_UNSUPPORTED_OPERATION");
        return true;
      },
    );
    await engine.dispose();
  }
});

test("queue admission is bounded and dispose drains only executing work", async () => {
  const started = deferred();
  const release = deferred();
  let executions = 0;
  let transportDisposed = false;
  const factory = transportFactory({
    async execute() {
      executions += 1;
      started.resolve();
      await release.promise;
      return success(`<svg data-execution="${executions}" />`);
    },
    async dispose() {
      transportDisposed = true;
    },
  });
  const engine = await createNodeEngine(
    { concurrency: 1, maxQueue: 1 },
    { loadTransport: factory.loadTransport },
  );

  const active = engine.renderSvg("flowchart TD\nA");
  await started.promise;
  const queued = engine.renderSvg("flowchart TD\nB");
  await assert.rejects(
    engine.renderSvg("flowchart TD\nC"),
    MermanQueueSaturatedError,
  );

  const disposing = engine.dispose();
  await assert.rejects(queued, MermanDisposedError);
  assert.equal(transportDisposed, false);
  release.resolve();
  assert.match(await active, /data-execution="1"/);
  await disposing;
  assert.equal(transportDisposed, true);
  assert.equal(executions, 1);
  await assert.rejects(engine.renderSvg("flowchart TD\nD"), MermanDisposedError);
});

test("AbortSignal cancels queued work but never claims to preempt executing work", async () => {
  const started = deferred();
  const release = deferred();
  let executions = 0;
  const factory = transportFactory({
    async execute() {
      executions += 1;
      started.resolve();
      await release.promise;
      return success();
    },
  });
  const engine = await createNodeEngine(
    { concurrency: 1, maxQueue: 1 },
    { loadTransport: factory.loadTransport },
  );

  const executingAbort = new AbortController();
  const active = engine.renderSvg("flowchart TD\nA", {
    signal: executingAbort.signal,
  });
  await started.promise;
  executingAbort.abort();

  const queuedAbort = new AbortController();
  const queued = engine.renderSvg("flowchart TD\nB", {
    signal: queuedAbort.signal,
  });
  queuedAbort.abort();
  await assert.rejects(queued, (error) => error?.name === "AbortError");

  const replacement = engine.renderSvg("flowchart TD\nC");
  release.resolve();
  assert.equal(await active, "<svg />");
  assert.equal(await replacement, "<svg />");
  assert.equal(executions, 2);
  await engine.dispose();
});

test("renderSvgSync is explicit and refuses lifecycle races", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  assert.equal(engine.renderSvgSync("flowchart TD\nA"), "<svg />");
  await engine.dispose();
  assert.throws(() => engine.renderSvgSync("flowchart TD\nB"), MermanDisposedError);
});
