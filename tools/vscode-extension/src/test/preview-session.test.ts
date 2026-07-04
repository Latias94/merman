import * as assert from "node:assert/strict";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import { PreviewSession } from "../preview-session.js";

describe("preview session", () => {
  it("resolves a remembered Mermaid resource after the webview takes active editor focus", () => {
    const session = new PreviewSession();
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");

    session.rememberResource(editor.document.uri);

    const snapshot = session.createSnapshot(undefined, [editor], () => ({
      summary: "0 errors, 0 warnings, 0 infos, 0 hints",
      totalCount: 0,
    }));

    assert.equal(snapshot?.input.kind, "mermaid-file");
    assert.equal(snapshot?.input.sourceId, "document");
    assert.equal(snapshot?.documentUri, "file:///workspace/example.mmd");
    assert.equal(snapshot?.background, "paper");
  });

  it("clears an explicit source selection so preview can follow the cursor again", () => {
    const session = new PreviewSession();
    const editor = textEditor(
      "file:///workspace/notes.md",
      "notes.md",
      [
        "# Notes",
        "",
        "```mermaid",
        "flowchart TD",
        "A --> B",
        "```",
        "",
        "```mermaid",
        "sequenceDiagram",
        "Alice->>Bob: hi",
        "```",
      ].join("\n"),
      "markdown",
      3,
    );

    session.rememberResource(editor.document.uri);
    assert.equal(session.selectSource(editor, [editor], "fence-2"), true);

    let snapshot = session.createSnapshot(editor, [editor], emptyDiagnostics);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.selected, true);

    session.clearSelectedSource();
    snapshot = session.createSnapshot(editor, [editor], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-1");
    assert.equal(snapshot?.selected, false);
  });

  it("prefers an explicitly opened resource once without disabling follow mode", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");

    session.rememberResource(second.document.uri, { preferOnce: true });

    let snapshot = session.createSnapshot(first, [first, second], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/two.mmd");

    snapshot = session.createSnapshot(first, [first, second], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
  });

  it("prefers an explicitly opened Markdown source once without disabling follow mode", () => {
    const session = new PreviewSession();
    const active = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const target = textEditor(
      "file:///workspace/notes.md",
      "notes.md",
      markdownWithTwoFences(),
      "markdown",
      7,
    );

    session.rememberResource(target.document.uri, { preferOnce: true });
    assert.equal(session.selectSource(target, [active, target], "fence-2"), true);

    let snapshot = session.createSnapshot(active, [active, target], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/notes.md");
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.selected, true);

    snapshot = session.createSnapshot(active, [active, target], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
    assert.equal(snapshot?.input.sourceId, "document");
    assert.equal(snapshot?.selected, false);
  });

  it("keeps a locked preview on the remembered source instead of following the active editor", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");

    session.rememberResource(first.document.uri);
    const initial = session.createSnapshot(first, [first], emptyDiagnostics);
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(second, [first, second], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps a locked preview on the last snapshot when the source editor is no longer visible", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");

    session.rememberResource(first.document.uri);
    const initial = session.createSnapshot(first, [first], emptyDiagnostics);
    assert.ok(initial);
    session.rememberSnapshot(initial);
    assert.equal(session.setLocked(true), true);
    assert.equal(session.setDiagramTheme("dark"), true);

    const snapshot = session.createSnapshot(second, [second], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
    assert.equal(snapshot?.input.source, "flowchart TD\nA --> B\n");
    assert.equal(snapshot?.diagramTheme, "dark");
    assert.equal(snapshot?.sourceKey.diagramTheme, "dark");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps a locked Markdown fence on the last snapshot when the selected fence disappears", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7);
    const edited = textEditor(
      uri,
      "notes.md",
      ["```mermaid", "flowchart TD", "A --> B", "```"].join("\n"),
      "markdown",
      1,
    );

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps a locked Markdown fence on the original body after a new fence is inserted before it", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7);
    const edited = textEditor(uri, "notes.md", markdownWithInsertedFence(), "markdown", 1);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-3");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps the locked snapshot when the selected fence is deleted but its old ordinal still exists", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7);
    const edited = textEditor(uri, "notes.md", markdownWithReplacementSecondFence(), "markdown", 7);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.locked, true);
  });

  it("locks the current Markdown fence even when it was selected by cursor position", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 6);
    const editedOnOtherFence = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 1);

    session.rememberResource(original.document.uri);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    let snapshot = session.createSnapshot(editedOnOtherFence, [editedOnOtherFence], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.locked, true);
    assert.equal(snapshot?.selected, true);

    assert.equal(session.setLocked(false), true);
    snapshot = session.createSnapshot(editedOnOtherFence, [editedOnOtherFence], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-1");
    assert.equal(snapshot?.locked, false);
    assert.equal(snapshot?.selected, false);
  });
});

function emptyDiagnostics() {
  return {
    summary: "0 errors, 0 warnings, 0 infos, 0 hints",
    totalCount: 0,
  };
}

function textEditor(
  uri: string,
  fileName: string,
  text: string,
  languageId = "mermaid",
  activeLine = 0,
): vscode.TextEditor {
  const lines = text.split(/\r?\n/);
  return {
    document: {
      uri: {
        fsPath: fileName,
        toString: () => uri,
      },
      languageId,
      fileName,
      version: 1,
      lineCount: lines.length,
      getText: () => text,
      lineAt: (lineIndex: number) => ({
        text: lines[lineIndex] ?? "",
      }),
    },
    selection: {
      active: {
        line: activeLine,
      },
    },
  } as unknown as vscode.TextEditor;
}

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

function markdownWithInsertedFence(): string {
  return [
    "```mermaid",
    "stateDiagram-v2",
    "[*] --> Inserted",
    "```",
    "",
    markdownWithTwoFences(),
  ].join("\n");
}

function markdownWithReplacementSecondFence(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "pie title Replacement",
    "  \"A\" : 1",
    "```",
  ].join("\n");
}

function assertDefined<T>(value: T | undefined): T {
  assert.ok(value);
  return value;
}
