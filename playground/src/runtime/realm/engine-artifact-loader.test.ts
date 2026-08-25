import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import {
  EPHEMERAL_STORAGE_BUDGETS,
  createEphemeralStorageFacade,
  sha256Hex,
  verifyAndCreateRealmEngineModuleLoader,
} from "./engine-artifact-loader.ts";

const source = "export const value = 1;";
const artifact = {
  bytes: Buffer.byteLength(source),
  id: "mermaid" as const,
  resourceUrl: null,
  schemaVersion: 1 as const,
  sha256: createHash("sha256").update(source).digest("hex"),
  source,
};

test("sha256Hex supports insecure-context fallback hashing", async () => {
  assert.equal(
    await sha256Hex(new TextEncoder().encode("abc"), null),
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  );
});

test("only a verified engine artifact can construct a module loader", async () => {
  const loader = await verifyAndCreateRealmEngineModuleLoader(
    artifact,
    (module) => module
  );
  assert.equal(typeof loader, "function");
});

test("engine artifact loader construction rejects source tampering", async () => {
  await assert.rejects(
    verifyAndCreateRealmEngineModuleLoader(
      { ...artifact, source: `${source}\n` },
      (module) => module
    ),
    /byte length is invalid|digest is invalid/
  );
});

test("ephemeral storage implements the Storage API without unbounded authority", () => {
  const storage = createEphemeralStorageFacade();
  storage.setItem("one", "1");
  storage.setItem("two", "2");
  assert.equal(storage.length, 2);
  assert.equal(storage.key(0), "one");
  assert.equal(storage.getItem("two"), "2");
  storage.setItem("one", "updated");
  assert.equal(storage.length, 2);
  storage.removeItem("two");
  assert.equal(storage.getItem("two"), null);
  storage.clear();
  assert.equal(storage.length, 0);
  assert(Object.isFrozen(storage));

  assert.throws(
    () =>
      storage.setItem(
        "oversized",
        "x".repeat(EPHEMERAL_STORAGE_BUDGETS.maxValueBytes + 1)
      ),
    (error: unknown) =>
      error instanceof Error && error.name === "QuotaExceededError"
  );
});

test("each ephemeral storage facade owns isolated state", () => {
  const left = createEphemeralStorageFacade();
  const right = createEphemeralStorageFacade();
  left.setItem("zenumlDebug", "true");
  assert.equal(left.getItem("zenumlDebug"), "true");
  assert.equal(right.getItem("zenumlDebug"), null);
});
