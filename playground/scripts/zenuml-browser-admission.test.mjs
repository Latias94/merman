import assert from "node:assert/strict";
import test from "node:test";

import { BROWSER_ADMISSION_SOURCE_RELATIVE_PATHS } from "./zenuml-browser-admission.mjs";

test("browser admission binds runtime security projections, not build implementation", () => {
  assert.equal(
    BROWSER_ADMISSION_SOURCE_RELATIVE_PATHS.includes(
      "playground/scripts/build-opaque-realm.mjs",
    ),
    false,
  );
  assert.equal(
    BROWSER_ADMISSION_SOURCE_RELATIVE_PATHS.includes(
      "playground/scripts/opaque-realm-artifact-plan.mjs",
    ),
    false,
  );
  assert.equal(
    BROWSER_ADMISSION_SOURCE_RELATIVE_PATHS.includes(
      "playground/scripts/opaque-realm-browser-projection.mjs",
    ),
    false,
  );
  assert.equal(
    BROWSER_ADMISSION_SOURCE_RELATIVE_PATHS.includes(
      "playground/src/benchmark/realm/generated/benchmark-mermaid.generated.ts",
    ),
    true,
  );
  assert.equal(
    BROWSER_ADMISSION_SOURCE_RELATIVE_PATHS.includes(
      "playground/src/runtime/realm/generated/opaque-realm-plan.generated.ts",
    ),
    true,
  );
});
