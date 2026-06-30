import type * as vscode from "vscode";

import type { PreviewInput } from "./preview-source.js";

export const SOURCE_ACTION_COMMANDS = {
  preview: "merman.openPreview",
  exportSvg: "merman.exportSvg",
  exportPng: "merman.exportPng",
  copySvg: "merman.copySvg",
  copyPng: "merman.copyPng",
} as const;

export type MermaidSourceActionCommand =
  (typeof SOURCE_ACTION_COMMANDS)[keyof typeof SOURCE_ACTION_COMMANDS];

export interface MermaidSourceCommandTarget {
  uri: vscode.Uri;
  sourceId?: string;
}

export type MermaidSourceCommandArgument =
  | vscode.Uri
  | MermaidSourceCommandTarget;

export interface MermaidSourceCodeLensSpec {
  line: number;
  sourceId: string;
  title: string;
  command: MermaidSourceActionCommand;
}

export interface MermaidSourceCodeLensOptions {
  includeCopyPng?: boolean;
}

interface SourceActionDescriptor {
  title: string;
  command: MermaidSourceActionCommand;
  requiresCopyPng?: boolean;
}

const SOURCE_ACTIONS: readonly SourceActionDescriptor[] = [
  { title: "Preview", command: SOURCE_ACTION_COMMANDS.preview },
  { title: "Export SVG", command: SOURCE_ACTION_COMMANDS.exportSvg },
  { title: "Export PNG", command: SOURCE_ACTION_COMMANDS.exportPng },
  { title: "Copy SVG", command: SOURCE_ACTION_COMMANDS.copySvg },
  {
    title: "Copy PNG",
    command: SOURCE_ACTION_COMMANDS.copyPng,
    requiresCopyPng: true,
  },
];

export function buildMermaidSourceCodeLensSpecs(
  inputs: readonly Pick<PreviewInput, "sourceId" | "sourceRange">[],
  options: MermaidSourceCodeLensOptions = {},
): MermaidSourceCodeLensSpec[] {
  const includeCopyPng = options.includeCopyPng ?? true;
  return inputs.flatMap((input) =>
    SOURCE_ACTIONS.filter((action) => includeCopyPng || !action.requiresCopyPng).map(
      (action) => ({
        line: input.sourceRange.startLine,
        sourceId: input.sourceId,
        title: action.title,
        command: action.command,
      }),
    ),
  );
}

export function mermaidSourceCommandTarget(
  uri: vscode.Uri,
  sourceId?: string,
): MermaidSourceCommandTarget {
  return sourceId ? { uri, sourceId } : { uri };
}

export function isMermaidSourceCommandTarget(
  value: unknown,
): value is MermaidSourceCommandTarget {
  if (!value || typeof value !== "object" || !("uri" in value)) {
    return false;
  }
  const sourceId = (value as { sourceId?: unknown }).sourceId;
  return sourceId === undefined || typeof sourceId === "string";
}

export function mermaidSourceCommandUri(
  argument: MermaidSourceCommandArgument | undefined,
): vscode.Uri | undefined {
  if (!argument) {
    return undefined;
  }
  return isMermaidSourceCommandTarget(argument) ? argument.uri : argument;
}

export function mermaidSourceCommandSourceId(
  argument: MermaidSourceCommandArgument | undefined,
): string | undefined {
  return isMermaidSourceCommandTarget(argument) ? argument.sourceId : undefined;
}
