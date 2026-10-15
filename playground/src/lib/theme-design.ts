import type { ThemePresetCatalogEntry } from "@mermanjs/web";

export type ThemeFamilyTreatment = "dedicated" | "base_only" | "unreviewed";

/** Curated design only: never derive support or portability from availability or qualification. */
export function themeFamilyTreatment(
  preset: Pick<ThemePresetCatalogEntry, "family_designs"> | undefined,
  familyId: string | undefined,
): ThemeFamilyTreatment {
  const treatment = preset?.family_designs.find((design) => design.family_id === familyId)?.treatment;
  return treatment === "dedicated" || treatment === "base_only" ? treatment : "unreviewed";
}
