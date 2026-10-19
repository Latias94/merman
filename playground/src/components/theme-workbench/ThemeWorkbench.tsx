import {
  lazy,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  CheckCircle2,
  Copy,
  Download,
  FileUp,
  Palette,
  Plus,
  Redo2,
  Undo2,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  normalizeThemeName,
  SUPPORTED_THEMES,
  type ThemeCatalog,
  type ThemeRecipeV1,
} from "@mermanjs/web";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { downloadBlob } from "@/src/lib/export";
import type { MermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import { SHARE_THEME_RECIPE_BYTES } from "@/src/lib/share";
import { themeFamilyTreatment } from "@/src/lib/theme-design";
import { validateThemeDocument } from "@/src/lib/theme-document";
import type { ThemeJsonRevealRequest } from "@/src/lib/theme-json-location";
import {
  isThemeDraftChanged,
  type ThemeCandidate,
  type ThemeEditGrouping,
  type ThemeValidationToken,
} from "@/src/lib/theme-edit-session";
import { exceedsUtf8ByteBudget } from "@/src/lib/utf8";
import { projectError } from "@/src/runtime/error-projection";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import {
  selectCurrentDiagramType,
  selectVisibleMermanRenderMetadata,
  selectVisibleRenderBatch,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import { useAppStore } from "@/src/store";

import { ThemePresetCards } from "./ThemePresetCards";
import { ThemeVisualEditor } from "./ThemeVisualEditor";

const ThemeJsonEditor = lazy(() =>
  import("./ThemeJsonEditorFeature").then((module) => ({
    default: module.ThemeJsonEditor,
  })),
);

const NEW_THEME: ThemeRecipeV1 = {
  schema_version: 1,
  kind: "design",
  design: {
    tokens: {
      background: { type: "paint", label: "Canvas paint", group: "Canvas", description: "Paint included in the exported image. Preview backing is a separate viewing preference.", value: "#fffaf0" },
      accent: { type: "color", label: "Accent", group: "Core colors", description: "Shared by the node border and connecting lines in this starter.", value: "#f16b50" },
      surface: { type: "paint", label: "Surface", group: "Core colors", description: "Fill of the node body, independent from its label.", value: "#ffffff" },
      text: { type: "paint", label: "Text", group: "Core colors", description: "Paint of the node label.", value: "#202020" },
    },
    canvas: { base: { ref: "background" } },
    styles: {
      node: {
        fill: { ref: "surface" },
        stroke: {
          paint: { kind: "solid", color: { ref: "accent" } },
          width: 2,
        },
      },
      "node-label": { fill: { ref: "text" } },
      edge: {
        stroke: {
          paint: { kind: "solid", color: { ref: "accent" } },
          width: 2,
        },
      },
    },
  },
};

type WorkbenchTab = "presets" | "visual" | "inspect" | "json";

export function ThemeWorkbench({
  className,
  onClose,
}: {
  className?: string;
  onClose(): void;
}) {
  const { t } = useTranslation();
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const visibleDiagramType = useRenderCoordinator(selectCurrentDiagramType);
  const visibleBatch = useRenderCoordinator(selectVisibleRenderBatch);
  const renderMetadata = useRenderCoordinator(
    selectVisibleMermanRenderMetadata,
  );
  const session = useAppStore((state) => state.themeEditSession);
  const diagramTheme = useAppStore((state) => state.diagramTheme);
  const [tab, setTab] = useState<WorkbenchTab>(() =>
    session?.rawJson === null ? "presets" : "visual",
  );
  const [pendingReplacement, setPendingReplacement] = useState<
    (() => void) | null
  >(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [visualDraft, setVisualDraft] = useState<{
    sessionId: number;
    raw: string;
    document: ThemeRecipeV1;
  } | null>(null);
  const [jsonNavigation, setJsonNavigation] = useState<{
    request: ThemeJsonRevealRequest;
    returnTokenId?: string;
  } | null>(null);
  const [visualReturn, setVisualReturn] = useState<{
    sessionId: number;
    tokenId: string;
  } | null>(null);

  const catalog = useMemo(() => {
    try {
      return facade?.themeCatalog() ?? null;
    } catch {
      return null;
    }
  }, [facade]);
  const familyId = useMemo(() => {
    try {
      return facade
        ?.diagramFamilyCapabilities()
        .find((family) => family.diagram_type === visibleDiagramType)
        ?.family_id;
    } catch {
      return undefined;
    }
  }, [facade, visibleDiagramType]);
  const mermaidThemes = useMemo(() => {
    try {
      return [
        ...new Set(
          (facade?.getThemes() ?? SUPPORTED_THEMES).map(normalizeThemeName),
        ),
      ];
    } catch {
      return [...SUPPORTED_THEMES].map(normalizeThemeName);
    }
  }, [facade]);
  const selectedPreset = catalog?.presets.find(
    (preset) =>
      session?.candidate.kind === "preset" &&
      preset.id === session.candidate.presetId,
  );
  const selectedTreatment = themeFamilyTreatment(selectedPreset, familyId);
  const sessionId = session?.id;
  const revision = session?.revision;
  const rawJson = session?.rawJson;
  const validation = session?.validation;
  const importing = session?.importing;
  const checkRecipeSize = useCallback(
    (raw: string | number) => {
      const exceeded =
        typeof raw === "number"
          ? raw > SHARE_THEME_RECIPE_BYTES
          : exceedsUtf8ByteBudget(raw, SHARE_THEME_RECIPE_BYTES);
      if (exceeded)
        throw new Error(
          t("customTheme.tooLarge", { limit: SHARE_THEME_RECIPE_BYTES / 1024 }),
        );
    },
    [t],
  );

  const validateNow = useCallback(
    (raw: string, token: ThemeValidationToken | null) => {
      if (!token || !facade) return;
      try {
        checkRecipeSize(raw);
        const validated = validateThemeDocument(raw, (text) =>
          facade.validateThemeRecipe(text),
        );
        checkRecipeSize(validated.recipeJson);
        useAppStore.getState().acceptThemeValidation(token, validated);
      } catch (cause) {
        useAppStore
          .getState()
          .rejectThemeValidation(token, projectError(cause).summary);
      }
    },
    [checkRecipeSize, facade],
  );

  useEffect(() => {
    heading.current?.focus();
  }, [sessionId]);

  useEffect(() => {
    if (
      !facade ||
      sessionId === undefined ||
      revision === undefined ||
      rawJson == null ||
      validation !== "pending" ||
      importing
    )
      return;
    const token = { sessionId, revision };
    const timeout = window.setTimeout(() => validateNow(rawJson, token), 180);
    return () => window.clearTimeout(timeout);
  }, [
    facade,
    sessionId,
    revision,
    rawJson,
    validation,
    importing,
    validateNow,
  ]);

  if (!session) return null;

  // Form-authored values remain editable after semantic rejection. The preview retains its
  // last valid candidate; arbitrary JSON imports never enter these controls.
  const visualDocument =
    session.document ??
    (visualDraft?.sessionId === session.id &&
    visualDraft.raw === session.rawJson
      ? visualDraft.document
      : null);
  const dirty = isThemeDraftChanged(session);
  const valid = session.validation === "valid" && !session.importing;
  const activeNavigation = jsonNavigation?.request.sessionId === session.id ? jsonNavigation : null;
  const reveal = activeNavigation?.request.revision === session.revision && activeNavigation.request.rawJson === session.rawJson
    ? activeNavigation.request : undefined;
  const returnTokenId = activeNavigation?.returnTokenId;
  const canReturnToSetting = returnTokenId !== undefined && visualDocument?.kind === "design" && Object.hasOwn(visualDocument.design.tokens ?? {}, returnTokenId);
  const initialTokenId = visualReturn?.sessionId === session.id && visualDocument?.kind === "design" && Object.hasOwn(visualDocument.design.tokens ?? {}, visualReturn.tokenId)
    ? visualReturn.tokenId : undefined;
  const candidateLabel =
    session.candidate.kind === "none"
      ? t("themeWorkbench.none", { defaultValue: "None" })
      : session.candidate.kind === "custom"
        ? t("themeWorkbench.custom", { defaultValue: "Custom recipe" })
        : t(`themePresets.${session.candidate.presetId}`, {
            defaultValue:
              selectedPreset?.display_name ?? session.candidate.presetId,
          });

  function replaceRaw(
    raw: string,
    nextTab: WorkbenchTab,
    group?: ThemeEditGrouping["key"],
  ) {
    setActionError(null);
    const token = useAppStore.getState().editThemeDraft(raw, group);
    validateNow(raw, token);
    setTab(nextTab);
  }

  function openJsonLocation(path: string, tokenId?: string) {
    if (session?.rawJson == null) return;
    setJsonNavigation({ request: { sessionId: session.id, revision: session.revision, rawJson: session.rawJson, pointer: `/design${path}` }, returnTokenId: tokenId });
    setVisualReturn(null);
    setTab("json");
  }

  function returnToSetting() {
    if (!session || !canReturnToSetting || returnTokenId === undefined) return;
    setVisualReturn({ sessionId: session.id, tokenId: returnTokenId });
    setJsonNavigation(null);
    setTab("visual");
  }

  function travelHistory(direction: "undo" | "redo") {
    if (!useAppStore.getState().travelThemeHistory(direction)) return;
    const current = useAppStore.getState().themeEditSession;
    if (current?.rawJson !== null && current?.rawJson !== undefined) {
      validateNow(current.rawJson, {
        sessionId: current.id,
        revision: current.revision,
      });
    }
    setActionError(null);
  }

  function requestReplacement(action: () => void, anyDirty = false) {
    const current = useAppStore.getState().themeEditSession;
    if (
      current &&
      isThemeDraftChanged(current) &&
      (anyDirty || current.rawJson !== null)
    ) {
      setPendingReplacement(() => action);
    } else {
      action();
    }
  }

  function close() {
    useAppStore.getState().cancelThemeEdit();
    onClose();
  }

  function selectCandidate(
    candidate: Exclude<ThemeCandidate, { kind: "custom" }>,
  ) {
    const current = useAppStore.getState().themeEditSession?.candidate;
    if (
      current?.kind === candidate.kind &&
      (current.kind !== "preset" ||
        candidate.kind !== "preset" ||
        current.presetId === candidate.presetId)
    )
      return;
    requestReplacement(() => {
      useAppStore.getState().selectThemeCandidate(candidate);
      setActionError(null);
    });
  }

  function customize(presetId?: string) {
    const candidate = useAppStore.getState().themeEditSession?.candidate;
    const id =
      presetId ??
      (candidate?.kind === "preset" ? candidate.presetId : undefined);
    if (!facade || !id) return;
    try {
      replaceRaw(
        JSON.stringify(facade.exportThemePreset(id), null, 2),
        "visual",
      );
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
      const validationToken = useAppStore
        .getState()
        .completeThemeImport(token, raw);
      if (!validationToken) return;
      validateNow(raw, validationToken);
      setTab("json");
    } catch (cause) {
      useAppStore
        .getState()
        .failThemeImport(token, projectError(cause).summary);
    }
  }

  function recipeForExchange(): string {
    if (!facade) throw new Error(t("wasm.notLoaded"));
    const current = useAppStore.getState().themeEditSession;
    if (!current || current.importing || current.validation !== "valid")
      throw new Error(
        t("themeWorkbench.validRecipeRequired", {
          defaultValue: "Fix the recipe before exporting it.",
        }),
      );
    const raw =
      current.candidate.kind === "preset"
        ? JSON.stringify(
            facade.exportThemePreset(current.candidate.presetId),
            null,
            2,
          )
        : current.candidate.kind === "custom"
          ? current.candidate.recipeJson
          : null;
    if (raw === null)
      throw new Error(
        t("themeWorkbench.noRecipe", {
          defaultValue: "Choose Customize or create a new theme first.",
        }),
      );
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
      downloadBlob(
        new Blob([recipe], { type: "application/json;charset=utf-8" }),
        "merman-theme.json",
      );
      setActionError(null);
    } catch (cause) {
      setActionError(projectError(cause).summary);
    }
  }

  async function copyJson() {
    try {
      await navigator.clipboard.writeText(recipeForExchange());
      setActionError(null);
      toast.success(
        t("themeWorkbench.copiedJson", { defaultValue: "Theme JSON copied" }),
        { position: "top-center" },
      );
    } catch (cause) {
      setActionError(projectError(cause).summary);
    }
  }

  return (
    <aside
      id="theme-workbench"
      role="region"
      aria-label={t("themeWorkbench.title", {
        defaultValue: "Theme workbench",
      })}
      data-theme-workbench
      onFocusCapture={() => useAppStore.getState().endThemeEditGrouping()}
      onPointerDownCapture={() => useAppStore.getState().endThemeEditGrouping()}
      onKeyDownCapture={(event) => {
        const target = event.target as HTMLElement;
        if (
          (target instanceof HTMLInputElement ||
            target instanceof HTMLTextAreaElement) &&
          !target.closest(".monaco-editor")
        )
          return;
        if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
        const key = event.key.toLowerCase();
        if (key !== "z" && key !== "y") return;
        event.preventDefault();
        event.stopPropagation();
        travelHistory(key === "y" || event.shiftKey ? "redo" : "undo");
      }}
      className={cn(
        "flex min-h-0 min-w-0 flex-col overflow-hidden border-t bg-background lg:border-l lg:border-t-0",
        className,
      )}
      onKeyDown={(event) => {
        if (
          event.key === "Escape" &&
          !event.defaultPrevented &&
          !pendingReplacement
        ) {
          event.preventDefault();
          event.stopPropagation();
          requestReplacement(close, true);
        }
      }}
    >
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain">
      <header className="flex shrink-0 items-start justify-between gap-2 border-b px-3 py-3">
        <div className="min-w-0 space-y-1">
          <h2
            ref={heading}
            id={headingId}
            tabIndex={-1}
            className="flex items-center gap-2 text-sm font-semibold outline-none"
          >
            <Palette className="size-4" aria-hidden="true" />
            {t("themeWorkbench.title", { defaultValue: "Theme workbench" })}
          </h2>
          <p className="truncate text-xs text-muted-foreground">
            {candidateLabel}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={t("themeWorkbench.undo")}
            title={t("themeWorkbench.undo")}
            disabled={!session.past.length}
            onClick={() => travelHistory("undo")}
          >
            <Undo2 className="size-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={t("themeWorkbench.redo")}
            title={t("themeWorkbench.redo")}
            disabled={!session.future.length}
            onClick={() => travelHistory("redo")}
          >
            <Redo2 className="size-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={t("themeWorkbench.close", {
              defaultValue: "Close theme workbench",
            })}
            onClick={() => requestReplacement(close, true)}
          >
            <X className="size-4" />
          </Button>
        </div>
      </header>
      <div className="shrink-0 space-y-2 border-b px-3 py-2">
        <div
          className="flex flex-wrap items-center justify-between gap-2 text-xs"
          data-theme-draft-status
          aria-live="polite"
        >
          <span
            className={
              session.validation === "invalid"
                ? "text-destructive"
                : "text-muted-foreground"
            }
          >
            {session.importing
              ? t("customTheme.importing")
              : session.validation === "pending"
                ? t("themeWorkbench.validating", {
                    defaultValue: "Validating recipe…",
                  })
                : session.validation === "invalid"
                  ? t("themeWorkbench.invalidDraft", {
                      defaultValue: "Invalid draft · showing last valid theme",
                    })
                  : dirty
                    ? t("themeWorkbench.draftPreview", {
                        defaultValue:
                          "Draft preview · Apply to share or export",
                      })
                    : t("themeWorkbench.applied", {
                        defaultValue: "Applied theme",
                      })}
          </span>
          {session.validation === "valid" && (
            <CheckCircle2
              className="size-3.5 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
          )}
        </div>
        <div className="flex flex-wrap gap-1.5">
          {session.candidate.kind === "preset" && (
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={!facade || !selectedPreset?.available}
              onClick={() => customize()}
            >
              {t("themeWorkbench.customize", { defaultValue: "Customize" })}
            </Button>
          )}
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!facade}
            onClick={() =>
              requestReplacement(() =>
                replaceRaw(JSON.stringify(NEW_THEME, null, 2), "visual"),
              )
            }
          >
            <Plus className="size-3.5" />
            {t("customTheme.new")}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => requestReplacement(() => fileInput.current?.click())}
          >
            <FileUp className="size-3.5" />
            {t("customTheme.import")}
          </Button>
          <input
            ref={fileInput}
            type="file"
            accept=".json,application/json"
            className="hidden"
            aria-label={t("customTheme.import")}
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = "";
              if (file) void importFile(file);
            }}
          />
        </div>
      </div>
      <Tabs
        value={tab}
        onValueChange={(value) => {
          setVisualReturn(null);
          setJsonNavigation(null);
          setTab(value as WorkbenchTab);
        }}
        className="min-h-64 flex-1 gap-0"
      >
        <div className="shrink-0 px-3 pt-2">
          <TabsList
            className="grid h-auto w-full grid-cols-2 gap-1 sm:grid-cols-4"
            aria-label={t("themeWorkbench.editorTabs", {
              defaultValue: "Theme editor views",
            })}
          >
            <TabsTrigger value="presets">
              {t("themeWorkbench.presets", { defaultValue: "Presets" })}
            </TabsTrigger>
            <TabsTrigger value="visual">
              {t("themeWorkbench.visual", { defaultValue: "Visual" })}
            </TabsTrigger>
            <TabsTrigger value="inspect">
              {t("themeWorkbench.inspect", { defaultValue: "Inspect" })}
            </TabsTrigger>
            <TabsTrigger value="json">JSON</TabsTrigger>
          </TabsList>
        </div>
        <TabsContent
          value="presets"
          className="min-h-0 overflow-y-auto overscroll-contain p-3"
        >
          <section className="mb-4 space-y-2 rounded-md border p-3">
            <label
              htmlFor={`${headingId}-mermaid-theme`}
              className="block text-xs font-semibold"
            >
              {t("themeWorkbench.mermaidTheme")}
            </label>
            <select
              id={`${headingId}-mermaid-theme`}
              className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={diagramTheme}
              onChange={(event) =>
                useAppStore
                  .getState()
                  .setDiagramTheme(event.target.value as MermaidThemeSelection)
              }
            >
              <option value="auto">
                {t("themes.auto", { defaultValue: "Automatic" })}
              </option>
              {mermaidThemes.map((theme) => (
                <option key={theme} value={theme}>
                  {t(`themes.${theme}`, { defaultValue: theme })}
                </option>
              ))}
            </select>
            <p className="text-[11px] leading-relaxed text-muted-foreground">
              {t("themeWorkbench.compatibilityHint")}
            </p>
          </section>
          <h3 className="mb-2 text-xs font-semibold">
            {t("themeWorkbench.mermanStyling")}
          </h3>
          <ThemePresetCards
            presets={catalog?.presets ?? []}
            candidate={session.candidate}
            familyId={familyId}
            onSelect={selectCandidate}
            onCreate={(id) => requestReplacement(() => customize(id))}
          />
          {session.candidate.kind === "preset" && !selectedPreset && (
            <p
              role="status"
              className="mt-3 break-words text-xs text-muted-foreground"
            >
              {t("themeWorkbench.unavailable", {
                defaultValue: "Unavailable in this runtime",
              })}
              .{" "}
              {t("themeWorkbench.unknownPreset", {
                defaultValue:
                  "Selected preset {{id}} is unavailable in this catalog. Its selection is preserved.",
                id: session.candidate.presetId,
              })}
            </p>
          )}
          {selectedPreset && !selectedPreset.available && (
            <p
              role="status"
              className="mt-3 break-words text-xs text-destructive"
            >
              {t("themeWorkbench.unavailable", {
                defaultValue: "Unavailable in this runtime",
              })}
              {selectedPreset.availability_reason_ids.length
                ? `: ${selectedPreset.availability_reason_ids.join(", ")}`
                : ""}
            </p>
          )}
          {selectedPreset && (
            <p
              data-theme-design-scope
              className="mt-3 text-xs text-muted-foreground"
            >
              {t("themeWorkbench.currentDesign", {
                defaultValue: "Current family: {{family}} · {{treatment}}",
                family: familyId ?? "unknown",
                treatment: t(`themeDesign.${selectedTreatment}`, {
                  defaultValue:
                    selectedTreatment === "dedicated"
                      ? "Dedicated family design"
                      : selectedTreatment === "base_only"
                        ? "Base styling only"
                        : "Design unreviewed",
                }),
              })}
            </p>
          )}
        </TabsContent>
        <TabsContent
          value="visual"
          className="min-h-0 overflow-y-auto overscroll-contain p-3"
        >
          {visualDocument ? (
            <ThemeVisualEditor
              key={`${session.id}:${visualDocument.kind}`}
              document={visualDocument}
              disabled={!facade || session.importing}
              facade={facade}
              catalog={catalog}
              familyId={familyId}
              initialTokenId={initialTokenId}
              onOpenJson={openJsonLocation}
              onChange={(document) => {
                const raw = JSON.stringify(document, null, 2);
                setVisualDraft({ sessionId: session.id, raw, document });
                replaceRaw(raw, "visual", "visual");
              }}
            />
          ) : (
            <div className="space-y-3 py-2 text-xs text-muted-foreground">
              <p>
                {session.rawJson !== null
                  ? t("themeWorkbench.fixJson", {
                      defaultValue:
                        "Fix the JSON recipe to use Visual controls. The last valid theme remains in the preview.",
                    })
                  : t("themeWorkbench.customizeHint", {
                      defaultValue:
                        "Customize the selected preset or create a new theme to edit its recipe.",
                    })}
              </p>
              {session.rawJson !== null && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setTab("json")}
                >
                  {t("themeWorkbench.editJson", { defaultValue: "Edit JSON" })}
                </Button>
              )}
            </div>
          )}
        </TabsContent>
        <TabsContent
          value="inspect"
          className="min-h-0 overflow-y-auto overscroll-contain p-3"
        >
          <ThemeTechnicalDetails
            catalog={catalog}
            preset={selectedPreset}
            familyId={familyId}
            diagramType={visibleDiagramType}
            batch={visibleBatch}
            metadata={renderMetadata}
          />
        </TabsContent>
        <TabsContent value="json" className="min-h-0 overflow-hidden p-0 pt-2 data-[state=active]:flex data-[state=active]:flex-col">
          {activeNavigation && <div className="flex shrink-0 items-center gap-2 border-b px-3 pb-2 text-xs">
            {returnTokenId !== undefined && <Button type="button" variant="outline" size="sm" disabled={!canReturnToSetting} onClick={returnToSetting}>
              {t("themeDesigner.returnToSetting", { defaultValue: "Return to shared setting" })}
            </Button>}
            {!reveal && <p role="status" className="text-muted-foreground">{t("themeDesigner.locationUnavailable", { defaultValue: "This location is no longer available. The document has not been changed." })}</p>}
          </div>}
          {session.rawJson !== null ? (
            <LazyFeatureBoundary
              feature={t("themeWorkbench.jsonEditor", {
                defaultValue: "Theme JSON editor",
              })}
              presentation={{ kind: "panel" }}
            >
              <ThemeJsonEditor
                sessionId={session.id}
                value={session.rawJson}
                reveal={reveal}
                onChange={(raw) => {
                  setActionError(null);
                  useAppStore.getState().editThemeDraft(raw, "json");
                }}
              />
            </LazyFeatureBoundary>
          ) : (
            <p className="p-3 text-xs leading-relaxed text-muted-foreground">
              {t("themeWorkbench.customizeHint", {
                defaultValue:
                  "Customize the selected preset or create a new theme to edit its recipe.",
              })}
            </p>
          )}
        </TabsContent>
      </Tabs>
      {(session.error || actionError) && (
        <div
          role="alert"
          className="pointer-events-none max-h-28 shrink-0 overflow-y-auto border-t border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive"
        >
          <p className="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">
            {actionError ?? session.error}
          </p>
        </div>
      )}
      </div>
      <div className="shrink-0 space-y-2 border-t bg-background px-3 py-2">
        <div className="flex flex-wrap gap-1.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={!facade || !valid || session.candidate.kind === "none"}
            onClick={download}
          >
            <Download className="size-3.5" />
            {t("customTheme.download")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={!facade || !valid || session.candidate.kind === "none"}
            onClick={() => void copyJson()}
          >
            <Copy className="size-3.5" />
            {t("themeWorkbench.copyJson", { defaultValue: "Copy JSON" })}
          </Button>
        </div>
        <div className="flex items-center justify-end gap-2">
          <Button type="button" variant="outline" size="sm" onClick={close}>
            {t("customTheme.cancel")}
          </Button>
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
      <Dialog
        open={pendingReplacement !== null}
        onOpenChange={(open) => {
          if (!open) setPendingReplacement(null);
        }}
      >
        <DialogContent className="max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>
              {t("themeWorkbench.discardTitle", {
                defaultValue: "Discard theme changes?",
              })}
            </DialogTitle>
            <DialogDescription>
              {t("themeWorkbench.discardDescription", {
                defaultValue:
                  "This replaces your unapplied theme draft. Source, font and configuration edits are kept.",
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setPendingReplacement(null)}
            >
              {t("themeWorkbench.keepEditing", {
                defaultValue: "Keep editing",
              })}
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                const action = pendingReplacement;
                setPendingReplacement(null);
                action?.();
              }}
            >
              {t("themeWorkbench.discardReplace", {
                defaultValue: "Discard and replace",
              })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </aside>
  );
}

function ThemeTechnicalDetails({
  catalog,
  preset,
  familyId,
  diagramType,
  batch,
  metadata,
}: {
  catalog: ThemeCatalog | null;
  preset: ThemeCatalog["presets"][number] | undefined;
  familyId?: string;
  diagramType: string;
  batch: ReturnType<typeof selectVisibleRenderBatch>;
  metadata: ReturnType<typeof selectVisibleMermanRenderMetadata>;
}) {
  const { t } = useTranslation();
  const evidence = metadata?.theme_execution_evidence;
  const operation = batch?.snapshot.operation;
  return (
    <div className="space-y-4 text-xs">
      <section className="space-y-2 rounded-md border p-3">
        <h3 className="font-semibold">
          {t("themeWorkbench.currentExecution", {
            defaultValue: "Current execution",
          })}
        </h3>
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 break-words [overflow-wrap:anywhere]">
          <dt className="text-muted-foreground">
            {t("themeWorkbench.diagramType", { defaultValue: "Diagram" })}
          </dt>
          <dd>{diagramType}</dd>
          <dt className="text-muted-foreground">
            {t("themeWorkbench.family", { defaultValue: "Family" })}
          </dt>
          <dd>{familyId ?? "unknown"}</dd>
          <dt className="text-muted-foreground">
            {t("themeWorkbench.recipeKind", { defaultValue: "Theme input" })}
          </dt>
          <dd>
            {operation?.themeRecipeJson
              ? "custom recipe"
              : (operation?.themePresetId ?? "none")}
          </dd>
          <dt className="text-muted-foreground">
            {t("themeWorkbench.executionStatus", {
              defaultValue: "Theme execution",
            })}
          </dt>
          <dd>
            {evidence
              ? String(evidence.theme_status)
              : t("themeWorkbench.evidenceUnavailable", {
                  defaultValue: "Unavailable",
                })}
          </dd>
          <dt className="text-muted-foreground">
            {t("themeWorkbench.targetStatus", {
              defaultValue: "Output target",
            })}
          </dt>
          <dd>
            {evidence
              ? `${evidence.target_kind} · ${evidence.target_status}`
              : "unknown"}
          </dd>
        </dl>
        <p className="leading-relaxed text-muted-foreground">
          {t("themeWorkbench.evidenceExplanation", {
            defaultValue:
              "These results describe this SVG execution. Preset design scope and runtime support are separate; an empty diagnostic list does not certify portability.",
          })}
        </p>
        {evidence && (
          <details>
            <summary className="cursor-pointer text-muted-foreground">
              {t("themeWorkbench.rawEvidence", {
                defaultValue: "Raw execution evidence",
              })}
            </summary>
            <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap text-[11px]">
              {JSON.stringify(evidence, null, 2)}
            </pre>
          </details>
        )}
      </section>
      <section className="space-y-2 rounded-md border p-3">
        <h3 className="font-semibold">
          {t("themeWorkbench.catalogDetails", {
            defaultValue: "Theme catalog details",
          })}
        </h3>
        <p className="leading-relaxed text-muted-foreground">
          {t("themeWorkbench.catalogDetailsHint", {
            defaultValue:
              "These fields describe authoring scope and runtime discovery. They do not gate editing or certify every diagram/output combination.",
          })}
        </p>
        {preset ? (
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 break-words [overflow-wrap:anywhere]">
            <dt className="text-muted-foreground">ID</dt>
            <dd>{preset.id}</dd>
            <dt className="text-muted-foreground">
              {t("themeWorkbench.designScope", {
                defaultValue: "Design scope",
              })}
            </dt>
            <dd>
              {preset.family_designs
                .map((design) => `${design.family_id}: ${design.treatment}`)
                .join("; ") || "none"}
            </dd>
            <dt className="text-muted-foreground">
              {t("themeWorkbench.qualifiedCells", {
                defaultValue: "Qualified cells",
              })}
            </dt>
            <dd>
              {preset.qualified_cells.length ||
                t("themeWorkbench.noneReported", {
                  defaultValue: "none reported",
                })}
            </dd>
            <dt className="text-muted-foreground">
              {t("themeWorkbench.availability", {
                defaultValue: "Availability",
              })}
            </dt>
            <dd>
              {preset.available
                ? "available"
                : preset.availability_reason_ids.join(", ") || "unavailable"}
            </dd>
          </dl>
        ) : (
          <p className="text-muted-foreground">
            {t("themeWorkbench.noPresetDetails", {
              defaultValue:
                "Select a Merman preset to inspect its catalog metadata.",
            })}
          </p>
        )}
        <p className="text-muted-foreground">
          {t("themeWorkbench.catalogCounts", {
            defaultValue:
              "Catalog: {{presets}} presets · {{targets}} semantic targets · {{variants}} variants",
            presets: catalog?.presets.length ?? 0,
            targets: catalog?.known_semantic_target_ids.length ?? 0,
            variants: catalog?.known_variant_ids.length ?? 0,
          })}
        </p>
      </section>
    </div>
  );
}
