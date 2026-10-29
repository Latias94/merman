import assert from "node:assert/strict";
import test from "node:test";

import {
  acceptThemeValidation,
  advanceThemeBaseline,
  beginThemeEditSession,
  beginThemeImport,
  completeThemeImport,
  failThemeImport,
  isThemeDraftChanged,
  rejectThemeValidation,
  restoreValidThemeDraft,
  selectThemeCandidate,
  themeValidationToken,
  type ThemeEditSession,
} from "./theme-edit-session.ts";

const recipe = '{"schema_version":1,"kind":"design","design":{"tokens":{"text":{"type":"color","value":"#123456"}}}}';

function importDraft(session: ThemeEditSession, rawJson: string): ThemeEditSession {
  const importing = beginThemeImport(session);
  return completeThemeImport(importing, {
    ...themeValidationToken(importing), generation: importing.importGeneration,
  }, rawJson);
}

test("opening an applied preset or None keeps only transactional theme state", () => {
  for (const candidate of [{ kind: "none" } as const, { kind: "preset", presetId: "cyberpunk" } as const]) {
    const session = beginThemeEditSession(1, candidate);
    assert.deepEqual(session, {
      id: 1, revision: 0, importGeneration: 0, importing: false,
      baseline: candidate, baselineRawJson: null, rawJson: null, candidate,
      lastValidCandidate: candidate, settings: {},
      validation: "valid", error: null, applyError: null,
    });
    assert.equal(isThemeDraftChanged(session), false);
  }
});

test("imported text stays pending, then invalid text retains the last valid candidate", () => {
  let session = importDraft(beginThemeEditSession(1, { kind: "preset", presetId: "cyberpunk" }), recipe);
  assert.equal(session.revision, 1);
  assert.equal(session.validation, "pending");
  assert.deepEqual(session.candidate, { kind: "preset", presetId: "cyberpunk" });
  assert.equal(advanceThemeBaseline(session), session);
  session = acceptThemeValidation(session, themeValidationToken(session), recipe);
  const validCandidate = session.candidate;
  session = importDraft(session, "{");
  session = rejectThemeValidation(session, themeValidationToken(session), "Invalid JSON");
  assert.equal(session.rawJson, "{");
  assert.equal(session.validation, "invalid");
  assert.deepEqual(session.candidate, validCandidate);
  assert.equal(isThemeDraftChanged(session), true);
  assert.equal(advanceThemeBaseline(session), session);
});

test("stale validations cannot replace a later import, selection or another session", () => {
  let session = importDraft(beginThemeEditSession(1, { kind: "none" }), recipe);
  const stale = themeValidationToken(session);
  session = importDraft(session, "{");
  assert.equal(acceptThemeValidation(session, stale, recipe), session);
  assert.equal(rejectThemeValidation(session, stale, "late error"), session);
  const selected = selectThemeCandidate(session, { kind: "none" });
  assert.equal(acceptThemeValidation(selected, themeValidationToken(session), recipe), selected);
  const replacement = importDraft(beginThemeEditSession(2, { kind: "none" }), recipe);
  assert.equal(acceptThemeValidation(replacement, stale, recipe), replacement);
});

test("superseded imports cannot overwrite later imports, selection or another session", () => {
  let session = beginThemeImport(beginThemeEditSession(1, { kind: "none" }));
  const first = { ...themeValidationToken(session), generation: session.importGeneration };
  session = beginThemeImport(session);
  const second = { ...themeValidationToken(session), generation: session.importGeneration };
  assert.equal(completeThemeImport(session, first, recipe), session);
  assert.equal(failThemeImport(session, first, "late error"), session);
  session = completeThemeImport(session, second, recipe);
  assert.equal(session.rawJson, recipe);
  assert.equal(session.importing, false);
  session = beginThemeImport(session);
  const pending = { ...themeValidationToken(session), generation: session.importGeneration };
  assert.equal(advanceThemeBaseline(session), session);
  session = selectThemeCandidate(session, { kind: "preset", presetId: "spotless" });
  assert.equal(completeThemeImport(session, pending, recipe), session);
  const replacement = beginThemeImport(beginThemeEditSession(2, { kind: "none" }));
  assert.equal(completeThemeImport(replacement, pending, recipe), replacement);
});

test("each completed import requires validation even when its raw text is unchanged", () => {
  let session = importDraft(beginThemeEditSession(1, { kind: "none" }), recipe);
  session = acceptThemeValidation(session, themeValidationToken(session), recipe);
  const previous = session;
  session = importDraft(session, recipe);
  assert.equal(session.revision, previous.revision + 1);
  assert.equal(session.validation, "pending");
  assert.deepEqual(session.candidate, previous.candidate);
  assert.equal(acceptThemeValidation(session, themeValidationToken(previous), recipe), session);
});

test("a failed file read preserves the candidate and records an actionable error", () => {
  let session = beginThemeImport(beginThemeEditSession(1, { kind: "preset", presetId: "spotless" }));
  session = failThemeImport(session, {
    ...themeValidationToken(session), generation: session.importGeneration,
  }, "Cannot read theme file");
  assert.equal(session.importing, false);
  assert.equal(session.error, "Cannot read theme file");
  assert.equal(session.rawJson, null);
  assert.equal(session.validation, "valid");
  assert.deepEqual(session.candidate, { kind: "preset", presetId: "spotless" });
  assert.equal(isThemeDraftChanged(session), false);
});

test("Apply advances the baseline while retaining original import formatting", () => {
  const raw = `  ${recipe}\n`;
  let session = importDraft(beginThemeEditSession(1, { kind: "none" }), raw);
  session = acceptThemeValidation(session, themeValidationToken(session), recipe);
  assert.equal(isThemeDraftChanged(session), true);
  session = advanceThemeBaseline(session);
  assert.equal(session.rawJson, raw);
  assert.equal(isThemeDraftChanged(session), false);
  assert.deepEqual(session.baseline, { kind: "custom", recipeJson: recipe });
});

test("validating an unchanged applied custom recipe keeps its existing bytes clean", () => {
  const raw = `  ${recipe}\n`;
  let session = beginThemeEditSession(1, { kind: "custom", recipeJson: raw });
  assert.equal(session.validation, "pending");
  session = acceptThemeValidation(session, themeValidationToken(session), recipe);
  assert.equal(session.candidate.kind, "custom");
  assert.equal(isThemeDraftChanged(session), false);
  assert.deepEqual(session.candidate, session.baseline);
});

test("selecting a preset or None clears imported text and validation errors", () => {
  let session = importDraft(beginThemeEditSession(1, { kind: "none" }), "{");
  session = rejectThemeValidation(session, themeValidationToken(session), "Invalid JSON");
  session = selectThemeCandidate(session, { kind: "preset", presetId: "spotless" });
  assert.equal(session.rawJson, null);
  assert.equal(session.validation, "valid");
  assert.equal(session.error, null);
  assert.equal(isThemeDraftChanged(session), true);
  session = selectThemeCandidate(session, { kind: "none" });
  assert.equal(isThemeDraftChanged(session), false);
});

test("recovering a validated custom draft rejects stale work and retains appearance choices", () => {
  let session = importDraft(beginThemeEditSession(1, { kind: "none" }), recipe);
  session = acceptThemeValidation(session, themeValidationToken(session), recipe);
  session = { ...session, settings: { diagramFont: "inherit", config: { flowchart: { look: "classic" } } } };
  session = importDraft(session, "{");
  const stale = themeValidationToken(session);
  session = rejectThemeValidation(session, stale, "Invalid JSON");
  session = restoreValidThemeDraft(session);
  assert.equal(session.validation, "valid");
  assert.equal(session.rawJson, recipe);
  assert.deepEqual(session.candidate, { kind: "custom", recipeJson: recipe });
  assert.equal(session.settings.diagramFont, "inherit");
  assert.equal(acceptThemeValidation(session, stale, "late recipe"), session);
});

test("an unvalidated initial custom recipe cannot be certified by recovery", () => {
  let session = beginThemeEditSession(1, { kind: "custom", recipeJson: "{" });
  session = rejectThemeValidation(session, themeValidationToken(session), "Invalid JSON");
  assert.equal(session.lastValidCandidate, null);
  assert.equal(restoreValidThemeDraft(session), session);
});
