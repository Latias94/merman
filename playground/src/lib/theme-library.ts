import type { ThemeSampleId, ThemeStyleId } from "../themes/families.ts";
import type { ThemeFamilyTreatment } from "./theme-discovery.ts";

export interface ThemeLibraryFilters {
  readonly query: string;
  readonly familyId: string | null;
  readonly appearance: "all" | "light" | "dark" | "paired";
  readonly style: "all" | ThemeStyleId;
}

export interface ThemeLibraryDesign {
  readonly id: string;
  readonly name: string;
  readonly appearance: "light" | "dark";
  readonly appearances?: readonly { readonly appearance: "light" | "dark" }[];
  readonly families: readonly string[];
  readonly sharedFamilies?: readonly string[];
  readonly styles: readonly ThemeStyleId[];
}

export function createThemeLibraryFilters(familyId: string | null): ThemeLibraryFilters {
  return { query: "", familyId, appearance: "all", style: "all" };
}

export function clearThemeLibraryFilters(filters: ThemeLibraryFilters): ThemeLibraryFilters {
  return { ...createThemeLibraryFilters(null), query: filters.query };
}

export function themeDesignTreatment(theme: Pick<ThemeLibraryDesign, "families" | "sharedFamilies">, familyId: string | null): ThemeFamilyTreatment {
  if (familyId !== null && theme.families.includes(familyId)) return "dedicated";
  if (familyId !== null && theme.sharedFamilies?.includes(familyId)) return "base_only";
  return "unreviewed";
}

export function themeSampleFor(
  theme: { readonly families: readonly ThemeSampleId[]; readonly featuredSample?: ThemeSampleId },
  familyId: string | null,
  fallback?: ThemeSampleId,
): ThemeSampleId {
  return theme.families.find((id) => id === familyId)
    ?? theme.families.find((id) => id === fallback)
    ?? theme.families.find((id) => id === theme.featuredSample)
    ?? theme.families[0] ?? "flowchart";
}

export function filterThemeLibrary<T extends ThemeLibraryDesign>(
  themes: readonly T[],
  filters: ThemeLibraryFilters,
  searchText: (theme: T) => string = () => "",
  locale?: string,
): T[] {
  const query = filters.query.trim().toLocaleLowerCase(locale);
  return themes.filter((theme) => {
    if (filters.familyId !== null && themeDesignTreatment(theme, filters.familyId) !== "dedicated") return false;
    const appearances = theme.appearances?.map((entry) => entry.appearance) ?? [theme.appearance];
    if (filters.appearance === "paired" ? appearances.length < 2
      : filters.appearance !== "all" && !appearances.includes(filters.appearance)) return false;
    if (filters.style !== "all" && !theme.styles.includes(filters.style)) return false;
    return `${theme.id} ${theme.name} ${searchText(theme)}`.toLocaleLowerCase(locale).includes(query);
  });
}
