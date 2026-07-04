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

export interface PreviewSourceIdentity {
  sourceId: string;
  sourceHash: string;
  kind: PreviewInput["kind"];
  sourceRange: {
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
  isMermaid: boolean;
}

export interface PreviewSourceSummary {
  sourceId: string;
  title: string;
  subtitle: string;
  kind: PreviewInput["kind"];
}

export interface PreviewInputTextSource {
  text?: string;
  languageId: string;
  fileName: string;
  activeLine?: number;
  sourceId?: string;
  sourceIdentity?: PreviewSourceIdentity;
  lineCount?: number;
  lineAt?: (lineIndex: number) => string;
}

export function extractPreviewInput(
  editor: vscode.TextEditor,
  sourceId?: string | PreviewSourceIdentity,
): PreviewInput | null {
  return extractPreviewInputFromDocument(editor.document, editor.selection.active.line, sourceId);
}

export function extractPreviewInputFromDocument(
  document: vscode.TextDocument,
  activeLine?: number,
  sourceId?: string | PreviewSourceIdentity,
): PreviewInput | null {
  const fileName = document.uri.fsPath || document.fileName;
  return extractPreviewInputFromText({
    languageId: document.languageId,
    fileName,
    text: isMermaidSource(document.languageId, fileName) ? document.getText() : undefined,
    lineCount: document.lineCount,
    lineAt: (lineIndex) => document.lineAt(lineIndex).text,
    activeLine,
    ...(typeof sourceId === "string" ? { sourceId } : { sourceIdentity: sourceId }),
  });
}

export function listPreviewInputsFromDocument(
  document: vscode.TextDocument,
  activeLine?: number,
): PreviewInput[] {
  const fileName = document.uri.fsPath || document.fileName;
  return listPreviewInputsFromText({
    languageId: document.languageId,
    fileName,
    text: isMermaidSource(document.languageId, fileName) ? document.getText() : undefined,
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

  if (sourceDocument.sourceIdentity) {
    return resolvePreviewInputIdentity(inputs, sourceDocument.sourceIdentity);
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
  const lineSource = lineSourceFor(sourceDocument);
  const lineCount = lineSource.lineCount;
  const lineAt = lineSource.lineAt;

  if (isMermaidSource(sourceDocument.languageId, sourceDocument.fileName)) {
    const source = sourceDocument.text ?? collectDocumentText(lineCount, lineAt);
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

  if (!isMarkdownLikeSource(sourceDocument.languageId, sourceDocument.fileName)) {
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

export function previewSourceIdentity(input: PreviewInput): PreviewSourceIdentity {
  return {
    sourceId: input.sourceId,
    sourceHash: hashPreviewSource(input.source),
    kind: input.kind,
    sourceRange: {
      startLine: input.sourceRange.startLine,
      endLine: input.sourceRange.endLine,
    },
  };
}

export function resolvePreviewInputIdentity(
  inputs: readonly PreviewInput[],
  identity: PreviewSourceIdentity,
): PreviewInput | null {
  const sameKind = inputs.filter((input) => input.kind === identity.kind);
  if (identity.kind === "mermaid-file") {
    return sameKind.find((input) => input.sourceId === identity.sourceId) ?? null;
  }

  const sameHash = sameKind.filter(
    (input) => hashPreviewSource(input.source) === identity.sourceHash,
  );
  if (sameHash.length === 0) {
    return null;
  }

  return (
    sameHash.find((input) => sameSourceRange(input.sourceRange, identity.sourceRange)) ??
    sameHash.find((input) => input.sourceId === identity.sourceId) ??
    sameHash[0] ??
    null
  );
}

export function hashPreviewSource(source: string): string {
  let hash = 2166136261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
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

function sameSourceRange(
  first: { startLine: number; endLine: number },
  second: { startLine: number; endLine: number },
): boolean {
  return first.startLine === second.startLine && first.endLine === second.endLine;
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

function isMarkdownLikeSource(languageId: string, fileName: string): boolean {
  const lowerFileName = fileName.toLowerCase();
  return (
    languageId === "markdown" ||
    languageId === "mdx" ||
    lowerFileName.endsWith(".md") ||
    lowerFileName.endsWith(".markdown") ||
    lowerFileName.endsWith(".mdx")
  );
}

function collectMermaidFences(
  lineCount: number,
  lineAt: (lineIndex: number) => string,
): MermaidFence[] {
  const fences: MermaidFence[] = [];
  let ordinal = 0;

  for (let lineIndex = 0; lineIndex < lineCount; lineIndex += 1) {
    const line = lineAt(lineIndex);
    const delimiter = markdownFenceDelimiter(line);
    if (!delimiter) {
      continue;
    }
    if (!delimiter.isMermaid) {
      lineIndex = skipMarkdownFence(lineCount, lineAt, lineIndex, delimiter);
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

function markdownFenceDelimiter(line: string): MermaidFenceDelimiter | null {
  const trimmed = trimFenceIndent(line);
  if (trimmed === null) {
    return null;
  }
  const marker = trimmed[0];
  if (marker !== "`" && marker !== "~" && marker !== ":") {
    return null;
  }

  const length = repeatedMarkerLength(trimmed, marker);
  if (length < 3) {
    return null;
  }

  const rest = trimmed.slice(length).trimStart();
  if (rest.length === 0) {
    return { marker, length, isMermaid: false };
  }
  const language = rest.slice(0, "mermaid".length);
  const tail = rest.slice("mermaid".length);
  const isMermaid =
    language.toLowerCase() === "mermaid" &&
    (tail.length === 0 || /\s/.test(tail[0] ?? ""));

  return { marker, length, isMermaid };
}

function isMatchingClosingFence(line: string, delimiter: MermaidFenceDelimiter): boolean {
  const trimmed = trimFenceIndent(line);
  if (trimmed === null) {
    return false;
  }
  const length = repeatedMarkerLength(trimmed, delimiter.marker);
  return length >= delimiter.length && trimmed.slice(length).trim().length === 0;
}

function skipMarkdownFence(
  lineCount: number,
  lineAt: (lineIndex: number) => string,
  openingLine: number,
  delimiter: MermaidFenceDelimiter,
): number {
  for (let cursor = openingLine + 1; cursor < lineCount; cursor += 1) {
    if (isMatchingClosingFence(lineAt(cursor), delimiter)) {
      return cursor;
    }
  }
  return Math.max(lineCount - 1, openingLine);
}

function trimFenceIndent(line: string): string | null {
  let spaces = 0;
  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (char === " " && spaces < 3) {
      spaces += 1;
      continue;
    }
    if (char === " " || char === "\t") {
      return null;
    }
    return line.slice(index);
  }
  return "";
}

function repeatedMarkerLength(line: string, marker: string): number {
  let length = 0;
  while (line[length] === marker) {
    length += 1;
  }
  return length;
}

function lineSourceFor(sourceDocument: PreviewInputTextSource): {
  lineCount: number;
  lineAt: (lineIndex: number) => string;
} {
  if (sourceDocument.lineAt && sourceDocument.lineCount !== undefined) {
    return {
      lineCount: sourceDocument.lineCount,
      lineAt: sourceDocument.lineAt,
    };
  }

  const lines = splitLines(sourceDocument.text ?? "");
  return {
    lineCount: sourceDocument.lineCount ?? lines.length,
    lineAt: (lineIndex) => lines[lineIndex] ?? "",
  };
}

function collectDocumentText(lineCount: number, lineAt: (lineIndex: number) => string): string {
  const lines: string[] = [];
  for (let lineIndex = 0; lineIndex < lineCount; lineIndex += 1) {
    lines.push(lineAt(lineIndex));
  }
  return lines.join("\n");
}

function splitLines(text: string): string[] {
  return text.split(/\r?\n/);
}
