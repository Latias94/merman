import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  CspContractError,
  createExpectedCspPolicies,
  extractMetaCsp,
  isCspHash,
  parseCspPolicy,
  verifyHtmlCsp,
} from "./csp-policy.mjs";
import {
  injectOpaqueRealmCspHashes,
  loadOpaqueRealmCspHashes,
} from "./opaque-realm-csp.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const hashes = loadOpaqueRealmCspHashes(root);
const expectedPolicies = createExpectedCspPolicies(hashes);
const mainPolicy = `default-src 'none'; script-src 'self' blob: ${hashes["index.html"].map((hash) => `'${hash}'`).join(" ")} 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; worker-src 'self'; frame-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'`;

test("CSP hash validation has one strict shared contract", () => {
  assert.equal(
    isCspHash("sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
    true,
  );
  assert.equal(isCspHash("'sha256-AAAAAAAA='"), false);
  assert.equal(isCspHash("sha384-AAAAAAAA="), false);
});

test("CSP meta parsing is insensitive to attribute and directive order", () => {
  const html = `<html><head><meta content="${mainPolicy}" data-owner="runtime" http-equiv='Content-Security-Policy'></head></html>`;
  assert.equal(extractMetaCsp(html), mainPolicy);
  assert.deepEqual(verifyHtmlCsp("index.html", html, expectedPolicies), []);
});

test("CSP parsing rejects duplicate directives and sources", () => {
  assert.throws(
    () => parseCspPolicy("default-src 'none'; default-src 'self'"),
    CspContractError,
  );
  assert.throws(
    () => parseCspPolicy("script-src 'self' 'self'"),
    CspContractError,
  );
});

test("CSP verification rejects broad or missing ownership boundaries", () => {
  const broad = mainPolicy.replace("worker-src 'self'", "worker-src *");
  const html = `<meta http-equiv="Content-Security-Policy" content="${broad}">`;
  assert.match(
    verifyHtmlCsp("index.html", html, expectedPolicies).join("\n"),
    /worker-src/u,
  );

  assert.match(
    verifyHtmlCsp(
      "index.html",
      "<html><head></head></html>",
      expectedPolicies,
    ).join("\n"),
    /exactly one CSP meta element/u,
  );
});

test("HTML entry CSPs receive exactly their creator-owned bootstrap hashes", () => {
  for (const fileName of [
    "index.html",
    "benchmark-corpus.html",
    "benchmark.html",
  ]) {
    const template = readFileSync(path.join(root, fileName), "utf8");
    const html = injectOpaqueRealmCspHashes(fileName, template, hashes);
    assert.deepEqual(verifyHtmlCsp(fileName, html, expectedPolicies), []);
    for (const hash of hashes[fileName]) {
      assert.match(html, new RegExp(escapeRegExp(hash), "u"));
    }
  }
  assert.equal(hashes["benchmark-corpus.html"].length, 1);
  assert.deepEqual(hashes["benchmark.html"], []);
});

test("opaque bootstrap CSP injection fails closed on template drift", () => {
  assert.throws(
    () => injectOpaqueRealmCspHashes("index.html", "<html></html>", hashes),
    /exactly one/u,
  );
  const wrongPolicies = createExpectedCspPolicies({
    ...hashes,
    "index.html": [
      "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
      hashes["index.html"][1],
    ],
  });
  const html = injectOpaqueRealmCspHashes(
    "index.html",
    readFileSync(path.join(root, "index.html"), "utf8"),
    hashes,
  );
  assert.match(
    verifyHtmlCsp("index.html", html, wrongPolicies).join("\n"),
    /script-src/u,
  );
  assert.throws(
    () =>
      injectOpaqueRealmCspHashes(
        "benchmark.html",
        "<meta content='__MERMAN_UNDECLARED_CSP_HASH__'>",
        hashes,
      ),
    /undeclared/u,
  );
});

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}
