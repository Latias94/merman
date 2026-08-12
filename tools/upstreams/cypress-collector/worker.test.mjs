import assert from "node:assert/strict";
import { once } from "node:events";
import test from "node:test";
import { Worker } from "node:worker_threads";

const workerUrl = new URL("./worker.mjs", import.meta.url);

const runWorker = async (code, allowedRuntimeEffects = []) => {
  const worker = new Worker(workerUrl, {
    workerData: { allowedRuntimeEffects, code, sourceSpec: "scope/example.spec.ts" },
  });
  const [message] = await once(worker, "message");
  return message;
};

test("captures runtime-evaluated render calls and skipped registrations", async () => {
  const result = await runWorker(`
    describe("outer", () => {
      it("active", () => {
        const suffix = ["A", "B"].join(" --> ");
        globalThis.__mermanCypressCollector.capture("imgSnapshotTest", [
          "flowchart LR\\n  " + suffix,
          { flowchart: { defaultRenderer: "elk" } },
        ]);
      });
      it.skip("skipped", () => {
        throw new Error("must not execute");
      });
    });
  `);

  assert.equal(result.calls.length, 1);
  assert.equal(result.calls[0].diagram, "flowchart LR\n  A --> B");
  assert.equal(result.registrations.length, 2);
  assert.equal(result.registrations[1].id, "outer > skipped");
  assert.equal(result.registrations[1].skipped, true);
});

test("records only explicitly allowed Cypress assertion effects", async () => {
  const result = await runWorker(
    `
      describe("outer", () => {
        it("active", () => {
          globalThis.__mermanCypressCollector.capture("renderGraph", ["flowchart-elk LR\\nA-->B"]);
          cy.get("svg").should(() => undefined);
        });
      });
    `,
    ["cy.get.should"]
  );

  assert.deepEqual(result.runtimeEffects, [
    {
      registration: "outer > active",
      operation: "cy.get.should",
      selector: "svg",
      argumentKinds: ["function"],
    },
  ]);
});

test("rejects unreviewed runtime effects", async () => {
  const result = await runWorker(`
    describe("outer", () => {
      it("active", () => cy.get("svg").should("be.visible"));
    });
  `);

  assert.match(result.error, /runtime effect cy\.get\.should is not allowed/);
});

test("rejects asynchronous registrations", async () => {
  const result = await runWorker(`
    describe("outer", () => {
      it("active", async () => undefined);
    });
  `);

  assert.match(result.error, /async test registration/);
});

test("rejects unreviewed process and timer side effects", async () => {
  const processResult = await runWorker(`
    describe("outer", () => {
      it("active", () => process.env.CI);
    });
  `);
  assert.match(processResult.error, /process\.env is unavailable/);

  const timerResult = await runWorker(`
    describe("outer", () => {
      it("active", () => setTimeout(() => undefined, 0));
    });
  `);
  assert.match(timerResult.error, /setTimeout is unavailable/);
});
