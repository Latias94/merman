import type { ThemeCatalog } from "@mermanjs/web";
import type { ExplorationTheme } from "./catalog.ts";
import { THEME_SAMPLE_IDS, type ThemeSampleId, type ThemeStyleId } from "./families.ts";

const PRESENTATION: Readonly<Record<string, { styles: readonly ThemeStyleId[]; featuredSample: ThemeSampleId }>> = {
  brutalist: { styles: ["expressive", "print"], featuredSample: "flowchart" },
  cyberpunk: { styles: ["technical", "expressive"], featuredSample: "flowchart" },
  ghibli: { styles: ["material", "reading"], featuredSample: "pie" },
  spotless: { styles: ["print", "reading"], featuredSample: "sequence" },
  editor: { styles: ["reading", "technical"], featuredSample: "class" },
  "one-dark": { styles: ["reading", "technical"], featuredSample: "class" },
  gruvbox: { styles: ["reading"], featuredSample: "xychart" },
  ayu: { styles: ["reading"], featuredSample: "xychart" },
};

export function presetLibraryDesigns(
  existing: readonly Pick<ExplorationTheme, "presetId">[],
  catalog: ThemeCatalog | null,
  exportPreset: (id: string) => unknown,
): ExplorationTheme[] {
  return (catalog?.presets ?? []).filter((preset) =>
    preset.available && !existing.some((theme) => theme.presetId === preset.id),
  ).map((preset) => {
    const appearance = preset.appearances.find((entry) => entry.appearance === preset.default_appearance)!;
    const presentation = PRESENTATION[preset.id];
    return {
      id: preset.id, presetId: preset.id, name: preset.display_name,
      appearance: preset.default_appearance,
      colors: appearance.preview_colors,
      recipeJson: JSON.stringify(exportPreset(preset.id), null, 2),
      families: THEME_SAMPLE_IDS.filter((id) => appearance.family_designs.some((entry) => entry.family_id === id && entry.treatment === "dedicated")),
      sharedFamilies: THEME_SAMPLE_IDS.filter((id) => appearance.family_designs.some((entry) => entry.family_id === id && entry.treatment === "base_only")),
      styles: presentation?.styles ?? [],
      featuredSample: presentation?.featuredSample,
      previews: null,
      sampleNotes: preset.appearances.flatMap((entry) => entry.application_notes.flatMap((note) => {
        const familyId = THEME_SAMPLE_IDS.find((id) => id === note.family_id);
        return familyId ? [{ familyId, appearance: entry.appearance, message: note.message }] : [];
      })),
      ...(preset.supported_appearances.length > 1 ? {
        appearances: preset.appearances.map((entry) => ({ appearance: entry.appearance, colors: entry.preview_colors })),
      } : {}),
    };
  });
}
