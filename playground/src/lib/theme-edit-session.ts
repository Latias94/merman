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
}

export function beginThemeEditSession(id: number, baseline: ThemeCandidate): ThemeEditSession {
  const rawJson = baseline.kind === "custom" ? baseline.recipeJson : null;
  return {
    id, revision: 0, importGeneration: 0, importing: false,
    baseline, baselineRawJson: rawJson, rawJson, candidate: baseline,
    document: null, validation: baseline.kind === "custom" ? "pending" : "valid", error: null,
  };
}

export function themeValidationToken(session: ThemeEditSession): ThemeValidationToken {
  return { sessionId: session.id, revision: session.revision };
}

function isCurrentValidation(session: ThemeEditSession, token: ThemeValidationToken): boolean {
  return session.id === token.sessionId && session.revision === token.revision;
}

export function editThemeDraft(session: ThemeEditSession, rawJson: string): ThemeEditSession {
  return {
    ...session, revision: session.revision + 1, importGeneration: session.importGeneration + 1,
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
    ...session, revision: session.revision + 1, importGeneration: session.importGeneration + 1,
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
  };
}
