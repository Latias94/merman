import assert from "node:assert/strict";
import test from "node:test";

import { createOperationQueue } from "./operation-queue.ts";

test("realm operation queue serializes work and recovers after rejection", async () => {
  const queue = createOperationQueue();
  const order: string[] = [];
  const gate = deferred<void>();

  const first = queue.enqueue(async () => {
    order.push("first:start");
    await gate.promise;
    order.push("first:fail");
    throw new Error("expected");
  });
  const second = queue.enqueue(async () => {
    order.push("second:start");
    return "ok";
  });

  await Promise.resolve();
  assert.deepEqual(order, ["first:start"]);
  gate.resolve();
  await assert.rejects(first, /expected/);
  assert.equal(await second, "ok");
  assert.deepEqual(order, ["first:start", "first:fail", "second:start"]);
});

function deferred<T>() {
  return Promise.withResolvers<T>();
}
