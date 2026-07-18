import assert from "node:assert/strict";
import test from "node:test";

import type { DiagramDetectionFacts, DiagramType } from "@mermanjs/web";
import { mermaidExternalRequirementsFor } from "./mermaid-requirements.ts";

test("external requirements use Mermaid 11.16 registered ELK layout ids", () => {
  for (const layout of [
    "elk",
    "elk.stress",
    "elk.force",
    "elk.mrtree",
    "elk.sporeOverlap",
  ]) {
    assert.deepEqual(mermaidExternalRequirementsFor(available("flowchart", layout)), {
      elkLayouts: true,
      zenuml: false,
    });
  }

  for (const layout of ["dagre", "swimlane", "elk.layered", "elk.unknown"]) {
    assert.deepEqual(mermaidExternalRequirementsFor(available("flowchart", layout)), {
      elkLayouts: false,
      zenuml: false,
    });
  }
});

test("external requirements map ZenUML only from canonical logical type", () => {
  assert.deepEqual(mermaidExternalRequirementsFor(available("zenuml", "dagre")), {
    elkLayouts: false,
    zenuml: true,
  });
  assert.deepEqual(mermaidExternalRequirementsFor(available("class", "elk")), {
    elkLayouts: true,
    zenuml: false,
  });
});

test("unavailable detection never requests external Mermaid modules", () => {
  assert.deepEqual(
    mermaidExternalRequirementsFor({
      status: "unavailable",
      diagramType: null,
      syntaxId: null,
      effectiveLayoutId: null,
    }),
    { elkLayouts: false, zenuml: false }
  );
});

function available(
  diagramType: DiagramType,
  effectiveLayoutId: string
): DiagramDetectionFacts {
  return {
    status: "available",
    diagramType,
    syntaxId: diagramType,
    effectiveLayoutId,
  };
}
