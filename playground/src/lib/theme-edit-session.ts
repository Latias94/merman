import type { DiagramFont } from "./diagram-font.ts";
import type { MermaidThemeSelection } from "./mermaid-theme-name.ts";
import { mergeRecommendedMermaidConfig, type MermaidConfigObject } from "./mermaid-config.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";

export type ThemeCandidate =
  | { readonly kind: "none" }
  | { readonly kind: "preset"; readonly presetId: string }
  | { readonly kind: "custom"; readonly recipeJson: string };

export interface ThemeValidationToken {
  readonly sessionId: number;
  readonly revision: number;
}

export interface ThemeImportToken extends ThemeValidationToken {
  readonly generation: number;
}

/** Only panel-owned choices are drafted; source and unrelated configuration stay live. */
export interface ThemeSettingsDraft {
  readonly diagramTheme?: MermaidThemeSelection;
  readonly diagramFont?: DiagramFont;
  readonly config?: Readonly<MermaidConfigObject>;
}

export interface ThemeEditSession {
  readonly id: number;
  readonly revision: number;
  readonly importGeneration: number;
  readonly importing: boolean;
  readonly baseline: ThemeCandidate;
  readonly baselineRawJson: string | null;
  readonly rawJson: string | null;
  readonly candidate: ThemeCandidate;
  readonly lastValidCandidate: ThemeCandidate | null;
  readonly settings: ThemeSettingsDraft;
  readonly validation: "valid" | "pending" | "invalid";
  readonly error: string | null;
  readonly applyError: string | null;
}

export function beginThemeEditSession(id: number, baseline: ThemeCandidate): ThemeEditSession {
  const rawJson = baseline.kind === "custom" ? baseline.recipeJson : null;
  return {
    id, revision: 0, importGeneration: 0, importing: false,
    baseline, baselineRawJson: rawJson, rawJson, candidate: baseline,
    lastValidCandidate: baseline.kind === "custom" ? null : baseline, settings: {},
    validation: baseline.kind === "custom" ? "pending" : "valid", error: null, applyError: null,
  };
}

export function themeValidationToken(session: ThemeEditSession): ThemeValidationToken {
  return { sessionId: session.id, revision: session.revision };
}

function isCurrentValidation(session: ThemeEditSession, token: ThemeValidationToken): boolean {
  return session.id === token.sessionId && session.revision === token.revision;
}

export function acceptThemeValidation(
  session: ThemeEditSession,
  token: ThemeValidationToken,
  validatedRecipeJson: string,
): ThemeEditSession {
  if (!isCurrentValidation(session, token) || session.rawJson === null) return session;
  const recipeJson = session.rawJson === session.baselineRawJson && session.baseline.kind === "custom"
    ? session.baseline.recipeJson
    : validatedRecipeJson;
  const candidate = { kind: "custom", recipeJson } as const;
  return {
    ...session, candidate, lastValidCandidate: candidate,
    validation: "valid", error: null,
  };
}

export function rejectThemeValidation(
  session: ThemeEditSession,
  token: ThemeValidationToken,
  error: string,
): ThemeEditSession {
  if (!isCurrentValidation(session, token) || session.rawJson === null) return session;
  return { ...session, validation: "invalid", error };
}

export function selectThemeCandidate(
  session: ThemeEditSession,
  candidate: Exclude<ThemeCandidate, { kind: "custom" }>,
): ThemeEditSession {
  return {
    ...session, revision: session.revision + 1, importGeneration: session.importGeneration + 1,
    importing: false, candidate, lastValidCandidate: candidate, rawJson: null, validation: "valid", error: null,
  };
}

export function beginThemeImport(session: ThemeEditSession): ThemeEditSession {
  return { ...session, importGeneration: session.importGeneration + 1, importing: true, error: null };
}

function isCurrentImport(session: ThemeEditSession, token: ThemeImportToken): boolean {
  return isCurrentValidation(session, token) && session.importGeneration === token.generation && session.importing;
}

export function completeThemeImport(
  session: ThemeEditSession,
  token: ThemeImportToken,
  rawJson: string,
): ThemeEditSession {
  if (!isCurrentImport(session, token)) return session;
  return {
    ...session, revision: session.revision + 1, importGeneration: session.importGeneration + 1,
    importing: false, rawJson, validation: "pending", error: null,
  };
}

export function failThemeImport(
  session: ThemeEditSession,
  token: ThemeImportToken,
  error: string,
): ThemeEditSession {
  return isCurrentImport(session, token) ? { ...session, importing: false, error } : session;
}

export function restoreValidThemeDraft(session: ThemeEditSession): ThemeEditSession {
  const candidate = session.lastValidCandidate;
  if (candidate === null || session.importing) return session;
  return {
    ...session, candidate, rawJson: candidate.kind === "custom" ? candidate.recipeJson : null,
    revision: session.revision + 1, importGeneration: session.importGeneration + 1,
    validation: "valid", error: null,
  };
}

export function themePreviewWorkspace(
  workspace: WorkspaceSnapshot,
  session: ThemeEditSession,
): WorkspaceSnapshot {
  const { config, ...settings } = session.settings;
  return {
    ...workspace, ...settings, ...themeCandidateFields(session.candidate),
    mermaidConfig: config ? mergeRecommendedMermaidConfig(workspace.mermaidConfig, config) : workspace.mermaidConfig,
  };
}

export function themeCandidateFields(candidate: ThemeCandidate): {
  themePresetId: string | null;
  themeRecipeJson: string | null;
} {
  return {
    themePresetId: candidate.kind === "preset" ? candidate.presetId : null,
    themeRecipeJson: candidate.kind === "custom" ? candidate.recipeJson : null,
  };
}

export function themeCandidateFromFields(fields: {
  readonly themePresetId: string | null;
  readonly themeRecipeJson: string | null;
}): ThemeCandidate {
  if (fields.themeRecipeJson !== null) return { kind: "custom", recipeJson: fields.themeRecipeJson };
  return fields.themePresetId === null ? { kind: "none" } : { kind: "preset", presetId: fields.themePresetId };
}

export function isThemeDraftChanged(session: ThemeEditSession | null): boolean {
  if (!session) return false;
  const candidate = themeCandidateFields(session.candidate);
  const baseline = themeCandidateFields(session.baseline);
  return session.rawJson !== session.baselineRawJson ||
    candidate.themePresetId !== baseline.themePresetId || candidate.themeRecipeJson !== baseline.themeRecipeJson ||
    Object.keys(session.settings).length > 0;
}

export function advanceThemeBaseline(session: ThemeEditSession): ThemeEditSession {
  if (session.validation !== "valid" || session.importing) return session;
  return {
    ...session, baseline: session.candidate, baselineRawJson: session.rawJson,
    settings: {},
    importGeneration: session.importGeneration + 1,
  };
}
