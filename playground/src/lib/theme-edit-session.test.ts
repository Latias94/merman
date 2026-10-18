import assert from "node:assert/strict";
import test from "node:test";

import { validateThemeDocument } from "./theme-document.ts";
import {
  acceptThemeValidation,
  advanceThemeBaseline,
  beginThemeEditSession,
  beginThemeImport,
  completeThemeImport,
  editThemeDraft,
  isThemeDraftChanged,
  rejectThemeValidation,
  selectThemeCandidate,
  themeValidationToken,
} from "./theme-edit-session.ts";

const recipe = '{"schema_version":1,"kind":"definition","definition":{"authoring_schema_version":1,"expansion_version":1,"tokens":{"text":"#123456"}}}';

test("opening an applied preset or None keeps its candidate and stays clean", () => {
  for (const candidate of [{ kind: "none" } as const, { kind: "preset", presetId: "cyberpunk" } as const]) {
    const session = beginThemeEditSession(1, candidate);
    assert.deepEqual(session.candidate, candidate);
    assert.equal(session.rawJson, null);
    assert.equal(isThemeDraftChanged(session), false);
  }
});

test("invalid raw text stays dirty and retains the last valid candidate", () => {
  let session = beginThemeEditSession(1, { kind: "preset", presetId: "cyberpunk" });
  session = editThemeDraft(session, recipe);
  session = acceptThemeValidation(session, themeValidationToken(session), validateThemeDocument(recipe, (raw) => raw));
  const validCandidate = session.candidate;
  session = editThemeDraft(session, "{");
  session = rejectThemeValidation(session, themeValidationToken(session), "Invalid JSON");
  assert.equal(session.rawJson, "{");
  assert.equal(session.validation, "invalid");
  assert.equal(session.document, null);
  assert.deepEqual(session.candidate, validCandidate);
  assert.equal(isThemeDraftChanged(session), true);
  assert.equal(advanceThemeBaseline(session), session);
});

test("stale validations cannot replace a later revision or another session", () => {
  let session = editThemeDraft(beginThemeEditSession(1, { kind: "none" }), recipe);
  const stale = themeValidationToken(session);
  session = editThemeDraft(session, "{");
  const validated = validateThemeDocument(recipe, (raw) => raw);
  assert.equal(acceptThemeValidation(session, stale, validated), session);
  assert.equal(rejectThemeValidation(session, stale, "late error"), session);
  const replacement = editThemeDraft(beginThemeEditSession(2, { kind: "none" }), recipe);
  assert.equal(acceptThemeValidation(replacement, stale, validated), replacement);
});

test("superseded imports do not overwrite edits or a later import", () => {
  let session = beginThemeEditSession(1, { kind: "none" });
  session = beginThemeImport(session);
  const first = { ...themeValidationToken(session), generation: session.importGeneration };
  session = beginThemeImport(session);
  const second = { ...themeValidationToken(session), generation: session.importGeneration };
  assert.equal(completeThemeImport(session, first, recipe), session);
  session = completeThemeImport(session, second, recipe);
  assert.equal(session.rawJson, recipe);
  assert.equal(session.importing, false);
  session = beginThemeImport(session);
  const pending = { ...themeValidationToken(session), generation: session.importGeneration };
  session = selectThemeCandidate(session, { kind: "preset", presetId: "spotless" });
  assert.equal(completeThemeImport(session, pending, recipe), session);
});

test("Apply advances the baseline while retaining original formatting", () => {
  const raw = `  ${recipe}\n`;
  let session = editThemeDraft(beginThemeEditSession(1, { kind: "none" }), raw);
  session = acceptThemeValidation(session, themeValidationToken(session), validateThemeDocument(raw, (input) => JSON.stringify(JSON.parse(input))));
  assert.equal(isThemeDraftChanged(session), true);
  session = advanceThemeBaseline(session);
  assert.equal(session.rawJson, raw);
  assert.equal(isThemeDraftChanged(session), false);
  assert.deepEqual(session.baseline, { kind: "custom", recipeJson: recipe });
});

test("validating an unchanged applied custom recipe keeps its existing bytes clean", () => {
  const raw = `  ${recipe}\n`;
  let session = beginThemeEditSession(1, { kind: "custom", recipeJson: raw });
  session = acceptThemeValidation(session, themeValidationToken(session), validateThemeDocument(raw, (input) => JSON.stringify(JSON.parse(input))));
  assert.equal(session.candidate.kind, "custom");
  assert.equal(isThemeDraftChanged(session), false);
  assert.deepEqual(session.candidate, session.baseline);
});
