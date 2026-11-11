import { useId, useState } from "react";
import { Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemePresetCatalogEntry, ThemeCatalog, ThemeAppearance } from "@mermanjs/web";

import type { DiagramThemeChoice } from "@/src/lib/diagram-appearance";
import type { MermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import { themeFamilyTreatment } from "@/src/lib/theme-discovery";
import type { WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import { EXPLORATION_THEMES } from "@/src/themes/catalog";
import { ThemeChoiceCards, type ThemeCatalogChoice } from "./ThemeChoiceCards";
import { currentRecipeIdentity, galleryExplorations, selectedExplorationTheme, selectedThemeKey } from "./theme-selection";
import { ThemeShowcaseLauncher } from "./ThemeShowcaseLauncher";
import { preferredAppearanceForSelection, selectedPresetCatalogEntry } from "@/src/lib/theme-appearance";
import { themeDesignTreatment } from "@/src/lib/theme-library";

export default function ThemeGallery({ workspace, mermaidThemes, presets, catalog, selectedAppearance, catalogReady, familyId, pending, canSelectRecipes, onSelect }: {
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly mermaidThemes: readonly MermaidThemeSelection[];
  readonly presets: readonly ThemePresetCatalogEntry[];
  readonly catalog: ThemeCatalog | null;
  readonly selectedAppearance: ThemeAppearance | null;
  readonly catalogReady: boolean;
  readonly familyId: string | null;
  readonly pending: boolean;
  readonly canSelectRecipes: boolean;
  onSelect(choice: DiagramThemeChoice): void;
}) {
  const { t, i18n } = useTranslation();
  const searchId = useId();
  const [query, setQuery] = useState("");
  const explorations = galleryExplorations(EXPLORATION_THEMES, catalog);
  const recipe = currentRecipeIdentity(workspace, explorations, t("appearance.importedTheme"));
  const selectedKey = selectedThemeKey(workspace, EXPLORATION_THEMES, catalog);
  const selectedExploration = selectedExplorationTheme(workspace, EXPLORATION_THEMES, catalog);
  const selectedPreset = selectedPresetCatalogEntry(catalog, workspace.themePresetId);
  const appearancePreference = workspace.themeAppearance ?? "default";
  const showcaseAppearance = selectedPreset?.pinned ?? appearancePreference;
  const recipePreset = recipe && selectedExploration
    ? selectedPresetCatalogEntry(catalog, selectedExploration.presetId ?? null) : null;
  const entries: ThemeCatalogChoice[] = [
    ...(catalog ? [{ key: "mermaid:auto", name: t("themes.auto"), source: "automatic" as const,
      choice: { kind: "mermaid" as const, theme: "auto" as const }, available: true },
    ...catalog.mermaid_theme_designs.filter((design) =>
      design.appearances.every((entry) => mermaidThemes.includes(entry.theme_id))).map((design): ThemeCatalogChoice => ({
      key: `mermaid:${design.id}`,
      name: t(`themes.${design.id}`, { defaultValue: design.display_name }),
      source: "mermaid",
      choice: { kind: "mermaid", theme: design.id as MermaidThemeSelection,
        appearance: design.appearances.length > 1 ? "follow" : "default" },
      appearances: design.appearances.map((entry) => entry.appearance),
      searchTerms: design.appearances.map((entry) => `mermaid:${entry.theme_id}`).join(" "),
      available: true,
    }))] : mermaidThemes.map((theme): ThemeCatalogChoice => ({
      key: `mermaid:${theme}`,
      name: t(`themes.${theme}`, { defaultValue: theme }),
      source: theme === "auto" ? "automatic" : "mermaid",
      choice: { kind: "mermaid", theme },
      available: true,
    }))),
    ...presets.map((preset): ThemeCatalogChoice => ({
      key: `preset:${preset.id}`,
      name: t(`themePresets.${preset.id}`, { defaultValue: preset.display_name }),
      source: "builtin",
      choice: { kind: "preset", presetId: preset.id, appearance: preferredAppearanceForSelection(preset) },
      colors: preset.appearances.find((entry) => entry.appearance === (selectedKey === `preset:${preset.id}` ? selectedAppearance : preset.default_appearance))?.preview_colors,
      appearances: preset.supported_appearances,
      searchTerms: preset.appearances.map((entry) => `preset:${entry.concrete_preset_id}`).join(" "),
      available: canSelectRecipes && preset.available,
      loading: !canSelectRecipes,
      unavailableReason: !preset.available ? preset.availability_reason_ids.join(", ") : undefined,
      maturity: preset.maturity,
      treatment: pending ? "waiting" : themeFamilyTreatment(preset.appearances.find((entry) =>
        entry.appearance === (selectedKey === `preset:${preset.id}` ? selectedAppearance : preset.default_appearance)) ?? { family_designs: [] }, familyId),
    })),
    ...explorations.map((theme): ThemeCatalogChoice => ({
      key: `exploration:${theme.id}`,
      name: recipe?.explorationId === theme.id ? recipe.name : theme.name,
      source: "exploration",
      choice: { kind: "recipe", recipeJson: theme.recipeJson, label: {
        name: recipe?.explorationId === theme.id ? recipe.name : theme.name, source: "exploration",
      } },
      description: t(`themeGallery.themes.${theme.id}`),
      colors: theme.appearances?.find((entry) => entry.appearance === (recipe?.explorationId === theme.id ? selectedAppearance : theme.appearance))?.colors ?? theme.colors,
      appearances: theme.appearances?.map((entry) => entry.appearance) ?? [theme.appearance],
      searchTerms: theme.families.map((family) => `${family} ${t(`themeGallery.samples.${family}`)}`).join(" "),
      treatment: pending ? "waiting" : themeDesignTreatment(theme, familyId),
      designedFamilies: theme.families,
      available: canSelectRecipes && (!theme.presetId || catalog !== null),
      loading: !canSelectRecipes || (!!theme.presetId && !catalogReady),
    })),
  ];
  if (recipe && !recipe.explorationId) entries.push({
    key: "recipe:current", name: recipe.name, source: recipe.source,
    choice: { kind: "recipe", recipeJson: recipe.recipeJson, compose: true, appearance: workspace.themeAppearance ?? "default", label: {
      name: recipe.name, source: recipe.source, ...(recipe.derivedFrom ? { derivedFrom: recipe.derivedFrom } : {}),
    } },
    description: t("appearance.currentRecipe"), available: canSelectRecipes && recipePreset?.available !== false, loading: !canSelectRecipes,
    unavailableReason: recipePreset && !recipePreset.available ? recipePreset.preset.availability_reason_ids.join(", ") : undefined,
  });
  if (workspace.themePresetId && !presets.some((preset) => preset.id === workspace.themePresetId) &&
    !catalog?.presets.some((entry) => entry.appearances.some((appearance) => appearance.concrete_preset_id === workspace.themePresetId))) entries.push({
    key: `preset:${workspace.themePresetId}`, name: workspace.themePresetId, source: "builtin",
    choice: { kind: "preset", presetId: workspace.themePresetId }, available: false, loading: !catalogReady,
    description: t(catalogReady ? "appearance.unavailablePresetChoice" : "appearance.catalogPending"),
  });
  const locale = i18n.resolvedLanguage;
  const search = query.trim().toLocaleLowerCase(locale);
  const groups = (["automatic", "mermaid", "builtin", "exploration", "imported"] as const).map((source) => {
    const title = t(`appearance.groups.${source}.title`);
    const description = t(`appearance.groups.${source}.description`);
    const choices = entries.filter((entry) => entry.source === source &&
      `${entry.key} ${entry.name} ${entry.description ?? ""} ${entry.searchTerms ?? ""} ${t(`appearance.sources.${source}`)} ${title} ${description}`
        .toLocaleLowerCase(locale).includes(search));
    if (source !== "mermaid") choices.sort((left, right) =>
      left.name.localeCompare(right.name, locale, { numeric: true, sensitivity: "base" }));
    return { source, title, description, choices };
  }).filter((group) => group.choices.length > 0);

  return <div className="min-w-0 space-y-3" data-testid="theme-catalog">
    <ThemeShowcaseLauncher panel themeId={selectedExploration?.id ?? selectedPreset?.preset.id} appearance={showcaseAppearance} />
    <div className="relative min-w-0">
      <label htmlFor={searchId} className="sr-only">{t("appearance.searchThemes")}</label>
      <Search className="pointer-events-none absolute left-2.5 top-2.5 size-3.5 text-muted-foreground" aria-hidden="true" />
      <input id={searchId} type="search" value={query} onChange={(event) => setQuery(event.target.value)}
        placeholder={t("appearance.searchThemes")} className="h-9 w-full min-w-0 rounded-md border bg-background pl-8 pr-2 text-xs"
        onKeyDown={(event) => {
          if (event.key === "Escape" && query) {
            event.preventDefault();
            event.stopPropagation();
            setQuery("");
          }
        }} />
    </div>
    <div className="space-y-4" data-testid="theme-catalog-choices">
      {groups.map((group) => <section key={group.source} aria-labelledby={`${searchId}-${group.source}`}
        data-theme-group={group.source} className="min-w-0 space-y-2 border-t pt-3 first:border-t-0 first:pt-0">
        <div className="space-y-1">
          <h4 id={`${searchId}-${group.source}`} className="flex items-center justify-between gap-2 text-xs font-semibold">
            <span>{group.title}</span>
            <span className="shrink-0 text-[10px] font-normal tabular-nums text-muted-foreground" data-theme-group-count>{group.choices.length}</span>
          </h4>
          <p className="text-[11px] leading-relaxed text-muted-foreground">{group.description}</p>
        </div>
        <ThemeChoiceCards entries={group.choices} selectedKey={selectedKey} onSelect={onSelect} />
      </section>)}
    </div>
    {groups.length === 0 && <p role="status" className="rounded-md border border-dashed px-3 py-5 text-center text-xs text-muted-foreground">{t("appearance.noThemes")}</p>}
    <p className="text-[11px] leading-relaxed text-muted-foreground">{t("appearance.catalogHint")}</p>
  </div>;
}
