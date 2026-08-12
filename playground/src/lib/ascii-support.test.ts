import assert from "node:assert/strict";
import test from "node:test";

import {
  FALLBACK_ASCII_CAPABILITIES,
  FALLBACK_ASCII_DIAGRAMMATIC_TYPES,
  FALLBACK_ASCII_SUPPORTED_TYPES,
  asciiSupportLabelKey,
} from "./ascii-support.ts";

test("fallback capabilities mirror the total runtime ASCII contract", () => {
  const capabilityTypes = FALLBACK_ASCII_CAPABILITIES.map(
    (capability) => capability.diagram_type
  );
  assert.equal(capabilityTypes.length, 31);
  assert.equal(new Set(capabilityTypes).size, capabilityTypes.length);

  const outputAvailable = FALLBACK_ASCII_CAPABILITIES.filter(
    (capability) => capability.primary_projection !== "none"
  ).map((capability) => capability.diagram_type);
  const diagrammatic = FALLBACK_ASCII_CAPABILITIES.filter(
    (capability) => capability.primary_projection === "diagrammatic"
  ).map((capability) => capability.diagram_type);

  assert.deepEqual(outputAvailable, [...FALLBACK_ASCII_SUPPORTED_TYPES]);
  assert.deepEqual(diagrammatic, [...FALLBACK_ASCII_DIAGRAMMATIC_TYPES]);
  for (const capability of FALLBACK_ASCII_CAPABILITIES.filter(
    (candidate) => candidate.primary_projection !== "none"
  )) {
    assert.ok(
      capability.supported_semantics.length > 0,
      `${capability.diagram_type} fallback must describe its supported semantics`
    );
    assert.ok(
      capability.limits.length > 0,
      `${capability.diagram_type} fallback must describe its limits`
    );
  }
  assert.ok(
    !(FALLBACK_ASCII_SUPPORTED_TYPES as readonly string[]).includes("zenuml")
  );
});

test("fallback projection fields derive the compatibility support level", () => {
  const byType = new Map(
    FALLBACK_ASCII_CAPABILITIES.map((capability) => [
      capability.diagram_type,
      capability,
    ])
  );

  for (const diagramType of ["flowchart", "sequence"] as const) {
    const capability = byType.get(diagramType)!;
    assert.equal(capability.semantic_coverage, "partial");
    assert.equal(capability.primary_projection, "diagrammatic");
    assert.equal(capability.support_level, "partial");
  }

  for (const diagramType of [
    "gantt",
    "gitgraph",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "timeline",
    "treeView",
  ] as const) {
    const capability = byType.get(diagramType)!;
    assert.equal(capability.semantic_coverage, "partial");
    assert.equal(capability.primary_projection, "structured_text");
    assert.equal(capability.support_level, "summary");
    assert.equal(
      asciiSupportLabelKey(capability),
      "asciiSupport.structuredText"
    );
  }

  const zenuml = byType.get("zenuml")!;
  assert.equal(zenuml.semantic_coverage, null);
  assert.equal(zenuml.primary_projection, "none");
  assert.equal(zenuml.support_level, "unsupported");
  assert.equal(asciiSupportLabelKey(zenuml), "asciiSupport.unsupported");

  const gantt = byType.get("gantt")!;
  assert.ok(gantt.supported_semantics.includes("stable task ids"));
  assert.ok(
    gantt.supported_semantics.includes("start and end constraint expressions")
  );
  assert.ok(
    gantt.limits.every(
      (limit) => !limit.includes("dependency source expressions are not disclosed")
    )
  );

  const xychart = byType.get("xychart")!;
  assert.equal(xychart.structured_text_fallback, true);
  assert.ok(
    xychart.supported_semantics.some((semantic) =>
      semantic.includes("model-owned x/y coordinates")
    )
  );
  assert.ok(
    xychart.supported_semantics.includes(
      "length-framed empty-chart metadata disclosure"
    )
  );
  assert.ok(
    xychart.limits.every((limit) => !limit.includes("SVG-coordinate"))
  );

  const classDiagram = byType.get("class")!;
  assert.ok(
    classDiagram.limits.some((limit) => limit.includes("cross-namespace"))
  );
  assert.ok(
    classDiagram.limits.some((limit) => limit.includes("ports do not fit"))
  );
  assert.ok(
    classDiagram.limits.every(
      (limit) => !limit.includes("namespace containers are not drawn")
    )
  );

  const sequence = byType.get("sequence")!;
  assert.ok(
    sequence.supported_semantics.includes(
      "participant-bounded nested control frames"
    )
  );

  const flowchart = byType.get("flowchart")!;
  assert.ok(
    flowchart.supported_semantics.includes(
      "terminal-cell wrapped node labels"
    )
  );
});
