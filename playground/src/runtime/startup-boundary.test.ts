import assert from "node:assert/strict";
import test from "node:test";

import { createStartupBoundary } from "./startup-boundary.ts";

test("startup work waits for the first activation owner", async () => {
  const boundary = createStartupBoundary();
  let observed: string | null = null;
  const waiting = boundary.wait().then((reason) => {
    observed = reason;
    return reason;
  });

  await Promise.resolve();
  assert.equal(observed, null);

  boundary.activate("preview-presented");
  assert.equal(await waiting, "preview-presented");
  assert.equal(boundary.reason(), "preview-presented");
});

test("startup activation is one-shot and late waiters observe the owner", async () => {
  const boundary = createStartupBoundary();

  boundary.activate("editor-intent");
  boundary.activate("preview-presented");

  assert.equal(boundary.reason(), "editor-intent");
  assert.equal(await boundary.wait(), "editor-intent");
});
