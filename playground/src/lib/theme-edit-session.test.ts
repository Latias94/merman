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
  endThemeEditGrouping,
  travelThemeHistory,
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

test("undo and redo preserve raw invalid drafts and invalidate imports and validation", () => {
  let session = editThemeDraft(beginThemeEditSession(1, { kind: "preset", presetId: "cyberpunk" }), recipe);
  session = acceptThemeValidation(session, themeValidationToken(session), validateThemeDocument(recipe, (raw) => raw));
  session = editThemeDraft(session, "{");
  const oldValidation = themeValidationToken(session);
  session = beginThemeImport(session);
  const oldImport = { ...themeValidationToken(session), generation: session.importGeneration };
  session = travelThemeHistory(session, "undo");
  assert.equal(session.rawJson, recipe);
  assert.equal(session.validation, "pending");
  assert.equal(session.importing, false);
  assert.equal(completeThemeImport(session, oldImport, "late import"), session);
  assert.equal(rejectThemeValidation(session, oldValidation, "late rejection"), session);
  session = travelThemeHistory(session, "redo");
  assert.equal(session.rawJson, "{");
  assert.deepEqual(session.candidate, { kind: "custom", recipeJson: recipe });
  session = travelThemeHistory(travelThemeHistory(session, "undo"), "undo");
  assert.deepEqual(session.candidate, { kind: "preset", presetId: "cyberpunk" });
  assert.equal(session.rawJson, null);
});

test("continuous input groups, explicit boundaries split, and new edits discard redo", () => {
  let session = beginThemeEditSession(1, { kind: "none" });
  session = editThemeDraft(session, "a", { key: "json", at: 0 });
  session = editThemeDraft(session, "ab", { key: "json", at: 100 });
  session = editThemeDraft(session, "abc", { key: "json", at: 200 });
  assert.equal(session.past.length, 1);
  session = endThemeEditGrouping(session);
  session = editThemeDraft(session, "abcd", { key: "json", at: 210 });
  assert.equal(session.past.length, 2);
  session = travelThemeHistory(session, "undo");
  assert.equal(session.rawJson, "abc");
  session = editThemeDraft(session, "new branch");
  assert.equal(session.future.length, 0);
  assert.equal(travelThemeHistory(session, "redo"), session);
  session = editThemeDraft(session, "slider first", { key: "visual", at: 300 });
  const count = session.past.length;
  session = editThemeDraft(session, "slider final", { key: "visual", at: 5000 });
  assert.equal(session.past.length, count);
});

test("Apply keeps history but undo does not move the applied baseline", () => {
  let session = editThemeDraft(beginThemeEditSession(1, { kind: "none" }), recipe);
  session = acceptThemeValidation(session, themeValidationToken(session), validateThemeDocument(recipe, (raw) => raw));
  session = advanceThemeBaseline(session);
  const baseline = session.baseline;
  session = travelThemeHistory(session, "undo");
  assert.equal(isThemeDraftChanged(session), true);
  assert.deepEqual(session.baseline, baseline);
  session = travelThemeHistory(session, "redo");
  assert.equal(isThemeDraftChanged(session), false);
});

test("history is bounded by count and retained text size", () => {
  let session = beginThemeEditSession(1, { kind: "none" });
  for (let i = 0; i < 100; i++) session = editThemeDraft(session, String(i));
  assert.equal(session.past.length, 50);
  for (let i = 0; i < 20; i++) session = editThemeDraft(session, String(i).padEnd(256 * 1024, " "));
  assert.ok(session.past.length < 20);
});
