import { useId, useMemo, useRef, useState, type Ref } from "react";
import { ArrowLeft, ArrowRight, Check, ChevronLeft, ChevronRight, Code, Download, Maximize2, Minimize2, Move, Package, RotateCcw, X, ZoomIn, ZoomOut } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { CodePreview } from "@/src/components/CodePreview";
import { useSvgViewportController, useSvgViewportZoom, type SvgViewportController } from "@/src/components/SvgViewport";
import { downloadBlob } from "@/src/lib/export";
import type { ResolvedThemeSelection, ThemeAppearancePreference } from "@/src/lib/theme-appearance";
import { themeDesignTreatment, themeSampleFor } from "@/src/lib/theme-library";
import type { WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import type { ExplorationTheme } from "@/src/themes/catalog";
import { isThemeSampleId, type ThemeSampleId } from "@/src/themes/families";
import { samplesForTheme, THEME_SAMPLES } from "@/src/themes/samples";
import { ThemeAppearanceControl } from "./ThemeAppearanceControl";
import { ThemeExamplePreview } from "./ThemeExamplePreview";
import { ThemeSampleNote } from "./ThemeSampleNote";
import { sampleWorkspaceUrl, ThemeSampleGallery } from "./ThemeSampleGallery";
import { ThemeShowcaseUse, type ThemeShowcaseUseState } from "./ThemeShowcaseUse";

export interface ThemeShowcasePreviewState {
  readonly mode: "sample" | "current";
  readonly showSource: boolean;
}

export function ThemeShowcaseDetail({ theme, selected, selection, sampleId, onSampleChange, onAppearanceChange,
  workspace, currentFamilyId, onBack, onApply, canApply, actionError, headingRef, active, previewState, onPreviewChange, adoptionState, onAdoptionChange }: {
  readonly theme: ExplorationTheme;
  readonly selected: boolean;
  readonly selection: ResolvedThemeSelection;
  readonly sampleId: ThemeSampleId;
  onSampleChange(id: ThemeSampleId): void;
  onAppearanceChange(preference: ThemeAppearancePreference): void;
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly currentFamilyId: string | null;
  onBack(): void;
  onApply(): void;
  readonly canApply: boolean;
  readonly actionError: string | null;
  readonly headingRef: Ref<HTMLHeadingElement>;
  readonly active: boolean;
  readonly previewState: ThemeShowcasePreviewState;
  onPreviewChange(state: ThemeShowcasePreviewState): void;
  readonly adoptionState: ThemeShowcaseUseState;
  onAdoptionChange(state: ThemeShowcaseUseState): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const adoption = useRef<HTMLDivElement>(null);
  const previewTrigger = useRef<HTMLButtonElement | null>(null);
  const currentExpandButton = useRef<HTMLButtonElement>(null);
  const { mode, showSource } = previewState;
  const viewport = useSvgViewportController();
  const [expanded, setExpanded] = useState(false);
  const appearance = selection.appearance ?? theme.appearance;
  const sample = THEME_SAMPLES.find((entry) => entry.id === themeSampleFor(theme, sampleId)) ?? THEME_SAMPLES[0];
  const samples = samplesForTheme(theme);
  const workspaceUrl = useMemo(() => sampleWorkspaceUrl(theme, sample.id, appearance), [theme, sample.id, appearance]);
  const sampleIndex = samples.findIndex((entry) => entry.id === sample.id);
  const previewFamilyId = mode === "current" ? currentFamilyId : sample.id;
  const familyName = previewFamilyId === null ? t("themeLibrary.currentDiagram") : t(`diagramTypes.${previewFamilyId}`, { defaultValue: previewFamilyId });
  const treatment = themeDesignTreatment(theme, previewFamilyId);

  function selectSample(next: ThemeSampleId) {
    onSampleChange(next);
    onPreviewChange({ ...previewState, mode: "sample" });
  }

  function inspectSample(next: ThemeSampleId, trigger: HTMLButtonElement) {
    previewTrigger.current = trigger;
    onSampleChange(next);
    onPreviewChange({ mode: "sample", showSource: false });
    setExpanded(true);
  }

  function stepSample(offset: number) {
    const next = samples[(sampleIndex + offset + samples.length) % samples.length];
    if (next) selectSample(next.id);
  }

  function showAdoption() {
    adoption.current?.scrollIntoView({ block: "start", behavior: "instant" });
    adoption.current?.focus({ preventScroll: true });
  }

  const preview = <section className={cn("flex min-h-0 min-w-0 flex-col overflow-hidden bg-card", expanded ? "flex-1" : "rounded-xl border")}
    aria-label={t("themeLibrary.preview")} data-testid="theme-library-preview" data-theme-sample={mode === "sample" ? sample.id : undefined}>
    <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b bg-background px-3 py-2">
      <div className="flex min-w-0 items-center gap-2 text-xs">
        <span className="font-medium">{mode === "sample" ? t(`themeGallery.samples.${sample.id}`) : t("themeLibrary.currentDiagram")}</span>
        <span className="text-muted-foreground">· {t(`appearanceMode.${appearance}`)}</span>
      </div>
      <div className="flex max-w-full flex-wrap items-center gap-1" role="group" aria-label={t("themeLibrary.previewControls")}>
        <ThemeCanvasControls controller={viewport} disabled={showSource} />
        <span className="mx-1 h-4 border-l" aria-hidden="true" />
        <Button type="button" variant="ghost" size="icon-sm" aria-label={t(showSource ? "themeLibrary.hideSource" : "themeLibrary.showSource")}
          data-theme-library-focus="source" aria-pressed={showSource} onClick={() => onPreviewChange({ ...previewState, showSource: !showSource })}><Code className="size-4" aria-hidden="true" /></Button>
        <Button ref={expanded ? undefined : currentExpandButton} type="button" variant="ghost" size="icon-sm"
          data-theme-library-focus="expand" aria-label={t(expanded ? "themeLibrary.collapse" : "themeLibrary.expand")} onClick={(event) => {
            if (!expanded) previewTrigger.current = event.currentTarget;
            setExpanded((value) => !value);
          }}>
          {expanded ? <Minimize2 className="size-4" aria-hidden="true" /> : <Maximize2 className="size-4" aria-hidden="true" />}
        </Button>
      </div>
    </div>
    {showSource && <div className={cn("min-h-0 overflow-y-auto bg-background p-3", expanded ? "flex-1" : "h-[clamp(20rem,48dvh,36rem)]")} data-testid="theme-library-source">
      <CodePreview value={mode === "sample" ? sample.source : workspace.code} filename={mode === "sample" ? `${sample.id}.mmd` : "diagram.mmd"}
        label={t(mode === "sample" ? "themeShowcase.exampleSource" : "themeLibrary.currentSource")} />
      <details className="mt-3 rounded-md border p-3 text-xs">
        <summary className="cursor-pointer font-medium">{t("themeLibrary.diagramConfiguration")}</summary>
        <CodePreview value={mode === "sample" ? sample.configJson : workspace.mermaidConfig} filename="diagram.config.json" label={t("themeLibrary.diagramConfiguration")} />
      </details>
    </div>}
    <div hidden={showSource} className={cn("min-h-0 overflow-hidden outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring", expanded ? "flex-1" : "h-[clamp(20rem,48dvh,36rem)]")}
      tabIndex={0} role="region" aria-label={t("themeLibrary.previewCanvas")} aria-describedby={`${id}-canvas-help`} data-testid="theme-library-preview-canvas">
      <ThemeExamplePreview theme={theme} appearance={appearance} sampleId={sample.id} workspace={mode === "current" ? workspace : undefined} active={active}
        viewport={{ controller: viewport, interactionMode: expanded ? "canvas" : "embedded" }} />
    </div>
    {!showSource && <p id={`${id}-canvas-help`} className="shrink-0 border-t bg-background px-3 py-2 text-[11px] leading-relaxed text-muted-foreground">
      {t(expanded ? "themeLibrary.expandedCanvasHint" : "themeLibrary.embeddedCanvasHint")}
    </p>}
    <ThemeSampleNote theme={theme} familyId={previewFamilyId} appearance={appearance} />
    <div className="flex shrink-0 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-t bg-background px-3 py-2.5 text-xs">
      <p className="text-muted-foreground" data-theme-coverage={treatment}>
        {treatment === "dedicated" ? t("themeLibrary.designedFor", { family: familyName }) : `${t(`themeLibrary.coverage.${treatment}`)} · ${familyName}`}
      </p>
      <a href={workspaceUrl} target="_blank" rel="noopener noreferrer"
        className="inline-flex items-center gap-1 rounded-sm font-medium underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={t("themeShowcase.openSampleNamed", { sample: t(`themeGallery.samples.${sample.id}`), name: theme.name })}>
        {t("themeShowcase.openSample")}<ArrowRight className="size-3" aria-hidden="true" />
      </a>
    </div>
  </section>;

  return <div className="min-w-0 space-y-4 p-4 sm:space-y-6 sm:p-6" data-testid="theme-showcase-detail" data-theme-design={theme.id}>
    <div className="flex flex-wrap items-center justify-between gap-3">
      <Button type="button" variant="ghost" size="sm" onClick={onBack} className="-ml-2"><ArrowLeft className="size-4" aria-hidden="true" />{t("themeShowcase.back")}</Button>
      <span className="text-[11px] text-muted-foreground">{t(theme.presetId ? "appearance.sources.builtin" : "themeLibrary.customTheme")}</span>
    </div>
    <div className="flex flex-wrap items-start justify-between gap-5">
      <div className="min-w-0 max-w-2xl space-y-2">
        <div className="flex flex-wrap items-center gap-3">
          <h2 ref={headingRef} tabIndex={-1} className="text-3xl font-semibold tracking-tight outline-none sm:text-4xl">{theme.name}</h2>
          {selection.supported_appearances.length === 1 && <span className="rounded-full border px-2 py-1 text-[11px] text-muted-foreground">{t(`appearanceMode.${appearance}`)}</span>}
          {selected && <span className="inline-flex items-center gap-1 rounded-full border px-2 py-1 text-[11px] text-primary"><Check className="size-3" aria-hidden="true" />{t("themeShowcase.current")}</span>}
        </div>
        <p className="text-sm leading-relaxed text-muted-foreground">{t(`themeGallery.themes.${theme.id}`)}</p>
        <p className="hidden text-xs leading-relaxed text-muted-foreground sm:block">{t(`themeShowcase.mechanisms.${theme.id}`)}</p>
      </div>
      <div className="flex w-full max-w-full flex-wrap items-center gap-2 sm:w-auto">
        <Button type="button" size="sm" className="px-2 text-[11px] sm:px-3 sm:text-xs" disabled={!canApply || !!selection.error} onClick={onApply}>
          {selected && <Check className="size-3.5" aria-hidden="true" />}{t("themeShowcase.apply")}
        </Button>
        <Button type="button" variant="outline" size="sm" className="px-2 text-[11px] sm:px-3 sm:text-xs" onClick={() => downloadBlob(new Blob([theme.recipeJson], { type: "application/json;charset=utf-8" }), `${theme.id}.theme.json`)}>
          <Download className="hidden size-3.5 sm:block" aria-hidden="true" />{t("themeShowcase.downloadJson")}
        </Button>
        <Button type="button" variant="outline" size="sm" className="px-2 text-[11px] sm:px-3 sm:text-xs" onClick={showAdoption}><Package className="hidden size-3.5 sm:block" aria-hidden="true" />{t("themeLibrary.useInProject")}</Button>
      </div>
    </div>
    {actionError && <p role="alert" className="break-words text-sm text-destructive">{actionError}</p>}
    <div className="flex min-w-0 flex-wrap items-start justify-between gap-3 border-b pb-4">
      <div className="min-w-0 flex-1 space-y-2">
        <fieldset className="flex max-w-full flex-wrap gap-1">
          <legend className="sr-only">{t("themeLibrary.previewInput")}</legend>
          {(["sample", "current"] as const).map((value) => <Button key={value} type="button" size="sm" variant={mode === value ? "secondary" : "ghost"}
            className="text-xs" aria-pressed={mode === value} onClick={() => onPreviewChange({ ...previewState, mode: value })}>
            {t(value === "sample" ? "themeLibrary.sampleDiagram" : "themeLibrary.currentDiagram")}
          </Button>)}
        </fieldset>
        {mode === "current" && <p className="max-w-2xl text-xs leading-relaxed text-muted-foreground">{t("themeLibrary.currentPreviewHint")}</p>}
      </div>
      {selection.supported_appearances.length > 1 && <ThemeAppearanceControl selection={selection} onChange={onAppearanceChange} />}
      {selection.error && selection.supported_appearances.length <= 1 && <p role="alert" className="text-xs text-destructive">{selection.error}</p>}
    </div>
    {mode === "sample" ? <ThemeSampleGallery theme={theme} appearance={appearance} selectedSampleId={sample.id} active={active && !expanded}
      onInspect={inspectSample} onUse={(next) => { selectSample(next); showAdoption(); }} /> : !expanded && preview}
    <Dialog open={expanded} onOpenChange={setExpanded}>
      <DialogContent safeArea={false} showCloseButton={false} className="flex h-[100dvh] w-screen max-w-none flex-col gap-0 rounded-none border-0 p-0 sm:max-w-none"
        style={{ paddingTop: "env(safe-area-inset-top)", paddingRight: "env(safe-area-inset-right)", paddingBottom: "env(safe-area-inset-bottom)", paddingLeft: "env(safe-area-inset-left)" }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          requestAnimationFrame(() => {
            const trigger = previewTrigger.current?.isConnected ? previewTrigger.current : currentExpandButton.current;
            trigger?.focus({ preventScroll: true });
          });
        }} data-testid="theme-library-expanded-preview">
        <div className="flex shrink-0 items-center justify-between gap-3 border-b px-4 py-3">
          <DialogTitle className="text-sm">{t("themeLibrary.expandedTitle", { name: theme.name })}</DialogTitle>
          <DialogDescription className="sr-only">{t("themeLibrary.previewCanvas")}</DialogDescription>
          <Button type="button" variant="ghost" size="icon-sm" aria-label={t("themeLibrary.closeExpanded")} onClick={() => setExpanded(false)}><X className="size-4" aria-hidden="true" /></Button>
        </div>
        {mode === "sample" && <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b bg-background px-4 py-2.5">
          <label htmlFor={`${id}-sample`} className="flex min-w-0 flex-wrap items-center gap-2 text-xs">
            <span className="font-medium">{t("themeLibrary.previewFamily")}</span>
            <Select value={sample.id} onValueChange={(value) => { if (isThemeSampleId(value)) selectSample(value); }}>
              <SelectTrigger id={`${id}-sample`} size="sm" aria-label={t("themeLibrary.previewFamily")} data-value={sample.id} className="min-w-0 max-w-full"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectGroup>{samples.map((entry) => <SelectItem key={entry.id} value={entry.id} data-value={entry.id}>{t(`themeGallery.samples.${entry.id}`)}</SelectItem>)}</SelectGroup>
              </SelectContent>
            </Select>
          </label>
          <div className="flex items-center gap-1">
            <span className="mr-2 text-[11px] tabular-nums text-muted-foreground">{sampleIndex + 1} / {samples.length}</span>
            <Button type="button" variant="outline" size="icon-sm" aria-label={t("themeLibrary.previousSample")} onClick={() => stepSample(-1)}><ChevronLeft className="size-4" aria-hidden="true" /></Button>
            <Button type="button" variant="outline" size="icon-sm" aria-label={t("themeLibrary.nextSample")} onClick={() => stepSample(1)}><ChevronRight className="size-4" aria-hidden="true" /></Button>
          </div>
        </div>}
        {expanded && preview}
      </DialogContent>
    </Dialog>
    <div ref={adoption} tabIndex={-1} className="min-w-0 scroll-mt-4 rounded-xl outline-none focus-visible:ring-2 focus-visible:ring-ring">
      <ThemeShowcaseUse theme={theme} appearance={appearance} sampleId={sample.id} onSampleChange={selectSample} state={adoptionState} onStateChange={onAdoptionChange} />
    </div>
  </div>;
}

function ThemeCanvasControls({ controller, disabled }: { readonly controller: SvgViewportController; readonly disabled: boolean }) {
  const { t } = useTranslation();
  const zoom = useSvgViewportZoom(controller);
  const percent = zoom * 100;
  const label = percent < 10 ? Number(percent.toFixed(1)) : Math.round(percent);
  return <>
    <Button type="button" variant="ghost" size="icon-sm" aria-label={t("themeLibrary.zoomOut")} disabled={disabled}
      onClick={controller.zoomOut}><ZoomOut className="size-4" aria-hidden="true" /></Button>
    <output className="w-10 text-center text-[11px] tabular-nums" aria-label={t("themeLibrary.zoomLevel")}>{label}%</output>
    <Button type="button" variant="ghost" size="icon-sm" aria-label={t("themeLibrary.zoomIn")} disabled={disabled || zoom >= 5}
      onClick={controller.zoomIn}><ZoomIn className="size-4" aria-hidden="true" /></Button>
    <Button type="button" variant="ghost" size="sm" className="h-8 text-xs" disabled={disabled} onClick={controller.fitToView}>
      <Move className="size-3.5" aria-hidden="true" />{t("themeLibrary.fit")}
    </Button>
    <Button type="button" variant="ghost" size="icon-sm" disabled={disabled} aria-label={t("preview.reset")} onClick={controller.reset}>
      <RotateCcw className="size-3.5" aria-hidden="true" />
    </Button>
  </>;
}
