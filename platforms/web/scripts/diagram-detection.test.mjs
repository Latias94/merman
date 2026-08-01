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

let analysisResult = facts("gitGraph", "dagre", "parsed");
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
  analysisResult = facts("gitGraph", "dagre", "parsed");
  assert.deepEqual(webApi.detectDiagramFacts("gitGraph\ncommit id: 0"), {
    status: "available",
    validity: "valid",
    diagramType: "gitgraph",
    syntaxId: "gitGraph",
    effectiveLayoutId: "dagre",
  });

  analysisResult = facts("railroad-abnf", "dagre", "parsed");
  assert.deepEqual(webApi.detectDiagramFacts("railroad-abnf\nrule = token"), {
    status: "available",
    validity: "valid",
    diagramType: "railroadAbnf",
    syntaxId: "railroad-abnf",
    effectiveLayoutId: "dagre",
  });
});

test("detectDiagramFacts forwards binding options without interpreting source", () => {
  analysisResult = facts("flowchart-v2", "elk", "parsed");
  const options = { site_config: { layout: "elk" } };
  assert.equal(webApi.detectDiagramFacts("flowchart TD\nA-->B", options).status, "available");
  assert.equal(receivedOptions, JSON.stringify(options));
});

test("detectDiagramFacts preserves syntax identity from parser recovery", () => {
  analysisResult = facts("flowchart-v2", "dagre", "recovered");
  assert.deepEqual(webApi.detectDiagramFacts("flowchart TD\nA[unterminated"), {
    status: "available",
    validity: "recoverable-invalid",
    diagramType: "flowchart",
    syntaxId: "flowchart-v2",
    effectiveLayoutId: "dagre",
  });
});

test("detectDiagramFacts ignores diagnostic severity when projecting parse validity", () => {
  analysisResult = { ...facts("flowchart-v2", "dagre", "parsed"), valid: false };
  assert.equal(webApi.detectDiagramFacts("ignored").validity, "valid");

  analysisResult = { ...facts("flowchart-v2", "dagre", "recovered"), valid: true };
  assert.equal(webApi.detectDiagramFacts("ignored").validity, "recoverable-invalid");
});

test("detectDiagramFacts fails closed for malformed or unsupported facts", () => {
  const unavailable = {
    status: "unavailable",
    validity: "unknown",
    diagramType: null,
    syntaxId: null,
    effectiveLayoutId: null,
  };
  const invalidFacts = [
    { ...facts("flowchart-v2", "dagre", "parsed"), version: 2 },
    { ...facts("flowchart-v2", "dagre", "parsed"), valid: "yes" },
    { ...facts("flowchart-v2", "dagre", "parsed"), diagrams: [] },
    {
      ...facts("flowchart-v2", "dagre", "parsed"),
      diagrams: [
        diagram("flowchart-v2", "dagre", "parsed"),
        diagram("gitGraph", "dagre", "parsed"),
      ],
    },
    { ...facts("flowchart-v2", "dagre", "parsed"), diagrams: [{ syntax: null }] },
    {
      ...facts("flowchart-v2", "dagre", "parsed"),
      diagrams: [
        {
          syntax: {
            diagram_type: "flowchart-v2",
            effective_layout: "dagre",
          },
        },
      ],
    },
    {
      ...facts("flowchart-v2", "dagre", "parsed"),
      diagrams: [diagram("flowchart-v2", "dagre", "future")],
    },
    {
      ...facts("flowchart-v2", "dagre", "parsed"),
      diagrams: [diagram(42, "dagre", "parsed")],
    },
    {
      ...facts("flowchart-v2", "dagre", "parsed"),
      diagrams: [diagram("flowchart-v2", null, "parsed")],
    },
    {
      ...facts("flowchart-v2", "dagre", "parsed"),
      diagrams: [diagram("flowchart-v2", "", "parsed")],
    },
    facts("unknown", "dagre", "parsed"),
    facts("error", "dagre", "parsed"),
    facts("ambiguous", "dagre", "parsed"),
    new Error("analysis failed"),
  ];

  for (const invalid of invalidFacts) {
    analysisResult = invalid;
    assert.deepEqual(webApi.detectDiagramFacts("ignored"), unavailable);
  }
});

test("detectDiagramFacts treats an unavailable parse disposition as unavailable", () => {
  analysisResult = facts("flowchart-v2", "dagre", "unavailable");
  assert.equal(webApi.detectDiagramFacts("ignored").status, "unavailable");
});

function facts(syntaxId, effectiveLayoutId, parseDisposition) {
  return {
    version: 1,
    valid: true,
    diagrams: [diagram(syntaxId, effectiveLayoutId, parseDisposition)],
  };
}

function diagram(syntaxId, effectiveLayoutId, parseDisposition) {
  return {
    parse_disposition: parseDisposition,
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
