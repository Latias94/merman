import assert from "node:assert/strict";
import test from "node:test";

import {
  projectSafeInlineSvg,
  type SafeInlineSvg,
} from "./render-artifact.ts";

// @ts-expect-error SafeInlineSvg is constructible only through the validated projector.
const forgedArtifact: SafeInlineSvg = {
  kind: "safe-inline-svg",
  svg: '<svg xmlns="http://www.w3.org/2000/svg" />',
  exportFormats: { png: true, svg: true },
};
void forgedArtifact;

test("render output is projected through the strict inline SVG path", () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><text>safe</text></svg>';
  const artifact = projectSafeInlineSvg(svg);
  assert.equal(artifact.kind, "safe-inline-svg");
  assert.equal(artifact.svg, svg);
  assert.deepEqual(artifact.exportFormats, { png: true, svg: true });
  assert.equal(Object.isFrozen(artifact), true);
});

test("strict inline rejection fails closed for unsafe publication shapes", () => {
  for (const svg of [
    "<div>not an SVG</div>",
    '<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)" />',
    '<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.test/tracker.png" /></svg>',
  ]) {
    assert.throws(() => projectSafeInlineSvg(svg));
  }
});
