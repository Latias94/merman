import * as path from "node:path";
import * as vscode from "vscode";

export interface PreviewInput {
  source: string;
  title: string;
  subtitle: string;
  kind: "mermaid-file" | "markdown-fence";
  diagnosticRange: {
    startLine: number;
    endLine: number;
  };
}

interface MermaidFence {
  source: string;
  startLine: number;
  endLine: number;
  contentStartLine: number;
  contentEndLine: number;
  ordinal: number;
}

export function extractPreviewInput(editor: vscode.TextEditor): PreviewInput | null {
  const { document } = editor;
  if (isMermaidDocument(document)) {
    const source = document.getText();
    if (source.trim().length === 0) {
      return null;
    }
    return {
      source,
      title: path.basename(document.uri.fsPath || document.fileName || "Untitled"),
      subtitle: "Mermaid source file",
      kind: "mermaid-file",
      diagnosticRange: {
        startLine: 0,
        endLine: Math.max(document.lineCount - 1, 0),
      },
    };
  }

  if (!isMarkdownLikeDocument(document)) {
    return null;
  }

  const fences = collectMermaidFences(document);
  if (fences.length === 0) {
    return null;
  }

  const activeLine = editor.selection.active.line;
  const selectedFence =
    fences.find((fence) => activeLine >= fence.startLine && activeLine <= fence.endLine) ??
    fences[0];

  if (!selectedFence || selectedFence.source.trim().length === 0) {
    return null;
  }

  return {
    source: selectedFence.source,
    title: path.basename(document.uri.fsPath || document.fileName || "Untitled"),
    subtitle: `Mermaid fence ${selectedFence.ordinal} · lines ${selectedFence.startLine + 1}-${selectedFence.endLine + 1}`,
    kind: "markdown-fence",
    diagnosticRange: {
      startLine: selectedFence.contentStartLine,
      endLine: selectedFence.contentEndLine,
    },
  };
}

function isMermaidDocument(document: vscode.TextDocument): boolean {
  const fileName = document.fileName.toLowerCase();
  return (
    document.languageId === "mermaid" ||
    fileName.endsWith(".mmd") ||
    fileName.endsWith(".mermaid")
  );
}

function isMarkdownLikeDocument(document: vscode.TextDocument): boolean {
  return document.languageId === "markdown" || document.languageId === "mdx";
}

function collectMermaidFences(document: vscode.TextDocument): MermaidFence[] {
  const fences: MermaidFence[] = [];
  let ordinal = 0;

  for (let lineIndex = 0; lineIndex < document.lineCount; lineIndex += 1) {
    const line = document.lineAt(lineIndex).text;
    const openMatch = line.match(/^\s*([`~]{3,})\s*mermaid(?:\s+.*)?$/i);
    if (!openMatch) {
      continue;
    }

    const openFence = openMatch[1] ?? "```";
    const closePattern = new RegExp(`^\\s*\\${openFence[0]}{${openFence.length},}\\s*$`);
    const sourceLines: string[] = [];
    let closeLine = lineIndex;

    for (let cursor = lineIndex + 1; cursor < document.lineCount; cursor += 1) {
      const nextLine = document.lineAt(cursor).text;
      if (closePattern.test(nextLine)) {
        closeLine = cursor;
        break;
      }
      sourceLines.push(nextLine);
      closeLine = cursor;
    }

    ordinal += 1;
    fences.push({
      source: sourceLines.join("\n"),
      startLine: lineIndex,
      endLine: closeLine,
      contentStartLine: lineIndex + 1,
      contentEndLine: Math.max(closeLine - 1, lineIndex),
      ordinal,
    });

    lineIndex = closeLine;
  }

  return fences;
}
