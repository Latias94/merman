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
