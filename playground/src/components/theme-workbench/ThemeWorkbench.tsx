import { lazy, useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { CheckCircle2, Copy, Download, FileUp, Palette, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { normalizeThemeName, SUPPORTED_THEMES, type ThemeRecipeV1 } from "@mermanjs/web";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { downloadBlob } from "@/src/lib/export";
import type { MermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import { SHARE_THEME_RECIPE_BYTES } from "@/src/lib/share";
import { themeFamilyTreatment } from "@/src/lib/theme-design";
import { validateThemeDocument } from "@/src/lib/theme-document";
import { isThemeDraftChanged, type ThemeCandidate, type ThemeValidationToken } from "@/src/lib/theme-edit-session";
import { exceedsUtf8ByteBudget } from "@/src/lib/utf8";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { selectCurrentDiagramType, useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
import { useAppStore } from "@/src/store";

import { ThemePresetCards } from "./ThemePresetCards";
import { ThemeVisualEditor } from "./ThemeVisualEditor";

const ThemeJsonEditor = lazy(() => import("./ThemeJsonEditorFeature").then((module) => ({ default: module.ThemeJsonEditor })));

const NEW_THEME: ThemeRecipeV1 = {
  schema_version: 1,
  kind: "definition",
  definition: {
    authoring_schema_version: 1,
    expansion_version: 1,
    tokens: { canvas: "#fffaf0", surface: "#ffffff", text: "#202020", border: "#202020", accent: "#f16b50" },
  },
};

type WorkbenchTab = "presets" | "visual" | "json";

export function ThemeWorkbench({ className, onClose }: {
  className?: string;
  onClose(): void;
}) {
  const { t } = useTranslation();
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const visibleDiagramType = useRenderCoordinator(selectCurrentDiagramType);
  const session = useAppStore((state) => state.themeEditSession);
  const diagramTheme = useAppStore((state) => state.diagramTheme);
  const [tab, setTab] = useState<WorkbenchTab>(() => session?.rawJson === null ? "presets" : "visual");
  const [pendingReplacement, setPendingReplacement] = useState<(() => void) | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const catalog = useMemo(() => {
    try {
      return facade?.themeCatalog() ?? null;
    } catch {
      return null;
    }
  }, [facade]);
  const familyId = useMemo(() => {
    try {
      return facade?.diagramFamilyCapabilities()
        .find((family) => family.diagram_type === visibleDiagramType)?.family_id;
    } catch {
      return undefined;
    }
  }, [facade, visibleDiagramType]);
  const mermaidThemes = useMemo(() => {
    try {
      return [...new Set((facade?.getThemes() ?? SUPPORTED_THEMES)
        .map(normalizeThemeName))];
    } catch {
      return [...SUPPORTED_THEMES].map(normalizeThemeName);
    }
  }, [facade]);
  const selectedPreset = catalog?.presets.find((preset) => session?.candidate.kind === "preset" && preset.id === session.candidate.presetId);
  const selectedTreatment = themeFamilyTreatment(selectedPreset, familyId);
  const sessionId = session?.id;
  const revision = session?.revision;
  const rawJson = session?.rawJson;
  const validation = session?.validation;
  const importing = session?.importing;
  const checkRecipeSize = useCallback((raw: string | number) => {
    const exceeded = typeof raw === "number"
      ? raw > SHARE_THEME_RECIPE_BYTES
      : exceedsUtf8ByteBudget(raw, SHARE_THEME_RECIPE_BYTES);
    if (exceeded) throw new Error(t("customTheme.tooLarge", { limit: SHARE_THEME_RECIPE_BYTES / 1024 }));
  }, [t]);

  const validateNow = useCallback((raw: string, token: ThemeValidationToken | null) => {
    if (!token || !facade) return;
    try {
      checkRecipeSize(raw);
      const validated = validateThemeDocument(raw, (text) => facade.validateThemeRecipe(text));
      checkRecipeSize(validated.recipeJson);
      useAppStore.getState().acceptThemeValidation(token, validated);
    } catch (cause) {
      useAppStore.getState().rejectThemeValidation(token, projectError(cause).summary);
    }
  }, [checkRecipeSize, facade]);

  useEffect(() => { heading.current?.focus(); }, [sessionId]);

  useEffect(() => {
    if (!facade || sessionId === undefined || revision === undefined || rawJson == null || validation !== "pending" || importing) return;
    const token = { sessionId, revision };
    const timeout = window.setTimeout(() => validateNow(rawJson, token), 180);
    return () => window.clearTimeout(timeout);
  }, [facade, sessionId, revision, rawJson, validation, importing, validateNow]);

  if (!session) return null;

  const dirty = isThemeDraftChanged(session);
  const valid = session.validation === "valid" && !session.importing;
  const candidateLabel = session.candidate.kind === "none"
    ? t("themeWorkbench.none", { defaultValue: "None" })
    : session.candidate.kind === "custom"
      ? t("themeWorkbench.custom", { defaultValue: "Custom recipe" })
      : t(`themePresets.${session.candidate.presetId}`, { defaultValue: selectedPreset?.display_name ?? session.candidate.presetId });

  function replaceRaw(raw: string, nextTab: WorkbenchTab) {
    setActionError(null);
    const token = useAppStore.getState().editThemeDraft(raw);
    validateNow(raw, token);
    setTab(nextTab);
  }

  function requestReplacement(action: () => void, anyDirty = false) {
    const current = useAppStore.getState().themeEditSession;
    if (current && isThemeDraftChanged(current) && (anyDirty || current.rawJson !== null)) {
      setPendingReplacement(() => action);
    } else {
      action();
    }
  }

  function close() {
    useAppStore.getState().cancelThemeEdit();
    onClose();
  }

  function selectCandidate(candidate: Exclude<ThemeCandidate, { kind: "custom" }>) {
    const current = useAppStore.getState().themeEditSession?.candidate;
    if (current?.kind === candidate.kind && (current.kind !== "preset" || candidate.kind !== "preset" || current.presetId === candidate.presetId)) return;
    requestReplacement(() => {
      useAppStore.getState().selectThemeCandidate(candidate);
      setActionError(null);
    });
  }

  function customize() {
    const candidate = useAppStore.getState().themeEditSession?.candidate;
    if (!facade || candidate?.kind !== "preset") return;
    try {
      replaceRaw(JSON.stringify(facade.exportThemePreset(candidate.presetId), null, 2), "visual");
    } catch (cause) {
      setActionError(projectError(cause).summary);
    }
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
      if (!validationToken) return;
      validateNow(raw, validationToken);
      setTab("json");
    } catch (cause) {
      useAppStore.getState().failThemeImport(token, projectError(cause).summary);
    }
  }

  function recipeForExchange(): string {
    if (!facade) throw new Error(t("wasm.notLoaded"));
    const current = useAppStore.getState().themeEditSession;
    if (!current || current.importing || current.validation !== "valid") throw new Error(t("themeWorkbench.validRecipeRequired", { defaultValue: "Fix the recipe before exporting it." }));
    const raw = current.candidate.kind === "preset"
      ? JSON.stringify(facade.exportThemePreset(current.candidate.presetId), null, 2)
      : current.candidate.kind === "custom"
        ? current.candidate.recipeJson
        : null;
    if (raw === null) throw new Error(t("themeWorkbench.noRecipe", { defaultValue: "Choose Customize or create a new theme first." }));
    checkRecipeSize(raw);
    if (current.candidate.kind === "preset") {
      const validated = facade.validateThemeRecipe(raw);
      checkRecipeSize(validated);
    }
    return raw;
  }

  function download() {
    try {
      const recipe = recipeForExchange();
      downloadBlob(new Blob([recipe], { type: "application/json;charset=utf-8" }), "merman-theme.json");
      setActionError(null);
    } catch (cause) {
      setActionError(projectError(cause).summary);
    }
  }

  async function copyJson() {
    try {
      await navigator.clipboard.writeText(recipeForExchange());
      setActionError(null);
      toast.success(t("themeWorkbench.copiedJson", { defaultValue: "Theme JSON copied" }), { position: "top-center" });
    } catch (cause) {
      setActionError(projectError(cause).summary);
    }
  }

  return (
    <aside
      id="theme-workbench"
      role="region"
      aria-label={t("themeWorkbench.title", { defaultValue: "Theme workbench" })}
      data-theme-workbench
      className={cn("flex min-h-0 min-w-0 flex-col overflow-hidden border-t bg-background lg:border-l lg:border-t-0", className)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented && !pendingReplacement) {
          event.preventDefault();
          event.stopPropagation();
          requestReplacement(close, true);
        }
      }}
    >
      <header className="flex shrink-0 items-start justify-between gap-2 border-b px-3 py-3">
        <div className="min-w-0 space-y-1">
          <h2 ref={heading} id={headingId} tabIndex={-1} className="flex items-center gap-2 text-sm font-semibold outline-none"><Palette className="size-4" aria-hidden="true" />{t("themeWorkbench.title", { defaultValue: "Theme workbench" })}</h2>
          <p className="truncate text-xs text-muted-foreground">{candidateLabel}</p>
        </div>
        <Button variant="ghost" size="icon-sm" aria-label={t("themeWorkbench.close", { defaultValue: "Close theme workbench" })} onClick={() => requestReplacement(close, true)}><X className="size-4" /></Button>
      </header>
      <div className="shrink-0 space-y-2 border-b px-3 py-2">
        <div className="flex flex-wrap items-center justify-between gap-2 text-xs" data-theme-draft-status aria-live="polite">
          <span className={session.validation === "invalid" ? "text-destructive" : "text-muted-foreground"}>
            {session.importing ? t("customTheme.importing") : session.validation === "pending" ? t("themeWorkbench.validating", { defaultValue: "Validating recipe…" }) : session.validation === "invalid" ? t("themeWorkbench.invalidDraft", { defaultValue: "Invalid draft · showing last valid theme" }) : dirty ? t("themeWorkbench.draftPreview", { defaultValue: "Draft preview · Apply to share or export" }) : t("themeWorkbench.applied", { defaultValue: "Applied theme" })}
          </span>
          {session.validation === "valid" && <CheckCircle2 className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />}
        </div>
        <div className="flex flex-wrap gap-1.5">
          {session.candidate.kind === "preset" && <Button type="button" variant="secondary" size="sm" disabled={!facade || !selectedPreset?.available} onClick={customize}>{t("themeWorkbench.customize", { defaultValue: "Customize" })}</Button>}
          <Button type="button" variant="outline" size="sm" disabled={!facade} onClick={() => requestReplacement(() => replaceRaw(JSON.stringify(NEW_THEME, null, 2), "visual"))}><Plus className="size-3.5" />{t("customTheme.new")}</Button>
          <Button type="button" variant="outline" size="sm" onClick={() => requestReplacement(() => fileInput.current?.click())}><FileUp className="size-3.5" />{t("customTheme.import")}</Button>
          <input ref={fileInput} type="file" accept=".json,application/json" className="hidden" aria-label={t("customTheme.import")} onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void importFile(file); }} />
        </div>
      </div>
      <Tabs value={tab} onValueChange={(value) => setTab(value as WorkbenchTab)} className="min-h-0 flex-1 gap-0">
        <div className="shrink-0 px-3 pt-2"><TabsList className="w-full" aria-label={t("themeWorkbench.editorTabs", { defaultValue: "Theme editor views" })}>
          <TabsTrigger value="presets">{t("themeWorkbench.presets", { defaultValue: "Presets" })}</TabsTrigger>
          <TabsTrigger value="visual">{t("themeWorkbench.visual", { defaultValue: "Visual" })}</TabsTrigger>
          <TabsTrigger value="json">JSON</TabsTrigger>
        </TabsList></div>
        <TabsContent value="presets" className="min-h-0 overflow-y-auto overscroll-contain p-3">
          <ThemePresetCards presets={catalog?.presets ?? []} candidate={session.candidate} familyId={familyId} onSelect={selectCandidate} />
          {session.candidate.kind === "preset" && !selectedPreset && <p role="status" className="mt-3 break-words text-xs text-muted-foreground">{t("themeWorkbench.unavailable", { defaultValue: "Unavailable in this runtime" })}. {t("themeWorkbench.unknownPreset", { defaultValue: "Selected preset {{id}} is unavailable in this catalog. Its selection is preserved.", id: session.candidate.presetId })}</p>}
          {selectedPreset && !selectedPreset.available && <p role="status" className="mt-3 break-words text-xs text-destructive">{t("themeWorkbench.unavailable", { defaultValue: "Unavailable in this runtime" })}{selectedPreset.availability_reason_ids.length ? `: ${selectedPreset.availability_reason_ids.join(", ")}` : ""}</p>}
          {selectedPreset && <p data-theme-design-scope className="mt-3 text-xs text-muted-foreground">{t("themeWorkbench.currentDesign", { defaultValue: "Current family: {{family}} · {{treatment}}", family: familyId ?? "unknown", treatment: t(`themeDesign.${selectedTreatment}`, { defaultValue: selectedTreatment === "dedicated" ? "Dedicated family design" : selectedTreatment === "base_only" ? "Base styling only" : "Design unreviewed" }) })}</p>}
          <div className="mt-4 space-y-2 border-t pt-3">
            <label htmlFor={`${headingId}-mermaid-theme`} className="block text-xs font-medium">{t("themeWorkbench.mermaidTheme", { defaultValue: "Mermaid compatibility theme" })}</label>
            <select id={`${headingId}-mermaid-theme`} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" value={diagramTheme} onChange={(event) => useAppStore.getState().setDiagramTheme(event.target.value as MermaidThemeSelection)}>
              <option value="auto">{t("themes.auto", { defaultValue: "Automatic" })}</option>
              {mermaidThemes.map((theme) => <option key={theme} value={theme}>{t(`themes.${theme}`, { defaultValue: theme })}</option>)}
            </select>
            <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.compatibilityHint", { defaultValue: "This workspace setting affects both engines. Merman recipes and source styles can override parts of it." })}</p>
          </div>
        </TabsContent>
        <TabsContent value="visual" className="min-h-0 overflow-y-auto overscroll-contain p-3">
          {session.document ? <ThemeVisualEditor key={`${session.id}:${session.document.kind}`} document={session.document} disabled={!valid || !facade} facade={facade} catalog={catalog} familyId={familyId} onChange={(document) => replaceRaw(JSON.stringify(document, null, 2), "visual")} /> : <div className="space-y-3 py-2 text-xs text-muted-foreground">
            <p>{session.rawJson !== null ? t("themeWorkbench.fixJson", { defaultValue: "Fix the JSON recipe to use Visual controls. The last valid theme remains in the preview." }) : t("themeWorkbench.customizeHint", { defaultValue: "Customize the selected preset or create a new theme to edit its recipe." })}</p>
            {session.rawJson !== null && <Button variant="outline" size="sm" onClick={() => setTab("json")}>{t("themeWorkbench.editJson", { defaultValue: "Edit JSON" })}</Button>}
          </div>}
        </TabsContent>
        <TabsContent value="json" className="min-h-0 overflow-hidden p-0 pt-2">
          {session.rawJson !== null ? <LazyFeatureBoundary feature={t("themeWorkbench.jsonEditor", { defaultValue: "Theme JSON editor" })} presentation={{ kind: "panel" }}><ThemeJsonEditor sessionId={session.id} value={session.rawJson} onChange={(raw) => { setActionError(null); useAppStore.getState().editThemeDraft(raw); }} /></LazyFeatureBoundary> : <p className="p-3 text-xs leading-relaxed text-muted-foreground">{t("themeWorkbench.customizeHint", { defaultValue: "Customize the selected preset or create a new theme to edit its recipe." })}</p>}
        </TabsContent>
      </Tabs>
      {(session.error || actionError) && <div role="alert" className="pointer-events-none max-h-28 shrink-0 overflow-y-auto border-t border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive"><p className="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">{actionError ?? session.error}</p></div>}
      <div className="shrink-0 space-y-2 border-t bg-background px-3 py-2">
        <div className="flex flex-wrap gap-1.5">
          <Button type="button" variant="ghost" size="sm" disabled={!facade || !valid || session.candidate.kind === "none"} onClick={download}><Download className="size-3.5" />{t("customTheme.download")}</Button>
          <Button type="button" variant="ghost" size="sm" disabled={!facade || !valid || session.candidate.kind === "none"} onClick={() => void copyJson()}><Copy className="size-3.5" />{t("themeWorkbench.copyJson", { defaultValue: "Copy JSON" })}</Button>
        </div>
        <div className="flex items-center justify-end gap-2">
          <Button type="button" variant="outline" size="sm" onClick={close}>{t("customTheme.cancel")}</Button>
          <Button
            type="button"
            size="sm"
            disabled={!valid || !facade}
            onClick={() => {
              if (useAppStore.getState().applyThemeEdit()) {
                setActionError(null);
              }
            }}
          >
            {t("customTheme.apply")}
          </Button>
        </div>
      </div>
      <Dialog open={pendingReplacement !== null} onOpenChange={(open) => { if (!open) setPendingReplacement(null); }}>
        <DialogContent className="max-w-sm" showCloseButton={false}>
          <DialogHeader><DialogTitle>{t("themeWorkbench.discardTitle", { defaultValue: "Discard theme changes?" })}</DialogTitle><DialogDescription>{t("themeWorkbench.discardDescription", { defaultValue: "This replaces your unapplied theme draft. Source, font and configuration edits are kept." })}</DialogDescription></DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setPendingReplacement(null)}>{t("themeWorkbench.keepEditing", { defaultValue: "Keep editing" })}</Button>
            <Button
              variant="destructive"
              onClick={() => {
                const action = pendingReplacement;
                setPendingReplacement(null);
                action?.();
              }}
            >
              {t("themeWorkbench.discardReplace", { defaultValue: "Discard and replace" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </aside>
  );
}
