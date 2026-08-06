import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  createExpectedCspPolicies,
  verifyHtmlCsp,
} from "./csp-policy.mjs";
import {
  injectOpaqueRealmCspHashes,
  loadOpaqueRealmCspHashes,
} from "./opaque-realm-csp.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("prepared HTML entries bind their generated opaque-realm CSP hashes", () => {
  const hashes = loadOpaqueRealmCspHashes(root);
  const expectedPolicies = createExpectedCspPolicies(hashes);

  for (const fileName of [
    "index.html",
    "benchmark-corpus.html",
    "benchmark.html",
  ]) {
    const template = readFileSync(path.join(root, fileName), "utf8");
    const html = injectOpaqueRealmCspHashes(fileName, template, hashes);
    assert.deepEqual(verifyHtmlCsp(fileName, html, expectedPolicies), []);
  }
});
