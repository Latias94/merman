import assert from "node:assert/strict";
import test from "node:test";

import {
  assertNavigableInlineSvgArtifact,
  projectNavigableInlineSvg,
  type NavigableInlineSvg,
} from "./render-artifact.ts";

// @ts-expect-error NavigableInlineSvg is constructible only through the validated projector.
const forgedArtifact: NavigableInlineSvg = {
  kind: "navigable-inline-svg",
  svg: '<svg xmlns="http://www.w3.org/2000/svg" />',
};
void forgedArtifact;

test("render output is projected through the navigable inline SVG path", () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><text>safe</text></svg>';
  const artifact = projectNavigableInlineSvg(svg);
  assert.equal(artifact.kind, "navigable-inline-svg");
  assert.equal(artifact.svg, svg);
  assert.equal(Object.isFrozen(artifact), true);
  assert.doesNotThrow(() => assertNavigableInlineSvgArtifact(artifact));
});

test("admits safe user-activated SVG anchor navigation", () => {
  for (const href of [
    "https://example.test/browse/MC-1",
    "http://example.test/browse/MC-1",
    "mailto:maintainer@example.test",
    "#local-ticket",
  ]) {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg"><a href="${href}"><text>ticket</text></a></svg>`;
    const artifact = projectNavigableInlineSvg(svg);
    assert.equal(artifact.svg, svg);
  }
});

test("rejects spread clones that retain the compile-time SVG brand", () => {
  const artifact = projectNavigableInlineSvg(
    '<svg xmlns="http://www.w3.org/2000/svg"><text>safe</text></svg>'
  );
  const forgedArtifact: NavigableInlineSvg = {
    ...artifact,
    svg: '<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)" />',
  };

  assert.throws(() => assertNavigableInlineSvgArtifact(forgedArtifact));
});

test("inline rejection still fails closed for unsafe publication shapes", () => {
  for (const svg of [
    "<div>not an SVG</div>",
    '<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)" />',
    '<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.test/tracker.png" /></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg"><a href="javascript:alert(1)"><text>unsafe</text></a></svg>',
  ]) {
    assert.throws(() => projectNavigableInlineSvg(svg));
  }
});
