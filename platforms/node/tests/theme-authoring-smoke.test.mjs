import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  assertAuthoringDiagnostic,
  fixtureCanonicalJson,
  parseSvg,
  stateFill,
} from "../scripts/theme-authoring-smoke.mjs";

const STATE = '<svg xmlns="http://www.w3.org/2000/svg"><g class="statediagram-state">'
  + '<rect class="label-container" width="80" height="40" style="stroke:#111; fill:#123abc !important"/>'
  + '<g class="label"><text>Active</text></g></g></svg>';

test("shared fixture oracle sorts nested keys and rejects values outside its bounded domain", () => {
  assert.equal(fixtureCanonicalJson({ z: [true, null, { b: 17, a: "é" }], a: -2 }),
    '{"a":-2,"z":[true,null,{"a":"é","b":17}]}');
  for (const value of [NaN, Infinity, -Infinity, 0.5, Number.MAX_SAFE_INTEGER + 1, undefined]) {
    assert.throws(() => fixtureCanonicalJson({ value }));
  }
  assert.throws(() => fixtureCanonicalJson({ é: 1 }), /ASCII/);
  assert.throws(() => fixtureCanonicalJson(new Date()), /JSON object/);
});

test("terminal State oracle rejects malformed or absent native labels and missing geometry", () => {
  assert.equal(stateFill(STATE), "#123abc");
  assert.equal(stateFill(STATE.replace('style="stroke:#111; fill:#123abc !important"',
    'fill="#456def"')), "#456def");
  for (const svg of [
    STATE.replace("</svg>", ""),
    STATE.replace("Active", "Dormant"),
    STATE.replace('width="80"', 'width="0"'),
    STATE.replace('height="40"', 'height="NaN"'),
    STATE.replace('class="label-container"', 'class="other"'),
    STATE.replace("<text>Active</text>", "<foreignObject>Active</foreignObject>"),
    STATE.replace("</svg>", STATE.slice(STATE.indexOf('<g class="statediagram-state">'), -6) + "</svg>"),
  ]) {
    assert.throws(() => stateFill(svg));
  }
  assert.throws(() => parseSvg('<html xmlns="http://www.w3.org/2000/svg"><text>Active</text></html>'));
});

test("authoring diagnostic oracle preserves the shared structured error vectors", () => {
  const vectors = JSON.parse(readFileSync(new URL(
    "../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/errors.json", import.meta.url,
  ), "utf8"));
  for (const vector of vectors) {
    const error = {
      codeName: vector.code_name,
      details: {
        theme_authoring: structuredClone(vector.theme_authoring),
        resource: structuredClone(vector.resource),
      },
    };
    error.details.theme_authoring.diagnostics[0].message = "Invalid authored theme";
    assert.equal(assertAuthoringDiagnostic(error, vector), true);
    for (const mutate of [
      (value) => { delete value.details; },
      (value) => { value.codeName = "MERMAN_PARSE_ERROR"; },
      (value) => { value.details.theme_authoring.schema_version = 2; },
      (value) => { value.details.theme_authoring.diagnostics[0].code = "generic"; },
      (value) => { value.details.theme_authoring.diagnostics[0].path = "/wrong"; },
      (value) => { value.details.theme_authoring.diagnostics[0].details = { extra: true }; },
      (value) => { value.details.theme_authoring.diagnostics[0].severity = "warning"; },
      (value) => { value.details.theme_authoring.diagnostics[0].message = " "; },
      (value) => { value.details.theme_authoring.diagnostics.push(value.details.theme_authoring.diagnostics[0]); },
    ]) {
      const changed = structuredClone(error);
      mutate(changed);
      assert.throws(() => assertAuthoringDiagnostic(changed, vector));
    }
    if (vector.resource) {
      for (const field of Object.keys(vector.resource)) {
        const changed = structuredClone(error);
        delete changed.details.resource[field];
        assert.throws(() => assertAuthoringDiagnostic(changed, vector));
      }
    }
  }
});
