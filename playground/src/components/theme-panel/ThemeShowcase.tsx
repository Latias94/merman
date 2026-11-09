import { useId, useMemo, useRef, useState, type FocusEvent } from "react";
import { SearchX, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import type { ThemeAppearance } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { clearThemeLibraryFilters, filterThemeLibrary, themeSampleFor, type ThemeLibraryFilters } from "@/src/lib/theme-library";
import { inspectThemeRecipe, resolveThemeSelection, type ThemeAppearancePreference } from "@/src/lib/theme-appearance";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { DEFAULT_WORKSPACE_SNAPSHOT, selectWorkspaceSnapshot, useAppStore } from "@/src/store";
import { EXPLORATION_THEMES, type ExplorationTheme } from "@/src/themes/catalog";
import { isThemeSampleId, THEME_SAMPLE_IDS, type ThemeSampleId } from "@/src/themes/families";
import { THEME_SAMPLES } from "@/src/themes/samples";
import { ThemeDesignCard } from "./ThemeDesignCard";
import { ThemeShowcaseDetail, type ThemeShowcasePreviewState } from "./ThemeShowcaseDetail";
import { ThemeShowcaseFilters } from "./ThemeShowcaseFilters";
import { createThemeShowcaseUseState } from "./ThemeShowcaseUse";

export interface ThemeShowcaseProps {
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
  readonly initialThemeId?: string;
  readonly filters: ThemeLibraryFilters;
  onFiltersChange(filters: ThemeLibraryFilters): void;
  readonly currentFamilyId: string | null;
  readonly initialSampleId?: ThemeSampleId;
}

type BrowseFocus = { readonly kind: "control"; readonly id: string } | { readonly kind: "card"; readonly id: string };

export default function ThemeShowcase({ open, onOpenChange, restoreFocus, initialThemeId, filters, onFiltersChange, currentFamilyId, initialSampleId }: ThemeShowcaseProps) {
  const { t, i18n } = useTranslation();
  const filtersId = useId();
  const search = useRef<HTMLInputElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const detailViewport = useRef<HTMLDivElement>(null);
  const gridScroll = useRef(0);
  const detailScroll = useRef(0);
  const detailFocus = useRef<string | null>(null);
  const browseFocus = useRef<BrowseFocus | null>(null);
  const searchSelection = useRef<{ start: number | null; end: number | null } | null>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const hostAppearance = useAppStore((state) => state.resolvedTheme);
  const [detailId, setDetailId] = useState<string | null>(initialThemeId ?? null);
  const [sampleId, setSampleId] = useState<ThemeSampleId | null>(initialSampleId ?? null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [detailPreference, setDetailPreference] = useState<ThemeAppearancePreference>(() => initialAppearance(filters));
  const [previewState, setPreviewState] = useState<ThemeShowcasePreviewState>({ mode: "sample", showSource: false });
  const [adoptionState, setAdoptionState] = useState(createThemeShowcaseUseState);
  const detail = EXPLORATION_THEMES.find((theme) => theme.id === detailId);
  const selected = EXPLORATION_THEMES.find((theme) => theme.recipeJson === workspace.themeRecipeJson);
  const themes = filterThemeLibrary(EXPLORATION_THEMES, filters, (theme) =>
    `${t(`themeGallery.themes.${theme.id}`)} ${t(`themeShowcase.mechanisms.${theme.id}`)} ${theme.families.map((family) => `${family} ${t(`themeGallery.samples.${family}`)}`).join(" ")}`,
  i18n.resolvedLanguage);
  const families = useMemo(() => [...new Set([...THEME_SAMPLE_IDS, ...(facade?.diagramFamilyCapabilities().map((entry) => entry.family_id) ?? [])])], [facade]);
  const detailSelection = resolveThemeSelection({
    ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: detail?.recipeJson ?? null,
    themeAppearance: detail?.appearances ? detailPreference : "default",
  }, hostAppearance, null, {
    supported_appearances: detail?.appearances?.map((entry) => entry.appearance) ?? (detail ? [detail.appearance] : []),
    default_appearance: detail?.appearance ?? null,
  });
  const cardSampleId = isThemeSampleId(filters.familyId) ? filters.familyId : null;

  function rememberFocus(event: FocusEvent<HTMLDivElement>) {
    const element = event.target;
    const card = element.closest<HTMLButtonElement>("[data-explore-theme]");
    if (card?.dataset.exploreTheme) browseFocus.current = { kind: "card", id: card.dataset.exploreTheme };
    else if (element.id && element.matches("button, select, input")) browseFocus.current = { kind: "control", id: element.id };
  }

  function restoreBrowse() {
    const target = browseFocus.current;
    const element = target?.kind === "card"
      ? viewport.current?.querySelector<HTMLButtonElement>(`[data-explore-theme="${target.id}"]`)
      : target ? document.getElementById(target.id) : null;
    (element ?? search.current)?.focus({ preventScroll: true });
    if ((!element || element === search.current) && searchSelection.current && search.current) {
      const { start, end } = searchSelection.current;
      if (start !== null && end !== null) search.current.setSelectionRange(start, end);
    }
    if (viewport.current) viewport.current.scrollTop = gridScroll.current;
  }

  function changeFilters(next: ThemeLibraryFilters) {
    onFiltersChange(next);
    gridScroll.current = 0;
    if (viewport.current) viewport.current.scrollTop = 0;
  }

  function explore(theme: ExplorationTheme) {
    gridScroll.current = viewport.current?.scrollTop ?? 0;
    browseFocus.current = { kind: "card", id: theme.id };
    detailScroll.current = 0;
    detailFocus.current = null;
    setSampleId(null);
    setDetailId(theme.id);
    setDetailPreference(initialAppearance(filters));
    setPreviewState({ mode: "sample", showSource: false });
    setAdoptionState(createThemeShowcaseUseState());
    setActionError(null);
    requestAnimationFrame(() => heading.current?.focus());
  }

  function back() {
    setDetailId(null);
    setActionError(null);
    requestAnimationFrame(restoreBrowse);
  }

  function apply(theme: ExplorationTheme) {
    try {
      if (!facade) return;
      inspectThemeRecipe(facade, theme.recipeJson);
      useAppStore.getState().selectTheme({ kind: "recipe", recipeJson: theme.recipeJson,
        appearance: theme.appearances ? detailPreference : "default",
        label: { name: theme.name, source: "exploration" } });
      setActionError(null);
      onOpenChange(false);
    } catch (error) { setActionError(projectError(error).summary); }
  }

  return <Dialog open={open} onOpenChange={onOpenChange}>
    <DialogContent safeArea={false} showCloseButton={false} data-testid="theme-showcase"
      className="flex h-[100dvh] w-screen max-w-none flex-col gap-0 overflow-hidden rounded-none border-0 p-0 sm:h-[min(94dvh,70rem)] sm:w-[calc(100vw-2rem)] sm:max-w-[94rem] sm:rounded-xl sm:border"
      style={{ paddingTop: "env(safe-area-inset-top)", paddingRight: "env(safe-area-inset-right)", paddingBottom: "env(safe-area-inset-bottom)", paddingLeft: "env(safe-area-inset-left)" }}
      onEscapeKeyDown={(event) => {
        if (!detail && filters.query && document.activeElement === search.current) {
          event.preventDefault();
          changeFilters({ ...filters, query: "" });
        }
      }}
      onOpenAutoFocus={(event) => {
        event.preventDefault();
        requestAnimationFrame(() => {
          if (detail) {
            const focused = [...(detailViewport.current?.querySelectorAll<HTMLElement>("button, select, input, a, textarea, summary") ?? [])]
              .find((element) => !element.matches(":disabled") && focusIdentity(element) === detailFocus.current);
            (focused ?? heading.current)?.focus({ preventScroll: true });
            focused?.scrollIntoView({ block: "nearest", inline: "nearest" });
            if (detailViewport.current) detailViewport.current.scrollTop = detailScroll.current;
          } else restoreBrowse();
        });
      }}
      onCloseAutoFocus={(event) => { event.preventDefault(); restoreFocus(); }}>
      <DialogHeader className="relative shrink-0 border-b px-4 py-4 pr-14 text-left sm:px-6">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <DialogTitle>{t("themeShowcase.title")}</DialogTitle>
          <span className="rounded-full border px-2 py-0.5 text-[10px] font-medium tracking-wide text-muted-foreground">{t("themeShowcase.recipeBadge")}</span>
        </div>
        <DialogDescription className="max-w-3xl text-xs leading-relaxed sm:text-sm">{t("themeShowcase.description")}</DialogDescription>
        <DialogClose asChild><Button type="button" variant="ghost" size="icon" className="absolute right-3 top-3" aria-label={t("themeShowcase.close")}><X className="size-5" aria-hidden="true" /></Button></DialogClose>
      </DialogHeader>
      <div className={detail ? "hidden" : "flex min-h-0 flex-1 flex-col"} onFocusCapture={rememberFocus}
        onBlurCapture={(event) => {
          if (event.target === search.current) searchSelection.current = { start: search.current.selectionStart, end: search.current.selectionEnd };
        }}>
        <ThemeShowcaseFilters id={filtersId} filters={filters} onChange={changeFilters} families={families} currentFamilyId={currentFamilyId} count={themes.length} searchRef={search} />
        <div ref={viewport} className="min-h-0 flex-1 overflow-y-auto overscroll-contain" data-testid="theme-showcase-viewport"
          onScroll={(event) => { if (!detail) gridScroll.current = event.currentTarget.scrollTop; }}>
          <div className="space-y-5 p-4 sm:p-6">
            <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
              <h2 className="text-2xl font-semibold tracking-tight">{t("themeShowcase.collectionTitle")}</h2>
              <p className="max-w-2xl text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.collectionHint", { count: THEME_SAMPLES.length })}</p>
            </div>
            {themes.length > 0 ? <div className="grid grid-cols-1 gap-5 md:grid-cols-2 xl:grid-cols-3" data-testid="theme-showcase-grid">
              {themes.map((theme) => <ThemeDesignCard key={theme.id} theme={theme} selected={theme.id === selected?.id}
                sampleId={cardSampleId} appearance={cardAppearance(theme, filters, hostAppearance)} active={open && !detail} onExplore={() => explore(theme)} />)}
            </div> : <div className="mx-auto flex max-w-lg flex-col items-center gap-3 px-3 py-14 text-center" data-testid="theme-library-empty">
              <SearchX className="size-7 text-muted-foreground" aria-hidden="true" />
              <h3 className="text-base font-semibold">{t("themeLibrary.empty.title")}</h3>
              <p role="status" className="text-sm leading-relaxed text-muted-foreground">{filters.familyId !== null
                ? t("themeLibrary.empty.family", { family: t(`diagramTypes.${filters.familyId}`, { defaultValue: filters.familyId }) })
                : t("themeLibrary.empty.hint")}</p>
              <p className="text-xs leading-relaxed text-muted-foreground">{t("themeLibrary.empty.searchHint")}</p>
              <div className="flex flex-wrap justify-center gap-2">
                <Button type="button" variant="outline" size="sm" onClick={() => { changeFilters(clearThemeLibraryFilters(filters)); search.current?.focus(); }}>{t("themeLibrary.clearFilters")}</Button>
                {filters.query && <Button type="button" variant="ghost" size="sm" onClick={() => { changeFilters({ ...filters, query: "" }); search.current?.focus(); }}>{t("themeLibrary.clearSearch")}</Button>}
              </div>
            </div>}
          </div>
        </div>
      </div>
      {detail && <div ref={detailViewport} className="min-h-0 flex-1 overflow-y-auto overscroll-contain" data-testid="theme-showcase-detail-viewport"
        onFocusCapture={(event) => { if (event.target.matches("button, select, input, a, textarea, summary")) detailFocus.current = focusIdentity(event.target); }}
        onScroll={(event) => { detailScroll.current = event.currentTarget.scrollTop; }}>
        <ThemeShowcaseDetail key={detail.id} theme={detail} selected={selected?.id === detail.id} selection={detailSelection} sampleId={themeSampleFor(detail, sampleId ?? filters.familyId, initialSampleId)}
          onSampleChange={setSampleId} onAppearanceChange={setDetailPreference} workspace={workspace} currentFamilyId={currentFamilyId}
          onBack={back} onApply={() => apply(detail)} canApply={facade !== null} actionError={actionError} headingRef={heading} active={open}
          previewState={previewState} onPreviewChange={setPreviewState} adoptionState={adoptionState} onAdoptionChange={setAdoptionState} />
      </div>}
    </DialogContent>
  </Dialog>;
}

function focusIdentity(element: HTMLElement): string {
  return element.dataset.themeLibraryFocus ?? JSON.stringify([
    element.tagName, element.getAttribute("aria-label"), element.textContent,
    element.matches("input") ? element.closest("label")?.textContent : null,
  ]);
}

function initialAppearance(filters: ThemeLibraryFilters): ThemeAppearancePreference {
  return filters.appearance === "light" || filters.appearance === "dark" ? filters.appearance : "follow";
}

function cardAppearance(theme: ExplorationTheme, filters: ThemeLibraryFilters, host: ThemeAppearance): ThemeAppearance {
  if (!theme.appearances) return theme.appearance;
  const preferred = filters.appearance === "light" || filters.appearance === "dark" ? filters.appearance : host;
  return theme.appearances.find((entry) => entry.appearance === preferred)?.appearance ?? theme.appearance;
}
