import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { JsxEmit, ModuleKind, transpileModule } from "typescript";

import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";
import { utf8ByteLength } from "./utf8.ts";
import { currentRecipeIdentity, explorationThemeChoice, galleryExplorations, importedThemeName, selectedExplorationTheme, selectedThemeKey } from "../components/theme-panel/theme-selection.ts";
import { themeCatalogFixture } from "./theme-appearance.test-fixture.ts";
import { themeChoiceFields } from "./diagram-appearance.ts";
import { decodeShareHash, encodeShareHash } from "./share.ts";
import type { DiagramThemeChoice } from "./diagram-appearance.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import type { ThemeCatalog } from "@mermanjs/web";
import type { ExplorationTheme } from "../themes/catalog.ts";
import type { ThemeCatalogChoice } from "../components/theme-panel/ThemeChoiceCards.tsx";
import type { MermanDomainFacade, MermanRuntimeState } from "../runtime/merman-core.ts";
import type { ThemeShowcaseProps } from "../components/theme-panel/ThemeShowcase.tsx";
import type { ThemeShowcaseDetail } from "../components/theme-panel/ThemeShowcaseDetail.tsx";

const explorations = [{ id: "atlas", name: "Atlas", recipeJson: "{\n  \"kind\": \"design\"\n}\n" }];
const curatedThemes: readonly ExplorationTheme[] = ["paper-cut", "carbon-copy", "pop-art"].map((id) => ({
  id, presetId: id, name: id, appearance: "light", colors: [], previews: null,
  families: ["flowchart"], styles: ["print"],
  recipeJson: readFileSync(new URL(`../../../crates/merman-render/assets/themes/${id}.theme.json`, import.meta.url), "utf8"),
}));

function curatedCatalog(available = true): ThemeCatalog {
  const catalog = themeCatalogFixture();
  const template = catalog.presets[0];
  return { ...catalog, presets: curatedThemes.map((theme) => ({
    ...template, id: theme.presetId!, display_name: theme.name,
    supported_appearances: ["light"], default_appearance: "light", available,
    availability_reason_ids: available ? [] : ["policy-disabled"],
    appearances: [{ ...template.appearances[0], concrete_preset_id: theme.presetId!, available }],
  })) };
}

test("an exploration keeps its name, provenance and original recipe", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: explorations[0].recipeJson,
    themeRecipeLabel: { name: "Atlas", source: "exploration" as const },
  };
  assert.equal(selectedThemeKey(workspace, explorations), "exploration:atlas");
  assert.deepEqual(currentRecipeIdentity(workspace, explorations, "Imported theme"), {
    name: "Atlas", source: "exploration", explorationId: "atlas", recipeJson: explorations[0].recipeJson,
  });
});

test("an imported copy of an exploration keeps the imported identity", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: explorations[0].recipeJson,
    themeRecipeLabel: { name: "Team review", source: "imported" as const },
  };
  assert.equal(selectedThemeKey(workspace, explorations), "recipe:current");
  assert.deepEqual(currentRecipeIdentity(workspace, explorations, "Imported theme"), {
    name: "Team review", source: "imported", explorationId: null, recipeJson: explorations[0].recipeJson,
  });
});

test("older recipes keep their JSON without inferring an exploration origin from matching content", () => {
  const exact = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: explorations[0].recipeJson };
  assert.equal(selectedThemeKey(exact, explorations), "recipe:current");
  assert.equal(currentRecipeIdentity(exact, explorations, "Imported theme")?.source, "imported");
  const authored = { ...exact, themeRecipeJson: JSON.stringify(JSON.parse(explorations[0].recipeJson)) };
  assert.equal(selectedThemeKey(authored, explorations), "recipe:current");
  assert.equal(currentRecipeIdentity(authored, explorations, "Imported theme")?.recipeJson, authored.themeRecipeJson);
});

test("unavailable presets retain their requested identity instead of selecting the base", () => {
  const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "future-preset" };
  assert.equal(selectedThemeKey(workspace, explorations), "preset:future-preset");
  assert.equal(selectedThemeKey(DEFAULT_WORKSPACE_SNAPSHOT, explorations), "mermaid:auto");
});

test("an exploration label survives recipe edits and a missing catalog entry", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: "{}",
    themeRecipeLabel: { name: "Atlas variant", source: "exploration" as const },
  };
  assert.deepEqual(currentRecipeIdentity(workspace, explorations, "Imported theme"), {
    name: "Atlas variant", source: "exploration", explorationId: null, recipeJson: "{}",
  });
});

test("file names provide readable imported names without losing the user's name", () => {
  assert.equal(importedThemeName("Team review.theme.json"), "Team review");
  assert.equal(importedThemeName("自定义.JSON"), "自定义");
  assert.equal(importedThemeName("nightfall.json"), "nightfall");
  assert.equal(importedThemeName(".json"), ".json");
  assert.equal(importedThemeName("  "), "Imported theme");
});

test("imported names fit the shared label envelope without cutting a Unicode character", () => {
  const name = importedThemeName(`${"🪴".repeat(80)}.theme.json`);
  assert.equal(utf8ByteLength(name), 256);
  assert.equal(name, "🪴".repeat(64));
  assert.equal(importedThemeName(`${"中".repeat(100)}.json`), "中".repeat(85));
  assert.equal(importedThemeName(`${"a".repeat(256)}.json`), "a".repeat(256));
});

test("a customized copy retains its original preset provenance when selected again", () => {
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: "{}",
    themeRecipeLabel: { name: "Editor", source: "imported" as const, derivedFrom: "preset:editor-dark" },
  };
  assert.equal(currentRecipeIdentity(workspace, [], "Imported")?.derivedFrom, "preset:editor-dark");
  assert.equal(selectedThemeKey(workspace, []), "recipe:current");
});

test("showcase uses registered curated presets and reflects their selection", () => {
  const catalog = curatedCatalog();
  for (const theme of curatedThemes) {
    const choice = explorationThemeChoice(theme, catalog, "follow");
    assert.deepEqual(choice, { kind: "preset", presetId: theme.presetId, appearance: "default" });
    const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, ...themeChoiceFields(choice!) };
    assert.equal(selectedThemeKey(workspace, curatedThemes, catalog), `preset:${theme.presetId}`);
    assert.equal(selectedExplorationTheme(workspace, curatedThemes, catalog), theme);
    assert.equal(workspace.themeRecipeJson, null);
  }
});

test("a runtime missing the preset ID uses the original exploration recipe", () => {
  const catalog = themeCatalogFixture();
  assert.deepEqual(galleryExplorations(curatedThemes, catalog), curatedThemes);
  for (const theme of curatedThemes) {
    const choice = explorationThemeChoice(theme, catalog, "follow");
    assert.deepEqual(choice, { kind: "recipe", recipeJson: theme.recipeJson, appearance: "default",
      label: { name: theme.name, source: "exploration" } });
    const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, ...themeChoiceFields(choice!) };
    assert.equal(selectedThemeKey(workspace, curatedThemes, catalog), `exploration:${theme.id}`);
    assert.equal(selectedExplorationTheme(workspace, curatedThemes, catalog), theme);
  }
});

test("unavailable presets and unknown catalogs cannot authorize a duplicate recipe fallback", () => {
  const catalog = curatedCatalog(false);
  assert.deepEqual(galleryExplorations(curatedThemes, catalog), []);
  for (const theme of curatedThemes) {
    assert.equal(explorationThemeChoice(theme, catalog, "default"), null);
    assert.equal(explorationThemeChoice(theme, null, "default"), null);
    const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: theme.presetId! };
    assert.equal(selectedThemeKey(workspace, curatedThemes, catalog), `preset:${theme.presetId}`);
    assert.equal(selectedExplorationTheme(workspace, curatedThemes, catalog), theme);
  }
});

test("old exploration snapshots retain their original bytes and identity after preset registration", () => {
  const catalog = curatedCatalog();
  for (const theme of curatedThemes) {
    const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: theme.recipeJson,
      themeRecipeLabel: { name: "Saved design", source: "exploration" as const } };
    assert.equal(selectedThemeKey(workspace, curatedThemes, catalog), "recipe:current");
    assert.equal(selectedExplorationTheme(workspace, curatedThemes, catalog), theme);
    assert.deepEqual(currentRecipeIdentity(workspace, galleryExplorations(curatedThemes, catalog), "Imported"), {
      name: "Saved design", source: "exploration", explorationId: null, recipeJson: theme.recipeJson,
    });
    assert.equal(workspace.themePresetId, null);
    assert.equal(workspace.themeRecipeJson, theme.recipeJson);
  }
});

test("imported, unlabelled and modified curated recipes do not select the canonical showcase design", () => {
  const catalog = curatedCatalog();
  for (const theme of curatedThemes) {
    for (const fields of [
      { themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: "Team copy", source: "imported" as const } },
      { themeRecipeJson: theme.recipeJson },
      { themeRecipeJson: `${theme.recipeJson}\n`, themeRecipeLabel: { name: "Edited copy", source: "exploration" as const } },
    ]) {
      const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, ...fields };
      assert.equal(selectedThemeKey(workspace, curatedThemes, catalog), "recipe:current");
      assert.equal(selectedExplorationTheme(workspace, curatedThemes, catalog), null);
      assert.equal(currentRecipeIdentity(workspace, curatedThemes, "Imported")?.recipeJson, fields.themeRecipeJson);
    }
  }
});

test("paired showcase recipes retain their appearance preference and concrete preset identity", () => {
  const catalog = themeCatalogFixture();
  const theme = { ...curatedThemes[0], presetId: "editor", appearances: [
    { appearance: "light" as const, colors: [] }, { appearance: "dark" as const, colors: [] },
  ] };
  assert.deepEqual(explorationThemeChoice(theme, catalog, "dark"), { kind: "preset", presetId: "editor", appearance: "dark" });
  assert.equal(selectedExplorationTheme({ ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "editor-dark" }, [theme], catalog), theme);
});

test("rendered gallery deduplicates curated choices and keeps old recipe snapshots selectable", async (context) => {
  const galleryUrl = new URL("../components/theme-panel/ThemeGallery.tsx", import.meta.url).href;
  const cardsUrl = new URL("../components/theme-panel/ThemeChoiceCards.tsx", import.meta.url).href;
  const moduleUrl = (source: string) => `data:text/javascript,${encodeURIComponent(source)}`;
  const catalogUrl = moduleUrl(`export const EXPLORATION_THEMES = ${JSON.stringify(curatedThemes)};`);
  const translationUrl = moduleUrl(`export function useTranslation() {
    return { t: (key, options) => options?.defaultValue ?? key, i18n: { resolvedLanguage: "en" } };
  }`);
  const observedCardsUrl = moduleUrl(`
    import { createElement } from ${JSON.stringify(import.meta.resolve("react"))};
    import { ThemeChoiceCards as Cards } from ${JSON.stringify(cardsUrl)};
    export const observed = [];
    export function ThemeChoiceCards(props) { observed.push(props); return createElement(Cards, props); }
  `);
  const launcherUrl = moduleUrl(`
    export const observedLaunchers = [];
    export function ThemeShowcaseLauncher(props) { observedLaunchers.push(props); return null; }
  `);
  const hooks = registerHooks({
    resolve(specifier, context, nextResolve) {
      if (specifier === "react-i18next") return { url: translationUrl, shortCircuit: true };
      if (context.parentURL === galleryUrl) {
        if (specifier === "@/src/themes/catalog") return { url: catalogUrl, shortCircuit: true };
        if (specifier === "./ThemeShowcaseLauncher") return { url: launcherUrl, shortCircuit: true };
        if (specifier === "./ThemeChoiceCards") return { url: observedCardsUrl, shortCircuit: true };
        if (specifier === "./theme-selection") return { url: new URL("../components/theme-panel/theme-selection.ts", import.meta.url).href, shortCircuit: true };
      }
      if (specifier.startsWith("@/")) return nextResolve(new URL(`../../${specifier.slice(2)}.ts`, import.meta.url).href, context);
      return nextResolve(specifier, context);
    },
    load(url, context, nextLoad) {
      if (url !== galleryUrl && url !== cardsUrl) return nextLoad(url, context);
      return { format: "module", source: transpileModule(readFileSync(new URL(url), "utf8"), {
        compilerOptions: { jsx: JsxEmit.ReactJSX, module: ModuleKind.ESNext },
      }).outputText, shortCircuit: true };
    },
  });
  let ThemeGallery: typeof import("../components/theme-panel/ThemeGallery.tsx").default;
  try { ({ default: ThemeGallery } = await import(galleryUrl)); }
  finally { hooks.deregister(); }
  const observed: { entries: readonly ThemeCatalogChoice[] }[] = (await import(observedCardsUrl)).observed;
  const observedLaunchers: { themeId?: string; appearance?: string }[] = (await import(launcherUrl)).observedLaunchers;

  function choices(workspace = DEFAULT_WORKSPACE_SNAPSHOT, catalog: ThemeCatalog | null = curatedCatalog()) {
    observed.length = 0;
    observedLaunchers.length = 0;
    const html = renderToStaticMarkup(createElement(ThemeGallery, {
      workspace, catalog, presets: catalog?.presets ?? [], mermaidThemes: ["auto"], selectedAppearance: "light",
      catalogReady: true, familyId: "flowchart", pending: false, canSelectRecipes: true, onSelect() {},
    }));
    return [...html.matchAll(/<button\b[^>]*data-theme-choice="([^"]+)"[^>]*>/gu)].map((match) => ({ key: match[1], button: match[0] }));
  }

  await context.test("each canonical preset appears once without its exploration duplicate", () => {
    const entries = choices();
    assert.equal(entries.length, curatedThemes.length + 1);
    for (const theme of curatedThemes) {
      assert.equal(entries.filter((entry) => entry.key === `preset:${theme.presetId}`).length, 1);
      assert.equal(entries.filter((entry) => entry.key === `exploration:${theme.id}`).length, 0);
    }
  });

  await context.test("the panel launcher retains canonical preferences and pinned concrete appearances", () => {
    for (const themePresetId of ["editor", "editor-dark", "editor-light"]) {
      for (const preference of ["default", "follow", "light", "dark"] as const) {
        choices({ ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId, themeAppearance: preference }, themeCatalogFixture());
        assert.deepEqual(observedLaunchers[0], {
          panel: true, themeId: "editor",
          appearance: themePresetId === "editor" ? preference
            : themePresetId === "editor-dark" ? "dark" : "light",
        });
      }
    }
  });

  await context.test("old shared recipes remain selected, enabled and separate from their preset", () => {
    for (const theme of curatedThemes) {
      const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: theme.recipeJson,
        themeRecipeLabel: { name: "Saved design", source: "exploration" as const } };
      const entries = choices(workspace);
      const current = entries.find((entry) => entry.key === "recipe:current");
      assert.ok(current);
      assert.match(current.button, /aria-pressed="true"/u);
      assert.doesNotMatch(current.button, /\bdisabled=/u);
      assert.match(current.button, /data-theme-source="exploration"/u);
      assert.equal(entries.filter((entry) => entry.key === `exploration:${theme.id}`).length, 0);
      assert.match(entries.find((entry) => entry.key === `preset:${theme.presetId}`)!.button, /aria-pressed="false"/u);
    }
  });

  await context.test("reselecting saved, imported or modified recipes preserves bytes, appearance and composition", () => {
    for (const theme of curatedThemes) {
      for (const fields of [
        { themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: "Saved design", source: "exploration" as const } },
        { themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: "Imported design", source: "imported" as const } },
        { themeRecipeJson: `${theme.recipeJson}\n`, themeRecipeLabel: { name: "Edited design", source: "exploration" as const } },
      ]) {
        const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, ...fields, diagramTheme: "forest" as const, themeAppearance: "light" as const };
        choices(workspace);
        const current = observed.flatMap((cards) => cards.entries).find((entry) => entry.key === "recipe:current");
        assert.ok(current?.available);
        assert.deepEqual({ ...workspace, ...themeChoiceFields(current.choice) }, workspace);
      }
    }
  });

  await context.test("old runtimes expose enabled exploration fallbacks", () => {
    const entries = choices(DEFAULT_WORKSPACE_SNAPSHOT, themeCatalogFixture());
    for (const theme of curatedThemes) {
      const exploration = entries.find((entry) => entry.key === `exploration:${theme.id}`);
      assert.ok(exploration);
      assert.doesNotMatch(exploration.button, /\bdisabled=/u);
    }
  });

  await context.test("policy-disabled presets have no enabled exploration or legacy duplicate", () => {
    for (const theme of curatedThemes) {
      const entries = choices({ ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: theme.recipeJson,
        themeRecipeLabel: { name: theme.name, source: "exploration" } }, curatedCatalog(false));
      assert.match(entries.find((entry) => entry.key === `preset:${theme.presetId}`)!.button, /\bdisabled=/u);
      assert.match(entries.find((entry) => entry.key === "recipe:current")!.button, /\bdisabled=/u);
      assert.equal(entries.filter((entry) => entry.key.startsWith("exploration:")).length, 0);
    }
  });

  await context.test("catalog failures cannot enable mapped recipe fallbacks", () => {
    const entries = choices(DEFAULT_WORKSPACE_SNAPSHOT, null);
    for (const theme of curatedThemes) {
      assert.match(entries.find((entry) => entry.key === `exploration:${theme.id}`)!.button, /\bdisabled=/u);
    }
  });
});

test("showcase entries retain workspace appearance and expose runtime recovery", async (context) => {
  const showcaseUrl = new URL("../components/theme-panel/ThemeShowcase.tsx", import.meta.url).href;
  const catalog = themeCatalogFixture();
  const recipe = JSON.stringify({
    schema_version: 1, kind: "design",
    design: { default_appearance: "light", appearances: { light: {}, dark: {} } },
  }, null, 2);
  const editor: ExplorationTheme = {
    id: "editor", presetId: "editor", name: "Editor", recipeJson: recipe,
    appearance: "light", colors: ["#ffffff"], families: [], sharedFamilies: ["class"],
    styles: ["reading"], previews: null, appearances: [
      { appearance: "light", colors: ["#ffffff"] }, { appearance: "dark", colors: ["#151515"] },
    ],
  };
  const authored: ExplorationTheme = {
    id: "reading", name: "Reading", recipeJson: recipe, appearance: "light",
    colors: ["#ffffff"], families: ["class"], styles: ["reading"], previews: null,
    appearances: editor.appearances,
  };
  const facade = {
    themeCatalog: () => catalog, exportThemePreset: () => JSON.parse(recipe),
    diagramFamilyCapabilities: () => [],
  };
  type DetailProps = Parameters<typeof ThemeShowcaseDetail>[0];
  const probe: {
    runtime: MermanRuntimeState;
    workspace: WorkspaceSnapshot & { resolvedTheme: "light" | "dark" };
    details: DetailProps[];
    buttons: { children?: unknown; onClick?: () => unknown }[];
    cards: string[];
    applied: DiagramThemeChoice[];
    retries: number;
    reloads: number;
  } = {
    runtime: { status: "ready", suspended: false, facade: facade as unknown as MermanDomainFacade },
    workspace: { ...DEFAULT_WORKSPACE_SNAPSHOT, resolvedTheme: "light" },
    details: [], buttons: [], cards: [], applied: [], retries: 0, reloads: 0,
  };
  const probeKey = "__themeShowcaseTestProbe";
  Object.defineProperty(globalThis, probeKey, { value: probe, configurable: true });
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", { configurable: true, value: {
    location: { reload() { probe.reloads += 1; } },
  } });
  const moduleUrl = (source: string) => `data:text/javascript,${encodeURIComponent(source)}`;
  const uiUrl = moduleUrl(`
    import { createElement } from ${JSON.stringify(import.meta.resolve("react"))};
    const Part = props => createElement("div", { role: props.role }, props.children);
    export const Button = props => {
      globalThis.${probeKey}.buttons.push(props);
      return createElement("button", { type: "button" }, props.children);
    };
    export { Part as Dialog, Part as DialogClose, Part as DialogContent, Part as DialogDescription, Part as DialogHeader, Part as DialogTitle };
  `);
  const modules: Record<string, string> = {
    "react-i18next": moduleUrl(`export const useTranslation = () => ({ t: key => key, i18n: { resolvedLanguage: "en" } });`),
    "zustand/react/shallow": moduleUrl(`export const useShallow = selector => selector;`),
    "@/src/runtime/use-merman-runtime": moduleUrl(`
      export const selectMermanFacade = state => state.status === "ready" ? state.facade : null;
      export const selectMermanFailure = state => state.status === "error" ? state.error : null;
      export const useMermanRuntime = selector => selector(globalThis.${probeKey}.runtime);
    `),
    "@/src/runtime/merman": moduleUrl(`export async function retryMermanRuntime() { globalThis.${probeKey}.retries += 1; }`),
    "@/src/store": moduleUrl(`
      export const DEFAULT_WORKSPACE_SNAPSHOT = ${JSON.stringify(DEFAULT_WORKSPACE_SNAPSHOT)};
      export const selectWorkspaceSnapshot = state => state;
      export const useAppStore = selector => selector(globalThis.${probeKey}.workspace);
      useAppStore.getState = () => ({ selectTheme: choice => globalThis.${probeKey}.applied.push(choice) });
    `),
    "@/src/themes/catalog": moduleUrl(`
      export const EXPLORATION_THEMES = ${JSON.stringify([authored])};
      export function themeDesignLibrary() { return ${JSON.stringify([authored, editor])}; }
    `),
    "@/src/themes/samples": moduleUrl(`export const THEME_SAMPLES = [{ id: "class" }];`),
    "./ThemeDesignCard": moduleUrl(`export function ThemeDesignCard(props) { globalThis.${probeKey}.cards.push(props.theme.id); return null; }`),
    "./ThemeShowcaseDetail": moduleUrl(`export function ThemeShowcaseDetail(props) { globalThis.${probeKey}.details.push(props); return null; }`),
    "./ThemeShowcaseFilters": moduleUrl(`export const ThemeShowcaseFilters = () => null;`),
    "./ThemeShowcaseUse": moduleUrl(`export const createThemeShowcaseUseState = () => ({ tab: "theme" });`),
  };
  const hooks = registerHooks({
    resolve(specifier, moduleContext, nextResolve) {
      if (moduleContext.parentURL === showcaseUrl) {
        if (modules[specifier]) return { url: modules[specifier], shortCircuit: true };
        if (specifier.startsWith("@/components/ui/")) return { url: uiUrl, shortCircuit: true };
        if (specifier.startsWith("@/")) return nextResolve(new URL(`../../${specifier.slice(2)}.ts`, import.meta.url).href, moduleContext);
        if (specifier === "./theme-selection") return nextResolve(new URL("../components/theme-panel/theme-selection.ts", import.meta.url).href, moduleContext);
      }
      return nextResolve(specifier, moduleContext);
    },
    load(url, moduleContext, nextLoad) {
      if (url !== showcaseUrl) return nextLoad(url, moduleContext);
      return { format: "module", shortCircuit: true, source: transpileModule(readFileSync(new URL(url), "utf8"), {
        compilerOptions: { jsx: JsxEmit.ReactJSX, module: ModuleKind.ESNext },
      }).outputText };
    },
  });
  try {
    const { default: ThemeShowcase } = await import(showcaseUrl);
    const props: ThemeShowcaseProps = {
      open: true, onOpenChange() {}, restoreFocus() {}, initialThemeId: "editor",
      filters: { query: "", familyId: null, appearance: "all", style: "all" },
      onFiltersChange() {}, currentFamilyId: "class",
    };
    function render(overrides: Partial<ThemeShowcaseProps> = {}) {
      probe.details = [];
      probe.buttons = [];
      probe.cards = [];
      probe.applied = [];
      return renderToStaticMarkup(createElement(ThemeShowcase, { ...props, ...overrides }));
    }

    await context.test("a URL-selected dark preset survives entering its current design", () => {
      for (const themePresetId of ["editor", "editor-dark"]) {
        const workspace = decodeShareHash(encodeShareHash({
          ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId,
          ...(themePresetId === "editor" ? { themeAppearance: "dark" as const } : {}),
        }))!;
        probe.workspace = { ...workspace, resolvedTheme: "light" };
        render({ initialAppearance: "dark" });
        const detail = probe.details[0];
        assert.equal(detail.selected, true);
        assert.equal(detail.selection.appearance, "dark");
        detail.onApply();
        assert.deepEqual(probe.applied, [{ kind: "preset", presetId: "editor", appearance: "dark" }]);
      }
    });

    await context.test("follow and document default remain preferences instead of concrete appearances", () => {
      for (const preference of ["follow", "default"] as const) {
        probe.workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "editor", themeAppearance: preference, resolvedTheme: "dark" };
        render({ initialAppearance: preference });
        const detail = probe.details[0];
        assert.equal(detail.selection.preference, preference);
        assert.equal(detail.selection.appearance, preference === "follow" ? "dark" : "light");
        detail.onApply();
        assert.deepEqual(probe.applied, [{ kind: "preset", presetId: "editor", appearance: preference }]);
      }
    });

    await context.test("collection browsing keeps its appearance filter", () => {
      render({ initialAppearance: undefined, filters: { ...props.filters, appearance: "dark" } });
      assert.equal(probe.details[0].selection.appearance, "dark");
    });

    await context.test("pending preset links show catalog status and recover into their requested detail", () => {
      probe.runtime = { status: "loading", stage: "wasm-fetch", suspended: false };
      const html = render();
      assert.match(html, /role="status"[^>]*>appearance.catalogPending/);
      assert.equal(probe.details.length, 0);
      probe.runtime = { status: "ready", suspended: false, facade: facade as unknown as MermanDomainFacade };
      render();
      assert.equal(probe.details[0].theme.id, "editor");
    });

    await context.test("failed preset links expose retry or reload and retain authored designs", async () => {
      for (const recovery of ["retry", "reload"] as const) {
        probe.runtime = { status: "error", suspended: false, error: {
          cause: new Error("WASM fetch failed"), detail: null, message: "WASM fetch failed", recovery, stage: "wasm-fetch",
        } };
        const html = render();
        assert.match(html, /role="alert"/);
        assert.match(html, /WASM fetch failed/);
        const button = probe.buttons.find((entry) => entry.children === `wasm.${recovery}`);
        assert.ok(button?.onClick);
        await button.onClick();
        assert.deepEqual(probe.cards, ["reading"]);
        assert.equal(probe.details.length, 0);
      }
      assert.equal(probe.retries, 1);
      assert.equal(probe.reloads, 1);
    });
  } finally {
    hooks.deregister();
    Reflect.deleteProperty(globalThis, probeKey);
    if (previousWindow) Object.defineProperty(globalThis, "window", previousWindow);
    else Reflect.deleteProperty(globalThis, "window");
  }
});
