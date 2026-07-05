import { test } from "node:test";

import { assertSvgSafetySourcesInParity } from "../../../scripts/assert-svg-safety-parity.mjs";

test("VS Code and Web SVG safety scanners stay in policy parity", () => {
  assertSvgSafetySourcesInParity();
});
