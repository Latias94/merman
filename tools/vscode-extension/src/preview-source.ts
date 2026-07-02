import * as path from "node:path";
import * as vscode from "vscode";

export interface PreviewInput {
  sourceId: string;
  source: string;
  title: string;
  subtitle: string;
  exportBaseName: string;
  kind: "mermaid-file" | "markdown-fence";
  sourceRange: {
    startLine: number;
    endLine: number;
  };
  diagnosticRange: {
    startLine: number;
    endLine: number;
  };
}

interface MermaidFence {
  sourceId: string;
  source: string;
  startLine: number;
  endLine: number;
  contentStartLine: number;
  contentEndLine: number;
  ordinal: number;
}

interface MermaidFenceDelimiter {
  marker: "`" | "~" | ":";
  length: number;
}

export interface PreviewSourceSummary {
  sourceId: string;
  title: string;
  subtitle: string;
  kind: PreviewInput["kind"];
}

export interface PreviewInputTextSource {
  text: string;
  languageId: string;
  fileName: string;
  activeLine?: number;
  sourceId?: string;
  lineCount?: number;
  lineAt?: (lineIndex: number) => string;
}

export function extractPreviewInput(editor: vscode.TextEditor, sourceId?: string): PreviewInput | null {
  return extractPreviewInputFromDocument(editor.document, editor.selection.active.line, sourceId);
}

export function extractPreviewInputFromDocument(
  document: vscode.TextDocument,
  activeLine?: number,
  sourceId?: string,
): PreviewInput | null {
  return extractPreviewInputFromText({
    text: document.getText(),
    languageId: document.languageId,
    fileName: document.uri.fsPath || document.fileName,
    lineCount: document.lineCount,
    lineAt: (lineIndex) => document.lineAt(lineIndex).text,
    activeLine,
    sourceId,
  });
}

export function listPreviewInputsFromDocument(
  document: vscode.TextDocument,
  activeLine?: number,
): PreviewInput[] {
  return listPreviewInputsFromText({
    text: document.getText(),
    languageId: document.languageId,
    fileName: document.uri.fsPath || document.fileName,
    lineCount: document.lineCount,
    lineAt: (lineIndex) => document.lineAt(lineIndex).text,
    activeLine,
  });
}

export function extractPreviewInputFromText(sourceDocument: PreviewInputTextSource): PreviewInput | null {
  const inputs = listPreviewInputsFromText(sourceDocument);
  if (inputs.length === 0) {
    return null;
  }

  if (sourceDocument.sourceId) {
    return inputs.find((input) => input.sourceId === sourceDocument.sourceId) ?? null;
  }

  if (inputs.length === 1) {
    return inputs[0] ?? null;
  }

  return selectInputByActiveLine(inputs, sourceDocument.activeLine) ?? inputs[0] ?? null;
}

export function listPreviewInputsFromText(sourceDocument: PreviewInputTextSource): PreviewInput[] {
  const lines = splitLines(sourceDocument.text);
  const lineCount = sourceDocument.lineCount ?? lines.length;
  const lineAt = sourceDocument.lineAt ?? ((lineIndex) => lines[lineIndex] ?? "");

  if (isMermaidSource(sourceDocument.languageId, sourceDocument.fileName)) {
    const source = sourceDocument.text;
    if (source.trim().length === 0) {
      return [];
    }
    return [
      {
        sourceId: "document",
        source,
        title: path.basename(sourceDocument.fileName || "Untitled"),
        subtitle: "Mermaid source file",
        exportBaseName: exportBaseName(sourceDocument.fileName, "diagram"),
        kind: "mermaid-file",
        sourceRange: {
          startLine: 0,
          endLine: Math.max(lineCount - 1, 0),
        },
        diagnosticRange: {
          startLine: 0,
          endLine: Math.max(lineCount - 1, 0),
        },
      },
    ];
  }

  if (!isMarkdownLikeSource(sourceDocument.languageId)) {
    return [];
  }

  return collectMermaidFences(lineCount, lineAt)
    .filter((fence) => fence.source.trim().length > 0)
    .map((fence) => ({
      sourceId: fence.sourceId,
      source: fence.source,
      title: path.basename(sourceDocument.fileName || "Untitled"),
      subtitle: `Mermaid fence ${fence.ordinal} · lines ${fence.startLine + 1}-${fence.endLine + 1}`,
      exportBaseName: `${exportBaseName(sourceDocument.fileName, "markdown")}-mermaid-${fence.ordinal}`,
      kind: "markdown-fence" as const,
      sourceRange: {
        startLine: fence.startLine,
        endLine: fence.endLine,
      },
      diagnosticRange: {
        startLine: fence.contentStartLine,
        endLine: fence.contentEndLine,
      },
    }));
}

export function listPreviewSourceSummaries(
  document: vscode.TextDocument,
  activeLine?: number,
): PreviewSourceSummary[] {
  return listPreviewInputsFromDocument(document, activeLine).map((input) => ({
    sourceId: input.sourceId,
    title: input.title,
    subtitle: input.subtitle,
    kind: input.kind,
  }));
}

function selectInputByActiveLine(
  inputs: PreviewInput[],
  activeLine: number | undefined,
): PreviewInput | null {
  if (activeLine === undefined) {
    return null;
  }

  return (
    inputs.find(
      (input) =>
        input.sourceRange.startLine <= activeLine && input.sourceRange.endLine >= activeLine,
    ) ?? null
  );
}

function exportBaseName(fileName: string, fallback: string): string {
  if (!fileName) {
    return fallback;
  }
  const parsed = path.parse(fileName);
  return parsed.name || fallback;
}

function isMermaidSource(languageId: string, fileName: string): boolean {
  const lowerFileName = fileName.toLowerCase();
  return (
    languageId === "mermaid" ||
    lowerFileName.endsWith(".mmd") ||
    lowerFileName.endsWith(".mermaid")
  );
}

function isMarkdownLikeSource(languageId: string): boolean {
  return languageId === "markdown" || languageId === "mdx";
}

function collectMermaidFences(
  lineCount: number,
  lineAt: (lineIndex: number) => string,
): MermaidFence[] {
  const fences: MermaidFence[] = [];
  let ordinal = 0;

  for (let lineIndex = 0; lineIndex < lineCount; lineIndex += 1) {
    const line = lineAt(lineIndex);
    const delimiter = mermaidFenceDelimiter(line);
    if (!delimiter) {
      continue;
    }

    const sourceLines: string[] = [];
    let closeLine = lineIndex;

    for (let cursor = lineIndex + 1; cursor < lineCount; cursor += 1) {
      const nextLine = lineAt(cursor);
      if (isMatchingClosingFence(nextLine, delimiter)) {
        closeLine = cursor;
        break;
      }
      sourceLines.push(nextLine);
      closeLine = cursor;
    }

    ordinal += 1;
    fences.push({
      sourceId: `fence-${ordinal}`,
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

function mermaidFenceDelimiter(line: string): MermaidFenceDelimiter | null {
  const trimmed = line.trimStart();
  const marker = trimmed[0];
  if (marker !== "`" && marker !== "~" && marker !== ":") {
    return null;
  }

  const length = repeatedMarkerLength(trimmed, marker);
  if (length < 3) {
    return null;
  }

  const rest = trimmed.slice(length).trimStart();
  const language = rest.slice(0, "mermaid".length);
  const tail = rest.slice("mermaid".length);
  if (language.toLowerCase() !== "mermaid") {
    return null;
  }
  if (tail.length > 0 && !/\s/.test(tail[0] ?? "")) {
    return null;
  }

  return { marker, length };
}

function isMatchingClosingFence(line: string, delimiter: MermaidFenceDelimiter): boolean {
  const trimmed = line.trimStart();
  const length = repeatedMarkerLength(trimmed, delimiter.marker);
  return length >= delimiter.length && trimmed.slice(length).trim().length === 0;
}

function repeatedMarkerLength(line: string, marker: string): number {
  let length = 0;
  while (line[length] === marker) {
    length += 1;
  }
  return length;
}

function splitLines(text: string): string[] {
  return text.split(/\r?\n/);
}
