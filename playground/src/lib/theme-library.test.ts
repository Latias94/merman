import assert from "node:assert/strict";
import test from "node:test";
import {
  clearThemeLibraryFilters,
  createThemeLibraryFilters,
  filterThemeLibrary,
  themeDesignTreatment,
  themeSampleFor,
} from "./theme-library.ts";
import { useThemeLibraryStore } from "./theme-library-state.ts";

const themes = [
  { id: "paper", name: "Paper", appearance: "light" as const, families: ["flowchart", "er"], styles: ["print" as const] },
  { id: "night", name: "Night", appearance: "dark" as const, families: ["flowchart", "state"], styles: ["technical" as const] },
  { id: "reading", name: "Reading", appearance: "light" as const, families: ["er", "state"], styles: ["reading" as const],
    appearances: [{ appearance: "light" as const }, { appearance: "dark" as const }] },
];

test("the detected diagram becomes an explicit filter, including a family without a designed sample", () => {
  assert.equal(createThemeLibraryFilters("er").familyId, "er");
  assert.equal(createThemeLibraryFilters("gantt").familyId, "gantt");
  assert.equal(createThemeLibraryFilters(null).familyId, null);
  assert.deepEqual(filterThemeLibrary(themes, createThemeLibraryFilters("er")).map((theme) => theme.id), ["paper", "reading"]);
  assert.deepEqual(filterThemeLibrary(themes, createThemeLibraryFilters("gantt")), []);
});

test("removing a family filter is an explicit choice and does not change the search", () => {
  const filters = { ...createThemeLibraryFilters("er"), query: "night" };
  assert.deepEqual(filterThemeLibrary(themes, { ...filters, familyId: null }).map((theme) => theme.id), ["night"]);
  assert.equal(filters.query, "night");
});

test("appearance, style, family and localized search intersect", () => {
  const filters = { ...createThemeLibraryFilters("er"), appearance: "dark" as const };
  assert.deepEqual(filterThemeLibrary(themes, filters).map((theme) => theme.id), ["reading"]);
  assert.deepEqual(filterThemeLibrary(themes, { ...filters, appearance: "paired" }).map((theme) => theme.id), ["reading"]);
  assert.deepEqual(filterThemeLibrary(themes, { ...filters, style: "print" }), []);
  assert.deepEqual(filterThemeLibrary(themes, { ...filters, query: "阅读" }, (theme) => theme.id === "reading" ? "舒适阅读" : "").map((theme) => theme.id), ["reading"]);
});

test("clearing structured filters preserves free text", () => {
  const filters = { ...createThemeLibraryFilters("er"), query: "paper", appearance: "light" as const, style: "print" as const };
  const cleared = clearThemeLibraryFilters(filters);
  assert.deepEqual(cleared, { query: "paper", familyId: null, appearance: "all", style: "all" });
  assert.deepEqual(filterThemeLibrary(themes, cleared).map((theme) => theme.id), ["paper"]);
});

test("coverage declarations distinguish dedicated, reviewed base and unknown families", () => {
  assert.equal(themeDesignTreatment(themes[0], "er"), "dedicated");
  assert.equal(themeDesignTreatment({ ...themes[0], sharedFamilies: ["gantt"] }, "gantt"), "base_only");
  assert.equal(themeDesignTreatment(themes[0], "gantt"), "unreviewed");
  assert.equal(themeDesignTreatment(themes[0], null), "unreviewed");
});

test("representative previews yield to supported filters and never select an unreviewed family", () => {
  const metro = { families: ["flowchart", "sequence", "gitGraph"] as const, featuredSample: "gitGraph" as const };
  assert.equal(themeSampleFor(metro, null), "gitGraph");
  assert.equal(themeSampleFor(metro, "sequence"), "sequence");
  assert.equal(themeSampleFor(metro, "gantt"), "gitGraph");
  assert.equal(themeSampleFor(metro, null, "flowchart"), "flowchart");
  assert.equal(themeSampleFor(metro, null, "radar"), "gitGraph");
  assert.equal(themeSampleFor({ families: ["flowchart"], featuredSample: "radar" }, null), "flowchart");
});

test("late detection initializes only an untouched browsing session", () => {
  useThemeLibraryStore.setState({ initialized: false, filters: createThemeLibraryFilters(null) });
  const store = useThemeLibraryStore.getState();
  store.initialize(null);
  assert.equal(useThemeLibraryStore.getState().initialized, false);
  store.initialize("er");
  assert.equal(useThemeLibraryStore.getState().filters.familyId, "er");
  store.changeFilters({ ...createThemeLibraryFilters(null), query: "paper" });
  store.initialize("mindmap");
  assert.equal(useThemeLibraryStore.getState().filters.familyId, null);
  assert.equal(useThemeLibraryStore.getState().filters.query, "paper");
});
