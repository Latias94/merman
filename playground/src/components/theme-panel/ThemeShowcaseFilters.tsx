import type { Ref } from "react";
import { Search, SlidersHorizontal, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { clearThemeLibraryFilters, type ThemeLibraryFilters } from "@/src/lib/theme-library";
import { THEME_STYLE_IDS } from "@/src/themes/families";

export function ThemeShowcaseFilters({ id, filters, onChange, families, currentFamilyId, count, searchRef }: {
  readonly id: string;
  readonly filters: ThemeLibraryFilters;
  onChange(filters: ThemeLibraryFilters): void;
  readonly families: readonly string[];
  readonly currentFamilyId: string | null;
  readonly count: number;
  readonly searchRef: Ref<HTMLInputElement>;
}) {
  const { t } = useTranslation();
  const familyName = filters.familyId === null ? "" : t(`diagramTypes.${filters.familyId}`, { defaultValue: filters.familyId });
  const familyOptions = filters.familyId && !families.includes(filters.familyId) ? [...families, filters.familyId] : families;
  const hasFilters = filters.familyId !== null || filters.appearance !== "all" || filters.style !== "all";

  return <div className="shrink-0 space-y-3 border-b bg-muted/15 px-4 py-3 sm:px-6" data-testid="theme-library-filters">
    <div className="flex min-w-0 flex-col gap-2 rounded-lg border bg-background p-2 sm:flex-row sm:items-center" data-testid="theme-library-search-area">
      <div className="relative min-w-0 flex-1">
        <label htmlFor={`${id}-search`} className="sr-only">{t("themeShowcase.search")}</label>
        <Search className="pointer-events-none absolute left-2 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
        <Input ref={searchRef} id={`${id}-search`} type="search" placeholder={t("themeShowcase.search")} value={filters.query}
          className="border-0 bg-transparent pl-8 pr-9 shadow-none [&::-webkit-search-cancel-button]:appearance-none"
          onChange={(event) => onChange({ ...filters, query: event.target.value })} />
        {filters.query && <Button id={`${id}-clear-search`} type="button" variant="ghost" size="icon-sm"
          className="absolute right-0 top-1/2 -translate-y-1/2" aria-label={t("themeLibrary.clearSearch")}
          onClick={() => { onChange({ ...filters, query: "" }); document.getElementById(`${id}-search`)?.focus(); }}><X className="size-3.5" aria-hidden="true" /></Button>}
      </div>
      <div className="flex min-w-0 max-w-full items-center self-start rounded-full border bg-muted/30 sm:max-w-[45%]"
        data-testid={filters.familyId === null ? undefined : "theme-library-family-chip"}>
        {filters.familyId !== null && <span className="shrink-0 pl-3 text-xs text-muted-foreground">{t("themeLibrary.familyPrefix")}</span>}
        <label htmlFor={`${id}-family`} className="sr-only">{t("themeLibrary.familyFilter")}</label>
        <Select value={filters.familyId ?? "all"} onValueChange={(value) => onChange({ ...filters, familyId: value === "all" ? null : value })}>
          <SelectTrigger id={`${id}-family`} size="sm" aria-label={t("themeLibrary.familyFilter")} data-value={filters.familyId ?? ""}
            className="min-w-0 max-w-full flex-1 rounded-full border-0 shadow-none"><SelectValue /></SelectTrigger>
          <SelectContent className="max-h-[min(20rem,var(--radix-select-content-available-height))]">
            <SelectGroup>
              <SelectItem value="all" data-value="">{t("themeLibrary.allFamilies")}</SelectItem>
              {familyOptions.map((family) => <SelectItem key={family} value={family} data-value={family}>{t(`diagramTypes.${family}`, { defaultValue: family })}</SelectItem>)}
            </SelectGroup>
          </SelectContent>
        </Select>
        {filters.familyId !== null && <Button id={`${id}-remove-family`} type="button" variant="ghost" size="icon-sm" className="size-8 rounded-full"
          aria-label={t("themeLibrary.removeFamily", { family: familyName })}
          onClick={() => { onChange({ ...filters, familyId: null }); document.getElementById(`${id}-family`)?.focus(); }}>
          <X className="size-3.5" aria-hidden="true" />
        </Button>}
      </div>
    </div>
    <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2">
      <fieldset className="flex max-w-full flex-wrap gap-0.5 rounded-md border bg-background p-0.5">
        <legend className="sr-only">{t("themeShowcase.appearance")}</legend>
        {(["all", "light", "dark", "paired"] as const).map((appearance) => <Button id={`${id}-${appearance}`} key={appearance} type="button" size="sm"
          variant={appearance === filters.appearance ? "secondary" : "ghost"} className="h-7 px-2.5 text-xs"
          aria-pressed={appearance === filters.appearance} onClick={() => onChange({ ...filters, appearance })}>
          {t(`themeLibrary.appearances.${appearance}`)}
        </Button>)}
      </fieldset>
      <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
        <SlidersHorizontal className="size-3.5 shrink-0" aria-hidden="true" />
        <Select value={filters.style} onValueChange={(style) => {
          if (style === "all" || THEME_STYLE_IDS.some((entry) => entry === style)) onChange({ ...filters, style: style as ThemeLibraryFilters["style"] });
          }}>
          <SelectTrigger id={`${id}-style`} size="sm" aria-label={t("themeLibrary.styleFilter")} data-value={filters.style} className="min-w-0 max-w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value="all" data-value="all">{t("themeLibrary.allStyles")}</SelectItem>
              {THEME_STYLE_IDS.map((style) => <SelectItem key={style} value={style} data-value={style}>{t(`themeLibrary.styles.${style}`)}</SelectItem>)}
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>
      {hasFilters && <Button id={`${id}-clear-filters`} type="button" variant="ghost" size="sm" className="h-8 text-xs"
        onClick={() => { onChange(clearThemeLibraryFilters(filters)); document.getElementById(`${id}-search`)?.focus(); }}>{t("themeLibrary.clearFilters")}</Button>}
      {currentFamilyId && filters.familyId !== currentFamilyId && <Button type="button" variant="ghost" size="sm" className="h-8 text-xs"
        onClick={() => { onChange({ ...filters, familyId: currentFamilyId }); document.getElementById(`${id}-family`)?.focus(); }}>{t("themeLibrary.currentFamily")}</Button>}
      <p role="status" className="ml-auto text-xs tabular-nums text-muted-foreground">{t("themeShowcase.count", { count })}</p>
    </div>
  </div>;
}
