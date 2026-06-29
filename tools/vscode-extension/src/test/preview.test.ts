import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { renderPreviewHtml, type PreviewDiagnostics } from "../preview-html.js";

describe("preview html", () => {
  it("uses local scripts with a nonce instead of command URIs or inline handlers", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
      input: previewInput("document"),
      svg: "<svg></svg>",
    });

    assert.match(html, /Content-Security-Policy/);
    assert.match(html, /script-src 'nonce-[A-Za-z0-9]+'/);
    assert.match(html, /src="vscode-resource:\/\/preview\.js"/);
    assert.doesNotMatch(html, /command:merman/);
    assert.doesNotMatch(html, /onclick=/);
  });

  it("renders a source picker when multiple preview sources are available", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
      input: previewInput("fence-2"),
      sources: [previewInput("fence-1"), previewInput("fence-2")],
    });

    assert.match(html, /data-action="source"/);
    assert.match(html, /value="fence-2" selected/);
  });

  it("renders diagnostics as validated message targets", () => {
    const diagnostics: PreviewDiagnostics = {
      summary: "1 errors, 0 warnings, 0 infos, 0 hints",
      visibleCount: 1,
      totalCount: 1,
      items: [
        {
          severityLabel: "Error",
          severityKey: "error",
          line: 2,
          column: 3,
          target: {
            uri: "file:///tmp/example.mmd",
            startLine: 1,
            startCharacter: 2,
            endLine: 1,
            endCharacter: 4,
          },
          source: "merman",
          code: "merman.parse.diagram_parse",
          message: "Mermaid syntax issue",
          hasQuickFixes: true,
        },
      ],
    };

    const html = renderPreviewHtml({
      resources: previewResources(),
      input: previewInput("document"),
      diagnostics,
    });

    assert.match(html, /data-action="diagnostic"/);
    assert.match(html, /data-action="quick-fix"/);
    assert.match(html, /&quot;startLine&quot;:1/);
  });
});

function previewResources() {
  return {
    cspSource: "vscode-resource:",
    stylesUri: "vscode-resource://preview.css",
    scriptUri: "vscode-resource://preview.js",
  };
}

function previewInput(sourceId: string) {
  return {
    sourceId,
    source: "flowchart TD\nA --> B\n",
    title: "example.mmd",
    subtitle: sourceId === "document" ? "Mermaid source file" : `Mermaid fence ${sourceId}`,
    exportBaseName: "example",
    kind: sourceId === "document" ? ("mermaid-file" as const) : ("markdown-fence" as const),
    sourceRange: {
      startLine: 0,
      endLine: 1,
    },
    diagnosticRange: {
      startLine: 0,
      endLine: 1,
    },
  };
}
