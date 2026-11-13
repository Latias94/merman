import * as assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";
import type * as vscode from "vscode";

import {
  extractPreviewInputFromText,
  listPreviewInputsFromDocument,
  listPreviewInputsFromText,
  previewSourceIdentity,
} from "../preview-source.js";

interface HostFenceCase {
  name: string;
  language: string;
  text: string;
  fences: {
    start_line: number;
    end_line: number;
    body: string;
    closing: string | null;
  }[];
}

const hostFenceCases: HostFenceCase[] = JSON.parse(readFileSync(
  path.resolve(__dirname, "../../../../crates/merman-analysis/tests/fixtures/editor_host_fences.json"),
  "utf8",
)).cases;

describe("shared editor-host fence contract", () => {
  for (const fixture of hostFenceCases) {
    it(fixture.name, () => {
      const expected = fixture.fences
        .map((fence, index) => ({
          sourceId: `fence-${index + 1}`,
          source: fence.closing === null
            ? fence.body.replace(/\r\n|\r/g, "\n")
            : fence.body.replace(/\r\n|\r/g, "\n").replace(/\n$/, ""),
          sourceRange: { startLine: fence.start_line, endLine: fence.end_line },
          diagnosticRange: {
            startLine: fence.start_line + 1,
            endLine: fence.closing === null
              ? fence.end_line
              : Math.max(fence.end_line - 1, fence.start_line),
          },
        }))
        .filter((input) => input.source.trim().length > 0);
      const project = (inputs: ReturnType<typeof listPreviewInputsFromText>) => inputs.map(
        ({ sourceId, source, sourceRange, diagnosticRange }) => ({
          sourceId, source, sourceRange, diagnosticRange,
        }),
      );
      const fileName = `/workspace/notes.${fixture.language === "mdx" ? "mdx" : "md"}`;
      const inputs = listPreviewInputsFromText({
        text: fixture.text,
        languageId: fixture.language,
        fileName,
      });
      assert.deepEqual(project(inputs), expected);

      const lines = fixture.text.split(/\r\n|[\r\n]/);
      const document = {
        languageId: fixture.language,
        fileName,
        uri: { fsPath: fileName },
        lineCount: lines.length,
        lineAt: (lineIndex: number) => ({ text: lines[lineIndex] ?? "" }),
        getText: () => {
          throw new Error("Markdown previews must scan original document lines");
        },
      } as unknown as vscode.TextDocument;
      assert.deepEqual(project(listPreviewInputsFromDocument(document)), expected);
    });
  }

  it("hides and reveals existing preview identities when host comments change", () => {
    const body = "```mermaid\r\nsequenceDiagram\r\nAlice->>Bob: 😀\r\n```\r\n";
    const source = { text: body, languageId: "markdown", fileName: "/workspace/notes.md" };
    const visible = listPreviewInputsFromText(source);
    assert.equal(visible.length, 1);
    const sourceIdentity = previewSourceIdentity(visible[0]!);
    assert.equal(extractPreviewInputFromText({
      ...source, text: `<!--\r\n${body}`, sourceIdentity,
    }), null);
    const revealed = extractPreviewInputFromText({
      ...source, text: `<!-- closed -->\r\n${body}`, sourceIdentity,
    });
    assert.equal(revealed?.source, "sequenceDiagram\nAlice->>Bob: 😀");
    assert.deepEqual(revealed?.sourceRange, { startLine: 1, endLine: 4 });
  });
});

describe("preview source extraction", () => {
  it("extracts Mermaid files as whole-document sources", () => {
    const input = extractPreviewInputFromText({
      text: "flowchart TD\n  A --> B\n",
      languageId: "mermaid",
      fileName: "/workspace/diagram.mmd",
    });

    assert.equal(input?.kind, "mermaid-file");
    assert.equal(input?.sourceId, "document");
    assert.equal(input?.source, "flowchart TD\n  A --> B\n");
    assert.equal(input?.exportBaseName, "diagram");
    assert.deepEqual(input?.sourceRange, { startLine: 0, endLine: 2 });
    assert.deepEqual(input?.diagnosticRange, { startLine: 0, endLine: 2 });
  });

  it("selects the Mermaid fence containing the active Markdown cursor", () => {
    const input = extractPreviewInputFromText({
      text: [
        "# Notes",
        "",
        "```mermaid",
        "flowchart TD",
        "  A --> B",
        "```",
        "",
        "```mermaid",
        "sequenceDiagram",
        "  Alice->>Bob: Hi",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
      activeLine: 8,
    });

    assert.equal(input?.kind, "markdown-fence");
    assert.equal(input?.sourceId, "fence-2");
    assert.equal(input?.source, "sequenceDiagram\n  Alice->>Bob: Hi");
    assert.equal(input?.exportBaseName, "notes-mermaid-2");
    assert.deepEqual(input?.sourceRange, { startLine: 7, endLine: 10 });
    assert.deepEqual(input?.diagnosticRange, { startLine: 8, endLine: 9 });
  });

  it("falls back to the first Mermaid fence when no cursor line is provided", () => {
    const input = extractPreviewInputFromText({
      text: [
        "```mermaid",
        "flowchart LR",
        "  A --> B",
        "```",
        "",
        "```mermaid",
        "pie title Work",
        "  \"A\" : 1",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.equal(input?.source, "flowchart LR\n  A --> B");
    assert.equal(input?.exportBaseName, "notes-mermaid-1");
  });

  it("selects a Markdown Mermaid fence by source id", () => {
    const input = extractPreviewInputFromText({
      text: [
        "```mermaid",
        "flowchart LR",
        "```",
        "",
        "```mermaid",
        "pie title Work",
        "  \"A\" : 1",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
      sourceId: "fence-2",
    });

    assert.equal(input?.source, "pie title Work\n  \"A\" : 1");
    assert.equal(input?.sourceId, "fence-2");
  });

  it("resolves a Markdown fence identity after another fence is inserted before it", () => {
    const original = listPreviewInputsFromText({
      text: markdownWithTwoFences(),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });
    const selected = original[1];
    assert.ok(selected);

    const input = extractPreviewInputFromText({
      text: [
        "```mermaid",
        "stateDiagram-v2",
        "  [*] --> Inserted",
        "```",
        "",
        markdownWithTwoFences(),
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
      sourceIdentity: previewSourceIdentity(selected),
    });

    assert.equal(input?.sourceId, "fence-3");
    assert.equal(input?.source, "sequenceDiagram\nA->>B: hi");
  });

  it("lists every Mermaid fence with stable ids", () => {
    const inputs = listPreviewInputsFromText({
      text: [
        "```mermaid",
        "flowchart LR",
        "```",
        "",
        "```mermaid",
        "sequenceDiagram",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.deepEqual(
      inputs.map((input) => input.sourceId),
      ["fence-1", "fence-2"],
    );
  });

  it("scans Markdown TextDocuments by line without copying the whole document", () => {
    const lines = [
      "# Notes",
      "```mermaid title=Main",
      "flowchart LR",
      "```",
    ];
    const document = {
      languageId: "markdown",
      fileName: "/workspace/notes.md",
      uri: { fsPath: "/workspace/notes.md" },
      lineCount: lines.length,
      lineAt: (lineIndex: number) => ({ text: lines[lineIndex] ?? "" }),
      getText: () => {
        throw new Error("Markdown preview extraction should not copy whole documents");
      },
    } as unknown as vscode.TextDocument;

    const inputs = listPreviewInputsFromDocument(document);

    assert.deepEqual(
      inputs.map((input) => input.source),
      ["flowchart LR"],
    );
  });

  it("accepts the same Markdown Mermaid fence forms as analysis", () => {
    const inputs = listPreviewInputsFromText({
      text: [
        "```` mermaid title=Main",
        "flowchart LR",
        "````",
        "",
        "~~~ Mermaid",
        "sequenceDiagram",
        "~~~",
        "",
        ":::MERMAID extra info",
        "pie title Work",
        ":::",
      ].join("\n"),
      languageId: "mdx",
      fileName: "/workspace/notes.mdx",
    });

    assert.deepEqual(
      inputs.map((input) => input.source),
      ["flowchart LR", "sequenceDiagram", "pie title Work"],
    );
  });

  it("skips non-Mermaid fences before looking for Mermaid fences", () => {
    const inputs = listPreviewInputsFromText({
      text: [
        "````js",
        "```mermaid",
        "flowchart LR",
        "```",
        "````",
        "",
        "```mermaid",
        "sequenceDiagram",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.deepEqual(
      inputs.map((input) => input.source),
      ["sequenceDiagram"],
    );
  });

  it("matches analysis fence indentation rules", () => {
    const inputs = listPreviewInputsFromText({
      text: [
        "   ```mermaid",
        "flowchart LR",
        "   ```",
        "",
        "    ```mermaid",
        "sequenceDiagram",
        "    ```",
        "",
        "\t```mermaid",
        "pie title Work",
        "\t```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.deepEqual(
      inputs.map((input) => input.source),
      ["flowchart LR"],
    );
  });

  it("treats .mdx filenames as Markdown-like sources without an mdx language id", () => {
    const inputs = listPreviewInputsFromText({
      text: ["```mermaid", "flowchart LR", "```"].join("\n"),
      languageId: "plaintext",
      fileName: "/workspace/notes.mdx",
    });

    assert.deepEqual(
      inputs.map((input) => input.source),
      ["flowchart LR"],
    );
  });

  it("does not treat mermaid-prefixed languages as Mermaid fences", () => {
    const inputs = listPreviewInputsFromText({
      text: ["```mermaidx", "flowchart LR", "```"].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.equal(inputs.length, 0);
  });

  it("treats fence delimiters as part of the selectable source range", () => {
    const input = extractPreviewInputFromText({
      text: [
        "```mermaid",
        "flowchart LR",
        "```",
        "",
        "```mermaid",
        "sequenceDiagram",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
      activeLine: 4,
    });

    assert.equal(input?.sourceId, "fence-2");
  });

  it("keeps the final content line in unclosed EOF Mermaid fence diagnostic ranges", () => {
    const input = extractPreviewInputFromText({
      text: [
        "# Notes",
        "",
        "```mermaid",
        "flowchart LR",
        "A -->",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.equal(input?.source, "flowchart LR\nA -->");
    assert.deepEqual(input?.sourceRange, { startLine: 2, endLine: 4 });
    assert.deepEqual(input?.diagnosticRange, { startLine: 3, endLine: 4 });
  });

  it("keeps source and diagnostic ranges when a Mermaid fence closes at EOF", () => {
    const input = extractPreviewInputFromText({
      text: [
        "# Notes",
        "",
        "```mermaid",
        "flowchart LR",
        "A --> B",
        "```",
      ].join("\n"),
      languageId: "markdown",
      fileName: "/workspace/notes.md",
    });

    assert.equal(input?.source, "flowchart LR\nA --> B");
    assert.deepEqual(input?.sourceRange, { startLine: 2, endLine: 5 });
    assert.deepEqual(input?.diagnosticRange, { startLine: 3, endLine: 4 });
  });
});

function markdownWithTwoFences(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "sequenceDiagram",
    "A->>B: hi",
    "```",
  ].join("\n");
}
