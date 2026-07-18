import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";
import { toast } from "sonner";
import {
  selectMermanFacade,
  selectMermanFailure,
  selectMermanLoadStage,
  selectMermanStatus,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { retryMermanRuntime } from "@/src/runtime/merman";
import type { MermanRuntimeFailure } from "@/src/runtime/merman-core";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import {
  asciiSupportDescription,
  asciiSupportLabelKey,
  type AsciiCapability,
} from "@/src/lib/ascii-support";
import { useAppStore } from "@/src/store";
import { MERMAID_JS_VERSION } from "@/src/runtime/mermaid-requirements";
import {
  markRenderCoordinatorPresented,
  refreshRenderCoordinator,
  setCompareEnabled,
  setCompareViewport,
  setDiagnosticsEnabled,
} from "@/src/runtime/render-coordinator-browser";
import {
  selectCompletedRenderBatch,
  selectCurrentDiagramType,
  selectRenderPending,
  selectVisibleRenderBatch,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import type {
  DiagnosticArtifact,
  EngineRenderFailure,
  MermanRenderSuccess,
  MermaidRenderSuccess,
} from "@/src/runtime/render-coordinator";
import { exportPNG, exportSVG } from "@/src/lib/export";
import {
  SvgViewport,
  useSvgViewport,
  type SvgViewportController,
} from "@/src/components/SvgViewport";
import { cn } from "@/lib/utils";
import {
  ZoomIn,
  ZoomOut,
  RotateCcw,
  Maximize2,
  Loader2,
  AlertCircle,
  Copy,
  Check,
  Code2,
  FileCode,
  ImageIcon,
  RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import Editor from "@monaco-editor/react";

interface PreviewProps {
  className?: string;
}

type PreviewMode = "svg" | "ascii" | "compare" | "diagnostics";
type SvgDisplayMode = "visual" | "source";
type EngineKey = "merman" | "mermaid";
type DiagnosticKey = "parse" | "layout";

interface CompareArtifact {
  key: EngineKey;
  artifactKey: string;
  presentationKey: number | null;
  actionsEnabled: boolean;
  title: string;
  version: string;
  svg: string | null;
  error: string | null;
  renderTime: number | null;
  loading: boolean;
  loadingLabel: string | null;
  unavailableLabel: string | null;
  stale: boolean;
}

interface ComparePaneModel {
  artifact: CompareArtifact;
  controller: SvgViewportController;
  viewportHostRef?: RefObject<HTMLDivElement | null>;
}

interface ComparePaneActions {
  copiedSvgKey: string | null;
  exportingPngEngines: ReadonlySet<EngineKey>;
  onCopySvg(svg: string | null, actionKey: string): void;
  onExportPng(engine: EngineKey, svg: string | null): void;
  onExportSvg(engine: EngineKey, svg: string | null): void;
  onPresentationReady(engine: EngineKey, at: number): void;
}

const EMPTY_DIAGNOSTICS: Record<DiagnosticKey, DiagnosticArtifact> = {
  parse: { json: null, error: null, elapsedMs: null },
  layout: { json: null, error: null, elapsedMs: null },
};

export function Preview({ className }: PreviewProps) {
  const { t } = useTranslation();
  const code = useAppStore((state) => state.code);
  const isDarkMode = useAppStore((state) => state.resolvedTheme === "dark");
  const previewMode = useAppStore((state) => state.previewMode);
  const setPreviewMode = useAppStore((state) => state.setPreviewMode);
  const renderState = useRenderCoordinator((state) => state);
  const currentBatch = selectCompletedRenderBatch(renderState);
  const visibleBatch = selectVisibleRenderBatch(renderState);
  const facade = useMermanRuntime(selectMermanFacade);
  const runtimeStatus = useMermanRuntime(selectMermanStatus);
  const runtimeFailure = useMermanRuntime(selectMermanFailure);
  const runtimeLoadStage = useMermanRuntime(selectMermanLoadStage);
  const ready = facade !== null;
  const loading = runtimeStatus === "idle" || runtimeStatus === "loading";
  const asciiSupport = useAsciiSupport();
  const [svgDisplayMode, setSvgDisplayMode] =
    useState<SvgDisplayMode>("visual");
  const [copiedAsciiKey, setCopiedAsciiKey] = useState<string | null>(null);
  const [copiedDiagnostic, setCopiedDiagnostic] = useState<DiagnosticKey | null>(null);
  const [copiedSvgKey, setCopiedSvgKey] = useState<string | null>(null);
  const [exportingPngEngines, setExportingPngEngines] = useState<Set<EngineKey>>(
    () => new Set()
  );
  const exportingPngEnginesRef = useRef<Set<EngineKey>>(new Set());
  const [diagnosticTab, setDiagnosticTab] = useState<DiagnosticKey>("parse");
  const compareHostRef = useRef<HTMLDivElement>(null);
  const asciiCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const diagnosticCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const previewRenderKey = currentBatch?.snapshot.key ?? "render-unavailable";
  const mermanSvgActionKey = useMemo(
    () => artifactActionKey("merman-svg", previewRenderKey),
    [previewRenderKey]
  );
  const mermaidSvgActionKey = useMemo(
    () => artifactActionKey("mermaid-svg", previewRenderKey),
    [previewRenderKey]
  );
  const asciiActionKey = useMemo(
    () => artifactActionKey("merman-ascii", previewRenderKey),
    [previewRenderKey]
  );
  const detectedDiagramType = selectCurrentDiagramType(renderState);
  const currentMerman = successfulMerman(currentBatch?.merman ?? null);
  const svg = currentMerman?.svg ?? null;
  const ascii = currentMerman?.ascii ?? null;
  const error = failedMessage(currentBatch?.merman ?? null);
  const visibleMerman = successfulMerman(visibleBatch?.merman ?? null);
  const visibleMermaid = successfulMermaid(visibleBatch?.mermaid ?? null);
  const mermaidSvg = visibleMermaid?.svg ?? null;
  const mermaidError = failedMessage(visibleBatch?.mermaid ?? null);
  const mermaidRenderTime = visibleMermaid?.renderTimeMs ?? null;
  const renderPending = selectRenderPending(renderState);
  const mermanRendering = Boolean(code.trim() && ready && renderPending);
  const actionsEnabled = currentBatch?.actionsEnabled ?? false;
  const compareStale = renderState.status === "updating";
  const diagnostics = currentBatch?.diagnostics ?? EMPTY_DIAGNOSTICS;
  const diagnosticsLoading = previewMode === "diagnostics" && renderPending;
  const isAsciiSupported = asciiSupport.isSupported(detectedDiagramType);
  const asciiCapability = asciiSupport.capabilityFor(detectedDiagramType);
  const asciiSupportLabel = t(asciiSupportLabelKey(asciiCapability));
  const asciiSupportLimit = asciiSupportDescription(asciiCapability);
  const svgViewport = useSvgViewport({
    svg,
    enabled: previewMode === "svg",
  });
  const mermanCompareViewport = useSvgViewport({
    svg: visibleMerman?.svg ?? null,
    enabled: previewMode === "compare",
  });
  const mermaidCompareViewport = useSvgViewport({
    svg: mermaidSvg,
    enabled: previewMode === "compare",
  });
  useEffect(() => {
    if (previewMode === "ascii" && !isAsciiSupported) {
      setPreviewMode("svg");
    }
  }, [isAsciiSupported, previewMode, setPreviewMode]);

  useEffect(() => {
    setDiagnosticsEnabled(previewMode === "diagnostics");
    return () => setDiagnosticsEnabled(false);
  }, [previewMode]);

  useLayoutEffect(() => {
    if (previewMode !== "compare") {
      setCompareEnabled(false);
      setCompareViewport(null);
      return;
    }

    const host = compareHostRef.current;
    if (!host) return;
    const updateViewport = () => {
      const width = Math.floor(host.clientWidth);
      const height = Math.floor(host.clientHeight);
      if (width <= 0 || height <= 0) return;
      setCompareViewport({ width, height });
      setCompareEnabled(true);
    };
    updateViewport();
    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(updateViewport);
    observer?.observe(host);
    return () => {
      observer?.disconnect();
      setCompareEnabled(false);
      setCompareViewport(null);
    };
  }, [previewMode]);

  useEffect(() => {
    return () => {
      if (copyTimeoutRef.current) {
        clearTimeout(copyTimeoutRef.current);
      }
      if (diagnosticCopyTimeoutRef.current) {
        clearTimeout(diagnosticCopyTimeoutRef.current);
      }
      if (asciiCopyTimeoutRef.current) {
        clearTimeout(asciiCopyTimeoutRef.current);
      }
    };
  }, []);

  const handleCopyAscii = useCallback(async () => {
    try {
      if (!actionsEnabled || !ascii) {
        throw new Error("Current ASCII artifact is unavailable.");
      }
      await navigator.clipboard.writeText(ascii);
      setCopiedAsciiKey(asciiActionKey);
      if (asciiCopyTimeoutRef.current) {
        clearTimeout(asciiCopyTimeoutRef.current);
      }
      asciiCopyTimeoutRef.current = setTimeout(
        () => setCopiedAsciiKey(null),
        2000
      );
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [actionsEnabled, ascii, asciiActionKey, t]);

  const handleCopySvg = useCallback(
    async (svgValue: string | null, actionKey: string) => {
      try {
        if (!actionsEnabled || !svgValue) {
          throw new Error("Current SVG artifact is unavailable.");
        }
        const safeSvg = requireSafeSvgString(svgValue);
        await navigator.clipboard.writeText(safeSvg);
        setCopiedSvgKey(actionKey);
        if (copyTimeoutRef.current) {
          clearTimeout(copyTimeoutRef.current);
        }
        copyTimeoutRef.current = setTimeout(() => setCopiedSvgKey(null), 2000);
        toast.success(t("share.copied"));
      } catch {
        toast.error(t("share.copyFailed"));
      }
    },
    [actionsEnabled, t]
  );

  const handleCopyDiagnosticJson = useCallback(async () => {
    const json = diagnostics[diagnosticTab].json;
    if (!json) return;

    try {
      await navigator.clipboard.writeText(json);
      setCopiedDiagnostic(diagnosticTab);
      if (diagnosticCopyTimeoutRef.current) {
        clearTimeout(diagnosticCopyTimeoutRef.current);
      }
      diagnosticCopyTimeoutRef.current = setTimeout(
        () => setCopiedDiagnostic(null),
        2000
      );
    } catch (err) {
      console.error("Failed to copy diagnostics JSON:", err);
    }
  }, [diagnosticTab, diagnostics]);

  const handleExportSvg = useCallback(
    (engine: EngineKey, svgValue: string | null) => {
      try {
        if (!actionsEnabled || !svgValue) {
          throw new Error("Current SVG artifact is unavailable.");
        }
        const safeSvg = requireSafeSvgString(svgValue);
        exportSVG(safeSvg, `merman-compare-${engine}`);
        toast.success(t("export.svgSuccess"));
      } catch {
        toast.error(t("export.failed"));
      }
    },
    [actionsEnabled, t]
  );

  const handleExportPng = useCallback(async (
    engine: EngineKey,
    svgValue: string | null
  ) => {
    if (exportingPngEnginesRef.current.has(engine)) {
      return;
    }
    exportingPngEnginesRef.current.add(engine);
    setExportingPngEngines(new Set(exportingPngEnginesRef.current));
    try {
      if (!actionsEnabled || !svgValue || !currentBatch) {
        throw new Error("Current SVG artifact is unavailable.");
      }
      const safeValue = requireSafeSvgString(svgValue);
      let exportSvg = safeValue;
      if (engine === "merman") {
        const snapshot = currentBatch.snapshot;
        const pngResult = facade?.render(
          snapshot.source,
          snapshot.theme,
          snapshot.configJson,
          { ...snapshot.options, pipeline: "resvg-safe" }
        );
        if (!pngResult?.svg) {
          throw new Error(pngResult?.error ?? "Failed to render PNG SVG");
        }
        exportSvg = requireSafeSvgString(pngResult.svg);
      }

      await exportPNG(exportSvg, `merman-compare-${engine}`, 2);
      toast.success(t("export.pngSuccess"));
    } catch {
      toast.error(t("export.failed"));
    } finally {
      exportingPngEnginesRef.current.delete(engine);
      setExportingPngEngines(new Set(exportingPngEnginesRef.current));
    }
  }, [actionsEnabled, currentBatch, facade, t]);

  const handleRefreshCompare = useCallback(() => {
    refreshRenderCoordinator();
  }, []);

  const copiedAscii = copiedAsciiKey === asciiActionKey;
  const mermanSvgUnavailableLabel = artifactUnavailableLabel({
    value: svg,
    error,
    loading: mermanRendering,
    t,
  });
  const mermaidRendering = Boolean(
    previewMode === "compare" &&
      code.trim() &&
      renderPending
  );
  const mermaidLoadingLabel = mermaidRendering
    ? t("preview.renderingCurrent")
    : null;
  const mermaidSvgUnavailableLabel = artifactUnavailableLabel({
    value: mermaidSvg,
    error: mermaidError,
    loading: mermaidRendering,
    t,
  });

  const renderTabBar = (rightContent?: ReactNode) => (
    <TabBar
      mode={previewMode}
      onModeChange={setPreviewMode}
      runtimeReady={ready}
      isAsciiSupported={isAsciiSupported}
      asciiCapability={asciiCapability}
      asciiSupportLabel={asciiSupportLabel}
      asciiSupportLimit={asciiSupportLimit}
      t={t}
      rightContent={rightContent}
    />
  );

  if (loading) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <div
          id="preview-mode-panel"
          role="tabpanel"
          aria-labelledby={`preview-${previewMode}-tab`}
          className="min-h-0 flex-1"
        >
          <CenteredMessage icon={<Loader2 className="size-8 animate-spin" />}>
            {runtimeLoadStage
              ? `${t("preview.loading")} (${runtimeLoadStage})`
              : t("preview.loading")}
          </CenteredMessage>
        </div>
      </div>
    );
  }

  if (runtimeFailure) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <div
          id="preview-mode-panel"
          role="tabpanel"
          aria-labelledby={`preview-${previewMode}-tab`}
          className="min-h-0 flex-1"
        >
          <RuntimeFailureView failure={runtimeFailure} t={t} />
        </div>
      </div>
    );
  }

  if (!code.trim()) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <div
          id="preview-mode-panel"
          role="tabpanel"
          aria-labelledby={`preview-${previewMode}-tab`}
          className="flex min-h-0 flex-1 items-center justify-center"
        >
          <div className="text-center text-muted-foreground">
            <p className="text-sm">{t("preview.empty")}</p>
            <p className="text-xs mt-1">{t("preview.emptyHint")}</p>
          </div>
        </div>
      </div>
    );
  }

  if (error && previewMode !== "compare" && previewMode !== "diagnostics") {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <div
          id="preview-mode-panel"
          role="tabpanel"
          aria-labelledby={`preview-${previewMode}-tab`}
          className="min-h-0 flex-1"
        >
          <RenderError message={error} t={t} />
        </div>
      </div>
    );
  }

  const mermanArtifact: CompareArtifact = {
    key: "merman",
    artifactKey: mermanSvgActionKey,
    presentationKey: visibleBatch?.snapshot.requestId ?? null,
    actionsEnabled,
    title: t("preview.mermanEngine"),
    version: visibleBatch?.snapshot.mermanVersion ?? facade?.packageVersion ?? "-",
    svg: visibleMerman?.svg ?? null,
    error: failedMessage(visibleBatch?.merman ?? null),
    renderTime: visibleMerman?.renderTimeMs ?? null,
    loading: mermanRendering,
    loadingLabel: mermanRendering ? t("preview.renderingCurrent") : null,
    unavailableLabel: mermanSvgUnavailableLabel,
    stale: compareStale,
  };
  const mermaidArtifact: CompareArtifact = {
    key: "mermaid",
    artifactKey: mermaidSvgActionKey,
    presentationKey: visibleBatch?.snapshot.requestId ?? null,
    actionsEnabled,
    title: t("preview.mermaidEngine"),
    version: visibleMermaid?.version ?? MERMAID_JS_VERSION,
    svg: mermaidSvg,
    error: mermaidError,
    renderTime: mermaidRenderTime,
    loading: mermaidRendering,
    loadingLabel: mermaidLoadingLabel,
    unavailableLabel: mermaidSvgUnavailableLabel,
    stale: compareStale,
  };

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {renderTabBar(
        <>
          {previewMode === "svg" && (
            <>
              {svgDisplayMode === "visual" && (
                <ViewportControls controller={svgViewport} t={t} />
              )}
              <IconButton
                label={
                  copiedSvgKey === mermanSvgActionKey
                    ? t("preview.copied")
                    : (mermanSvgUnavailableLabel ?? t("preview.copySvg"))
                }
                onClick={() =>
                  handleCopySvg(svg, mermanSvgActionKey)
                }
                disabled={!actionsEnabled || Boolean(mermanSvgUnavailableLabel)}
              >
                {copiedSvgKey === mermanSvgActionKey ? (
                  <Check className="size-4 text-green-500" />
                ) : (
                  <Copy className="size-4" />
                )}
              </IconButton>
              <IconButton
                label={
                  svgDisplayMode === "visual"
                    ? t("preview.viewSvgSource")
                    : t("preview.viewSvgPreview")
                }
                onClick={() =>
                  setSvgDisplayMode((value) =>
                    value === "visual" ? "source" : "visual"
                  )
                }
                disabled={!svg}
              >
                {svgDisplayMode === "visual" ? (
                  <Code2 className="size-4" />
                ) : (
                  <ImageIcon className="size-4" />
                )}
              </IconButton>
            </>
          )}
          {previewMode === "ascii" && ascii && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={handleCopyAscii}
                  aria-label={
                    copiedAscii ? t("preview.copied") : t("preview.copyAscii")
                  }
                >
                  {copiedAscii ? (
                    <Check className="size-4 text-green-500" />
                  ) : (
                    <Copy className="size-4" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {copiedAscii ? t("preview.copied") : t("preview.copyAscii")}
              </TooltipContent>
            </Tooltip>
          )}
          {previewMode === "diagnostics" && (
            <IconButton
              label={
                copiedDiagnostic === diagnosticTab
                  ? t("preview.copied")
                  : t("preview.copyJson")
              }
              onClick={handleCopyDiagnosticJson}
              disabled={diagnosticsLoading || !diagnostics[diagnosticTab].json}
            >
              {copiedDiagnostic === diagnosticTab ? (
                <Check className="size-4 text-green-500" />
              ) : diagnosticsLoading ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Copy className="size-4" />
              )}
            </IconButton>
          )}
          {previewMode === "compare" && (
            <IconButton
              label={t("preview.refreshCompare")}
              onClick={handleRefreshCompare}
            >
              <RefreshCw className="size-4" />
            </IconButton>
          )}
        </>
      )}

      <div
        id="preview-mode-panel"
        role="tabpanel"
        aria-labelledby={`preview-${previewMode}-tab`}
        className="relative min-h-0 flex-1 overflow-hidden"
      >
        {previewMode === "svg" && (
          svgDisplayMode === "source" ? (
            <SvgSourceEditor svg={svg} isDarkMode={isDarkMode} />
          ) : (
            <SvgViewport
              svg={svg}
              presentationKey={currentBatch?.snapshot.requestId ?? null}
              controller={svgViewport}
              onPresentationReady={(at) => {
                if (currentBatch) {
                  markRenderCoordinatorPresented(
                    currentBatch.snapshot.requestId,
                    "merman",
                    at
                  );
                }
              }}
            />
          )
        )}

        {previewMode === "compare" && (
          <CompareView
            merman={{
              artifact: mermanArtifact,
              controller: mermanCompareViewport,
            }}
            mermaid={{
              artifact: mermaidArtifact,
              controller: mermaidCompareViewport,
              viewportHostRef: compareHostRef,
            }}
            actions={{
              copiedSvgKey,
              exportingPngEngines,
              onCopySvg: handleCopySvg,
              onExportPng: handleExportPng,
              onExportSvg: handleExportSvg,
              onPresentationReady: (engine, at) => {
                if (visibleBatch) {
                  markRenderCoordinatorPresented(
                    visibleBatch.snapshot.requestId,
                    engine,
                    at
                  );
                }
              },
            }}
            isDarkMode={isDarkMode}
            t={t}
          />
        )}

        {previewMode === "diagnostics" && (
          <DiagnosticsView
            activeTab={diagnosticTab}
            diagnostics={diagnostics}
            loading={diagnosticsLoading}
            isDarkMode={isDarkMode}
            onActiveTabChange={setDiagnosticTab}
            t={t}
          />
        )}

        {previewMode === "ascii" && (
          <div className="h-full w-full">
            {ascii ? (
              <div className="flex h-full flex-col">
                <AsciiSupportBanner
                  capability={asciiCapability}
                  label={asciiSupportLabel}
                  limit={asciiSupportLimit}
                  t={t}
                />
                <div className="min-h-0 flex-1">
                  <Editor
                    height="100%"
                    language="plaintext"
                    value={ascii}
                    theme={isDarkMode ? "vs-dark" : "light"}
                    options={{
                      readOnly: true,
                      minimap: { enabled: false },
                      fontSize: 13,
                      fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
                      lineNumbers: "off",
                      scrollBeyondLastLine: false,
                      wordWrap: "off",
                      renderLineHighlight: "none",
                      selectionHighlight: false,
                      occurrencesHighlight: "off",
                      folding: false,
                      padding: { top: 16, bottom: 16 },
                      domReadOnly: true,
                    }}
                  />
                </div>
              </div>
            ) : (
              <div className="flex items-center justify-center h-full text-muted-foreground">
                <div className="max-w-sm text-center">
                  <p>{t("preview.asciiNotAvailable")}</p>
                  <p className="mt-1 text-xs">
                    {asciiSupportLimit || t("preview.asciiNotSupported")}
                  </p>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface TabBarProps {
  mode: PreviewMode;
  onModeChange: (mode: PreviewMode) => void;
  runtimeReady: boolean;
  isAsciiSupported: boolean;
  asciiCapability: AsciiCapability | null;
  asciiSupportLabel: string;
  asciiSupportLimit: string;
  t: (key: string) => string;
  rightContent?: ReactNode;
}

function TabBar({
  mode,
  onModeChange,
  runtimeReady,
  isAsciiSupported,
  asciiCapability,
  asciiSupportLabel,
  asciiSupportLimit,
  t,
  rightContent,
}: TabBarProps) {
  return (
    <div className="flex h-10 shrink-0 items-center justify-between gap-2 overflow-hidden border-b bg-muted/30 px-2">
      <div
        role="tablist"
        aria-label={t("preview.title")}
        aria-orientation="horizontal"
        className="scrollbar-thin flex min-w-0 items-center gap-1 overflow-x-auto"
        onKeyDown={handleTabListKeyDown}
      >
        <TabButton
          value="svg"
          active={mode === "svg"}
          onClick={() => onModeChange("svg")}
        >
          SVG
        </TabButton>
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              id="preview-ascii-tab"
              type="button"
              role="tab"
              tabIndex={mode === "ascii" ? 0 : -1}
              aria-selected={mode === "ascii"}
              aria-controls="preview-mode-panel"
              onClick={() =>
                runtimeReady && isAsciiSupported && onModeChange("ascii")
              }
              disabled={!runtimeReady || !isAsciiSupported}
              className={cn(
                "shrink-0 px-3 py-1.5 text-sm rounded-md transition-colors",
                mode === "ascii"
                  ? "bg-background text-foreground shadow-sm font-medium"
                  : "text-muted-foreground hover:text-foreground hover:bg-background/50",
                (!runtimeReady || !isAsciiSupported) &&
                  "opacity-50 cursor-not-allowed hover:bg-transparent hover:text-muted-foreground"
              )}
            >
              ASCII
            </button>
          </TooltipTrigger>
          <TooltipContent>
            {isAsciiSupported
              ? asciiSupportTooltip(asciiCapability, asciiSupportLabel, asciiSupportLimit)
              : t("preview.asciiNotSupported")}
          </TooltipContent>
        </Tooltip>
        {isAsciiSupported && (
          <span className="hidden shrink-0 rounded bg-muted px-2 py-1 text-xs text-muted-foreground sm:inline">
            {asciiSupportLabel}
          </span>
        )}
        <TabButton
          value="compare"
          active={mode === "compare"}
          onClick={() => onModeChange("compare")}
          disabled={!runtimeReady}
        >
          {t("preview.compareMode")}
        </TabButton>
        <TabButton
          value="diagnostics"
          active={mode === "diagnostics"}
          onClick={() => onModeChange("diagnostics")}
          disabled={!runtimeReady}
        >
          {t("preview.diagnosticsMode")}
        </TabButton>
      </div>

      <div className="scrollbar-thin flex shrink-0 items-center gap-1 overflow-x-auto">
        {rightContent}
      </div>
    </div>
  );
}

function AsciiSupportBanner({
  capability,
  label,
  limit,
  t,
}: {
  capability: AsciiCapability | null;
  label: string;
  limit: string;
  t: (key: string) => string;
}) {
  if (!capability) {
    return null;
  }

  return (
    <div className="flex shrink-0 items-center gap-2 border-b bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
      <span className="rounded bg-background px-2 py-0.5 text-foreground">
        {label}
      </span>
      {capability.summary_fallback && (
        <span>{t("asciiSupport.summaryFallback")}</span>
      )}
      {limit && <span className="truncate">{limit}</span>}
    </div>
  );
}

function asciiSupportTooltip(
  capability: AsciiCapability | null,
  label: string,
  limit: string
): string {
  if (!capability) {
    return label;
  }
  const parts = [capability.display_name, label, limit].filter(Boolean);
  return parts.join(" · ");
}

interface TabButtonProps {
  value: PreviewMode;
  active: boolean;
  onClick(): void;
  disabled?: boolean;
  children: ReactNode;
}

function TabButton({
  value,
  active,
  onClick,
  disabled = false,
  children,
}: TabButtonProps) {
  return (
    <button
      id={`preview-${value}-tab`}
      type="button"
      role="tab"
      tabIndex={active ? 0 : -1}
      aria-selected={active}
      aria-controls="preview-mode-panel"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "shrink-0 px-3 py-1.5 text-sm rounded-md transition-colors",
        active
          ? "bg-background text-foreground shadow-sm font-medium"
          : "text-muted-foreground hover:text-foreground hover:bg-background/50",
        disabled && "cursor-not-allowed opacity-50 hover:bg-transparent"
      )}
    >
      {children}
    </button>
  );
}

function handleTabListKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
  if (!(event.target instanceof HTMLButtonElement)) return;
  const tabs = Array.from(
    event.currentTarget.querySelectorAll<HTMLButtonElement>(
      '[role="tab"]:not(:disabled)'
    )
  );
  const currentIndex = tabs.indexOf(event.target);
  if (currentIndex < 0) return;

  let nextIndex: number | null = null;
  if (event.key === "ArrowRight") nextIndex = (currentIndex + 1) % tabs.length;
  if (event.key === "ArrowLeft") {
    nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
  }
  if (event.key === "Home") nextIndex = 0;
  if (event.key === "End") nextIndex = tabs.length - 1;
  if (nextIndex === null) return;
  event.preventDefault();
  tabs[nextIndex]?.focus();
}

function DiagnosticsView({
  activeTab,
  diagnostics,
  loading,
  isDarkMode,
  onActiveTabChange,
  t,
}: {
  activeTab: DiagnosticKey;
  diagnostics: Record<DiagnosticKey, DiagnosticArtifact>;
  loading: boolean;
  isDarkMode: boolean;
  onActiveTabChange(tab: DiagnosticKey): void;
  t: (key: string) => string;
}) {
  const current = diagnostics[activeTab];

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => onActiveTabChange(value as DiagnosticKey)}
      activationMode="manual"
      className="h-full gap-0 bg-background"
    >
      <div className="flex min-h-10 items-center justify-between gap-2 border-b bg-muted/20 px-3 py-2">
        <TabsList
          aria-label={t("preview.diagnosticsMode")}
          className="h-8 bg-muted/70 p-0.5"
        >
          <TabsTrigger value="parse" className="px-2.5 text-xs">
            {t("preview.parseJson")}
          </TabsTrigger>
          <TabsTrigger value="layout" className="px-2.5 text-xs">
            {t("preview.layoutJson")}
          </TabsTrigger>
        </TabsList>
        <p className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {loading
            ? t("preview.runningDiagnostics")
            : current.elapsedMs !== null
              ? `${current.elapsedMs.toFixed(1)}ms`
              : "-"}
        </p>
      </div>

      {(["parse", "layout"] as const).map((tab) => (
        <TabsContent
          key={tab}
          value={tab}
          forceMount
          className="mt-0 min-h-0 data-[state=inactive]:hidden"
        >
          <DiagnosticArtifactView
            artifact={diagnostics[tab]}
            loading={loading}
            isDarkMode={isDarkMode}
            t={t}
          />
        </TabsContent>
      ))}
    </Tabs>
  );
}

function DiagnosticArtifactView({
  artifact,
  loading,
  isDarkMode,
  t,
}: {
  artifact: DiagnosticArtifact;
  loading: boolean;
  isDarkMode: boolean;
  t: (key: string) => string;
}) {
  if (loading) {
    return (
      <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
        {t("preview.runningDiagnostics")}
      </CenteredMessage>
    );
  }
  if (artifact.error) {
    return <RenderError message={artifact.error} t={t} compact />;
  }
  if (!artifact.json) {
    return (
      <CenteredMessage icon={<FileCode className="size-8" />}>
        {t("preview.diagnosticsEmpty")}
      </CenteredMessage>
    );
  }
  return (
    <Editor
      height="100%"
      language="json"
      value={artifact.json}
      theme={isDarkMode ? "vs-dark" : "light"}
      options={{
        readOnly: true,
        domReadOnly: true,
        minimap: { enabled: false },
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        renderLineHighlight: "none",
        selectionHighlight: false,
        occurrencesHighlight: "off",
        folding: true,
        automaticLayout: true,
        padding: { top: 16, bottom: 16 },
      }}
    />
  );
}

function ViewportControls({
  controller,
  t,
}: {
  controller: SvgViewportController;
  t: (key: string) => string;
}) {
  return (
    <div className="flex items-center gap-1">
      <IconButton label={t("preview.zoomOut")} onClick={controller.zoomOut}>
        <ZoomOut className="size-4" />
      </IconButton>
      <span className="text-xs text-muted-foreground w-12 text-center tabular-nums">
        {Math.round(controller.zoom * 100)}%
      </span>
      <IconButton label={t("preview.zoomIn")} onClick={controller.zoomIn}>
        <ZoomIn className="size-4" />
      </IconButton>
      <IconButton label={t("preview.fitToView")} onClick={controller.fitToView}>
        <Maximize2 className="size-4" />
      </IconButton>
      <IconButton label={t("preview.reset")} onClick={controller.reset}>
        <RotateCcw className="size-4" />
      </IconButton>
    </div>
  );
}

function CompareView({
  merman,
  mermaid,
  actions,
  isDarkMode,
  t,
}: {
  merman: ComparePaneModel;
  mermaid: ComparePaneModel;
  actions: ComparePaneActions;
  isDarkMode: boolean;
  t: (key: string) => string;
}) {
  return (
    <div className="h-full overflow-auto p-3">
      <div className="grid min-h-full grid-cols-1 gap-3 xl:grid-cols-2">
        {[merman, mermaid].map((pane) => (
          <ComparePane
            key={pane.artifact.key}
            model={pane}
            actions={actions}
            isDarkMode={isDarkMode}
            t={t}
          />
        ))}
      </div>
    </div>
  );
}

function ComparePane({
  model,
  actions,
  isDarkMode,
  t,
}: {
  model: ComparePaneModel;
  actions: ComparePaneActions;
  isDarkMode: boolean;
  t: (key: string) => string;
}) {
  const { artifact, controller, viewportHostRef } = model;
  const copied = actions.copiedSvgKey === artifact.artifactKey;
  const exporting = actions.exportingPngEngines.has(artifact.key);
  const hasSvg = Boolean(artifact.svg);
  const actionsDisabled =
    !artifact.actionsEnabled || Boolean(artifact.unavailableLabel);
  const [svgDisplayMode, setSvgDisplayMode] =
    useState<SvgDisplayMode>("visual");
  const statusLabel = compareArtifactStatusLabel(artifact, t);

  return (
    <section className="flex min-h-[320px] flex-col overflow-hidden rounded-md border bg-background xl:min-h-0">
      <div className="border-b bg-muted/30 px-3 py-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate text-sm font-medium">{artifact.title}</span>
            <span className="shrink-0 rounded-sm bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
              {artifact.version}
            </span>
            {artifact.stale && (
              <span className="shrink-0 rounded-sm bg-amber-500/15 px-1.5 py-0.5 text-[11px] text-amber-700 dark:text-amber-300">
                {t("preview.updatingStale")}
              </span>
            )}
          </div>
          <p className="shrink-0 text-xs text-muted-foreground">
            {statusLabel}
          </p>
        </div>
        <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
          {hasSvg && <ViewportControls controller={controller} t={t} />}
          {!hasSvg && <div />}
          <div className="flex items-center gap-1">
            <IconButton
              label={
                copied
                  ? t("preview.copied")
                  : (artifact.unavailableLabel ?? t("preview.copySvg"))
              }
              onClick={() =>
                actions.onCopySvg(artifact.svg, artifact.artifactKey)
              }
              disabled={actionsDisabled}
            >
              {copied ? (
                <Check className="size-4 text-green-500" />
              ) : (
                <Copy className="size-4" />
              )}
            </IconButton>
            <IconButton
              label={artifact.unavailableLabel ?? t("preview.exportSvg")}
              onClick={() =>
                actions.onExportSvg(artifact.key, artifact.svg)
              }
              disabled={actionsDisabled}
            >
              <FileCode className="size-4" />
            </IconButton>
            <IconButton
              label={
                svgDisplayMode === "visual"
                  ? t("preview.viewSvgSource")
                  : t("preview.viewSvgPreview")
              }
              onClick={() =>
                setSvgDisplayMode((value) =>
                  value === "visual" ? "source" : "visual"
                )
              }
              disabled={!hasSvg}
            >
              {svgDisplayMode === "visual" ? (
                <Code2 className="size-4" />
              ) : (
                <ImageIcon className="size-4" />
              )}
            </IconButton>
            <IconButton
              label={
                exporting
                  ? t("preview.exporting")
                  : (artifact.unavailableLabel ?? t("preview.exportPng"))
              }
              onClick={() =>
                actions.onExportPng(artifact.key, artifact.svg)
              }
              disabled={actionsDisabled || exporting}
            >
              {exporting ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <ImageIcon className="size-4" />
              )}
            </IconButton>
          </div>
        </div>
      </div>
      <div
        ref={viewportHostRef}
        data-merman-compare-viewport-host={viewportHostRef ? "" : undefined}
        className={cn(
          "min-h-0 flex-1 transition-opacity",
          artifact.stale && "opacity-60"
        )}
        aria-busy={artifact.loading || artifact.stale}
      >
        <ComparePaneBody
          artifact={artifact}
          controller={controller}
          displayMode={svgDisplayMode}
          isDarkMode={isDarkMode}
          onPresentationReady={(at) =>
            actions.onPresentationReady(artifact.key, at)
          }
          t={t}
        />
      </div>
    </section>
  );
}

function compareArtifactStatusLabel(
  artifact: CompareArtifact,
  t: (key: string) => string
): string {
  if (artifact.loading || artifact.stale) {
    return artifact.loadingLabel ?? t("preview.renderingMermaid");
  }
  if (artifact.renderTime !== null) {
    return `${artifact.renderTime.toFixed(1)}ms`;
  }
  return "-";
}

function ComparePaneBody({
  artifact,
  controller,
  displayMode,
  isDarkMode,
  onPresentationReady,
  t,
}: {
  artifact: CompareArtifact;
  controller: SvgViewportController;
  displayMode: SvgDisplayMode;
  isDarkMode: boolean;
  onPresentationReady(at: number): void;
  t: (key: string) => string;
}) {
  if (artifact.loading && !artifact.svg) {
    return (
      <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
        {artifact.loadingLabel ?? t("preview.renderingMermaid")}
      </CenteredMessage>
    );
  }
  if (artifact.error) {
    return <RenderError message={artifact.error} t={t} compact />;
  }
  if (displayMode === "source") {
    return <SvgSourceEditor svg={artifact.svg} isDarkMode={isDarkMode} />;
  }
  return (
    <SvgViewport
      svg={artifact.svg}
      presentationKey={artifact.presentationKey}
      controller={controller}
      onPresentationReady={onPresentationReady}
      empty={
        <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
          {t("preview.empty")}
        </div>
      }
    />
  );
}

function artifactActionKey(kind: string, renderKey: string): string {
  return JSON.stringify([kind, renderKey]);
}

function requireSafeSvgString(svg: string): string {
  assertSafeSvgForDom(svg);
  return svg;
}

function artifactUnavailableLabel({
  value,
  error,
  loading,
  t,
}: {
  value: string | null;
  error: string | null;
  loading: boolean;
  t: (key: string) => string;
}): string | null {
  if (value) {
    return null;
  }
  if (loading) {
    return t("preview.renderingCurrent");
  }
  if (error) {
    return t("preview.currentRenderFailed");
  }
  return t("preview.noCurrentArtifact");
}

function SvgSourceEditor({
  svg,
  isDarkMode,
}: {
  svg: string | null;
  isDarkMode: boolean;
}) {
  if (!svg) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        -
      </div>
    );
  }

  return (
    <Editor
      height="100%"
      language="xml"
      value={svg}
      theme={isDarkMode ? "vs-dark" : "light"}
      options={{
        readOnly: true,
        domReadOnly: true,
        minimap: { enabled: false },
        fontSize: 12,
        fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        renderLineHighlight: "none",
        selectionHighlight: false,
        occurrencesHighlight: "off",
        folding: true,
        automaticLayout: true,
        padding: { top: 16, bottom: 16 },
      }}
    />
  );
}

function IconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick(): void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function CenteredMessage({
  icon,
  children,
}: {
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full flex-1 items-center justify-center">
      <div className="flex flex-col items-center gap-3 text-muted-foreground">
        {icon}
        <span className="text-sm">{children}</span>
      </div>
    </div>
  );
}

function RenderError({
  message,
  t,
  compact = false,
}: {
  message: string;
  t: (key: string) => string;
  compact?: boolean;
}) {
  return (
    <div className="flex h-full flex-1 items-center justify-center p-6">
      <div className={cn("text-center", compact ? "max-w-sm" : "max-w-md")}>
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-destructive/10">
          <AlertCircle className="size-6 text-destructive" />
        </div>
        <h3 className="mb-2 font-medium text-foreground">{t("preview.error")}</h3>
        <p className="rounded-md bg-muted/50 p-3 font-mono text-sm text-muted-foreground">
          {message}
        </p>
      </div>
    </div>
  );
}

function RuntimeFailureView({
  failure,
  t,
}: {
  failure: MermanRuntimeFailure;
  t: (key: string) => string;
}) {
  const handleRecovery = () => {
    if (failure.recovery === "reload") {
      window.location.reload();
      return;
    }
    void retryMermanRuntime().catch(() => undefined);
  };

  return (
    <div
      className="flex h-full flex-1 items-center justify-center p-6"
      role="alert"
    >
      <div className="max-w-md text-center">
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-destructive/10">
          <AlertCircle className="size-6 text-destructive" />
        </div>
        <h3 className="mb-2 font-medium text-foreground">
          {t("preview.error")}
        </h3>
        <p className="mb-4 text-xs text-muted-foreground">{failure.stage}</p>
        <p className="mb-4 rounded-md bg-muted/50 p-3 font-mono text-sm text-muted-foreground">
          {failure.message}
        </p>
        <Button onClick={handleRecovery}>
          {t(failure.recovery === "reload" ? "wasm.reload" : "wasm.retry")}
        </Button>
      </div>
    </div>
  );
}


function successfulMerman(
  artifact: MermanRenderSuccess | EngineRenderFailure | null
): MermanRenderSuccess | null {
  return artifact?.status === "success" ? artifact : null;
}

function successfulMermaid(
  artifact: MermaidRenderSuccess | EngineRenderFailure | null
): MermaidRenderSuccess | null {
  return artifact?.status === "success" ? artifact : null;
}

function failedMessage(
  artifact:
    | MermanRenderSuccess
    | MermaidRenderSuccess
    | EngineRenderFailure
    | null
): string | null {
  return artifact?.status === "failure" ? artifact.message : null;
}
