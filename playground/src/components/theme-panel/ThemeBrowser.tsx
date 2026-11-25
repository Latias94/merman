import { useCallback, useMemo, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemeCatalog, ThemePresetCatalogEntry } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { ThemeAppearancePreference } from "@/src/lib/theme-appearance";
import { retryMermanRuntime } from "@/src/runtime/merman";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { selectMermanFacade, selectMermanFailure, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { useAppStore } from "@/src/store";
import { ThemeBrowserPreview } from "./ThemeBrowserPreview";

export default function ThemeBrowser({ linkedPreset, onOpenChange, restoreFocus }: {
  linkedPreset: string | null;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
}) {
  const { t } = useTranslation();
  const title = useRef<HTMLHeadingElement>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const failure = useMermanRuntime(selectMermanFailure);
  const catalog = useMemo(() => facade?.themeCatalog(), [facade]);

  return <Dialog open onOpenChange={onOpenChange}>
    <DialogContent safeArea={false} showCloseButton={false}
      className="flex h-dvh min-h-0 w-screen max-w-none flex-col gap-0 overflow-hidden rounded-none border-0 p-0 sm:h-[min(85dvh,46rem)] sm:w-[calc(100vw-2rem)] sm:max-w-5xl sm:rounded-lg sm:border"
      style={{ paddingTop: "env(safe-area-inset-top)", paddingRight: "env(safe-area-inset-right)", paddingBottom: "env(safe-area-inset-bottom)", paddingLeft: "env(safe-area-inset-left)" }}
      onOpenAutoFocus={(event) => { event.preventDefault(); title.current?.focus(); }}
      onCloseAutoFocus={(event) => { event.preventDefault(); restoreFocus(); }}>
      <DialogHeader className="flex-row items-center justify-between gap-3 border-b px-4 py-3 sm:px-5">
        <DialogTitle ref={title} tabIndex={-1} className="outline-none">{t("themeBrowser.title")}</DialogTitle>
        <DialogDescription className="sr-only">{t("themeBrowser.description")}</DialogDescription>
        <DialogClose asChild><Button variant="ghost" size="icon-sm" aria-label={t("themeBrowser.close")}><X aria-hidden="true" /></Button></DialogClose>
      </DialogHeader>
      {failure ? <div className="space-y-3 p-5">
        <p role="alert" className="break-words text-sm text-destructive">{failure.message}</p>
        <Button variant="outline" size="sm" onClick={() => {
          if (failure.recovery === "reload") window.location.reload();
          else void retryMermanRuntime().catch(() => undefined);
        }}>{t(failure.recovery === "reload" ? "wasm.reload" : "wasm.retry")}</Button>
      </div> : catalog && facade ? <BrowsePresets catalog={catalog} facade={facade} linkedPreset={linkedPreset} onApply={() => onOpenChange(false)} />
        : <p role="status" className="p-5 text-sm text-muted-foreground">{t("themeWorkshop.loading")}</p>}
    </DialogContent>
  </Dialog>;
}

function BrowsePresets({ catalog, facade, linkedPreset, onApply }: {
  catalog: ThemeCatalog;
  facade: MermanDomainFacade;
  linkedPreset: string | null;
  onApply(): void;
}) {
  const { t } = useTranslation();
  const hostAppearance = useAppStore((state) => state.resolvedTheme);
  const [query, setQuery] = useState("");
  const revealSelected = useCallback((button: HTMLButtonElement | null) => {
    button?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, []);
  const [selection, setSelection] = useState<{ id?: string; preference: ThemeAppearancePreference }>(() => {
    const { themePresetId, themeAppearance } = useAppStore.getState();
    const current = catalog.presets.find(({ id }) => id === themePresetId?.trim());
    const id = linkedPreset ?? current?.id ?? catalog.presets[0]?.id;
    return { id, preference: current && id === current.id ? themeAppearance : "default" };
  });
  const preset = catalog.presets.find(({ id }) => id === selection.id);
  const preferred = selection.preference === "follow" ? hostAppearance : selection.preference;
  const appearance = preset?.appearances.find((mode) => mode === preferred) ?? preset?.default_appearance;
  const search = query.trim().toLowerCase();
  const description = (entry: ThemePresetCatalogEntry) => t(`themeWorkshop.presetDescriptions.${entry.id}`, { defaultValue: entry.description });
  const matches = catalog.presets.filter((entry) => [entry.id, entry.name, entry.description, description(entry), t(`themeWorkshop.categories.${entry.category}`)]
    .some((text) => text.toLowerCase().includes(search)));

  return <div className="grid min-h-0 min-w-0 flex-1 grid-cols-1 grid-rows-[auto_minmax(0,1fr)] sm:grid-cols-[14rem_minmax(0,1fr)] sm:grid-rows-1 lg:grid-cols-[16rem_minmax(0,1fr)]">
    <aside className="flex min-h-0 min-w-0 flex-col border-b bg-muted/20 sm:border-r sm:border-b-0" aria-label={t("themeWorkshop.gallery")}>
      <div className="relative m-2 sm:m-3">
        <Search aria-hidden="true" className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} aria-label={t("themeBrowser.search")}
          placeholder={t("themeBrowser.search")} className="h-8 bg-background pl-8" />
      </div>
      <div className="flex min-w-0 gap-1 overflow-x-auto overscroll-contain px-2 pb-2 sm:flex-1 sm:flex-col sm:overflow-x-hidden sm:overflow-y-auto sm:px-3 sm:pb-3">
        {matches.map((entry) => <button type="button" key={entry.id} ref={preset?.id === entry.id ? revealSelected : undefined}
          data-browser-preset={entry.id} aria-pressed={preset?.id === entry.id}
          className="flex min-w-24 shrink-0 flex-col gap-1.5 rounded-md border border-transparent px-2.5 py-2 text-left hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring aria-pressed:border-primary/50 aria-pressed:bg-background"
          onClick={() => { if (selection.id !== entry.id) setSelection({ id: entry.id, preference: "default" }); }}>
          <span className="text-sm font-medium">{entry.name}</span>
          <span aria-hidden="true" className="flex h-1.5 w-full overflow-hidden rounded-sm ring-1 ring-border">
            {entry.swatches.map((color, index) => <span key={index} className="flex-1" style={{ backgroundColor: color }} />)}
          </span>
        </button>)}
        {matches.length === 0 && <p role="status" className="col-span-full p-2 text-xs text-muted-foreground">{t("themeBrowser.noMatches")}</p>}
      </div>
    </aside>
    {preset && appearance ? <section className="min-h-0 min-w-0 space-y-4 overflow-y-auto overscroll-contain p-4 sm:p-6" aria-label={preset.name}>
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <h2 data-testid="theme-browser-name" className="text-2xl font-semibold tracking-tight sm:text-3xl">{preset.name}</h2>
        <Button size="sm" onClick={() => {
          const preference = selection.preference;
          useAppStore.getState().selectTheme({ kind: "preset", presetId: preset.id,
            appearance: preference === "follow" || preference === "default" ? preference : appearance });
          onApply();
        }}>{t("themeBrowser.apply")}</Button>
      </div>
      <p className="text-sm leading-relaxed text-muted-foreground">{description(preset)}</p>
      {preset.appearances.length > 1 && <div role="group" aria-label={t("appearanceMode.title")} className="flex gap-1">
        {preset.appearances.map((mode) => <Button key={mode} size="sm" variant={appearance === mode ? "secondary" : "ghost"}
          data-browser-appearance={mode} aria-pressed={appearance === mode} onClick={() => setSelection({ id: preset.id, preference: mode })}>
          {t(`appearanceMode.${mode}`)}
        </Button>)}
      </div>}
      <ThemeBrowserPreview facade={facade} presetId={preset.id} appearance={appearance} />
    </section> : <p role="status" className="p-5 text-sm text-muted-foreground">
      {linkedPreset ? t("themeBrowser.unavailable", { id: linkedPreset }) : t("themeBrowser.noMatches")}
    </p>}
  </div>;
}
