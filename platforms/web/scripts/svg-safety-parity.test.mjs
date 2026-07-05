import { test } from "node:test";

import { assertSvgSafetySourcesInParity } from "../../../scripts/assert-svg-safety-parity.mjs";

test("Web and VS Code SVG safety scanners stay in policy parity", () => {
  assertSvgSafetySourcesInParity();
});
