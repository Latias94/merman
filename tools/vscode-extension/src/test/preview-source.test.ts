import * as assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";
import type * as vscode from "vscode";

import {
  extractPreviewInputFromText,
  getPreviewSourceSnapshot,
  listPreviewInputsFromText,
  previewSourceIdentity,
  selectPreviewInput,
} from "../preview-source.js";
import { buildMermaidSourceCodeLensSpecs } from "../source-actions.js";

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
      const inputs = listPreviewInputsFromText({
        text: fixture.text,
        languageId: fixture.language,
        fileName: `/workspace/notes.${fixture.language === "mdx" ? "mdx" : "md"}`,
      });
      assert.deepEqual(project(inputs), expected);

      const { document, state } = sourceDocument(fixture.text);
      state.lines = fixture.text.split(/\r\n|[\r\n]/);
      state.languageId = fixture.language;
      const snapshot = getPreviewSourceSnapshot(document);
      assert.deepEqual(project([...snapshot.inputs]), expected);
      assert.equal(getPreviewSourceSnapshot(document), snapshot);
      assert.equal(state.textReads, 0);
      assert.equal(state.lineReads, state.lines.length);

      const specs = buildMermaidSourceCodeLensSpecs(snapshot.inputs);
      assert.equal(specs.length, expected.length * 2);
      assert.deepEqual(
        specs.filter((spec) => spec.title === "Preview")
          .map((spec) => [spec.sourceId, spec.line]),
        expected.map((input) => [input.sourceId, input.sourceRange.startLine]),
      );
      for (const input of inputs) {
        assert.equal(
          selectPreviewInput(inputs, { sourceIdentity: previewSourceIdentity(input) }),
          input,
        );
      }
      if (expected.length === 0) {
        assert.equal(selectPreviewInput(inputs), null);
      }
    });
  }
});

describe("preview source extraction", () => {
  it("preserves JavaScript trim semantics when filtering preview content", () => {
    for (const [text, expectedCount] of [["\ufeff", 0], ["\u0085", 1]] as const) {
      const documentInputs = listPreviewInputsFromText({
        text,
        languageId: "mermaid",
        fileName: "/workspace/diagram.mmd",
      });
      const fenceInputs = listPreviewInputsFromText({
        text: `\`\`\`mermaid\n${text}\n\`\`\``,
        languageId: "markdown",
        fileName: "/workspace/notes.md",
      });
      assert.equal(documentInputs.length, expectedCount);
      assert.equal(fenceInputs.length, expectedCount);
      if (expectedCount > 0) {
        assert.equal(documentInputs[0]?.source, text);
        assert.equal(fenceInputs[0]?.source, text);
      }
    }
  });

  it("refreshes preview and CodeLens sources when comments hide and reveal a fence", () => {
    const lines = ["```mermaid", "sequenceDiagram", "Alice->>Bob: 😀", "```"];
    const { document, state } = sourceDocument(lines.join("\n"));
    const visible = getPreviewSourceSnapshot(document);
    const identity = previewSourceIdentity(visible.inputs[0]!);

    state.lines = ["<!--", ...lines];
    state.version += 1;
    const hidden = getPreviewSourceSnapshot(document);
    assert.deepEqual(hidden.inputs, []);
    assert.deepEqual(buildMermaidSourceCodeLensSpecs(hidden.inputs), []);
    assert.equal(selectPreviewInput(hidden.inputs, { sourceIdentity: identity }), null);

    state.lines = ["<!-- closed -->", ...lines];
    state.version += 1;
    const revealed = getPreviewSourceSnapshot(document);
    const selected = selectPreviewInput(revealed.inputs, { sourceIdentity: identity });
    assert.equal(selected?.source, "sequenceDiagram\nAlice->>Bob: 😀");
    assert.deepEqual(selected?.sourceRange, { startLine: 1, endLine: 4 });
    assert.deepEqual(
      buildMermaidSourceCodeLensSpecs(revealed.inputs).map((spec) => spec.line),
      [1, 1],
    );
    assert.equal(visible.inputs.length, 1);
    assert.equal(state.textReads, 0);
  });

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

  it("shares unchanged Markdown source facts without copying or rescanning the document", () => {
    const lines = [
      "# Notes",
      "```mermaid title=Main",
      "flowchart LR",
      "```",
    ];
    let lineReads = 0;
    const document = {
      languageId: "markdown",
      fileName: "/workspace/notes.md",
      uri: {
        fsPath: "/workspace/notes.md",
        toString: () => "file:///workspace/notes.md",
      },
      version: 1,
      lineCount: lines.length,
      lineAt: (lineIndex: number) => {
        lineReads += 1;
        return { text: lines[lineIndex] ?? "" };
      },
      getText: () => {
        throw new Error("Markdown preview extraction should not copy whole documents");
      },
    } as unknown as vscode.TextDocument;

    const snapshot = getPreviewSourceSnapshot(document);

    assert.deepEqual(
      snapshot.inputs.map((input) => input.source),
      ["flowchart LR"],
    );
    assert.equal(getPreviewSourceSnapshot(document), snapshot);
    assert.equal(selectPreviewInput(snapshot.inputs, { activeLine: 2 }), snapshot.inputs[0]);
    assert.equal(lineReads, lines.length);
  });

  it("replaces the current version facts while keeping earlier snapshots unchanged", () => {
    const { document, state } = sourceDocument(markdownWithTwoFences());
    const original = getPreviewSourceSnapshot(document);

    state.lines = ["```mermaid", "pie title Updated", "```"];
    state.version = 4;
    const updated = getPreviewSourceSnapshot(document);

    assert.notEqual(updated, original);
    assert.equal(updated.documentVersion, 4);
    assert.deepEqual(updated.inputs.map((input) => input.source), ["pie title Updated"]);
    assert.deepEqual(original.inputs.map((input) => input.source), [
      "flowchart TD\nA --> B",
      "sequenceDiagram\nA->>B: hi",
    ]);
    assert.equal(getPreviewSourceSnapshot(document), updated);
    assert.equal(state.lineReads, 12);
    assert.equal(state.textReads, 0);
    assert.equal(Object.isFrozen(updated.inputs), true);
    assert.equal(Object.isFrozen(updated.inputs[0]), true);
    assert.equal(Object.isFrozen(updated.inputs[0]?.sourceRange), true);
    assert.equal(Object.isFrozen(updated.inputs[0]?.diagnosticRange), true);
  });

  it("keeps only current language facts even when the document version does not change", () => {
    const { document, state } = sourceDocument("```mermaid\nflowchart LR\n```", "notes.txt");
    const markdown = getPreviewSourceSnapshot(document);
    assert.equal(markdown.inputs[0]?.kind, "markdown-fence");

    state.languageId = "plaintext";
    const plain = getPreviewSourceSnapshot(document);
    assert.deepEqual(plain.inputs, []);
    assert.equal(getPreviewSourceSnapshot(document), plain);

    state.languageId = "mermaid";
    const mermaid = getPreviewSourceSnapshot(document);
    assert.equal(mermaid.inputs[0]?.kind, "mermaid-file");
    assert.equal(mermaid.inputs[0]?.source, "```mermaid\nflowchart LR\n```");
    assert.equal(getPreviewSourceSnapshot(document), mermaid);

    state.languageId = "markdown";
    const restored = getPreviewSourceSnapshot(document);
    assert.notEqual(restored, markdown);
    assert.equal(restored.inputs[0]?.source, "flowchart LR");
    assert.equal(state.lineReads, 6);
    assert.equal(state.textReads, 1);
  });

  it("does not share source facts between replacement documents with the same URI and version", () => {
    const first = sourceDocument("```mermaid\nflowchart LR\n```");
    const replacement = sourceDocument("```mermaid\nsequenceDiagram\n```");

    const original = getPreviewSourceSnapshot(first.document);
    const updated = getPreviewSourceSnapshot(replacement.document);

    assert.equal(original.documentUri, updated.documentUri);
    assert.equal(original.documentVersion, updated.documentVersion);
    assert.equal(original.inputs[0]?.source, "flowchart LR");
    assert.equal(updated.inputs[0]?.source, "sequenceDiagram");
    assert.equal(first.state.lineReads, 3);
    assert.equal(replacement.state.lineReads, 3);
  });

  it("does not reuse closed-document facts when the URI is reopened", () => {
    const first = sourceDocument("```mermaid\nflowchart LR\n```");
    const original = getPreviewSourceSnapshot(first.document);
    first.state.isClosed = true;

    assert.deepEqual(getPreviewSourceSnapshot(first.document).inputs, []);
    assert.equal(first.state.lineReads, 3);

    const reopened = sourceDocument("```mermaid\nsequenceDiagram\n```");
    const snapshot = getPreviewSourceSnapshot(reopened.document);
    assert.equal(snapshot.inputs[0]?.source, "sequenceDiagram");
    assert.equal(original.inputs[0]?.source, "flowchart LR");
    assert.equal(reopened.state.lineReads, 3);
  });

  it("shares empty source facts without repeating Markdown extraction", () => {
    const { document, state } = sourceDocument("# Notes\n\nNo diagrams");
    const snapshot = getPreviewSourceSnapshot(document);

    assert.deepEqual(snapshot.inputs, []);
    assert.equal(selectPreviewInput(snapshot.inputs), null);
    assert.equal(getPreviewSourceSnapshot(document), snapshot);
    assert.equal(state.lineReads, 3);
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

function sourceDocument(text: string, fileName = "notes.md") {
  const state = {
    lines: text.split(/\r?\n/),
    version: 1,
    languageId: "markdown",
    isClosed: false,
    lineReads: 0,
    textReads: 0,
  };
  const document = {
    uri: {
      fsPath: `/workspace/${fileName}`,
      toString: () => `file:///workspace/${fileName}`,
    },
    fileName,
    get languageId() {
      return state.languageId;
    },
    get version() {
      return state.version;
    },
    get isClosed() {
      return state.isClosed;
    },
    get lineCount() {
      return state.lines.length;
    },
    lineAt: (lineIndex: number) => {
      state.lineReads += 1;
      return { text: state.lines[lineIndex] ?? "" };
    },
    getText: () => {
      state.textReads += 1;
      return state.lines.join("\n");
    },
  } as unknown as vscode.TextDocument;
  return { document, state };
}
