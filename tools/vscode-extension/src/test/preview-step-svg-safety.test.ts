import * as assert from "node:assert/strict";
import { test } from "node:test";

import { assertSelfContainedPreviewSvg } from "../preview-svg-safety.js";

function stepSvg(step = 1, duration = 1, count = 3): string {
  const motion = "step_preview_2d_edge_2d_step";
  const transform = `<animateTransform attributeName="transform" type="scale" values="1;1" dur="${duration}s" begin="${motion}.begin" fill="remove"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" id="preview"><g class="edgePaths"><path id="preview-edge" data-edge="true" d="M0,0L100,0"/><path class="edge-step-highlight" aria-hidden="true" d="M0,0L100,0" fill="none" stroke="#333333" stroke-width="4" stroke-linecap="round" stroke-opacity="0.6" pointer-events="none" transform="translate(0,0) scale(0)" data-step="${step}">${transform}</path><circle class="edge-step-dot" r="4" aria-hidden="true" transform="translate(0,0) scale(0)" data-step="${step}" fill="#333333" stroke="none"><animateMotion id="${motion}" path="M0,0L100,0" dur="${duration}s" begin="${(step - 1) * duration}s;${motion}.end+${(count - 1) * duration}s" fill="remove"/>${transform}</circle></g></svg>`;
}

test("preview admits the closed Step artifact while retaining its self-contained host policy", () => {
  const svg = stepSvg();
  assert.doesNotThrow(() => assertSelfContainedPreviewSvg(svg));
  for (const invalid of [
    svg.replace("attributeName=\"transform\"", "attributeName=\"href\""),
    svg.replace("<animateMotion ", "<animateMotion href=\"#outside\" "),
    svg.replace(".end+2s", ".end+indefinite"),
    svg.replace("</svg>", "<animateMotion path=\"M0,0L1,1\"/></svg>"),
    svg.replace("</svg>", "<a href=\"https://example.com\"><text>external</text></a></svg>"),
  ]) {
    assert.throws(() => assertSelfContainedPreviewSvg(invalid), /Step animation|external resource/i);
  }
});

test("preview accepts decimal clocks at large step counts and rejects fractional cycles", () => {
  const svg = stepSvg(999999999, 0.1, 999999999);
  assert.doesNotThrow(() => assertSelfContainedPreviewSvg(svg));
  for (const gap of ["99999999.825s", "99999999.7s", "1e309s"]) {
    const invalid = svg.replace(".end+99999999.80000001s", `.end+${gap}`);
    assert.notEqual(invalid, svg);
    assert.throws(() => assertSelfContainedPreviewSvg(invalid), /Step animation/i);
  }
});

test("preview rejects matching malformed or nonfinite Step paths", () => {
  const svg = stepSvg();
  for (const path of ["javascript:alert(1)", "M0,0L1e999,0", "M0,0L1", "M0,0A2,2,0,2,0,4,4"]) {
    const invalid = svg.replaceAll("M0,0L100,0", path);
    assert.notEqual(invalid, svg);
    assert.throws(() => assertSelfContainedPreviewSvg(invalid), /Step animation/i);
  }
});
