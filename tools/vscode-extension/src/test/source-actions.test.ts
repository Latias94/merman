import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  SOURCE_ACTIONS_ENABLED_SETTING,
  SOURCE_ACTION_COMMANDS,
  buildMermaidSourceCodeLensSpecs,
  isMermaidSourceCommandTarget,
  mermaidSourceExportCopyActions,
  mermaidSourceCommandIdentity,
  mermaidSourceCommandSourceId,
  mermaidSourceCommandTarget,
  mermaidSourceCommandUri,
  shouldRefreshSourceActionCodeLens,
  type MermaidSourceCommandArgument,
} from "../source-actions.js";
import { previewSourceIdentity, type PreviewInput } from "../preview-source.js";

describe("Mermaid source actions", () => {
  it("builds one low-noise action group for a Mermaid file", () => {
    const specs = buildMermaidSourceCodeLensSpecs([
      { sourceId: "document", sourceRange: { startLine: 0, endLine: 4 } },
    ]);

    assert.deepEqual(
      specs.map((spec) => [spec.line, spec.sourceId, spec.title, spec.command]),
      [
        [0, "document", "Preview", SOURCE_ACTION_COMMANDS.preview],
        [0, "document", "Export / Copy", SOURCE_ACTION_COMMANDS.exportCopy],
      ],
    );
  });

  it("builds source-scoped actions for each Markdown Mermaid fence", () => {
    const specs = buildMermaidSourceCodeLensSpecs([
      { sourceId: "fence-1", sourceRange: { startLine: 2, endLine: 5 } },
      { sourceId: "fence-2", sourceRange: { startLine: 8, endLine: 11 } },
    ]);

    assert.deepEqual(
      specs.filter((spec) => spec.title === "Preview").map((spec) => [spec.line, spec.sourceId]),
      [
        [2, "fence-1"],
        [8, "fence-2"],
      ],
    );
  });

  it("can disable source CodeLens actions for preview coexistence", () => {
    const specs = buildMermaidSourceCodeLensSpecs(
      [{ sourceId: "document", sourceRange: { startLine: 0, endLine: 4 } }],
      { enabled: false },
    );

    assert.deepEqual(specs, []);
  });

  it("refreshes CodeLens when the source action setting changes", () => {
    assert.equal(
      shouldRefreshSourceActionCodeLens((section) => section === SOURCE_ACTIONS_ENABLED_SETTING),
      true,
    );
    assert.equal(
      shouldRefreshSourceActionCodeLens((section) => section === "merman.diagnostics.enabled"),
      false,
    );
  });

  it("keeps platform-sensitive copy commands out of the top-level CodeLens row", () => {
    const specs = buildMermaidSourceCodeLensSpecs([
      { sourceId: "document", sourceRange: { startLine: 0, endLine: 0 } },
    ]);

    assert.equal(specs.some((spec) => spec.command === SOURCE_ACTION_COMMANDS.copyPng), false);
    assert.equal(specs.some((spec) => spec.command === SOURCE_ACTION_COMMANDS.copySvg), false);
    assert.equal(specs.some((spec) => spec.command === SOURCE_ACTION_COMMANDS.exportCopy), true);
  });

  it("keeps export and copy commands available from the Export / Copy action", () => {
    assert.deepEqual(
      mermaidSourceExportCopyActions({ includeCopyPng: false }).map((action) => [
        action.title,
        action.command,
      ]),
      [
        ["Export SVG", SOURCE_ACTION_COMMANDS.exportSvg],
        ["Export PNG", SOURCE_ACTION_COMMANDS.exportPng],
        ["Copy SVG", SOURCE_ACTION_COMMANDS.copySvg],
      ],
    );
  });

  it("carries source identity through CodeLens specs and command targets", () => {
    const input = previewInput("fence-2", "sequenceDiagram\nA->>B: hi", 8, 11);
    const specs = buildMermaidSourceCodeLensSpecs([input]);
    const target = mermaidSourceCommandTarget(
      { toString: () => "file:///workspace/notes.md" } as never,
      specs[0]?.sourceIdentity,
    );

    assert.deepEqual(specs[0]?.sourceIdentity, previewSourceIdentity(input));
    assert.deepEqual(mermaidSourceCommandIdentity(target), previewSourceIdentity(input));
    assert.equal(mermaidSourceCommandSourceId(target), "fence-2");
  });

  it("carries the source id through command targets without depending on cursor state", () => {
    const uri = { toString: () => "file:///workspace/notes.md" };
    const target = mermaidSourceCommandTarget(uri as never, "fence-2");

    assert.equal(isMermaidSourceCommandTarget(target), true);
    assert.equal(mermaidSourceCommandUri(target), uri);
    assert.equal(mermaidSourceCommandSourceId(target), "fence-2");
    assert.equal(
      mermaidSourceCommandUri(uri as MermaidSourceCommandArgument),
      uri,
    );
    assert.equal(
      mermaidSourceCommandSourceId(uri as MermaidSourceCommandArgument),
      undefined,
    );
  });
});

function previewInput(
  sourceId: string,
  source: string,
  startLine: number,
  endLine: number,
): PreviewInput {
  return {
    sourceId,
    source,
    title: "notes.md",
    subtitle: "Mermaid fence 2",
    exportBaseName: "notes-mermaid-2",
    kind: "markdown-fence",
    sourceRange: { startLine, endLine },
    diagnosticRange: { startLine: startLine + 1, endLine: endLine - 1 },
  };
}
