import { lazy, useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { Copy, Download, FileUp, Palette, Pencil, Undo2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { normalizeThemeName, SUPPORTED_THEMES, type ThemeCatalog } from "@mermanjs/web";
import { useShallow } from "zustand/react/shallow";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { CodePreview } from "@/src/components/CodePreview";
import type { DiagramThemeChoice } from "@/src/lib/diagram-appearance";
import { downloadBlob } from "@/src/lib/export";
import { DIAGRAM_FONT_VALUES, isDiagramFont } from "@/src/lib/diagram-font";
import { isMermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import { SHARE_THEME_RECIPE_BYTES } from "@/src/lib/share";
import { currentAppearanceOverrides, currentDiagramDetection, currentThemeRender, detectedThemeFamily, reviewedThemeFamilies, themeFamilyMetadataId, themeFamilyTreatment } from "@/src/lib/theme-discovery";
import { exceedsUtf8ByteBudget } from "@/src/lib/utf8";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";
import { ThemeRenderEvidence } from "./ThemeRenderEvidence";
import { ThemeApplicationNotes } from "./ThemeApplicationNotes";
import { importedThemeName } from "./theme-selection";
import { exportSelectedThemeRecipe, inspectThemeRecipe, preferredAppearanceForSelection, resolveThemeSelection, selectedPresetCatalogEntry, type ThemeAppearancePreference } from "@/src/lib/theme-appearance";
import { sameThemeEditOrigin } from "@/src/lib/theme-draft";
import type { ThemeRecipeLabel, WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import { ThemeAppearanceControl } from "./ThemeAppearanceControl";
import { ThemeDraftWorkbench } from "./ThemeDraftWorkbench";

const ThemeGallery = lazy(() => import("./ThemeGallery"));

type ThemeImport =
  | { readonly status: "reading"; readonly name: string }
  | { readonly status: "ready"; readonly name: string; readonly recipeJson: string }
  | { readonly status: "invalid"; readonly name: string; readonly error: string };

export function ThemePanel({ className, onClose }: { className?: string; onClose(): void }) {
  const { t, i18n } = useTranslation();
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const importGeneration = useRef(0);
  const facade = useMermanRuntime(selectMermanFacade);
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const hostAppearance = useAppStore((state) => state.resolvedTheme);
  const historyLength = useAppStore((state) => state.appearanceHistory.length);
  const coordinator = useRenderCoordinator((state) => state);
  const detection = currentDiagramDetection(coordinator, workspace, hostAppearance);
  const render = currentThemeRender(coordinator, workspace, hostAppearance);
  const batch = render.batch;
  const metadata = batch?.merman.status === "success" ? batch.merman.metadata : null;
  const [advanced, setAdvanced] = useState(false);
  const [themeImport, setThemeImport] = useState<ThemeImport | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [draft, setDraft] = useState<{ raw: string; origin: WorkspaceSnapshot; label: ThemeRecipeLabel; preference: ThemeAppearancePreference } | null>(null);
  const [catalogState, setCatalogState] = useState<{ catalog: ThemeCatalog | null; error: string | null }>({ catalog: null, error: null });

  const loadCatalog = useCallback(() => {
    if (!facade) { setCatalogState({ catalog: null, error: null }); return; }
    try { setCatalogState({ catalog: facade.themeCatalog(), error: null }); }
    catch (cause) { setCatalogState({ catalog: null, error: projectError(cause).summary }); }
  }, [facade]);
  useEffect(loadCatalog, [loadCatalog]);
  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    const unsubscribe = useAppStore.subscribe((state, previous) => {
      if (!state.themePanelOpen || state.appearanceHistory !== previous.appearanceHistory ||
        state.diagramTheme !== previous.diagramTheme || state.themePresetId !== previous.themePresetId ||
        state.themeRecipeJson !== previous.themeRecipeJson || state.themeRecipeLabel !== previous.themeRecipeLabel ||
        state.themeAppearance !== previous.themeAppearance ||
        state.diagramFont !== previous.diagramFont) {
        importGeneration.current += 1;
        setThemeImport(null);
      }
      if (!state.themePanelOpen || !sameThemeEditOrigin(selectWorkspaceSnapshot(state), selectWorkspaceSnapshot(previous))) setDraft(null);
    });
    return () => { importGeneration.current += 1; unsubscribe(); };
  }, []);

  const { catalog, error: catalogError } = catalogState;
  const mermaidThemes = useMemo(() => {
    try {
      const available = new Set((facade?.getThemes() ?? SUPPORTED_THEMES).map(normalizeThemeName));
      return SUPPORTED_THEMES.filter((theme) => available.has(theme));
    } catch { return [...SUPPORTED_THEMES]; }
  }, [facade]);
  const families = useMemo(() => {
    try { return facade?.diagramFamilyCapabilities() ?? []; }
    catch { return []; }
  }, [facade]);
  const familyId = detectedThemeFamily(detection, families);
  const familyLabel = (id: string) => t(`diagramTypes.${themeFamilyMetadataId(id, families) ?? id}`, { defaultValue: id });
  const appearanceOverrides = currentAppearanceOverrides(coordinator, workspace, hostAppearance);
  const recipeInspection = useMemo(() => {
    if (!facade || workspace.themeRecipeJson === null) return null;
    try { return inspectThemeRecipe(facade, workspace.themeRecipeJson); }
    catch { return null; }
  }, [facade, workspace.themeRecipeJson]);
  const selection = resolveThemeSelection(workspace, hostAppearance, catalog, recipeInspection);
  const presets = [...(catalog?.presets ?? [])].sort((left, right) =>
    t(`themePresets.${left.id}`, { defaultValue: left.display_name }).localeCompare(
      t(`themePresets.${right.id}`, { defaultValue: right.display_name }), i18n.resolvedLanguage,
      { numeric: true, sensitivity: "base" }));
  const selectedPresetChoice = selectedPresetCatalogEntry(catalog, workspace.themePresetId);
  const selectedPreset = selectedPresetChoice?.preset;
  const selectedPresetAppearance = selectedPreset?.appearances.find((entry) => entry.appearance === selection.appearance);
  const selectedMermaidDesign = catalog?.mermaid_theme_designs.find((entry) =>
    entry.appearances.some((entry) => entry.theme_id === workspace.diagramTheme));
  const unavailablePresetId = workspace.themePresetId !== null && catalog !== null && !selectedPresetChoice?.available ? workspace.themePresetId : null;
  const selectionName = workspace.themePresetId !== null
    ? t(`themePresets.${selectedPreset?.id ?? workspace.themePresetId}`, { defaultValue: selectedPreset?.display_name ?? workspace.themePresetId })
    : workspace.themeRecipeJson !== null ? workspace.themeRecipeLabel?.name ?? t("appearance.importedTheme")
    : t(`themes.${selectedMermaidDesign?.id ?? workspace.diagramTheme}`, { defaultValue: selectedMermaidDesign?.display_name ?? workspace.diagramTheme });
  const selectionSource = workspace.themePresetId !== null ? "builtin"
    : workspace.themeRecipeJson !== null ? workspace.themeRecipeLabel?.source ?? "imported"
    : workspace.diagramTheme === "auto" ? "automatic" : "mermaid";
  const reviewedFamilies = selectedPresetAppearance ? reviewedThemeFamilies(selectedPresetAppearance) : [];
  const applicationNotes = selectedPresetAppearance?.application_notes.filter((note) => note.family_id === familyId) ?? [];
  const canExchange = Boolean(facade && (workspace.themeRecipeJson !== null || selectedPresetChoice?.available));
  const jsonView = useMemo(() => {
    if (!advanced) return { value: "", error: null };
    if (workspace.themeRecipeJson !== null) return { value: workspace.themeRecipeJson, error: null };
    try {
      return { value: selectedPresetChoice?.available && workspace.themePresetId && facade
        ? exportSelectedThemeRecipe(facade, workspace.themePresetId) : "", error: null };
    } catch (cause) { return { value: "", error: projectError(cause).summary }; }
  }, [advanced, facade, selectedPresetChoice?.available, workspace.themePresetId, workspace.themeRecipeJson]);

  function dismissImport() {
    importGeneration.current += 1;
    setThemeImport(null);
  }

  function close() {
    dismissImport();
    useAppStore.getState().closeThemePanel();
    onClose();
  }

  function checkRecipeSize(raw: string | number) {
    if (typeof raw === "number" ? raw > SHARE_THEME_RECIPE_BYTES : exceedsUtf8ByteBudget(raw, SHARE_THEME_RECIPE_BYTES)) {
      throw new Error(t("customTheme.tooLarge", { limit: SHARE_THEME_RECIPE_BYTES / 1024 }));
    }
  }

  function validateRecipe(raw: string) {
    if (!facade) throw new Error(t("wasm.notLoaded"));
    checkRecipeSize(raw);
    return inspectThemeRecipe(facade, raw);
  }

  function selectTheme(choice: DiagramThemeChoice) {
    dismissImport();
    try {
      if (choice.kind === "recipe") {
        const support = validateRecipe(choice.recipeJson);
        choice = { ...choice, appearance: choice.appearance ?? preferredAppearanceForSelection(support) };
      }
      useAppStore.getState().selectTheme(choice);
      setActionError(null);
    } catch (cause) { setActionError(projectError(cause).summary); }
  }

  async function importFile(file: File) {
    const generation = ++importGeneration.current;
    const name = importedThemeName(file.name, t("appearance.importedTheme"));
    setThemeImport({ status: "reading", name });
    setActionError(null);
    try {
      checkRecipeSize(file.size);
      const raw = await file.text();
      if (generation !== importGeneration.current) return;
      validateRecipe(raw);
      if (generation !== importGeneration.current) return;
      setThemeImport({ status: "ready", name, recipeJson: raw });
    } catch (cause) {
      if (generation === importGeneration.current) setThemeImport({ status: "invalid", name, error: projectError(cause).summary });
    }
  }

  function recipeForExchange(): string {
    if (!facade) throw new Error(t("wasm.notLoaded"));
    const current = selectWorkspaceSnapshot(useAppStore.getState());
    const raw = current.themeRecipeJson ?? (current.themePresetId !== null
      ? exportSelectedThemeRecipe(facade, current.themePresetId) : null);
    if (raw === null) throw new Error(t("appearance.noRecipe"));
    validateRecipe(raw);
    return raw;
  }

  function download() {
    try {
      downloadBlob(new Blob([recipeForExchange()], { type: "application/json;charset=utf-8" }), "merman-theme.json");
      setActionError(null);
    } catch (cause) { setActionError(projectError(cause).summary); }
  }

  async function copyJson() {
    try {
      await navigator.clipboard.writeText(recipeForExchange());
      setActionError(null);
      toast.success(t("themePanel.copiedJson"), { position: "top-center" });
    } catch (cause) { setActionError(projectError(cause).summary); }
  }

  function customize() {
    try {
      const raw = recipeForExchange();
      const origin = selectWorkspaceSnapshot(useAppStore.getState());
      const label = origin.themeRecipeLabel ?? {
        name: selectionName, source: "imported" as const,
        ...(origin.themePresetId !== null ? { derivedFrom: `preset:${origin.themePresetId}` } : {}),
      };
      setDraft({ raw, origin, label, preference: selection.preference === "default" && selectedPresetChoice?.pinned
        ? selectedPresetChoice.pinned : selection.preference });
      setActionError(null);
    } catch (cause) { setActionError(projectError(cause).summary); }
  }

  return <aside id="theme-panel" role="region" aria-labelledby={headingId} data-theme-panel
    className={cn("flex min-h-0 min-w-0 flex-col overflow-hidden border-t bg-background lg:border-l lg:border-t-0", className)}
    onKeyDown={(event) => {
      if (event.key === "Escape" && !event.defaultPrevented) { event.preventDefault(); event.stopPropagation(); close(); }
    }}>
    <header className="flex shrink-0 items-start justify-between gap-2 border-b px-3 py-2">
      <div className="min-w-0 space-y-1">
        <h2 ref={heading} id={headingId} tabIndex={-1} className="flex items-center gap-2 text-sm font-semibold outline-none">
          <Palette className="size-4 shrink-0" aria-hidden="true" />{t("appearance.title")}
        </h2>
        <p className="break-words text-xs [overflow-wrap:anywhere]" data-testid="theme-current-name">{selectionName}</p>
        <p className="text-[10px] text-muted-foreground" data-testid="theme-current-source">{t(`appearance.sources.${selectionSource}`)}</p>
      </div>
      <div className="flex shrink-0 gap-1">
        <Button type="button" variant="ghost" size="icon-sm" disabled={historyLength === 0} aria-label={t("appearance.undo")} title={t("appearance.undo")}
          onClick={() => { dismissImport(); useAppStore.getState().undoAppearance(); setActionError(null); }}><Undo2 className="size-4" aria-hidden="true" /></Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label={t("appearance.close")} onClick={close}><X className="size-4" aria-hidden="true" /></Button>
      </div>
    </header>
    <div className="min-h-0 flex-1 space-y-4 overflow-y-auto overscroll-contain p-3">
      <section className="space-y-2" data-testid="theme-selection-summary">
        <p className="text-[11px] leading-relaxed text-muted-foreground">{t("appearance.immediateHint")}</p>
        <p className="text-xs font-medium" data-testid="theme-current-family">
          {familyId ? t("themePanel.currentFamily", { family: familyLabel(familyId) })
            : t(detection === null ? "themePanel.familyWaiting" : "themePanel.familyUnknown")}
        </p>
        {selectedPresetAppearance && familyId && <p className="text-xs text-muted-foreground" data-testid="theme-current-treatment">{t(`themePanel.treatments.${themeFamilyTreatment(selectedPresetAppearance, familyId)}`)}</p>}
        {selectedPreset && <p className="text-[11px] text-muted-foreground" data-testid="theme-current-maturity">
          {selectedPreset.maturity === "alpha" ? t("appearance.alphaHint") : t("themePanel.maturityLabel", { maturity: t(`themePanel.maturity.${selectedPreset.maturity}`, { defaultValue: selectedPreset.maturity }) })}
        </p>}
      </section>
      <ThemeAppearanceControl selection={selection} onChange={(appearance) => {
        dismissImport(); useAppStore.getState().setThemeAppearance(appearance); setActionError(null);
      }} />
      {(workspace.themePresetId !== null || workspace.themeRecipeJson !== null) && workspace.diagramTheme !== "auto" &&
        <p className="rounded-md border p-2 text-xs text-muted-foreground" data-testid="theme-active-combination">
          {t("appearanceMode.composition", { theme: selectionName, base: t(`themes.${workspace.diagramTheme}`, { defaultValue: workspace.diagramTheme }) })}
        </p>}
      {appearanceOverrides && Object.values(appearanceOverrides).some(Boolean) && <section className="space-y-2 rounded-md border p-2 text-[11px]" data-testid="theme-appearance-overrides">
        <h3 className="font-semibold">{t("themePanel.detectedOverrides")}</h3>
        <p className="leading-relaxed text-muted-foreground">{t("themePanel.detectedOverridesHint")}</p>
        <div className="flex flex-wrap gap-2">
          {(["config", "source"] as const).map((origin) => Object.values(appearanceOverrides).some((value) => value?.origin === origin) &&
            <Button key={origin} type="button" variant="outline" size="sm" onClick={() => {
              useAppStore.getState().setEditorMode(origin === "config" ? "config" : "code");
              useAppStore.getState().setWorkspacePane("editor");
            }}>{t(`themePanel.editOverride.${origin}`)}</Button>)}
        </div>
      </section>}
      {canExchange && <Button type="button" variant="outline" size="sm" disabled={draft !== null} onClick={customize}>
        <Pencil className="size-3.5" aria-hidden="true" />{t("themeDraft.customize")}
      </Button>}
      {draft && facade && <ThemeDraftWorkbench raw={draft.raw} origin={draft.origin} label={draft.label} facade={facade}
        appearancePreference={draft.preference} onClose={() => setDraft(null)} />}
      <ThemeRenderEvidence render={render} unavailablePresetId={unavailablePresetId} />
      <section className="space-y-2">
        <label htmlFor={`${headingId}-font`} className="block text-xs font-semibold">{t("themePanel.diagramFont")}</label>
        <select id={`${headingId}-font`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
          value={workspace.diagramFont} onChange={(event) => {
            if (isDiagramFont(event.target.value)) {
              dismissImport();
              useAppStore.getState().setDiagramFont(event.target.value);
              setActionError(null);
            }
          }}>
          {DIAGRAM_FONT_VALUES.map((font) => <option key={font} value={font}>{t(`diagramFonts.${font}`)}</option>)}
        </select>
        <p className="text-[11px] leading-relaxed text-muted-foreground">{t(workspace.diagramFont === "inherit" ? "themePanel.inheritFontHint" : "themePanel.explicitFontHint")}</p>
      </section>
      <section className="min-w-0 space-y-3 border-t pt-3" aria-label={t("appearance.themeCatalog")}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-xs font-semibold">{t("appearance.themeCatalog")}</h3>
          <Button type="button" variant="outline" size="sm" disabled={!facade} onClick={() => fileInput.current?.click()}>
            <FileUp className="size-3.5" aria-hidden="true" />{t("appearance.importJson")}
          </Button>
        </div>
        <input ref={fileInput} type="file" accept=".json,application/json" className="hidden" aria-label={t("appearance.importJson")}
          onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void importFile(file); }} />
        {themeImport && <section className="space-y-2 rounded-md border bg-muted/15 p-3 text-xs" aria-label={t("appearance.importCandidate")} data-testid="theme-import-candidate" data-theme-import-state={themeImport.status}>
          <p className="break-words font-medium [overflow-wrap:anywhere]">{themeImport.name}</p>
          {themeImport.status === "reading" && <p role="status" className="text-muted-foreground">{t("appearance.readingImport")}</p>}
          {themeImport.status === "ready" && <>
            <p role="status" className="leading-relaxed text-muted-foreground">{t("appearance.importReady")}</p>
            <Button type="button" size="sm" className="h-auto max-w-full whitespace-normal py-1.5" onClick={() => {
              selectTheme({ kind: "recipe", recipeJson: themeImport.recipeJson, label: { name: themeImport.name, source: "imported" } });
              heading.current?.focus();
            }}>{t("appearance.useImportedTheme")}</Button>
          </>}
          {themeImport.status === "invalid" && <>
            <p role="alert" className="whitespace-pre-wrap break-words text-destructive [overflow-wrap:anywhere]">{themeImport.error}</p>
            <p className="leading-relaxed text-muted-foreground">{t("appearance.invalidImportHint")}</p>
          </>}
          <Button type="button" variant="ghost" size="sm" onClick={() => { dismissImport(); heading.current?.focus(); }}>{t("appearance.discardImport")}</Button>
        </section>}
        {!catalog && (catalogError ? <div className="space-y-2 text-xs">
          <p role="alert" className="break-words text-destructive">{t("themePanel.catalogError", { error: catalogError })}</p>
          <Button type="button" variant="outline" size="sm" onClick={loadCatalog}>{t("themePanel.retryCatalog")}</Button>
        </div> : <p role="status" className="text-xs text-muted-foreground">{t("appearance.catalogPending")}</p>)}
        <LazyFeatureBoundary feature={t("appearance.themeCatalog")} presentation={{ kind: "panel" }}>
          <ThemeGallery workspace={workspace} mermaidThemes={["auto", ...mermaidThemes]} presets={presets} catalog={catalog} selectedAppearance={selection.appearance}
            catalogReady={catalog !== null} familyId={familyId} pending={detection === null} canSelectRecipes={Boolean(facade)} onSelect={selectTheme} />
        </LazyFeatureBoundary>
      </section>
      {applicationNotes.length > 0 && <ThemeApplicationNotes key={`${workspace.themePresetId}:${familyId}`}
        notes={applicationNotes} workspace={workspace} onApply={() => { setActionError(null); heading.current?.focus(); }} />}
      <details open={advanced} onToggle={(event) => setAdvanced(event.currentTarget.open)} className="min-w-0 border-t pt-3" data-testid="theme-advanced">
        <summary className="cursor-pointer text-xs font-medium">{t("themePanel.advanced")}</summary>
        {advanced && <div className="mt-3 min-w-0 space-y-4 text-xs">
          <section className="space-y-2" data-testid="theme-combination">
            <h3 className="font-semibold">{t("appearance.combination")}</h3>
            <label htmlFor={`${headingId}-mermaid-theme`} className="block text-xs">{t("themePanel.mermaidTheme")}</label>
            <select id={`${headingId}-mermaid-theme`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2"
              value={workspace.diagramTheme} onChange={(event) => {
                if (isMermaidThemeSelection(event.target.value)) {
                  dismissImport();
                  if (workspace.themePresetId !== null || workspace.themeRecipeJson !== null) useAppStore.getState().setDiagramTheme(event.target.value);
                  else useAppStore.getState().selectTheme({ kind: "mermaid", theme: event.target.value });
                  setActionError(null);
                }
              }}>
              <option value="auto">{t("themes.auto")}</option>
              {mermaidThemes.map((theme) => <option key={theme} value={theme}>{t(`themes.${theme}`, { defaultValue: theme })}</option>)}
            </select>
            <label htmlFor={`${headingId}-extension`} className="block text-xs">{t("appearance.extension")}</label>
            <select id={`${headingId}-extension`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2"
              value={workspace.themePresetId !== null ? `preset:${workspace.themePresetId}` : workspace.themeRecipeJson !== null ? "recipe" : "none"}
              onChange={(event) => {
                const value = event.target.value;
                if (value === "none") selectTheme({ kind: "none" });
                else {
                  const preset = presets.find((preset) => `preset:${preset.id}` === value && preset.available);
                  if (preset) selectTheme({ kind: "preset", presetId: preset.id, appearance: preferredAppearanceForSelection(preset), compose: true });
                }
              }}>
              <option value="none">{t("appearance.noExtension")}</option>
              {workspace.themeRecipeJson !== null && <option value="recipe" disabled>{selectionName}</option>}
              {workspace.themePresetId !== null && workspace.themePresetId !== selectedPreset?.id && <option value={`preset:${workspace.themePresetId}`} disabled>
                {selectionName}{selection.appearance ? ` · ${t(`appearanceMode.${selection.appearance}`)}` : ""}
              </option>}
              {presets.map((preset) => <option key={preset.id} value={`preset:${preset.id}`} disabled={!preset.available}>
                {t(`themePresets.${preset.id}`, { defaultValue: preset.display_name })}
              </option>)}
            </select>
            <p className="leading-relaxed text-muted-foreground">{t("appearance.combinationHint")}</p>
            <p className="leading-relaxed text-muted-foreground">{t("themePanel.layeringHint")}</p>
            <p className="leading-relaxed text-muted-foreground">{t("themePanel.fontPrecedenceHint")}</p>
          </section>
          {appearanceOverrides && Object.values(appearanceOverrides).some(Boolean) && <section className="space-y-2 rounded-md border p-2 text-[11px]" data-testid="theme-appearance-override-details">
            <h3 className="font-semibold">{t("themePanel.detectedOverrides")}</h3>
            <dl className="space-y-1.5">
              {(["theme", "font", "look"] as const).map((key) => {
                const override = appearanceOverrides[key];
                return override ? <div key={key} className="space-y-0.5">
                  <dt className="text-muted-foreground">{t(`themePanel.overrideKeys.${key}`)} · {t(`themePanel.overrideOrigins.${override.origin}`)}</dt>
                  <dd className="break-words font-mono [overflow-wrap:anywhere]">{override.value}</dd>
                </div> : null;
              })}
            </dl>
            <div className="flex flex-wrap gap-2">
              {(["config", "source"] as const).map((origin) => Object.values(appearanceOverrides).some((value) => value?.origin === origin) &&
                <Button key={origin} type="button" variant="outline" size="sm" className="h-auto whitespace-normal py-1" onClick={() => {
                  const store = useAppStore.getState();
                  store.setEditorMode(origin === "config" ? "config" : "code");
                  store.setWorkspacePane("editor");
                }}>{t(`themePanel.editOverride.${origin}`)}</Button>)}
            </div>
            <p className="leading-relaxed text-muted-foreground">{t("themePanel.detectedOverridesHint")}</p>
          </section>}
          {selectedPreset && <section className="space-y-2 rounded-md border p-3" aria-label={t("themePanel.reviewedFamilies")} data-testid="theme-reviewed-families">
            <h3 className="font-semibold">{t("themePanel.reviewedFamilies")}</h3>
            {reviewedFamilies.length > 0 ? <ul className="space-y-1.5">
              {reviewedFamilies.map((design) => <li key={design.family_id} className="flex flex-wrap justify-between gap-x-2 gap-y-0.5">
                <span className="font-medium">{familyLabel(design.family_id)}</span>
                <span className="text-muted-foreground">{t(`themePanel.treatments.${design.treatment}`)}</span>
              </li>)}
            </ul> : <p className="text-muted-foreground">{t("themePanel.noReviewedFamilies")}</p>}
            <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.reviewedFamiliesHint")}</p>
          </section>}
          <section className="space-y-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <label htmlFor={`${headingId}-json`} className="font-semibold">{t("themePanel.json")}</label>
              <Button type="button" variant="ghost" size="sm" disabled={!canExchange} onClick={() => void copyJson()}><Copy className="size-3.5" aria-hidden="true" />{t("themePanel.copyJson")}</Button>
            </div>
            <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.jsonHint")}</p>
            {jsonView.error ? <p role="alert" className="text-destructive">{jsonView.error}</p>
              : jsonView.value ? <CodePreview value={jsonView.value} filename="theme.json" label={t("themePanel.json")} testId="theme-json-document" />
              : <p className="text-muted-foreground">{t("appearance.noRecipe")}</p>}
          </section>
          <section className="space-y-2">
            <h3 className="font-semibold">{t("themePanel.executionMetadata")}</h3>
            <p className="leading-relaxed text-muted-foreground">{t("appearance.evidenceHint")}</p>
            {metadata ? <pre data-testid="theme-execution-metadata" className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded border bg-muted/20 p-2 text-[11px] [overflow-wrap:anywhere]">{JSON.stringify(metadata, null, 2)}</pre>
              : <p className="text-muted-foreground">{t("themePanel.evidenceUnavailable")}</p>}
          </section>
          {selectedPreset && <details>
            <summary className="cursor-pointer text-muted-foreground">{t("themePanel.catalogDetails")}</summary>
            <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap break-words text-[11px] [overflow-wrap:anywhere]">{JSON.stringify(selectedPreset, null, 2)}</pre>
          </details>}
          <details>
            <summary className="cursor-pointer text-muted-foreground">{t("themePanel.authoringTitle")}</summary>
            <p className="mt-2 leading-relaxed text-muted-foreground">{t("themePanel.authoringHint")}</p>
          </details>
        </div>}
      </details>
    </div>
    <div className="shrink-0 space-y-2 border-t bg-background px-3 py-2">
      {actionError && <p role="alert" className="max-h-20 overflow-y-auto whitespace-pre-wrap break-words text-xs text-destructive [overflow-wrap:anywhere]">{actionError}</p>}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="min-w-0 flex-1 text-[11px] leading-relaxed text-muted-foreground">{t("appearance.exchangeHint")}</p>
        <Button type="button" variant="outline" size="sm" disabled={!canExchange} onClick={download}><Download className="size-3.5" aria-hidden="true" />{t("customTheme.download")}</Button>
      </div>
      {workspace.themeAppearance === "follow" && <p className="text-[11px] leading-relaxed text-muted-foreground">{t("appearanceMode.shareFollow")}</p>}
    </div>
  </aside>;
}
