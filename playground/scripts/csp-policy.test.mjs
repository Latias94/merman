import assert from "node:assert/strict";
import test from "node:test";

import {
  CspContractError,
  extractMetaCsp,
  parseCspPolicy,
  verifyHtmlCsp,
} from "./csp-policy.mjs";

const mainPolicy = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; worker-src 'self'; frame-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'";

test("CSP meta parsing is insensitive to attribute and directive order", () => {
  const html = `<html><head><meta content="${mainPolicy}" data-owner="runtime" http-equiv='Content-Security-Policy'></head></html>`;
  assert.equal(extractMetaCsp(html), mainPolicy);
  assert.deepEqual(verifyHtmlCsp("index.html", html), []);
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
  assert.match(verifyHtmlCsp("index.html", html).join("\n"), /worker-src/u);

  assert.match(
    verifyHtmlCsp("index.html", "<html><head></head></html>").join("\n"),
    /exactly one CSP meta element/u,
  );
});
