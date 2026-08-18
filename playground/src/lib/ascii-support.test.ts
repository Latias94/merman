import assert from "node:assert/strict";
import test from "node:test";

import { GENERATED_ASCII_CAPABILITIES } from "../generated/ascii-capabilities.ts";
import {
  FALLBACK_ASCII_CAPABILITIES,
  FALLBACK_ASCII_SUPPORTED_TYPES,
  asciiSupportLabelKey,
} from "./ascii-support.ts";

test("fallback uses the complete generated binding capability projection", () => {
  assert.equal(FALLBACK_ASCII_CAPABILITIES, GENERATED_ASCII_CAPABILITIES);
  assert.equal(FALLBACK_ASCII_CAPABILITIES.length, 31);
  assert.equal(
    new Set(FALLBACK_ASCII_CAPABILITIES.map(({ diagram_type }) => diagram_type))
      .size,
    FALLBACK_ASCII_CAPABILITIES.length
  );

  assert.deepEqual(
    FALLBACK_ASCII_SUPPORTED_TYPES,
    FALLBACK_ASCII_CAPABILITIES.filter(
      ({ primary_projection }) => primary_projection !== "none"
    ).map(({ diagram_type }) => diagram_type)
  );
});

test("support labels follow the generated projection kind", () => {
  const byType = new Map(
    FALLBACK_ASCII_CAPABILITIES.map((capability) => [
      capability.diagram_type,
      capability,
    ])
  );

  assert.equal(
    asciiSupportLabelKey(byType.get("flowchart") ?? null),
    "asciiSupport.levels.partial"
  );
  assert.equal(
    asciiSupportLabelKey(byType.get("gantt") ?? null),
    "asciiSupport.structuredText"
  );
  assert.equal(
    asciiSupportLabelKey(byType.get("zenuml") ?? null),
    "asciiSupport.unsupported"
  );
});
