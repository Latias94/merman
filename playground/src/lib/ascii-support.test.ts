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
  assert.equal(FALLBACK_ASCII_CAPABILITIES.length, 34);
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
  assert.deepEqual(FALLBACK_ASCII_SUPPORTED_TYPES, [
    "agentflow",
    "architecture",
    "block",
    "c4",
    "class",
    "er",
    "flowchart",
    "gantt",
    "gitgraph",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "requirement",
    "sequence",
    "state",
    "swimlane",
    "timeline",
    "treeView",
    "usecase",
    "xychart",
  ]);

  const flowchart = FALLBACK_ASCII_CAPABILITIES.find(
    ({ diagram_type }) => diagram_type === "flowchart"
  );
  assert.deepEqual(flowchart?.layout_profiles, ["canonical", "compact", "auto"]);
  assert.deepEqual(flowchart?.width_profiles, ["unicode", "cjk"]);
  assert.deepEqual(flowchart?.encodings, [
    "plain",
    "ansi16",
    "ansi256",
    "truecolor",
    "html",
  ]);
  assert.deepEqual(flowchart?.fallback_encodings, ["plain"]);

  for (const diagramType of ["zenuml"]) {
    const capability = FALLBACK_ASCII_CAPABILITIES.find(
      ({ diagram_type }) => diagram_type === diagramType
    );
    assert.ok(capability, `${diagramType} is included in the total matrix`);
    assert.equal(asciiSupportLabelKey(capability), "asciiSupport.unsupported");
  }
  const agentflow = FALLBACK_ASCII_CAPABILITIES.find(
    ({ diagram_type }) => diagram_type === "agentflow"
  );
  assert.equal(
    asciiSupportLabelKey(agentflow ?? null),
    "asciiSupport.levels.partial"
  );
  assert.equal(agentflow?.primary_projection, "diagrammatic");
  assert.deepEqual(agentflow?.layout_profiles, ["canonical"]);
  assert.deepEqual(agentflow?.graph_presentations, ["strict", "adaptive"]);
  assert.deepEqual(agentflow?.fallback_encodings, ["plain"]);
  assert.ok(FALLBACK_ASCII_SUPPORTED_TYPES.includes("agentflow"));
  for (const diagramType of ["architecture", "usecase", "block", "c4"]) {
    const capability = FALLBACK_ASCII_CAPABILITIES.find(
      ({ diagram_type }) => diagram_type === diagramType
    );
    assert.equal(
      asciiSupportLabelKey(capability ?? null),
      "asciiSupport.levels.partial"
    );
    assert.equal(capability?.primary_projection, "diagrammatic");
    assert.deepEqual(capability?.layout_profiles, ["canonical"]);
    assert.deepEqual(capability?.graph_presentations, ["strict"]);
    assert.deepEqual(capability?.fallback_encodings, ["plain"]);
    assert.ok(FALLBACK_ASCII_SUPPORTED_TYPES.includes(diagramType));
  }
  const swimlane = FALLBACK_ASCII_CAPABILITIES.find(
    ({ diagram_type }) => diagram_type === "swimlane"
  );
  assert.deepEqual(swimlane?.layout_profiles, ["canonical", "compact", "auto"]);
  assert.equal(swimlane?.primary_projection, "diagrammatic");
  assert.ok(FALLBACK_ASCII_SUPPORTED_TYPES.includes("swimlane"));

  const state = FALLBACK_ASCII_CAPABILITIES.find(
    ({ diagram_type }) => diagram_type === "state"
  );
  assert.deepEqual(state?.layout_profiles, ["canonical"]);
});

test("Requirement fallback metadata admits canonical spatial and adaptive records", () => {
  const requirement = FALLBACK_ASCII_CAPABILITIES.find(
    ({ diagram_type }) => diagram_type === "requirement"
  );
  assert.ok(requirement);
  assert.equal(asciiSupportLabelKey(requirement), "asciiSupport.levels.partial");
  assert.equal(requirement.semantic_coverage, "partial");
  assert.equal(requirement.primary_projection, "diagrammatic");
  assert.equal(requirement.structured_text_fallback, true);
  assert.deepEqual(requirement.layout_profiles, ["canonical"]);
  assert.deepEqual(requirement.graph_presentations, ["strict", "adaptive"]);
  assert.deepEqual(requirement.width_profiles, ["unicode", "cjk"]);
  assert.deepEqual(requirement.encodings, [
    "plain",
    "ansi16",
    "ansi256",
    "truecolor",
    "html",
  ]);
  assert.deepEqual(requirement.fallback_encodings, ["plain"]);
  assert.ok(FALLBACK_ASCII_SUPPORTED_TYPES.includes("requirement"));
});

test("Pie fallback metadata admits canonical strict percent bars and complete records", () => {
  const pie = FALLBACK_ASCII_CAPABILITIES.find(
    ({ diagram_type }) => diagram_type === "pie"
  );
  assert.ok(pie);
  assert.equal(asciiSupportLabelKey(pie), "asciiSupport.levels.partial");
  assert.equal(pie.semantic_coverage, "partial");
  assert.equal(pie.primary_projection, "diagrammatic");
  assert.equal(pie.structured_text_fallback, true);
  assert.deepEqual(pie.layout_profiles, ["canonical"]);
  assert.deepEqual(pie.graph_presentations, ["strict"]);
  assert.deepEqual(pie.width_profiles, ["unicode", "cjk"]);
  assert.deepEqual(pie.encodings, [
    "plain",
    "ansi16",
    "ansi256",
    "truecolor",
    "html",
  ]);
  assert.deepEqual(pie.fallback_encodings, ["plain"]);
  assert.ok(FALLBACK_ASCII_SUPPORTED_TYPES.includes("pie"));
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
    asciiSupportLabelKey(byType.get("swimlane") ?? null),
    "asciiSupport.levels.partial"
  );
  assert.equal(
    asciiSupportLabelKey(byType.get("pie") ?? null),
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
