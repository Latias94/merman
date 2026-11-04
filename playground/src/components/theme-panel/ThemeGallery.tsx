import { useId, useState } from "react";
import { Download, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemePresetCatalogEntry } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import type { DiagramThemeChoice } from "@/src/lib/diagram-appearance";
import { downloadBlob } from "@/src/lib/export";
import type { MermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import { themeFamilyTreatment } from "@/src/lib/theme-discovery";
import type { WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import { EXPLORATION_THEMES } from "@/src/themes/catalog";
import { THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";
import { ThemeChoiceCards, type ThemeCatalogChoice } from "./ThemeChoiceCards";
import { currentRecipeIdentity, selectedThemeKey } from "./theme-selection";

export default function ThemeGallery({ workspace, mermaidThemes, presets, catalogReady, familyId, pending, canSelectRecipes, onSelect }: {
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly mermaidThemes: readonly MermaidThemeSelection[];
  readonly presets: readonly ThemePresetCatalogEntry[];
  readonly catalogReady: boolean;
  readonly familyId: string | null;
  readonly pending: boolean;
  readonly canSelectRecipes: boolean;
  onSelect(choice: DiagramThemeChoice): void;
}) {
  const { t } = useTranslation();
  const searchId = useId();
  const [query, setQuery] = useState("");
  const [sampleId, setSampleId] = useState<ThemeSampleId>("flowchart");
  const recipe = currentRecipeIdentity(workspace, EXPLORATION_THEMES, t("appearance.importedTheme"));
  const selectedKey = selectedThemeKey(workspace, EXPLORATION_THEMES);
  const selectedExploration = EXPLORATION_THEMES.find((theme) => theme.id === recipe?.explorationId);
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId)!;
  const entries: ThemeCatalogChoice[] = [
    ...mermaidThemes.map((theme): ThemeCatalogChoice => ({
      key: `mermaid:${theme}`,
      name: t(`themes.${theme}`, { defaultValue: theme }),
      source: "mermaid",
      choice: { kind: "mermaid", theme },
      available: true,
    })),
    ...presets.map((preset): ThemeCatalogChoice => ({
      key: `preset:${preset.id}`,
      name: t(`themePresets.${preset.id}`, { defaultValue: preset.display_name }),
      source: "builtin",
      choice: { kind: "preset", presetId: preset.id },
      colors: preset.preview_colors,
      available: canSelectRecipes && preset.available,
      loading: !canSelectRecipes,
      unavailableReason: !preset.available ? preset.availability_reason_ids.join(", ") : undefined,
      maturity: preset.maturity,
      treatment: pending ? "waiting" : themeFamilyTreatment(preset, familyId),
    })),
    ...EXPLORATION_THEMES.map((theme): ThemeCatalogChoice => ({
      key: `exploration:${theme.id}`,
      name: recipe?.explorationId === theme.id ? recipe.name : theme.name,
      source: "exploration",
      choice: { kind: "recipe", recipeJson: theme.recipeJson, label: {
        name: recipe?.explorationId === theme.id ? recipe.name : theme.name, source: "exploration",
      } },
      description: t(`themeGallery.themes.${theme.id}`),
      colors: theme.colors,
      available: canSelectRecipes,
      loading: !canSelectRecipes,
    })),
  ];
  if (recipe && !recipe.explorationId) entries.push({
    key: "recipe:current", name: recipe.name, source: recipe.source,
    choice: { kind: "recipe", recipeJson: recipe.recipeJson, label: { name: recipe.name, source: recipe.source } },
    description: t("appearance.currentRecipe"), available: canSelectRecipes, loading: !canSelectRecipes,
  });
  if (workspace.themePresetId && !presets.some((preset) => preset.id === workspace.themePresetId)) entries.push({
    key: `preset:${workspace.themePresetId}`, name: workspace.themePresetId, source: "builtin",
    choice: { kind: "preset", presetId: workspace.themePresetId }, available: false, loading: !catalogReady,
    description: t(catalogReady ? "appearance.unavailablePresetChoice" : "appearance.catalogPending"),
  });
  const search = query.trim().toLocaleLowerCase();
  const visible = entries.filter((entry) =>
    `${entry.name} ${entry.description ?? ""} ${t(`appearance.sources.${entry.source}`)}`.toLocaleLowerCase().includes(search))
    .sort((left, right) => left.name.localeCompare(right.name));

  return <div className="min-w-0 space-y-3" data-testid="theme-catalog">
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
    <ThemeChoiceCards entries={visible} selectedKey={selectedKey} onSelect={onSelect} />
    {visible.length === 0 && <p role="status" className="rounded-md border border-dashed px-3 py-5 text-center text-xs text-muted-foreground">{t("appearance.noThemes")}</p>}
    <p className="text-[11px] leading-relaxed text-muted-foreground">{t("appearance.catalogHint")}</p>
    {selectedExploration && <details key={selectedExploration.id} className="min-w-0 rounded-md border text-xs" data-testid="theme-examples">
      <summary className="cursor-pointer px-3 py-2 font-medium">{t("appearance.exampleDiagrams", { name: recipe?.name ?? selectedExploration.name })}</summary>
      <div className="min-w-0 space-y-3 border-t p-3" data-testid="theme-gallery-detail">
        <p className="text-[11px] leading-relaxed text-muted-foreground">{t("appearance.exampleHint")}</p>
        <fieldset className="flex flex-wrap gap-1">
          <legend className="sr-only">{t("themeGallery.sample")}</legend>
          {THEME_SAMPLES.map((entry) => <Button key={entry.id} type="button" size="sm" variant={sampleId === entry.id ? "secondary" : "ghost"}
            aria-pressed={sampleId === entry.id} className="h-7 px-2 text-[11px]" onClick={() => setSampleId(entry.id)}>
            {t(`themeGallery.samples.${entry.id}`)}
          </Button>)}
        </fieldset>
        <div className="flex min-h-40 min-w-0 items-center justify-center overflow-hidden rounded border p-2" style={{ backgroundColor: selectedExploration.colors[0] }}>
          <img key={`${selectedExploration.id}-${sampleId}`} src={selectedExploration.previews[sampleId]}
            alt={t("appearance.exampleAlt", { name: selectedExploration.name, sample: t(`themeGallery.samples.${sampleId}`) })}
            loading="lazy" className="max-h-64 w-full min-w-0 object-contain" data-testid="theme-gallery-preview" />
        </div>
        <details className="min-w-0 rounded border" key={sampleId}>
          <summary className="cursor-pointer px-2 py-1.5 font-medium">{t("themeGallery.sampleSource")}</summary>
          <div className="min-w-0 space-y-2 border-t p-2">
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] [overflow-wrap:anywhere]">{sample.source}</pre>
            <p className="font-medium">{t("themeGallery.sampleConfig")}</p>
            <pre className="overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] [overflow-wrap:anywhere]">{sample.configJson}</pre>
          </div>
        </details>
        <Button type="button" variant="outline" size="sm" className="h-auto max-w-full whitespace-normal py-1.5" onClick={() =>
          downloadBlob(new Blob([selectedExploration.recipeJson], { type: "application/json;charset=utf-8" }), `${selectedExploration.id}.theme.json`)}>
          <Download className="size-3.5 shrink-0" aria-hidden="true" />{t("appearance.downloadExampleRecipe", { name: selectedExploration.name })}
        </Button>
      </div>
    </details>}
  </div>;
}
