import assert from "node:assert/strict";
import test from "node:test";

import { findForbiddenWebArchitecture } from "./web-architecture.mjs";

test("Web architecture guard rejects Mermaid loading policy", () => {
  const violations = findForbiddenWebArchitecture(
    `
      import mermaid from "mermaid";
      type MermaidApi = import("mermaid").Mermaid;
      const load = () => import("@mermaid-js/layout-elk");
      const cdn = "https://cdn.jsdelivr.net/npm/mermaid";
      interface MermaidExternalRequirements {}
      registerExternalDiagrams([]);
      registerLayoutLoaders([]);
    `,
    "synthetic.ts"
  );

  assert.deepEqual(
    new Set(violations.map(({ rule }) => rule)),
    new Set([
      "mermaid-module",
      "mermaid-cdn",
      "mermaid-policy-name",
    ])
  );
});

test("Web architecture guard permits neutral parser facts", () => {
  assert.deepEqual(
    findForbiddenWebArchitecture(
      `
        const facts = {
          diagramType: "zenuml",
          syntaxId: "zenuml",
          effectiveLayoutId: "elk",
        };
      `,
      "neutral-facts.ts"
    ),
    []
  );
});
