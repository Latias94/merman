import assert from "node:assert/strict";
import test from "node:test";

import type { DiagramDetectionFacts, DiagramType } from "@mermanjs/web";
import {
  NO_MERMAID_EXTERNAL_REQUIREMENTS,
  MERMAID_EXTERNAL_DIAGRAM_ALIAS_TO_MODULE,
  MERMAID_LAYOUT_ID_TO_MODULE,
  mermaidExternalRequirementsFor,
  normalizeMermaidExternalRequirements,
} from "./mermaid-requirements.ts";

test("external requirements consume every generated layout registration", () => {
  for (const [layout, moduleId] of Object.entries(
    MERMAID_LAYOUT_ID_TO_MODULE
  )) {
    assert.deepEqual(mermaidExternalRequirementsFor(available("flowchart", layout)), {
      externalDiagrams: [],
      layoutModules: [moduleId],
    });
  }

  for (const layout of ["dagre", "swimlane", "elk.layered", "elk.unknown"]) {
    assert.deepEqual(mermaidExternalRequirementsFor(available("flowchart", layout)), {
      externalDiagrams: [],
      layoutModules: [],
    });
  }
});

test("external requirements map the exact tidy-tree layout module", () => {
  assert.deepEqual(
    mermaidExternalRequirementsFor(available("mindmap", "tidy-tree")),
    {
      externalDiagrams: [],
      layoutModules: ["tidy-tree"],
    }
  );
});

test("external requirements consume every generated external diagram alias", () => {
  for (const [alias, moduleId] of Object.entries(
    MERMAID_EXTERNAL_DIAGRAM_ALIAS_TO_MODULE
  )) {
    assert.deepEqual(mermaidExternalRequirementsFor(available(alias as DiagramType, "dagre")), {
      externalDiagrams: [moduleId],
      layoutModules: [],
    });
  }
  assert.deepEqual(mermaidExternalRequirementsFor(available("class", "elk")), {
    externalDiagrams: [],
    layoutModules: ["elk"],
  });
});

test("unavailable detection never requests external Mermaid modules", () => {
  assert.deepEqual(
    mermaidExternalRequirementsFor({
      status: "unavailable",
      validity: "unknown",
      diagramType: null,
      syntaxId: null,
      effectiveLayoutId: null,
    }),
    NO_MERMAID_EXTERNAL_REQUIREMENTS
  );
});

test("normalization sorts, deduplicates, freezes, and rejects unknown ids", () => {
  const requirements = normalizeMermaidExternalRequirements({
    externalDiagrams: ["zenuml", "zenuml"],
    layoutModules: ["tidy-tree", "elk", "tidy-tree"],
  });
  assert.deepEqual(requirements, {
    externalDiagrams: ["zenuml"],
    layoutModules: ["elk", "tidy-tree"],
  });
  assert.equal(Object.isFrozen(requirements), true);
  assert.equal(Object.isFrozen(requirements.externalDiagrams), true);
  assert.equal(Object.isFrozen(requirements.layoutModules), true);
  assert.throws(
    () =>
      normalizeMermaidExternalRequirements({
        externalDiagrams: ["sequence"],
        layoutModules: [],
      }),
    /external diagram module id/
  );
});

function available(
  diagramType: DiagramType,
  effectiveLayoutId: string
): DiagramDetectionFacts {
  return {
    status: "available",
    validity: "valid",
    diagramType,
    syntaxId: diagramType,
    effectiveLayoutId,
  };
}
