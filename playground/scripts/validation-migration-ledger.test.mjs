import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

test("every validation migration names its invariant, replacement, and proof", async () => {
  const ledger = JSON.parse(
    await readFile(
      path.join(import.meta.dirname, "validation-migration-ledger.json"),
      "utf8",
    ),
  );
  assert.equal(ledger.schemaVersion, 1);
  assert.equal(new Set(ledger.entries.map((entry) => entry.id)).size, ledger.entries.length);
  for (let index = 1; index <= 15; index += 1) {
    assert.ok(
      ledger.entries.some((entry) => entry.id === `VG-${String(index).padStart(2, "0")}`),
      `validation migration VG-${String(index).padStart(2, "0")} is missing`,
    );
  }
  for (const entry of ledger.entries) {
    assert.match(entry.id, /^VG-\d{2}$/u);
    assert.ok(entry.disposition === "removed" || entry.disposition === "retained");
    for (const field of ["legacyGate", "stableInvariant", "replacementEvidence"]) {
      assert.equal(typeof entry[field], "string");
      assert.ok(entry[field].trim().length > 0);
    }
    assert.ok(Array.isArray(entry.provingTests) && entry.provingTests.length > 0);
    for (const proof of entry.provingTests) {
      assert.match(proof, /(?:\.test\.(?:mjs|ts)|\.spec\.ts)$/u);
      await access(path.resolve(import.meta.dirname, "..", proof));
    }
  }
  assert.ok(
    ledger.entries
      .find((entry) => entry.id === "VG-09")
      .provingTests.includes("scripts/runtime-module-request-policy.test.mjs"),
  );
});
