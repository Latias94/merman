import type { ThemeRecipeV1 } from "@mermanjs/web";
import type { ValidatedThemeDocument } from "./theme-document.ts";

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

interface ThemeDraftSnapshot {
  readonly rawJson: string | null;
  readonly candidate: ThemeCandidate;
}

export interface ThemeEditGrouping {
  readonly key: "json" | "visual";
  readonly at: number;
}

const HISTORY_LIMIT = 50;
const HISTORY_BYTES = 4 * 1024 * 1024;
const HISTORY_GROUP_MS = 800;

export interface ThemeEditSession {
  readonly id: number;
  readonly revision: number;
  readonly importGeneration: number;
  readonly importing: boolean;
  readonly baseline: ThemeCandidate;
  readonly baselineRawJson: string | null;
  readonly rawJson: string | null;
  readonly candidate: ThemeCandidate;
  readonly document: ThemeRecipeV1 | null;
  readonly validation: "valid" | "pending" | "invalid";
  readonly error: string | null;
  readonly past: readonly ThemeDraftSnapshot[];
  readonly future: readonly ThemeDraftSnapshot[];
  readonly editGrouping: ThemeEditGrouping | null;
}

export function beginThemeEditSession(id: number, baseline: ThemeCandidate): ThemeEditSession {
  const rawJson = baseline.kind === "custom" ? baseline.recipeJson : null;
  return {
    id, revision: 0, importGeneration: 0, importing: false,
    baseline, baselineRawJson: rawJson, rawJson, candidate: baseline,
    document: null, validation: baseline.kind === "custom" ? "pending" : "valid", error: null,
    past: [], future: [], editGrouping: null,
  };
}

export function themeValidationToken(session: ThemeEditSession): ThemeValidationToken {
  return { sessionId: session.id, revision: session.revision };
}

function isCurrentValidation(session: ThemeEditSession, token: ThemeValidationToken): boolean {
  return session.id === token.sessionId && session.revision === token.revision;
}

export function editThemeDraft(session: ThemeEditSession, rawJson: string, grouping?: ThemeEditGrouping): ThemeEditSession {
  if (rawJson === session.rawJson && !session.importing) return session;
  return {
    ...recordThemeEdit(session, grouping), revision: session.revision + 1, importGeneration: session.importGeneration + 1,
    importing: false, rawJson, document: null, validation: "pending", error: null,
  };
}

export function acceptThemeValidation(
  session: ThemeEditSession,
  token: ThemeValidationToken,
  validated: ValidatedThemeDocument,
): ThemeEditSession {
  if (!isCurrentValidation(session, token) || session.rawJson === null) return session;
  const recipeJson = session.rawJson === session.baselineRawJson && session.baseline.kind === "custom"
    ? session.baseline.recipeJson
    : validated.recipeJson;
  return {
    ...session, candidate: { kind: "custom", recipeJson },
    document: validated.document, validation: "valid", error: null,
  };
}

export function rejectThemeValidation(
  session: ThemeEditSession,
  token: ThemeValidationToken,
  error: string,
): ThemeEditSession {
  if (!isCurrentValidation(session, token) || session.rawJson === null) return session;
  return { ...session, document: null, validation: "invalid", error };
}

export function selectThemeCandidate(
  session: ThemeEditSession,
  candidate: Exclude<ThemeCandidate, { kind: "custom" }>,
): ThemeEditSession {
  return {
    ...recordThemeEdit(session), revision: session.revision + 1, importGeneration: session.importGeneration + 1,
    importing: false, candidate, rawJson: null, document: null, validation: "valid", error: null,
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
  return isCurrentImport(session, token) ? editThemeDraft(session, rawJson) : session;
}

export function failThemeImport(
  session: ThemeEditSession,
  token: ThemeImportToken,
  error: string,
): ThemeEditSession {
  return isCurrentImport(session, token) ? { ...session, importing: false, error } : session;
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
    candidate.themePresetId !== baseline.themePresetId || candidate.themeRecipeJson !== baseline.themeRecipeJson;
}

export function advanceThemeBaseline(session: ThemeEditSession): ThemeEditSession {
  if (session.validation !== "valid" || session.importing) return session;
  return {
    ...session, baseline: session.candidate, baselineRawJson: session.rawJson,
    importGeneration: session.importGeneration + 1,
    editGrouping: null,
  };
}

function snapshot(session: ThemeEditSession): ThemeDraftSnapshot {
  return { rawJson: session.rawJson, candidate: session.candidate };
}

function boundedHistory(entries: readonly ThemeDraftSnapshot[]): readonly ThemeDraftSnapshot[] {
  let bytes = 0;
  let start = entries.length;
  while (start > 0 && entries.length - start < HISTORY_LIMIT) {
    const entry = entries[start - 1];
    bytes += 2 * ((entry.rawJson?.length ?? 0) + (entry.candidate.kind === "custom" ? entry.candidate.recipeJson.length : 0));
    if (bytes > HISTORY_BYTES) break;
    start--;
  }
  return entries.slice(start);
}

function recordThemeEdit(session: ThemeEditSession, grouping?: ThemeEditGrouping): ThemeEditSession {
  const previous = session.editGrouping;
  const coalesce = grouping && previous && grouping.key === previous.key &&
    grouping.at >= previous.at && (grouping.key === "visual" || grouping.at - previous.at < HISTORY_GROUP_MS);
  return {
    ...session,
    past: coalesce ? session.past : boundedHistory([...session.past, snapshot(session)]),
    future: [],
    editGrouping: grouping ?? null,
  };
}

export function endThemeEditGrouping(session: ThemeEditSession): ThemeEditSession {
  return session.editGrouping ? { ...session, editGrouping: null } : session;
}

export function travelThemeHistory(session: ThemeEditSession, direction: "undo" | "redo"): ThemeEditSession {
  const source = direction === "undo" ? session.past : session.future;
  const restored = source.at(-1);
  if (!restored) return session;
  return {
    ...session, ...restored,
    revision: session.revision + 1,
    importGeneration: session.importGeneration + 1,
    importing: false, document: null, error: null,
    validation: restored.rawJson === null ? "valid" : "pending",
    editGrouping: null,
    past: direction === "undo" ? source.slice(0, -1) : boundedHistory([...session.past, snapshot(session)]),
    future: direction === "redo" ? source.slice(0, -1) : boundedHistory([...session.future, snapshot(session)]),
  };
}
