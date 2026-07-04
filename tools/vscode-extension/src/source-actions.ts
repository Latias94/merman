import type * as vscode from "vscode";

import {
  previewSourceIdentity,
  type PreviewInput,
  type PreviewSourceIdentity,
} from "./preview-source.js";

export const SOURCE_ACTION_COMMANDS = {
  preview: "merman.openPreview",
  exportCopy: "merman.sourceActions",
  exportSvg: "merman.exportSvg",
  exportPng: "merman.exportPng",
  copySvg: "merman.copySvg",
  copyPng: "merman.copyPng",
} as const;

export const SOURCE_ACTIONS_ENABLED_SETTING = "merman.sourceActions.enabled";

export type MermaidSourceActionCommand =
  (typeof SOURCE_ACTION_COMMANDS)[keyof typeof SOURCE_ACTION_COMMANDS];

export interface MermaidSourceCommandTarget {
  uri: vscode.Uri;
  sourceId?: string;
  sourceIdentity?: PreviewSourceIdentity;
}

export type MermaidSourceCommandArgument =
  | vscode.Uri
  | MermaidSourceCommandTarget;

export interface MermaidSourceCodeLensSpec {
  line: number;
  sourceId: string;
  sourceIdentity?: PreviewSourceIdentity;
  title: string;
  command: MermaidSourceActionCommand;
}

export interface MermaidSourceCodeLensOptions {
  enabled?: boolean;
}

export interface MermaidSourceExportCopyActionOptions {
  includeCopyPng?: boolean;
}

export interface SourceActionDescriptor {
  title: string;
  command: MermaidSourceActionCommand;
  requiresCopyPng?: boolean;
}

const SOURCE_ACTIONS: readonly SourceActionDescriptor[] = [
  { title: "Preview", command: SOURCE_ACTION_COMMANDS.preview },
  { title: "Export / Copy", command: SOURCE_ACTION_COMMANDS.exportCopy },
];

export const SOURCE_EXPORT_COPY_ACTIONS: readonly SourceActionDescriptor[] = [
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
  inputs: readonly SourceActionInput[],
  options: MermaidSourceCodeLensOptions = {},
): MermaidSourceCodeLensSpec[] {
  if (options.enabled === false) {
    return [];
  }

  return inputs.flatMap((input) =>
    SOURCE_ACTIONS.map((action) => ({
      line: input.sourceRange.startLine,
      sourceId: input.sourceId,
      sourceIdentity: sourceActionIdentity(input),
      title: action.title,
      command: action.command,
    })),
  );
}

export function mermaidSourceExportCopyActions(
  options: MermaidSourceExportCopyActionOptions = {},
): readonly SourceActionDescriptor[] {
  const includeCopyPng = options.includeCopyPng ?? true;
  return SOURCE_EXPORT_COPY_ACTIONS.filter((action) => includeCopyPng || !action.requiresCopyPng);
}

export function shouldRefreshSourceActionCodeLens(
  affectsConfiguration: (section: string) => boolean,
): boolean {
  return affectsConfiguration(SOURCE_ACTIONS_ENABLED_SETTING);
}

export function mermaidSourceCommandTarget(
  uri: vscode.Uri,
  source?: string | PreviewSourceIdentity,
): MermaidSourceCommandTarget {
  if (!source) {
    return { uri };
  }
  if (typeof source === "string") {
    return { uri, sourceId: source };
  }
  return { uri, sourceId: source.sourceId, sourceIdentity: source };
}

export function isMermaidSourceCommandTarget(
  value: unknown,
): value is MermaidSourceCommandTarget {
  if (!value || typeof value !== "object" || !("uri" in value)) {
    return false;
  }
  const sourceId = (value as { sourceId?: unknown }).sourceId;
  const sourceIdentity = (value as { sourceIdentity?: unknown }).sourceIdentity;
  return (
    (sourceId === undefined || typeof sourceId === "string") &&
    (sourceIdentity === undefined || isPreviewSourceIdentity(sourceIdentity))
  );
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

export function mermaidSourceCommandIdentity(
  argument: MermaidSourceCommandArgument | undefined,
): PreviewSourceIdentity | undefined {
  return isMermaidSourceCommandTarget(argument) ? argument.sourceIdentity : undefined;
}

type SourceActionInput =
  Pick<PreviewInput, "sourceId" | "sourceRange"> &
  Partial<Pick<PreviewInput, "kind" | "source">>;

function sourceActionIdentity(input: SourceActionInput): PreviewSourceIdentity | undefined {
  if (!input.kind || typeof input.source !== "string") {
    return undefined;
  }
  return previewSourceIdentity(input as PreviewInput);
}

function isPreviewSourceIdentity(value: unknown): value is PreviewSourceIdentity {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<PreviewSourceIdentity>;
  return (
    typeof candidate.sourceId === "string" &&
    typeof candidate.sourceHash === "string" &&
    (candidate.kind === "mermaid-file" || candidate.kind === "markdown-fence") &&
    !!candidate.sourceRange &&
    typeof candidate.sourceRange.startLine === "number" &&
    typeof candidate.sourceRange.endLine === "number"
  );
}
