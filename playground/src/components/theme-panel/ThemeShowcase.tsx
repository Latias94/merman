import { useId, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, Check, Download, Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { downloadBlob } from "@/src/lib/export";
import { encodeShareHash } from "@/src/lib/share";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { DEFAULT_WORKSPACE_SNAPSHOT, selectWorkspaceSnapshot, useAppStore } from "@/src/store";
import { EXPLORATION_THEMES, type ExplorationTheme } from "@/src/themes/catalog";
import { THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";
import { ThemeShowcaseUse } from "./ThemeShowcaseUse";

type AppearanceFilter = "all" | "light" | "dark";

export default function ThemeShowcase({ open, onOpenChange, restoreFocus, initialThemeId }: {
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
  readonly initialThemeId?: string;
}) {
  const { t, i18n } = useTranslation();
  const searchId = useId();
  const search = useRef<HTMLInputElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const gridScroll = useRef(0);
  const returnCard = useRef<string | null>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const [query, setQuery] = useState("");
  const [appearance, setAppearance] = useState<AppearanceFilter>("all");
  const [detailId, setDetailId] = useState<string | null>(initialThemeId ?? null);
  const [sampleId, setSampleId] = useState<ThemeSampleId>("flowchart");
  const [actionError, setActionError] = useState<string | null>(null);
  const detail = EXPLORATION_THEMES.find((theme) => theme.id === detailId);
  const selected = EXPLORATION_THEMES.find((theme) => theme.recipeJson === workspace.themeRecipeJson);
  const normalizedQuery = query.trim().toLocaleLowerCase(i18n.resolvedLanguage);
  const themes = EXPLORATION_THEMES.filter((theme) =>
    (appearance === "all" || appearance === theme.appearance) &&
    `${theme.id} ${theme.name} ${t(`themeGallery.themes.${theme.id}`)} ${t(`themeShowcase.mechanisms.${theme.id}`)}`
      .toLocaleLowerCase(i18n.resolvedLanguage).includes(normalizedQuery));

  function explore(theme: ExplorationTheme) {
    gridScroll.current = viewport.current?.scrollTop ?? 0;
    returnCard.current = theme.id;
    setDetailId(theme.id);
    setActionError(null);
    if (viewport.current) viewport.current.scrollTop = 0;
    requestAnimationFrame(() => heading.current?.focus());
  }

  function back() {
    setDetailId(null);
    setActionError(null);
    requestAnimationFrame(() => {
      if (viewport.current) viewport.current.scrollTop = gridScroll.current;
      const card = returnCard.current && viewport.current?.querySelector<HTMLButtonElement>(`[data-explore-theme="${returnCard.current}"]`);
      if (card) card.focus({ preventScroll: true });
      else search.current?.focus();
    });
  }

  function apply(theme: ExplorationTheme) {
    try {
      if (!facade) return;
      facade.validateThemeRecipe(theme.recipeJson);
      useAppStore.getState().selectTheme({ kind: "recipe", recipeJson: theme.recipeJson, label: { name: theme.name, source: "exploration" } });
      setActionError(null);
      onOpenChange(false);
    } catch (error) { setActionError(projectError(error).summary); }
  }

  return <Dialog open={open} onOpenChange={onOpenChange}>
    <DialogContent safeArea={false} showCloseButton={false} data-testid="theme-showcase"
      className="flex h-[100dvh] w-screen max-w-none flex-col gap-0 overflow-hidden rounded-none border-0 p-0 sm:h-[min(94dvh,70rem)] sm:w-[calc(100vw-2rem)] sm:max-w-[94rem] sm:rounded-lg sm:border"
      style={{ paddingTop: "env(safe-area-inset-top)", paddingRight: "env(safe-area-inset-right)", paddingBottom: "env(safe-area-inset-bottom)", paddingLeft: "env(safe-area-inset-left)" }}
      onEscapeKeyDown={(event) => {
        if (!detail && query && document.activeElement === search.current) {
          event.preventDefault();
          setQuery("");
        }
      }}
      onOpenAutoFocus={(event) => { event.preventDefault(); if (detail) heading.current?.focus(); else search.current?.focus(); }}
      onCloseAutoFocus={(event) => { event.preventDefault(); restoreFocus(); }}>
      <DialogHeader className="relative shrink-0 border-b px-4 py-4 pr-14 text-left sm:px-6">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <DialogTitle>{t("themeShowcase.title")}</DialogTitle>
          <span className="rounded-full border px-2 py-0.5 text-[10px] font-medium tracking-wide text-muted-foreground">{t("themeShowcase.recipeBadge")}</span>
        </div>
        <DialogDescription className="max-w-3xl text-xs leading-relaxed sm:text-sm">{t("themeShowcase.description")}</DialogDescription>
        <DialogClose asChild><Button type="button" variant="ghost" size="icon" className="absolute right-3 top-3" aria-label={t("themeShowcase.close")}><X className="size-5" aria-hidden="true" /></Button></DialogClose>
      </DialogHeader>
      {!detail && <div className="flex shrink-0 flex-wrap items-center gap-3 border-b bg-muted/15 px-4 py-3 sm:px-6">
        <div className="relative min-w-40 flex-1 sm:max-w-md">
          <label htmlFor={searchId} className="sr-only">{t("themeShowcase.search")}</label>
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
          <Input ref={search} id={searchId} type="search" placeholder={t("themeShowcase.search")} value={query} className="pl-9"
            onChange={(event) => { setQuery(event.target.value); if (viewport.current) viewport.current.scrollTop = 0; }} />
        </div>
        <fieldset className="flex gap-1"><legend className="sr-only">{t("themeShowcase.appearance")}</legend>
          {(["all", "light", "dark"] as const).map((filter) => <Button key={filter} type="button" size="sm" variant={filter === appearance ? "secondary" : "ghost"}
            aria-pressed={filter === appearance} onClick={() => { setAppearance(filter); if (viewport.current) viewport.current.scrollTop = 0; }}>{t(`themeShowcase.filters.${filter}`)}</Button>)}
        </fieldset>
        <p role="status" className="text-xs text-muted-foreground">{t("themeShowcase.count", { count: themes.length })}</p>
      </div>}
      <div ref={viewport} className="min-h-0 flex-1 overflow-y-auto overscroll-contain" data-testid="theme-showcase-viewport">
        {detail ? <div className="space-y-7 p-4 sm:p-6" data-testid="theme-showcase-detail" data-theme-design={detail.id}>
          <div className="space-y-4">
            <Button type="button" variant="ghost" size="sm" onClick={back} className="-ml-2"><ArrowLeft className="size-4" aria-hidden="true" />{t("themeShowcase.back")}</Button>
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="min-w-0 max-w-2xl space-y-2">
                <h2 ref={heading} tabIndex={-1} className="text-3xl font-semibold tracking-tight outline-none">{detail.name}</h2>
                {selected?.id === detail.id && <p className="text-xs font-medium text-primary">{t("themeShowcase.current")}</p>}
                <p className="text-sm leading-relaxed text-muted-foreground">{t(`themeGallery.themes.${detail.id}`)}</p>
                <p className="text-xs font-medium text-muted-foreground">{t(`themeShowcase.mechanisms.${detail.id}`)}</p>
              </div>
              <div className="flex max-w-full flex-wrap items-center gap-2">
                <Button type="button" variant="outline" size="sm" onClick={() => downloadTheme(detail)}><Download className="size-3.5" aria-hidden="true" />{t("themeShowcase.downloadJson")}</Button>
                <Button type="button" size="sm" disabled={!facade} onClick={() => apply(detail)}>
                  {selected?.id === detail.id && <Check className="size-3.5" aria-hidden="true" />}{t("themeShowcase.apply")}
                </Button>
              </div>
            </div>
            <p className="text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.applyHint")}</p>
            {actionError && <p role="alert" className="break-words text-sm text-destructive">{actionError}</p>}
          </div>
          <section aria-labelledby={`${searchId}-examples`} className="space-y-3">
            <h3 id={`${searchId}-examples`} className="text-sm font-semibold">{t("themeShowcase.acrossDiagrams")}</h3>
            <p className="text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.fixedExamples")}</p>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
              {THEME_SAMPLES.map((sample) => <article key={sample.id} className="min-w-0 overflow-hidden rounded-md border bg-card" data-theme-sample={sample.id}>
                <div className="flex items-center justify-between gap-2 border-b px-3 py-2">
                  <h4 className="text-xs font-medium">{t(`themeGallery.samples.${sample.id}`)}</h4>
                  <a href={sampleWorkspaceUrl(detail, sample.id)} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-1 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                    aria-label={t("themeShowcase.openSampleNamed", { sample: t(`themeGallery.samples.${sample.id}`), name: detail.name })}>
                    {t("themeShowcase.openSample")}<ArrowRight className="size-3" aria-hidden="true" />
                  </a>
                </div>
                <div className="flex h-72 items-center justify-center p-4" style={{ backgroundColor: detail.colors[0] }}>
                  <img src={detail.previews[sample.id]} alt={t("appearance.exampleAlt", { name: detail.name, sample: t(`themeGallery.samples.${sample.id}`) })}
                    loading="lazy" className="h-full w-full min-w-0 object-contain" />
                </div>
              </article>)}
            </div>
          </section>
          <ThemeShowcaseUse key={detail.id} theme={detail} sampleId={sampleId} onSampleChange={setSampleId} />
        </div> : <div className="space-y-5 p-4 sm:p-6">
          <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
            <h2 className="text-2xl font-semibold tracking-tight">{t("themeShowcase.collectionTitle")}</h2>
            <p className="max-w-2xl text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.collectionHint")}</p>
          </div>
          {themes.length > 0 ? <div className="grid grid-cols-1 gap-5 md:grid-cols-2 xl:grid-cols-3" data-testid="theme-showcase-grid">
            {themes.map((theme) => <ThemeDesignCard key={theme.id} theme={theme} selected={theme.id === selected?.id} onExplore={() => explore(theme)} />)}
          </div> : <p role="status" className="py-16 text-center text-sm text-muted-foreground">{t("appearance.noThemes")}</p>}
        </div>}
      </div>
    </DialogContent>
  </Dialog>;
}

function ThemeDesignCard({ theme, selected, onExplore }: { theme: ExplorationTheme; selected: boolean; onExplore(): void }) {
  const { t } = useTranslation();
  const currentId = useId();
  return <article className={cn("min-w-0 overflow-hidden rounded-lg border bg-card", selected && "border-primary")} data-theme-design-card={theme.id}>
    <button type="button" onClick={onExplore} data-explore-theme={theme.id} aria-label={t("themeShowcase.exploreNamed", { name: theme.name })}
      aria-describedby={selected ? currentId : undefined}
      className="block w-full min-w-0 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring">
      <div className="space-y-2 px-4 pb-4 pt-4">
        <div className="flex items-center justify-between gap-3"><h3 className="text-xl font-semibold tracking-tight">{theme.name}</h3>
          {selected ? <><Check className="size-4 text-primary" aria-hidden="true" /><span id={currentId} className="sr-only">{t("themeShowcase.current")}</span></>
            : <span className="text-[10px] text-muted-foreground">{t(`themeShowcase.filters.${theme.appearance}`)}</span>}
        </div>
        <p className="min-h-8 text-xs leading-relaxed text-muted-foreground">{t(`themeShowcase.mechanisms.${theme.id}`)}</p>
      </div>
      <div className="grid h-72 grid-cols-[minmax(0,1.05fr)_minmax(0,1fr)] gap-px overflow-hidden bg-border" aria-hidden="true">
        <div className="row-span-2 flex min-h-0 min-w-0 items-center justify-center p-3" style={{ backgroundColor: theme.colors[0] }}>
          <img src={theme.previews.flowchart} alt="" loading="lazy" className="h-full w-full object-contain" />
        </div>
        {(["sequence", "pie"] as const).map((family) => <div key={family} className="flex min-h-0 min-w-0 items-center justify-center p-2" style={{ backgroundColor: theme.colors[0] }}>
          <img src={theme.previews[family]} alt="" loading="lazy" className="h-full w-full object-contain" />
        </div>)}
      </div>
      <div className="flex min-w-0 items-center justify-between gap-3 border-t px-4 py-3">
        <span className="text-xs text-muted-foreground">{t("themeShowcase.fiveDiagrams")}</span>
        <span className="inline-flex items-center gap-1 text-xs font-medium">{t("themeShowcase.explore")}<ArrowRight className="size-3.5" aria-hidden="true" /></span>
      </div>
    </button>
  </article>;
}

function downloadTheme(theme: ExplorationTheme) {
  downloadBlob(new Blob([theme.recipeJson], { type: "application/json;charset=utf-8" }), `${theme.id}.theme.json`);
}

function sampleWorkspaceUrl(theme: ExplorationTheme, sampleId: ThemeSampleId): string {
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId)!;
  return `${import.meta.env.BASE_URL}${encodeShareHash({ ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: sample.source, mermaidConfig: sample.configJson, diagramTheme: "default", diagramFont: "inherit",
    themePresetId: null, themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: theme.name, source: "exploration" },
  })}`;
}
