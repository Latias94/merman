import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { Copy, Download, FileUp, Palette, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { normalizeThemeName, SUPPORTED_THEMES, type ThemeCatalog } from "@mermanjs/web";
import { useShallow } from "zustand/react/shallow";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { downloadBlob } from "@/src/lib/export";
import { DIAGRAM_FONT_VALUES, isDiagramFont } from "@/src/lib/diagram-font";
import { readMermaidAppearanceOverrides } from "@/src/lib/mermaid-appearance";
import type { MermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import { SHARE_THEME_RECIPE_BYTES } from "@/src/lib/share";
import { isThemeDraftChanged, type ThemeValidationToken } from "@/src/lib/theme-edit-session";
import { currentDiagramDetection, currentThemeRender, detectedThemeFamily, reviewedThemeFamilies, themeFamilyMetadataId, themeFamilyTreatment } from "@/src/lib/theme-discovery";
import { exceedsUtf8ByteBudget } from "@/src/lib/utf8";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
import { selectRenderInputSnapshot, useAppStore } from "@/src/store";
import { ThemePresetCards } from "./ThemePresetCards";
import { ThemeRenderEvidence } from "./ThemeRenderEvidence";
import { ThemeApplicationNotes } from "./ThemeApplicationNotes";

export function ThemePanel({ className, onClose }: { className?: string; onClose(): void }) {
  const { t } = useTranslation();
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const selectionHeading = useRef<HTMLHeadingElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const session = useAppStore((state) => state.themeEditSession);
  const workspace = useAppStore(useShallow(selectRenderInputSnapshot));
  const coordinator = useRenderCoordinator((state) => state);
  const detection = currentDiagramDetection(coordinator, workspace);
  const render = currentThemeRender(coordinator, workspace, session);
  const batch = render.batch;
  const metadata = batch?.merman.status === "success" ? batch.merman.metadata : null;
  const [advanced, setAdvanced] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [catalogState, setCatalogState] = useState<{ catalog: ThemeCatalog | null; error: string | null }>({ catalog: null, error: null });

  const loadCatalog = useCallback(() => {
    if (!facade) { setCatalogState({ catalog: null, error: null }); return; }
    try { setCatalogState({ catalog: facade.themeCatalog(), error: null }); }
    catch (cause) { setCatalogState({ catalog: null, error: projectError(cause).summary }); }
  }, [facade]);
  useEffect(loadCatalog, [loadCatalog]);
  const { catalog, error: catalogError } = catalogState;
  const mermaidThemes = useMemo(() => {
    try { return [...new Set((facade?.getThemes() ?? SUPPORTED_THEMES).map(normalizeThemeName))]; }
    catch { return [...SUPPORTED_THEMES].map(normalizeThemeName); }
  }, [facade]);
  const families = useMemo(() => {
    try { return facade?.diagramFamilyCapabilities() ?? []; }
    catch { return []; }
  }, [facade]);
  const familyId = detectedThemeFamily(detection, families);
  const configNamespace = families.find((family) => family.family_id === familyId)?.config_namespace;
  const familyLabel = (id: string) => t(`diagramTypes.${themeFamilyMetadataId(id, families) ?? id}`, { defaultValue: id });
  const appearanceOverrides = useMemo(() => {
    try { return readMermaidAppearanceOverrides(workspace.code, workspace.mermaidConfig, configNamespace); }
    catch { return null; }
  }, [workspace.code, workspace.mermaidConfig, configNamespace]);
  const presets = useMemo(() => {
    const rank = { dedicated: 0, base_only: 1, unreviewed: 2 };
    return [...(catalog?.presets ?? [])].sort((left, right) =>
      rank[themeFamilyTreatment(left, familyId)] - rank[themeFamilyTreatment(right, familyId)]);
  }, [catalog, familyId]);

  const checkRecipeSize = useCallback((raw: string | number) => {
    if (typeof raw === "number" ? raw > SHARE_THEME_RECIPE_BYTES : exceedsUtf8ByteBudget(raw, SHARE_THEME_RECIPE_BYTES)) {
      throw new Error(t("customTheme.tooLarge", { limit: SHARE_THEME_RECIPE_BYTES / 1024 }));
    }
  }, [t]);
  const validateNow = useCallback((raw: string, token: ThemeValidationToken | null) => {
    if (!token || !facade) return;
    try {
      checkRecipeSize(raw);
      const recipeJson = facade.validateThemeRecipe(raw);
      checkRecipeSize(recipeJson);
      useAppStore.getState().acceptThemeValidation(token, raw);
    } catch (cause) {
      useAppStore.getState().rejectThemeValidation(token, projectError(cause).summary);
    }
  }, [checkRecipeSize, facade]);

  const sessionId = session?.id;
  const revision = session?.revision;
  const rawJson = session?.rawJson;
  const validation = session?.validation;
  const importing = session?.importing;
  useEffect(() => { heading.current?.focus(); }, [sessionId]);
  useEffect(() => {
    if (!facade || sessionId === undefined || revision === undefined || rawJson == null || validation !== "pending" || importing) return;
    validateNow(rawJson, { sessionId, revision });
  }, [facade, sessionId, revision, rawJson, validation, importing, validateNow]);

  const selectedPreset = catalog?.presets.find((preset) => session?.candidate.kind === "preset" && preset.id === session.candidate.presetId);
  const jsonView = useMemo(() => {
    if (!advanced || !session) return { value: "", error: null };
    if (session.rawJson !== null) return { value: session.rawJson, error: null };
    try {
      return { value: session.candidate.kind === "preset" && facade
        ? JSON.stringify(facade.exportThemePreset(session.candidate.presetId), null, 2) : "", error: null };
    } catch (cause) { return { value: "", error: projectError(cause).summary }; }
  }, [advanced, facade, session]);

  if (!session) return null;
  const dirty = isThemeDraftChanged(session);
  const valid = session.validation === "valid" && !session.importing;
  const available = session.candidate.kind !== "preset" || Boolean(selectedPreset?.available);
  const reviewedFamilies = selectedPreset ? reviewedThemeFamilies(selectedPreset) : [];
  const applicationNotes = selectedPreset?.application_notes.filter((note) => note.family_id === familyId) ?? [];
  const canExchange = Boolean(facade && valid && available && session.candidate.kind !== "none");
  const candidateLabel = session.candidate.kind === "none" ? t("themePanel.none")
    : session.candidate.kind === "custom" ? t("themePanel.custom")
    : t(`themePresets.${session.candidate.presetId}`, { defaultValue: selectedPreset?.display_name ?? session.candidate.presetId });
  const status = session.importing ? t("customTheme.importing")
    : session.validation === "pending" ? t("themePanel.validating")
    : session.validation === "invalid" ? t("themePanel.invalidDraft")
    : dirty ? t("themePanel.draftPreview") : t("themePanel.applied");

  function close() {
    useAppStore.getState().cancelThemeEdit();
    onClose();
  }

  async function importFile(file: File) {
    const token = useAppStore.getState().beginThemeImport();
    if (!token) return;
    setActionError(null);
    try {
      checkRecipeSize(file.size);
      const raw = await file.text();
      checkRecipeSize(raw);
      const validationToken = useAppStore.getState().completeThemeImport(token, raw);
      if (validationToken) validateNow(raw, validationToken);
    } catch (cause) {
      useAppStore.getState().failThemeImport(token, projectError(cause).summary);
    }
  }

  function recipeForExchange(): string {
    if (!facade) throw new Error(t("wasm.notLoaded"));
    const current = useAppStore.getState().themeEditSession;
    if (!current || current.importing || current.validation !== "valid") throw new Error(t("themePanel.validRecipeRequired"));
    const raw = current.candidate.kind === "preset"
      ? JSON.stringify(facade.exportThemePreset(current.candidate.presetId), null, 2)
      : current.candidate.kind === "custom" ? current.rawJson ?? current.candidate.recipeJson : null;
    if (raw === null) throw new Error(t("themePanel.noRecipe"));
    checkRecipeSize(raw);
    checkRecipeSize(facade.validateThemeRecipe(raw));
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

  return (
    <aside id="theme-panel" role="region" aria-labelledby={headingId} data-theme-panel
      className={cn("flex min-h-0 min-w-0 flex-col overflow-hidden border-t bg-background lg:border-l lg:border-t-0", className)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) { event.preventDefault(); event.stopPropagation(); close(); }
      }}>
      <header className="flex shrink-0 items-start justify-between gap-2 border-b px-3 py-2">
        <div className="min-w-0 space-y-1">
          <h2 ref={heading} id={headingId} tabIndex={-1} className="flex items-center gap-2 text-sm font-semibold outline-none">
            <Palette className="size-4" aria-hidden="true" />{t("themePanel.title")}
          </h2>
          <p className="truncate text-xs text-muted-foreground">{candidateLabel}</p>
        </div>
        <Button type="button" variant="ghost" size="icon-sm" aria-label={t("themePanel.close")} onClick={close}><X className="size-4" /></Button>
      </header>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto overscroll-contain p-3">
        <section className="space-y-2">
          <label htmlFor={`${headingId}-mermaid-theme`} className="block text-xs font-semibold">{t("themePanel.mermaidTheme")}</label>
          <select id={`${headingId}-mermaid-theme`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
            value={workspace.diagramTheme} onChange={(event) => useAppStore.getState().previewThemeSettings({ diagramTheme: event.target.value as MermaidThemeSelection })}>
            <option value="auto">{t("themes.auto")}</option>
            {mermaidThemes.map((theme) => <option key={theme} value={theme}>{t(`themes.${theme}`, { defaultValue: theme })}</option>)}
          </select>
          <label htmlFor={`${headingId}-font`} className="block pt-1 text-xs font-semibold">{t("themePanel.diagramFont")}</label>
          <select id={`${headingId}-font`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
            value={workspace.diagramFont} onChange={(event) => {
              if (isDiagramFont(event.target.value)) useAppStore.getState().previewThemeSettings({ diagramFont: event.target.value });
            }}>
            {DIAGRAM_FONT_VALUES.map((font) => <option key={font} value={font}>{t(`diagramFonts.${font}`)}</option>)}
          </select>
          <p className="text-[11px] leading-relaxed text-muted-foreground">{t(workspace.diagramFont === "inherit" ? "themePanel.inheritFontHint" : "themePanel.explicitFontHint")}</p>
          <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.compatibilityHint")}</p>
          {appearanceOverrides && Object.values(appearanceOverrides).some(Boolean) && <div className="space-y-2 rounded-md border p-2 text-[11px]" data-testid="theme-appearance-overrides">
            <p className="font-medium">{t("themePanel.detectedOverrides")}</p>
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
          </div>}
        </section>
        <section className="space-y-2 border-t pt-3" data-testid="theme-selection-summary">
          <h3 ref={selectionHeading} tabIndex={-1} className="text-xs font-semibold outline-none" data-testid="theme-selection-heading">{t("themePanel.mermanStyling")}</h3>
          <label htmlFor={`${headingId}-preset`} className="sr-only">{t("themePanel.choosePreset")}</label>
          <select id={`${headingId}-preset`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
            value={session.candidate.kind === "preset" ? `preset:${session.candidate.presetId}` : session.candidate.kind}
            onChange={(event) => {
              const value = event.target.value;
              if (value !== "none" && !value.startsWith("preset:")) return;
              useAppStore.getState().selectThemeCandidate(value === "none" ? { kind: "none" } : { kind: "preset", presetId: value.slice(7) });
              setActionError(null);
            }}>
            <option value="none">{t("themePanel.none")}</option>
            {session.candidate.kind === "custom" && <option value="custom" disabled>{t("themePanel.custom")}</option>}
            {session.candidate.kind === "preset" && !selectedPreset && <option value={`preset:${session.candidate.presetId}`} disabled>{candidateLabel}</option>}
            {presets.map((preset) => <option key={preset.id} value={`preset:${preset.id}`} disabled={!preset.available}>
              {t(`themePresets.${preset.id}`, { defaultValue: preset.display_name })}
              {` · ${t(`themePanel.maturity.${preset.maturity}`, { defaultValue: preset.maturity })}`}
              {familyId ? ` · ${t(`themePanel.treatments.${themeFamilyTreatment(preset, familyId)}`)}` : ""}
            </option>)}
          </select>
          <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.previewHint")}</p>
          <p className="text-xs font-medium" data-testid="theme-current-family">
            {familyId ? t("themePanel.currentFamily", { family: familyLabel(familyId) })
              : t(detection === null ? "themePanel.familyWaiting" : "themePanel.familyUnknown")}
          </p>
          {selectedPreset && familyId && <p className="text-xs text-muted-foreground" data-testid="theme-current-treatment">
            {t(`themePanel.treatments.${themeFamilyTreatment(selectedPreset, familyId)}`)}
          </p>}
          {selectedPreset && <p className="text-[11px] text-muted-foreground" data-testid="theme-current-maturity">
            {selectedPreset.maturity === "alpha" ? t("themePanel.alphaHint") : t("themePanel.maturityLabel", { maturity: t(`themePanel.maturity.${selectedPreset.maturity}`, { defaultValue: selectedPreset.maturity }) })}
          </p>}
          {!catalog && (catalogError ? <div className="space-y-2 text-xs">
            <p role="alert" className="break-words text-destructive">{t("themePanel.catalogError", { error: catalogError })}</p>
            <Button type="button" variant="outline" size="sm" onClick={loadCatalog}>{t("themePanel.retryCatalog")}</Button>
          </div> : <p role="status" className="text-xs text-muted-foreground">{t("themePanel.catalogLoading")}</p>)}
          {catalog && session.candidate.kind === "preset" && !available && <p role="status" className="break-words text-xs text-destructive">{t("themePanel.unknownPreset", { id: session.candidate.presetId })}</p>}
          <details className="text-[11px] leading-relaxed text-muted-foreground">
            <summary className="cursor-pointer">{t("themePanel.howThemesCombine")}</summary>
            <p className="mt-2">{t("themePanel.layeringHint")}</p>
            <p className="mt-2">{t("themePanel.fontPrecedenceHint")}</p>
          </details>
        </section>
        {applicationNotes.length > 0 && <ThemeApplicationNotes key={`${session.id}:${session.candidate.kind === "preset" ? session.candidate.presetId : ""}:${familyId}`}
          notes={applicationNotes} workspace={workspace} declaredLook={appearanceOverrides?.look} onPreview={() => selectionHeading.current?.focus()} />}
        <ThemeRenderEvidence render={render} />
        {selectedPreset && <section className="space-y-2 rounded-md border p-3 text-xs" aria-label={t("themePanel.reviewedFamilies")} data-testid="theme-reviewed-families">
          <h3 className="font-semibold">{t("themePanel.reviewedFamilies")}</h3>
          {reviewedFamilies.length > 0 ? <ul className="space-y-1.5">
            {reviewedFamilies.map((design) => <li key={design.family_id} className="flex flex-wrap justify-between gap-x-2 gap-y-0.5">
              <span className="font-medium">{familyLabel(design.family_id)}</span>
              <span className="text-muted-foreground">{t(`themePanel.treatments.${design.treatment}`)}</span>
            </li>)}
          </ul> : <p className="text-muted-foreground">{t("themePanel.noReviewedFamilies")}</p>}
          <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.reviewedFamiliesHint")}</p>
        </section>}
        <section className="space-y-2 rounded-md border p-3">
          <h3 className="text-xs font-semibold">{t("themePanel.importTitle")}</h3>
          <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.importHint")}</p>
          <Button type="button" variant="outline" size="sm" onClick={() => fileInput.current?.click()}><FileUp className="size-3.5" />{t("customTheme.import")}</Button>
          <input ref={fileInput} type="file" accept=".json,application/json" className="hidden" aria-label={t("customTheme.import")}
            onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void importFile(file); }} />
          {session.rawJson !== null && <p className="text-xs text-muted-foreground">{t("themePanel.imported")}</p>}
        </section>
        <details open className="space-y-3 border-t pt-3" data-testid="theme-preset-discovery">
          <summary className="cursor-pointer text-xs font-semibold">{t("themePanel.browsePresets")}</summary>
          <ThemePresetCards presets={presets} candidate={session.candidate} familyId={familyId} pending={detection === null} onSelect={(candidate) => {
            useAppStore.getState().selectThemeCandidate(candidate);
            setActionError(null);
            selectionHeading.current?.focus({ preventScroll: true });
            selectionHeading.current?.scrollIntoView({ block: "start" });
          }} />
        </details>
        <details open={advanced} onToggle={(event) => setAdvanced(event.currentTarget.open)} className="min-w-0 border-t pt-3">
          <summary className="cursor-pointer text-xs font-medium">{t("themePanel.advanced")}</summary>
          {advanced && <div className="mt-3 min-w-0 space-y-4 text-xs">
            <section className="space-y-2">
              <div className="flex items-center justify-between gap-2">
                <label htmlFor={`${headingId}-json`} className="font-semibold">{t("themePanel.json")}</label>
                <Button type="button" variant="ghost" size="sm" disabled={!canExchange} onClick={() => void copyJson()}><Copy className="size-3.5" />{t("themePanel.copyJson")}</Button>
              </div>
              <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.jsonHint")}</p>
              {jsonView.error ? <p role="alert" className="text-destructive">{jsonView.error}</p>
                : jsonView.value ? <textarea id={`${headingId}-json`} readOnly value={jsonView.value} spellCheck={false} className="h-48 w-full resize-y rounded border bg-muted/20 p-2 font-mono text-[11px]" />
                : <p className="text-muted-foreground">{t("themePanel.noRecipe")}</p>}
            </section>
            <section className="space-y-2">
              <h3 className="font-semibold">{t("themePanel.executionMetadata")}</h3>
              <p className="leading-relaxed text-muted-foreground">{t("themePanel.evidenceExplanation")}</p>
              {metadata ? <pre data-testid="theme-execution-metadata" className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded border bg-muted/20 p-2 text-[11px]">{JSON.stringify(metadata, null, 2)}</pre>
                : <p className="text-muted-foreground">{t("themePanel.evidenceUnavailable")}</p>}
              {batch && <p className="break-words text-muted-foreground">{t("themePanel.renderInput")}: <code>{batch.snapshot.operation.themePresetId ?? (batch.snapshot.operation.themeRecipeJson ? t("themePanel.custom") : t("themePanel.none"))}</code></p>}
            </section>
            {selectedPreset && <details>
              <summary className="cursor-pointer text-muted-foreground">{t("themePanel.catalogDetails")}</summary>
              <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap break-words text-[11px]">{JSON.stringify(selectedPreset, null, 2)}</pre>
            </details>}
            <details>
              <summary className="cursor-pointer text-muted-foreground">{t("themePanel.authoringTitle")}</summary>
              <p className="mt-2 leading-relaxed text-muted-foreground">{t("themePanel.authoringHint")}</p>
            </details>
          </div>}
        </details>
      </div>
      <div className="shrink-0 space-y-2 border-t bg-background px-3 py-2">
        <p role="status" data-theme-draft-status className={cn("text-xs", session.validation === "invalid" ? "text-destructive" : "text-muted-foreground")}>{status}</p>
        {(session.error || session.applyError || actionError) && <p role="alert" className="max-h-20 overflow-y-auto whitespace-pre-wrap break-words text-xs text-destructive [overflow-wrap:anywhere]">{actionError ?? session.error ?? session.applyError}</p>}
        {(session.validation === "invalid" || session.error) && session.lastValidCandidate !== null && !session.importing &&
          <Button type="button" variant="outline" size="sm" className="h-auto max-w-full whitespace-normal py-1.5" onClick={() => {
            if (useAppStore.getState().restoreValidThemeDraft()) {
              setActionError(null);
              selectionHeading.current?.focus();
            }
          }}>{t("themePanel.restoreValidDraft")}</Button>}
        {canExchange && <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.exchangeHint")}</p>}
        <div className="flex flex-wrap items-center justify-end gap-2">
          <Button type="button" variant="ghost" size="sm" className="mr-auto" disabled={!canExchange} onClick={download}><Download className="size-3.5" />{t("customTheme.download")}</Button>
          <Button type="button" variant="outline" size="sm" onClick={close}>{t("customTheme.cancel")}</Button>
          <Button type="button" size="sm" disabled={!valid || !facade || !available} onClick={() => {
            setActionError(null);
            useAppStore.getState().applyThemeEdit();
          }}>{t("customTheme.apply")}</Button>
        </div>
      </div>
    </aside>
  );
}
