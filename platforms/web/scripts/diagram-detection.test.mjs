import assert from "node:assert/strict";
import test from "node:test";

import * as webApi from "../dist/index.js";

const capabilities = [
  capability("flowchart-v2", "flowchart"),
  capability("gitGraph", "gitgraph"),
  capability("railroad-abnf", "railroadAbnf"),
  capability("error", null),
  capability("ambiguous", "flowchart"),
  capability("ambiguous", "gitgraph"),
];

let analysisResult = facts("gitGraph", "dagre");
let receivedOptions;

await webApi.initMerman({
  loader: async () => ({
    default: async () => {},
    analysisFacts(_source, options) {
      receivedOptions = options;
      if (analysisResult instanceof Error) {
        throw analysisResult;
      }
      return analysisResult;
    },
    diagramFamilyCapabilities() {
      return capabilities;
    },
  }),
});

test("detectDiagramFacts projects raw parser ids through canonical metadata ids", () => {
  analysisResult = facts("gitGraph", "dagre");
  assert.deepEqual(webApi.detectDiagramFacts("gitGraph\ncommit id: 0"), {
    status: "available",
    diagramType: "gitgraph",
    syntaxId: "gitGraph",
    effectiveLayoutId: "dagre",
  });

  analysisResult = facts("railroad-abnf", "dagre");
  assert.deepEqual(webApi.detectDiagramFacts("railroad-abnf\nrule = token"), {
    status: "available",
    diagramType: "railroadAbnf",
    syntaxId: "railroad-abnf",
    effectiveLayoutId: "dagre",
  });
});

test("detectDiagramFacts forwards binding options without interpreting source", () => {
  analysisResult = facts("flowchart-v2", "elk");
  const options = { site_config: { layout: "elk" } };
  assert.equal(webApi.detectDiagramFacts("flowchart TD\nA-->B", options).status, "available");
  assert.equal(receivedOptions, JSON.stringify(options));
});

test("detectDiagramFacts fails closed for invalid or unsupported facts", () => {
  const unavailable = {
    status: "unavailable",
    diagramType: null,
    syntaxId: null,
    effectiveLayoutId: null,
  };
  const invalidFacts = [
    { ...facts("flowchart-v2", "dagre"), version: 2 },
    { ...facts("flowchart-v2", "dagre"), valid: false },
    { ...facts("flowchart-v2", "dagre"), diagrams: [] },
    {
      ...facts("flowchart-v2", "dagre"),
      diagrams: [diagram("flowchart-v2", "dagre"), diagram("gitGraph", "dagre")],
    },
    { ...facts("flowchart-v2", "dagre"), diagrams: [{ syntax: null }] },
    { ...facts("flowchart-v2", "dagre"), diagrams: [diagram(42, "dagre")] },
    { ...facts("flowchart-v2", "dagre"), diagrams: [diagram("flowchart-v2", null)] },
    { ...facts("flowchart-v2", "dagre"), diagrams: [diagram("flowchart-v2", "")] },
    facts("unknown", "dagre"),
    facts("error", "dagre"),
    facts("ambiguous", "dagre"),
    new Error("analysis failed"),
  ];

  for (const invalid of invalidFacts) {
    analysisResult = invalid;
    assert.deepEqual(webApi.detectDiagramFacts("ignored"), unavailable);
  }
});

function facts(syntaxId, effectiveLayoutId) {
  return {
    version: 1,
    valid: true,
    diagrams: [diagram(syntaxId, effectiveLayoutId)],
  };
}

function diagram(syntaxId, effectiveLayoutId) {
  return {
    syntax: {
      diagram_type: syntaxId,
      effective_layout: effectiveLayoutId,
    },
  };
}

function capability(diagramType, metadataId) {
  return {
    diagram_type: diagramType,
    logical_family_kind: diagramType,
    metadata_id: metadataId,
    render_model_kind: null,
    has_detector: true,
    has_semantic_parser: true,
    has_editor_parser: true,
    has_combined_parser: true,
    has_render_parser: true,
    has_header: true,
    config_namespace: null,
  };
}
