import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  collectMermanPreviewDiagnostics,
  type PreviewDiagnosticInput,
} from "../preview-diagnostics.js";

describe("preview diagnostics", () => {
  it("ignores non-Merman diagnostics in the preview summary", () => {
    const diagnostics = collectMermanPreviewDiagnostics(
      [
        diagnostic({
          source: "markdownlint",
          severity: 0,
          message: "external markdown issue",
        }),
        diagnostic({
          source: "cspell",
          severity: 1,
          message: "external spelling issue",
        }),
        diagnostic({
          source: "merman",
          severity: 0,
          message: "Mermaid syntax issue",
          startLine: 2,
        }),
      ],
      "file:///workspace/notes.md",
      { startLine: 1, endLine: 3 },
    );

    assert.equal(diagnostics.summary, "1 error, 0 warnings, 0 infos, 0 hints");
    assert.equal(diagnostics.totalCount, 1);
    assert.equal(diagnostics.firstTarget?.startLine, 2);
  });

  it("deduplicates Merman diagnostics and ignores diagnostics outside the source range", () => {
    const duplicate = diagnostic({
      source: "merman",
      severity: 1,
      message: "Recovered parser facts",
      startLine: 3,
      code: { value: "merman.parse.recovered_editor_facts" },
    });

    const diagnostics = collectMermanPreviewDiagnostics(
      [
        duplicate,
        duplicate,
        diagnostic({
          source: "merman",
          severity: 0,
          message: "Outside current Mermaid fence",
          startLine: 10,
        }),
      ],
      "file:///workspace/notes.md",
      { startLine: 1, endLine: 4 },
    );

    assert.equal(diagnostics.summary, "0 errors, 1 warning, 0 infos, 0 hints");
    assert.equal(diagnostics.totalCount, 1);
    assert.equal(diagnostics.firstTarget?.startLine, 3);
  });

  it("uses singular labels only for one diagnostic of each severity", () => {
    const diagnostics = collectMermanPreviewDiagnostics(
      [
        diagnostic({ source: "merman", severity: 0, message: "syntax", startLine: 1 }),
        diagnostic({ source: "merman", severity: 1, message: "recovered", startLine: 2 }),
        diagnostic({ source: "merman", severity: 1, message: "style", startLine: 3 }),
        diagnostic({ source: "merman", severity: 2, message: "info", startLine: 4 }),
        diagnostic({ source: "merman", severity: 3, message: "hint", startLine: 5 }),
      ],
      "file:///workspace/notes.md",
      { startLine: 1, endLine: 5 },
    );

    assert.equal(diagnostics.summary, "1 error, 2 warnings, 1 info, 1 hint");
  });
});

function diagnostic(options: {
  source?: string;
  severity: number;
  message: string;
  startLine?: number;
  code?: PreviewDiagnosticInput["code"];
}): PreviewDiagnosticInput {
  const startLine = options.startLine ?? 1;
  return {
    range: {
      start: {
        line: startLine,
        character: 0,
      },
      end: {
        line: startLine,
        character: 10,
      },
    },
    severity: options.severity,
    source: options.source,
    code: options.code,
    message: options.message,
  };
}
