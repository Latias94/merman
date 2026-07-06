import { test } from "node:test";

import { assertGeneratedSvgSafetyPolicyCurrent } from "../../../scripts/svg-safety-policy.mjs";

test("VS Code SVG safety policy is generated from the Web canonical policy", async () => {
  await assertGeneratedSvgSafetyPolicyCurrent();
});
