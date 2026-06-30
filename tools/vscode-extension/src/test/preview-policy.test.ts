import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { PreviewDiagnostics } from "../preview-model.js";
import {
  createPreviewSnapshot,
  type PreviewSnapshot,
} from "../preview-model.js";
import { planPreviewUpdate } from "../preview-policy.js";
import type { PreviewInput } from "../preview-source.js";

describe("preview update policy", () => {
  it("does not request a render for cursor movement inside the same source", () => {
    const previous = snapshot({ selectionLine: 3 });
    const next = snapshot({ selectionLine: 4 });

    const actions = planPreviewUpdate(previous, next, "selection");

    assert.deepEqual(
      actions.map((action) => action.type),
      ["selectionChanged"],
    );
  });

  it("requests a render when selection resolves to another unpinned source", () => {
    const previous = snapshot({ sourceId: "fence-1", selectionLine: 2 });
    const next = snapshot({ sourceId: "fence-2", source: "sequenceDiagram\nA->>B: hi", selectionLine: 8 });

    const actions = planPreviewUpdate(previous, next, "selection");

    assert.ok(actions.some((action) => action.type === "renderRequested"));
    assert.ok(actions.some((action) => action.type === "sourceListUpdated"));
  });

  it("does not request a render for diagnostics-only changes", () => {
    const previous = snapshot({ diagnostics: diagnostics("0 errors, 0 warnings, 0 infos, 0 hints") });
    const next = snapshot({ diagnostics: diagnostics("1 errors, 0 warnings, 0 infos, 0 hints") });

    const actions = planPreviewUpdate(previous, next, "diagnostics");

    assert.deepEqual(
      actions.map((action) => action.type),
      ["diagnosticsUpdated"],
    );
  });

  it("requests a render when the active source text changes", () => {
    const previous = snapshot({ source: "flowchart TD\nA --> B\n" });
    const next = snapshot({ source: "flowchart TD\nA --> C\n", documentVersion: 2 });

    const actions = planPreviewUpdate(previous, next, "document-change");

    assert.ok(actions.some((action) => action.type === "renderRequested"));
  });

  it("does not request a render when document version changes but active source text is unchanged", () => {
    const previous = snapshot({ documentVersion: 1 });
    const next = snapshot({ documentVersion: 2 });

    const actions = planPreviewUpdate(previous, next, "document-change");

    assert.deepEqual(actions, []);
  });

  it("requests a render when the diagram theme changes", () => {
    const previous = snapshot({ diagramTheme: "source" });
    const next = snapshot({ diagramTheme: "forest" });

    const actions = planPreviewUpdate(previous, next, "diagram-theme");

    assert.ok(actions.some((action) => action.type === "settingsUpdated"));
    assert.ok(actions.some((action) => action.type === "renderRequested"));
  });

  it("requests a render when the display mode changes", () => {
    const previous = snapshot({ displayMode: "svg" });
    const next = snapshot({ displayMode: "ascii" });

    const actions = planPreviewUpdate(previous, next, "display-mode");

    assert.ok(actions.some((action) => action.type === "settingsUpdated"));
    assert.ok(actions.some((action) => action.type === "renderRequested"));
  });

  it("requests a render when the preview background changes", () => {
    const previous = snapshot({ background: "transparent" });
    const next = snapshot({ background: "paper" });

    const actions = planPreviewUpdate(previous, next, "background");

    assert.ok(actions.some((action) => action.type === "settingsUpdated"));
    assert.ok(actions.some((action) => action.type === "renderRequested"));
  });

  it("shows an empty preview when no source is available", () => {
    const previous = snapshot({});

    const actions = planPreviewUpdate(previous, undefined, "active-editor");

    assert.deepEqual(actions, [{ type: "showEmpty" }]);
  });
});

function snapshot(
  options: {
    documentUri?: string;
    documentVersion?: number;
    sourceId?: string;
    source?: string;
    selectionLine?: number;
    diagramTheme?: PreviewSnapshot["diagramTheme"];
    displayMode?: PreviewSnapshot["displayMode"];
    background?: PreviewSnapshot["background"];
    diagnostics?: PreviewDiagnostics;
  },
): PreviewSnapshot {
  const input = previewInput(options.sourceId ?? "fence-1", options.source ?? "flowchart TD\nA --> B\n");
  return createPreviewSnapshot({
    documentUri: options.documentUri ?? "file:///workspace/notes.md",
    documentVersion: options.documentVersion ?? 1,
    input,
    sources: [previewInput("fence-1", "flowchart TD\nA --> B\n"), previewInput("fence-2", "sequenceDiagram\nA->>B: hi")],
    diagnostics: options.diagnostics ?? diagnostics("0 errors, 0 warnings, 0 infos, 0 hints"),
    selectionLine: options.selectionLine ?? 1,
    pinned: false,
    diagramTheme: options.diagramTheme ?? "source",
    displayMode: options.displayMode ?? "svg",
    background: options.background ?? "transparent",
  });
}

function previewInput(sourceId: string, source: string): PreviewInput {
  const ordinal = sourceId === "fence-2" ? 2 : 1;
  return {
    sourceId,
    source,
    title: "notes.md",
    subtitle: `Mermaid fence ${ordinal}`,
    exportBaseName: `notes-mermaid-${ordinal}`,
    kind: "markdown-fence",
    sourceRange: {
      startLine: ordinal === 1 ? 0 : 6,
      endLine: ordinal === 1 ? 3 : 9,
    },
    diagnosticRange: {
      startLine: ordinal === 1 ? 1 : 7,
      endLine: ordinal === 1 ? 2 : 8,
    },
  };
}

function diagnostics(summary: string): PreviewDiagnostics {
  return {
    summary,
    visibleCount: summary.startsWith("0 ") ? 0 : 1,
    totalCount: summary.startsWith("0 ") ? 0 : 1,
    items: summary.startsWith("0 ")
      ? []
      : [
          {
            severityLabel: "Error",
            severityKey: "error",
            line: 2,
            column: 1,
            target: {
              uri: "file:///workspace/notes.md",
              startLine: 1,
              startCharacter: 0,
              endLine: 1,
              endCharacter: 1,
            },
            message: "syntax issue",
            hasQuickFixes: false,
          },
        ],
  };
}
