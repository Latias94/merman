import assert from "node:assert/strict";
import test from "node:test";

import { findForbiddenWebArchitecture } from "./web-architecture.mjs";

test("Web dependency boundary rejects Mermaid modules in every import form", () => {
  const violations = findForbiddenWebArchitecture(
    `
      import mermaid from "mermaid";
      type MermaidApi = import("mermaid").Mermaid;
      const load = () => import("@mermaid-js/layout-elk");
    `,
    "synthetic.ts"
  );

  assert.deepEqual(
    new Set(violations.map(({ rule }) => rule)),
    new Set(["mermaid-module"])
  );
});

test("Web dependency boundary does not infer ownership from private spelling", () => {
  assert.deepEqual(
    findForbiddenWebArchitecture(
      `
        const facts = {
          diagramType: "zenuml",
          syntaxId: "zenuml",
          effectiveLayoutId: "elk",
        };
        interface MermaidExternalRequirements {}
        const registerExternalDiagrams = () => undefined;
        const policyLabel = "VITE_MERMAID_MODULE_URL";
        void [facts, registerExternalDiagrams, policyLabel];
      `,
      "neutral-facts.ts"
    ),
    []
  );
});
