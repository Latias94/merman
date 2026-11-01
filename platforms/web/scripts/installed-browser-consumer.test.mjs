import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { assertRequestLog } from "./installed-browser-consumer.mjs";

const baseUrl = "http://127.0.0.1:43210/installed-consumer/editor/";
const wasmUrls = [
  `${baseUrl}assets/merman-test.wasm?consumer=main`,
  `${baseUrl}assets/merman-test.wasm?consumer=worker`,
];
const requests = [
  baseUrl,
  `${baseUrl}assets/index-test.js`,
  `${baseUrl}assets/worker-test.js`,
  ...wasmUrls,
];

test("request log accepts one self-hosted WASM override per realm", () => {
  assert.doesNotThrow(() => assertRequestLog({ baseUrl, requests, wasmUrls }));
});

test("rejects a hidden default WASM request after an override", () => {
  assert.throws(() => assertRequestLog({
    baseUrl,
    requests: [...requests, `${baseUrl}assets/merman-test.wasm`],
    wasmUrls,
  }), /WASM requests/);
});

test("rejects repeated overrides and missing Worker WASM execution", () => {
  assert.throws(() => assertRequestLog({
    baseUrl, requests: [...requests, wasmUrls[0]], wasmUrls,
  }), /WASM requests/);
  assert.throws(() => assertRequestLog({
    baseUrl, requests: requests.slice(0, -1), wasmUrls,
  }), /WASM requests/);
});

test("rejects resources escaping the non-root deployment or its origin", () => {
  for (const escaped of [
    "http://127.0.0.1:43210/assets/merman.wasm",
    "https://cdn.invalid/merman.wasm",
  ]) {
    assert.throws(() => assertRequestLog({
      baseUrl, requests: [...requests, escaped], wasmUrls,
    }), /outside the consumer base/);
  }
});

test("CLI offers explicit tool paths and rejects unsupported package choices", () => {
  const script = fileURLToPath(new URL("installed-browser-consumer.mjs", import.meta.url));
  const help = execFileSync(process.execPath, [script, "--help"], { encoding: "utf8" });
  assert.match(help, /--package/);
  assert.match(help, /--vite-module/);
  assert.match(help, /--playwright-module/);
  const invalid = spawnSync(process.execPath, [script, "--package", "../full"], {
    encoding: "utf8",
  });
  assert.equal(invalid.status, 1);
  assert.match(invalid.stderr, /Unknown package/);
});
