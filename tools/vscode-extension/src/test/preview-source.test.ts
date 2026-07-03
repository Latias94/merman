import * as assert from "node:assert/strict";
import { describe, it } from "node:test";
import type * as vscode from "vscode";

import { extractPreviewInputFromText, listPreviewInputsFromDocument, listPreviewInputsFromText } from "../preview-source.js";

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
});
