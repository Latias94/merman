import assert from "node:assert/strict";
import test from "node:test";

import { assertNoRuntimeModuleRequests } from "./runtime-module-request-policy.mjs";

test("engine output without module requests is accepted", () => {
  assert.doesNotThrow(() =>
    assertNoRuntimeModuleRequests(
      "const render = () => 'svg'; export { render };",
      "engine.js",
    ),
  );
});

test("every runtime module request form is rejected", () => {
  const cases = [
    ['import "./dependency.js";', /import declaration/u],
    [
      'export { render } from "./dependency.js";',
      /export-from declaration/u,
    ],
    ['import dependency = require("dependency");', /import-equals declaration/u],
    ["import(/* @vite-ignore */ url);", /dynamic import/u],
  ];

  for (const [source, expected] of cases) {
    assert.throws(
      () => assertNoRuntimeModuleRequests(source, "engine.js"),
      expected,
    );
  }
});
