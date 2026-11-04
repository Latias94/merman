import { useId, useRef, useState } from "react";
import { ArrowRight, Download, Palette, Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { downloadBlob } from "@/src/lib/export";
import { EXPLORATION_THEMES } from "@/src/themes/catalog";
import { THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";

export default function ThemeGallery({
  open,
  onOpenChange,
  restoreFocus,
  canTry,
  onTryRecipe,
}: {
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
  readonly canTry: boolean;
  onTryRecipe(recipeJson: string): void;
}) {
  const { t } = useTranslation();
  const searchId = useId();
  const searchRef = useRef<HTMLInputElement>(null);
  const detailRef = useRef<HTMLElement>(null);
  const [query, setQuery] = useState("");
  const [appearance, setAppearance] = useState("all");
  const [selectedId, setSelectedId] = useState(EXPLORATION_THEMES[0].id);
  const [sampleId, setSampleId] = useState<ThemeSampleId>("flowchart");
  const selected = EXPLORATION_THEMES.find((theme) => theme.id === selectedId)!;
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId)!;
  const visibleThemes = EXPLORATION_THEMES.filter((theme) =>
    (appearance === "all" || theme.appearance === appearance) &&
    `${theme.name} ${t(`themeGallery.themes.${theme.id}`)}`.toLocaleLowerCase()
      .includes(query.trim().toLocaleLowerCase()));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        safeArea={false}
        showCloseButton={false}
        className="grid h-[100dvh] w-screen max-w-none grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden rounded-none border-0 p-0 sm:h-[min(92dvh,57rem)] sm:w-[calc(100vw-2rem)] sm:max-w-[78rem] sm:rounded-lg sm:border"
        style={{
          paddingTop: "env(safe-area-inset-top)",
          paddingRight: "env(safe-area-inset-right)",
          paddingBottom: "env(safe-area-inset-bottom)",
          paddingLeft: "env(safe-area-inset-left)",
        }}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          searchRef.current?.focus();
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          restoreFocus();
        }}
        onEscapeKeyDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "Escape") event.stopPropagation();
        }}
      >
        <DialogHeader className="relative border-b bg-muted/15 px-4 py-5 pr-14 text-left sm:px-6">
          <p className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-widest text-muted-foreground">
            <Palette className="size-3.5" aria-hidden="true" />{t("themeGallery.exploration")}
          </p>
          <DialogTitle className="text-2xl font-semibold tracking-tight">{t("themeGallery.title")}</DialogTitle>
          <DialogDescription className="max-w-2xl text-xs leading-relaxed sm:text-sm">{t("themeGallery.description")}</DialogDescription>
          <DialogClose asChild>
            <Button type="button" variant="ghost" size="icon-sm" className="absolute right-3 top-3" aria-label={t("themeGallery.close")}>
              <X className="size-4" />
            </Button>
          </DialogClose>
        </DialogHeader>
        <div className="min-h-0 overflow-y-auto overscroll-contain lg:grid lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] lg:overflow-hidden">
          <section className="min-w-0 space-y-4 border-b p-4 sm:p-6 lg:min-h-0 lg:overflow-y-auto lg:border-b-0 lg:border-r" aria-label={t("themeGallery.collection")}>
            <div className="flex flex-wrap items-center gap-2">
              <div className="relative min-w-36 flex-1">
                <label htmlFor={searchId} className="sr-only">{t("themeGallery.search")}</label>
                <Search className="pointer-events-none absolute left-2.5 top-2.5 size-3.5 text-muted-foreground" aria-hidden="true" />
                <input ref={searchRef} id={searchId} type="search" value={query} onChange={(event) => setQuery(event.target.value)}
                  placeholder={t("themeGallery.search")} className="h-9 w-full min-w-0 rounded-md border bg-background pl-8 pr-2 text-xs" />
              </div>
              <label className="sr-only" htmlFor={`${searchId}-appearance`}>{t("themeGallery.appearance")}</label>
              <select id={`${searchId}-appearance`} value={appearance} onChange={(event) => setAppearance(event.target.value)}
                className="h-9 rounded-md border bg-background px-2 text-xs">
                {(["all", "light", "dark"] as const).map((value) => <option key={value} value={value}>{t(`themeGallery.${value}`)}</option>)}
              </select>
            </div>
            <div className="grid grid-cols-1 gap-3 min-[360px]:grid-cols-2">
              {visibleThemes.map((theme) => <button key={theme.id} type="button" aria-pressed={selectedId === theme.id}
                data-theme-exploration={theme.id} onClick={() => {
                  setSelectedId(theme.id);
                  if (window.matchMedia("(max-width: 1023px)").matches) detailRef.current?.scrollIntoView({ block: "start" });
                }}
                className={cn("group min-w-0 overflow-hidden rounded-lg border bg-background text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                  selectedId === theme.id ? "border-foreground/60 ring-1 ring-foreground/30" : "border-border hover:border-foreground/35")}>
                <div className="flex aspect-[4/3] items-center justify-center overflow-hidden border-b p-2" style={{ backgroundColor: theme.colors[0] }}>
                  <img src={theme.previews.xychart} alt="" loading="lazy" className="size-full object-contain" />
                </div>
                <div className="space-y-2 p-3">
                  <div className="flex flex-wrap items-center justify-between gap-1">
                    <span className="text-sm font-semibold tracking-tight">{theme.name}</span>
                    <span className="text-[10px] text-muted-foreground">{t(`themeGallery.${theme.appearance}`)}</span>
                  </div>
                  <p className="min-h-12 text-[11px] leading-relaxed text-muted-foreground">{t(`themeGallery.themes.${theme.id}`)}</p>
                  <div className="flex h-1.5 overflow-hidden rounded-full" aria-hidden="true">
                    {theme.colors.map((color, index) => <span key={index} className="flex-1" style={{ backgroundColor: color }} />)}
                  </div>
                </div>
              </button>)}
            </div>
            {visibleThemes.length === 0 && <p role="status" className="rounded-md border border-dashed px-4 py-8 text-center text-xs text-muted-foreground">{t("themeGallery.noResults")}</p>}
            <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeGallery.explorationHint")}</p>
          </section>
          <section ref={detailRef} className="flex min-w-0 flex-col lg:min-h-0" aria-label={t("themeGallery.detail", { name: selected.name })} data-testid="theme-gallery-detail">
            <div className="min-h-0 flex-1 space-y-4 p-4 sm:p-6 lg:overflow-y-auto">
              <div className="space-y-1">
                <h2 className="text-xl font-semibold tracking-tight">{selected.name}</h2>
                <p className="text-xs leading-relaxed text-muted-foreground">{t(`themeGallery.themes.${selected.id}`)}</p>
              </div>
              <fieldset className="flex flex-wrap gap-1.5">
                <legend className="sr-only">{t("themeGallery.sample")}</legend>
                {THEME_SAMPLES.map((entry) => <Button key={entry.id} type="button" size="sm" variant={sampleId === entry.id ? "secondary" : "ghost"}
                  aria-pressed={sampleId === entry.id} className="h-7 px-2 text-[11px]" onClick={() => setSampleId(entry.id)}>
                  {t(`themeGallery.samples.${entry.id}`)}
                </Button>)}
              </fieldset>
              <div className="flex min-h-64 items-center justify-center overflow-hidden rounded-lg border p-3 sm:min-h-80" style={{ backgroundColor: selected.colors[0] }}>
                <img key={`${selected.id}-${sampleId}`} src={selected.previews[sampleId]}
                  alt={t("themeGallery.previewAlt", { name: selected.name, sample: t(`themeGallery.samples.${sampleId}`) })}
                  className="max-h-[48dvh] w-full object-contain" data-testid="theme-gallery-preview" />
              </div>
              <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeGallery.previewHint")}</p>
              <details className="rounded-md border text-xs" key={sampleId}>
                <summary className="cursor-pointer px-3 py-2 font-medium">{t("themeGallery.sampleSource")}</summary>
                <div className="space-y-3 border-t px-3 py-3">
                  <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px]">{sample.source}</pre>
                  <p className="font-medium">{t("themeGallery.sampleConfig")}</p>
                  <pre className="overflow-auto whitespace-pre-wrap break-words font-mono text-[11px]">{sample.configJson}</pre>
                </div>
              </details>
            </div>
            <div className="space-y-3 border-t bg-muted/10 p-4 sm:px-6">
              <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeGallery.tryHint")}</p>
              <div className="flex flex-wrap items-center justify-between gap-2">
                <Button type="button" variant="outline" size="sm" onClick={() =>
                  downloadBlob(new Blob([selected.recipeJson], { type: "application/json;charset=utf-8" }), `${selected.id}.theme.json`)}>
                  <Download className="size-3.5" />{t("themeGallery.download", { name: selected.name })}
                </Button>
                <Button type="button" size="sm" disabled={!canTry} onClick={() => onTryRecipe(selected.recipeJson)}>
                  {t("themeGallery.try")}<ArrowRight className="size-3.5" />
                </Button>
              </div>
            </div>
          </section>
        </div>
      </DialogContent>
    </Dialog>
  );
}
