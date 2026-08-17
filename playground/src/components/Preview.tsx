import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  selectMermanFailure,
  selectMermanLoadStage,
  selectMermanStatus,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { retryMermanRuntime } from "@/src/runtime/merman";
import type { MermanRuntimeFailure } from "@/src/runtime/merman-core";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import { resolveMermaidCanvasTone } from "@/src/lib/mermaid-canvas-tone";
import {
  SVG_PRESENTATION_MODES,
  type SvgPresentationMode,
} from "@/src/lib/svg-presentation";
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
  setRenderFeatures,
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
  MermanAsciiBatchResult,
  MermanRenderSuccess,
  MermaidRenderSuccess,
  RenderPublicationId,
} from "@/src/runtime/render-coordinator";
import { executeArtifactAction } from "@/src/runtime/artifact-actions-browser";
import { useExportWorkbench } from "@/src/components/ExportDialog";
import {
  SvgViewport,
  useSvgViewportController,
} from "@/src/components/SvgViewport";
import {
  CompareView,
  type CompareArtifact,
  type CompareEngineKey,
} from "@/src/components/CompareView";
import {
  SvgSourceEditor,
  ViewportControls,
} from "@/src/components/PreviewArtifactViews";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import type { WorkbenchEditorThemeName } from "@/src/editor/workbench-editor-theme";
import { cn } from "@/lib/utils";
import {
  Loader2,
  AlertCircle,
  Copy,
  Check,
  Code2,
  FileCode,
  ImageIcon,
  RefreshCw,
  SquareDashed,
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
type EngineKey = CompareEngineKey;
type DiagnosticKey = "parse" | "layout";
type CopiedSvgTarget = {
  readonly engine: EngineKey;
  readonly publicationId: RenderPublicationId;
};

const EMPTY_DIAGNOSTICS: Record<DiagnosticKey, DiagnosticArtifact> = {
  parse: { json: null, error: null, errorDetail: null, elapsedMs: null },
  layout: { json: null, error: null, errorDetail: null, elapsedMs: null },
};

export function Preview({ className }: PreviewProps) {
  const { t } = useTranslation();
  const code = useAppStore((state) => state.code);
  const diagramTheme = useAppStore((state) => state.diagramTheme);
  const mermaidConfig = useAppStore((state) => state.mermaidConfig);
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const editorTheme = WORKBENCH_EDITOR_THEMES[resolvedTheme].name;
  const previewMode = useAppStore((state) => state.previewMode);
  const setPreviewMode = useAppStore((state) => state.setPreviewMode);
  const svgPresentationMode = useAppStore(
    (state) => state.svgPresentationMode,
  );
  const setSvgPresentationMode = useAppStore(
    (state) => state.setSvgPresentationMode,
  );
  const showSvgBounds = useAppStore((state) => state.showSvgBounds);
  const setShowSvgBounds = useAppStore((state) => state.setShowSvgBounds);
  const { openExport } = useExportWorkbench();
  const renderState = useRenderCoordinator((state) => state);
  const currentBatch = selectCompletedRenderBatch(renderState);
  const visibleBatch = selectVisibleRenderBatch(renderState);
  const runtimeStatus = useMermanRuntime(selectMermanStatus);
  const runtimeFailure = useMermanRuntime(selectMermanFailure);
  const runtimeLoadStage = useMermanRuntime(selectMermanLoadStage);
  const ready = runtimeStatus === "ready";
  const loading = runtimeStatus === "idle" || runtimeStatus === "loading";
  const asciiSupport = useAsciiSupport();
  const [svgDisplayMode, setSvgDisplayMode] =
    useState<SvgDisplayMode>("visual");
  const [copiedAsciiPublicationId, setCopiedAsciiPublicationId] =
    useState<RenderPublicationId | null>(null);
  const [copiedDiagnostic, setCopiedDiagnostic] =
    useState<DiagnosticKey | null>(null);
  const [copiedSvgTarget, setCopiedSvgTarget] =
    useState<CopiedSvgTarget | null>(null);
  const [diagnosticTab, setDiagnosticTab] = useState<DiagnosticKey>("parse");
  const asciiCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const diagnosticCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );

  const currentPublicationId = currentBatch?.snapshot.publicationId ?? null;
  const detectedDiagramType = selectCurrentDiagramType(renderState);
  const currentMerman = successfulMerman(currentBatch?.merman ?? null);
  const svgArtifact = currentMerman?.artifact ?? null;
  const svg = svgArtifact?.svg ?? null;
  const asciiResult = currentBatch?.ascii ?? null;
  const error = failedMessage(currentBatch?.merman ?? null);
  const errorDetail = failedDetail(currentBatch?.merman ?? null);
  const errorStage = failedStage(currentBatch?.merman ?? null);
  const visibleMerman = successfulMerman(visibleBatch?.merman ?? null);
  const visibleMermaid = successfulMermaid(visibleBatch?.mermaid ?? null);
  const mermaidSvgArtifact = visibleMermaid?.artifact ?? null;
  const mermaidError = failedMessage(visibleBatch?.mermaid ?? null);
  const mermaidErrorDetail = failedDetail(visibleBatch?.mermaid ?? null);
  const mermaidRenderTime = visibleMermaid?.renderTimeMs ?? null;
  const renderPending = selectRenderPending(renderState);
  const renderingCurrent = Boolean(code.trim() && ready && renderPending);
  const actionsEnabled = currentBatch !== null;
  const compareStale = renderState.status === "updating";
  const diagnostics = currentBatch?.diagnostics ?? EMPTY_DIAGNOSTICS;
  const diagnosticsLoading = previewMode === "diagnostics" && renderPending;
  const isAsciiSupported = asciiSupport.isSupported(detectedDiagramType);
  const asciiCapability = asciiSupport.capabilityFor(detectedDiagramType);
  const asciiSupportLabel = t(asciiSupportLabelKey(asciiCapability));
  const asciiSupportLimit = asciiSupportDescription(asciiCapability);
  const svgViewport = useSvgViewportController();
  const canvasOperation =
    previewMode === "compare"
      ? visibleBatch?.snapshot.operation
      : currentBatch?.snapshot.operation;
  const canvasTone = useMemo(
    () =>
      resolveMermaidCanvasTone(
        canvasOperation?.configJson ?? mermaidConfig,
        canvasOperation?.theme ?? diagramTheme,
        canvasOperation?.source ?? code,
      ),
    [canvasOperation, code, diagramTheme, mermaidConfig],
  );
  const canvasMode = previewMode === "svg" || previewMode === "compare";

  useEffect(() => {
    setRenderFeatures({
      compareEnabled: previewMode === "compare",
      diagnosticsEnabled: previewMode === "diagnostics",
    });
    return () =>
      setRenderFeatures({
        compareEnabled: false,
        diagnosticsEnabled: false,
      });
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
      await executeArtifactAction({
        action: "copy-ascii",
        publicationId: requirePublicationId(currentPublicationId),
      });
      setCopiedAsciiPublicationId(currentPublicationId);
      if (asciiCopyTimeoutRef.current) {
        clearTimeout(asciiCopyTimeoutRef.current);
      }
      asciiCopyTimeoutRef.current = setTimeout(
        () => setCopiedAsciiPublicationId(null),
        2000,
      );
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [currentPublicationId, t]);

  const handleCopySvg = useCallback(
    async (engine: EngineKey, publicationId: RenderPublicationId) => {
      try {
        await executeArtifactAction({
          action: "copy-svg",
          engine,
          publicationId,
        });
        setCopiedSvgTarget({ engine, publicationId });
        if (copyTimeoutRef.current) {
          clearTimeout(copyTimeoutRef.current);
        }
        copyTimeoutRef.current = setTimeout(
          () => setCopiedSvgTarget(null),
          2000,
        );
        toast.success(t("share.copied"));
      } catch {
        toast.error(t("share.copyFailed"));
      }
    },
    [t],
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
        2000,
      );
    } catch (err) {
      console.error("Failed to copy diagnostics JSON:", err);
    }
  }, [diagnosticTab, diagnostics]);

  const handleRefreshCompare = useCallback(() => {
    refreshRenderCoordinator();
  }, []);

  const copiedAscii =
    copiedAsciiPublicationId !== null &&
    copiedAsciiPublicationId === currentPublicationId;
  const mermanSvgUnavailableLabel = artifactUnavailableLabel({
    available: Boolean(svg),
    error,
    loading: renderingCurrent,
    t,
  });
  const mermaidRendering = Boolean(
    previewMode === "compare" && code.trim() && renderPending,
  );
  const mermaidLoadingLabel = mermaidRendering
    ? t("preview.renderingCurrent")
    : null;
  const mermaidSvgUnavailableLabel = artifactUnavailableLabel({
    available: Boolean(mermaidSvgArtifact),
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

  const mermanArtifact: CompareArtifact = {
    key: "merman",
    publicationId: visibleBatch?.snapshot.publicationId ?? null,
    title: t("preview.mermanEngine"),
    version: visibleBatch?.snapshot.operation.versions.merman ?? "-",
    svgArtifact: visibleMerman?.artifact ?? null,
    error: failedMessage(visibleBatch?.merman ?? null),
    errorDetail: failedDetail(visibleBatch?.merman ?? null),
    errorStage: failedStage(visibleBatch?.merman ?? null),
    renderTime: visibleMerman?.renderTimeMs ?? null,
    loading: renderingCurrent,
    loadingLabel: renderingCurrent ? t("preview.renderingCurrent") : null,
    unavailableLabel: mermanSvgUnavailableLabel,
    stale: compareStale,
  };
  const mermaidArtifact: CompareArtifact = {
    key: "mermaid",
    publicationId: visibleBatch?.snapshot.publicationId ?? null,
    title: t("preview.mermaidEngine"),
    version: visibleMermaid?.version ?? MERMAID_JS_VERSION,
    svgArtifact: mermaidSvgArtifact,
    error: mermaidError,
    errorDetail: mermaidErrorDetail,
    errorStage: failedStage(visibleBatch?.mermaid ?? null),
    renderTime: mermaidRenderTime,
    loading: mermaidRendering,
    loadingLabel: mermaidLoadingLabel,
    unavailableLabel: mermaidSvgUnavailableLabel,
    stale: compareStale,
  };
  const showToolbarActions =
    !loading &&
    !runtimeFailure &&
    Boolean(code.trim()) &&
    !(error && previewMode === "svg");
  const toolbarActions =
    canvasMode || showToolbarActions ? (
      <>
        {canvasMode && (
          <>
            <SvgPresentationModeToggle
              value={svgPresentationMode}
              groupLabel={t("preview.presentationMode")}
              infiniteLabel={t("preview.infiniteCanvas")}
              infiniteShortLabel={t("preview.infiniteCanvasShort")}
              viewBoxLabel={t("preview.viewBoxFrame")}
              viewBoxShortLabel={t("preview.viewBoxFrameShort")}
              onValueChange={setSvgPresentationMode}
            />
            <SvgBoundsToggle
              pressed={showSvgBounds}
              label={t("preview.svgBounds")}
              onPressedChange={setShowSvgBounds}
            />
          </>
        )}
        {showToolbarActions && (
          <>
            {previewMode === "svg" && (
              <>
                {svgDisplayMode === "visual" && (
                  <ViewportControls controller={svgViewport} t={t} />
                )}
                <IconButton
                  label={
                    copiedSvgTarget?.engine === "merman" &&
                    copiedSvgTarget.publicationId === currentPublicationId
                      ? t("preview.copied")
                      : (mermanSvgUnavailableLabel ?? t("preview.copySvg"))
                  }
                  onClick={() =>
                    currentPublicationId &&
                    handleCopySvg("merman", currentPublicationId)
                  }
                  disabled={
                    !actionsEnabled || Boolean(mermanSvgUnavailableLabel)
                  }
                >
                  {copiedSvgTarget?.engine === "merman" &&
                  copiedSvgTarget.publicationId === currentPublicationId ? (
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
                      value === "visual" ? "source" : "visual",
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
            {previewMode === "ascii" &&
              asciiResult?.status === "success" && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      data-testid="copy-ascii-button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={handleCopyAscii}
                      aria-label={
                        copiedAscii
                          ? t("preview.copied")
                          : t("preview.copyAscii")
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
                    {copiedAscii
                      ? t("preview.copied")
                      : t("preview.copyAscii")}
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
                disabled={
                  diagnosticsLoading || !diagnostics[diagnosticTab].json
                }
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
      </>
    ) : undefined;

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {renderTabBar(toolbarActions)}

      <div
        id="preview-mode-panel"
        role="tabpanel"
        aria-labelledby={`preview-${previewMode}-tab`}
        className={cn(
          "relative min-h-0 flex-1 overflow-hidden",
          canvasMode && "preview-canvas",
        )}
        data-preview-canvas-tone={canvasMode ? canvasTone : undefined}
        data-svg-presentation-mode={
          canvasMode ? svgPresentationMode : undefined
        }
      >
        {loading ? (
          <CenteredMessage icon={<Loader2 className="size-8 animate-spin" />}>
            {runtimeLoadStage
              ? `${t("preview.loading")} (${runtimeLoadStage})`
              : t("preview.loading")}
          </CenteredMessage>
        ) : runtimeFailure ? (
          <RuntimeFailureView failure={runtimeFailure} t={t} />
        ) : !code.trim() ? (
          <CenteredMessage>{t("preview.empty")}</CenteredMessage>
        ) : error && previewMode === "svg" ? (
          <RenderError
            engine={t("preview.mermanEngine")}
            stage={errorStage}
            message={error}
            detail={errorDetail}
            t={t}
          />
        ) : (
          <>
            {previewMode === "svg" &&
              (svgDisplayMode === "source" ? (
                <SvgSourceEditor svg={svg} editorTheme={editorTheme} />
              ) : (
                <SvgViewport
                  artifact={svgArtifact}
                  canvasTone={canvasTone}
                  presentationKey={currentPublicationId}
                  controller={svgViewport}
                  showSvgBounds={showSvgBounds}
                  presentationMode={svgPresentationMode}
                  renderMountError={(mountError) => (
                    <RenderError
                      engine={t("preview.mermanEngine")}
                      stage="svg-mount"
                      message={mountError.message}
                      detail={mountError.stack}
                      t={t}
                    />
                  )}
                  onPresentationReady={(at) => {
                    if (currentBatch) {
                      markRenderCoordinatorPresented(
                        currentBatch.snapshot.publicationId,
                        "merman",
                        at,
                      );
                    }
                  }}
                />
              ))}

            {previewMode === "compare" && (
              <CompareView
                merman={{
                  artifact: mermanArtifact,
                }}
                mermaid={{
                  artifact: mermaidArtifact,
                }}
                actions={{
                  copiedSvgTarget,
                  onCopySvg: handleCopySvg,
                  onExport: openExport,
                  onRetry: handleRefreshCompare,
                  onPresentationReady: (engine, at) => {
                    if (visibleBatch) {
                      markRenderCoordinatorPresented(
                        visibleBatch.snapshot.publicationId,
                        engine,
                        at,
                      );
                    }
                  },
                }}
                canvasTone={canvasTone}
                editorTheme={editorTheme}
                showSvgBounds={showSvgBounds}
                presentationMode={svgPresentationMode}
                t={t}
              />
            )}

            {previewMode === "diagnostics" && (
              <DiagnosticsView
                activeTab={diagnosticTab}
                diagnostics={diagnostics}
                loading={diagnosticsLoading}
                editorTheme={editorTheme}
                onActiveTabChange={setDiagnosticTab}
                t={t}
              />
            )}

            {previewMode === "ascii" && (
              <AsciiArtifactView
                result={asciiResult}
                rendering={renderingCurrent}
                capability={asciiCapability}
                supportLabel={asciiSupportLabel}
                supportLimit={asciiSupportLimit}
                editorTheme={editorTheme}
                t={t}
              />
            )}
          </>
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
  const tabListRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    tabListRef.current
      ?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [mode]);

  return (
    <div className="flex shrink-0 flex-col overflow-hidden border-b bg-muted/30 xl:h-10 xl:flex-row xl:items-center xl:justify-between xl:gap-2 xl:px-2">
      <div
        ref={tabListRef}
        role="tablist"
        aria-label={t("preview.title")}
        aria-orientation="horizontal"
        className="scrollbar-thin flex min-h-10 w-full min-w-0 items-center gap-1 overflow-x-auto px-2 xl:min-h-0 xl:flex-1 xl:px-0"
        onKeyDown={handleTabListKeyDown}
      >
        <TabButton
          value="svg"
          active={mode === "svg"}
          onClick={() => onModeChange("svg")}
        >
          SVG
        </TabButton>
        <TabButton
          value="ascii"
          active={mode === "ascii"}
          onClick={() => onModeChange("ascii")}
          tooltip={
            !runtimeReady
              ? t("preview.loading")
              : isAsciiSupported
                ? asciiSupportTooltip(
                    asciiCapability,
                    asciiSupportLabel,
                    asciiSupportLimit,
                  )
                : t("preview.asciiNotSupported")
          }
        >
          ASCII
        </TabButton>
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

      {rightContent && (
        <div
          data-testid="preview-toolbar-actions"
          className="scrollbar-thin flex min-h-10 w-full shrink-0 items-center justify-start gap-1 overflow-x-auto border-t px-2 xl:ml-auto xl:min-h-0 xl:w-auto xl:min-w-0 xl:max-w-full xl:shrink xl:border-t-0 xl:px-0"
        >
          {rightContent}
        </div>
      )}
    </div>
  );
}

function AsciiArtifactView({
  result,
  rendering,
  capability,
  supportLabel,
  supportLimit,
  editorTheme,
  t,
}: {
  result: MermanAsciiBatchResult | null;
  rendering: boolean;
  capability: AsciiCapability | null;
  supportLabel: string;
  supportLimit: string;
  editorTheme: WorkbenchEditorThemeName;
  t: (key: string) => string;
}) {
  if (rendering) {
    return (
      <CenteredMessage icon={<Loader2 className="size-8 animate-spin" />}>
        {t("preview.renderingCurrent")}
      </CenteredMessage>
    );
  }
  if (!result) {
    return (
      <CenteredMessage icon={<AlertCircle className="size-8" />}>
        {t("preview.asciiNotAvailable")}
      </CenteredMessage>
    );
  }
  if (result.status === "failure") {
    return (
      <RenderError
        engine={t("preview.mermanEngine")}
        stage="ascii-render"
        message={result.error.summary}
        detail={result.error.detail}
        t={t}
        compact
      />
    );
  }
  if (result.status === "unsupported") {
    return (
      <div className="flex h-full items-center justify-center px-4 text-muted-foreground">
        <div className="max-w-sm text-center">
          <AlertCircle className="mx-auto mb-3 size-8" />
          <p>{t("preview.asciiNotSupported")}</p>
          <p className="mt-1 font-mono text-xs">{result.diagramType}</p>
          {supportLimit && <p className="mt-2 text-xs">{supportLimit}</p>}
        </div>
      </div>
    );
  }
  if (result.status === "unavailable") {
    return (
      <CenteredMessage icon={<AlertCircle className="size-8" />}>
        {t("preview.asciiDetectionUnavailable")}
      </CenteredMessage>
    );
  }
  return (
    <div className="flex h-full flex-col">
      <AsciiSupportBanner
        capability={capability}
        label={supportLabel}
        limit={supportLimit}
        t={t}
      />
      <div
        data-testid="ascii-artifact-editor"
        className="min-h-0 flex-1"
      >
        <Editor
          height="100%"
          language="plaintext"
          value={result.artifact}
          theme={editorTheme}
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
  limit: string,
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
  tooltip?: ReactNode;
  children: ReactNode;
}

function TabButton({
  value,
  active,
  onClick,
  disabled = false,
  tooltip,
  children,
}: TabButtonProps) {
  const button = (
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
        disabled && "cursor-not-allowed opacity-50 hover:bg-transparent",
      )}
    >
      {children}
    </button>
  );

  if (tooltip === undefined) {
    return button;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent>{tooltip}</TooltipContent>
    </Tooltip>
  );
}

function handleTabListKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
  if (!(event.target instanceof HTMLButtonElement)) return;
  const tabs = Array.from(
    event.currentTarget.querySelectorAll<HTMLButtonElement>(
      '[role="tab"]:not(:disabled)',
    ),
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
  editorTheme,
  onActiveTabChange,
  t,
}: {
  activeTab: DiagnosticKey;
  diagnostics: Record<DiagnosticKey, DiagnosticArtifact>;
  loading: boolean;
  editorTheme: WorkbenchEditorThemeName;
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
            stage={tab}
            loading={loading}
            editorTheme={editorTheme}
            t={t}
          />
        </TabsContent>
      ))}
    </Tabs>
  );
}

function DiagnosticArtifactView({
  artifact,
  stage,
  loading,
  editorTheme,
  t,
}: {
  artifact: DiagnosticArtifact;
  stage: DiagnosticKey;
  loading: boolean;
  editorTheme: WorkbenchEditorThemeName;
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
    return (
      <RenderError
        engine="Merman"
        stage={stage}
        message={artifact.error}
        detail={artifact.errorDetail}
        t={t}
        compact
      />
    );
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
      theme={editorTheme}
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

function artifactUnavailableLabel({
  available,
  error,
  loading,
  t,
}: {
  available: boolean;
  error: string | null;
  loading: boolean;
  t: (key: string) => string;
}): string | null {
  if (available) {
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

function requirePublicationId(
  publicationId: RenderPublicationId | null,
): RenderPublicationId {
  if (publicationId === null) {
    throw new Error("Current render publication is unavailable.");
  }
  return publicationId;
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

function SvgBoundsToggle({
  pressed,
  label,
  onPressedChange,
}: {
  pressed: boolean;
  label: string;
  onPressedChange(pressed: boolean): void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          data-testid="svg-bounds-toggle"
          variant={pressed ? "secondary" : "ghost"}
          size="icon-sm"
          aria-label={label}
          aria-pressed={pressed}
          className={cn(pressed && "ring-1 ring-ring")}
          onClick={() => onPressedChange(!pressed)}
        >
          <SquareDashed className="size-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function SvgPresentationModeToggle({
  value,
  groupLabel,
  infiniteLabel,
  infiniteShortLabel,
  viewBoxLabel,
  viewBoxShortLabel,
  onValueChange,
}: {
  value: SvgPresentationMode;
  groupLabel: string;
  infiniteLabel: string;
  infiniteShortLabel: string;
  viewBoxLabel: string;
  viewBoxShortLabel: string;
  onValueChange(value: SvgPresentationMode): void;
}) {
  const labels = {
    infinite: {
      label: infiniteLabel,
      shortLabel: infiniteShortLabel,
    },
    viewbox: {
      label: viewBoxLabel,
      shortLabel: viewBoxShortLabel,
    },
  } satisfies Record<
    SvgPresentationMode,
    { readonly label: string; readonly shortLabel: string }
  >;

  return (
    <div
      role="group"
      aria-label={groupLabel}
      data-testid="svg-presentation-mode-toggle"
      className="flex shrink-0 items-center rounded-md border bg-background/70 p-0.5"
    >
      {SVG_PRESENTATION_MODES.map((choice) => {
        const pressed = value === choice;
        return (
          <Button
            key={choice}
            type="button"
            variant={pressed ? "secondary" : "ghost"}
            size="sm"
            aria-label={labels[choice].label}
            aria-pressed={pressed}
            className={cn(
              "h-7 px-2 text-xs",
              pressed && "ring-1 ring-ring",
            )}
            onClick={() => onValueChange(choice)}
          >
            <span className="sm:hidden">{labels[choice].shortLabel}</span>
            <span className="hidden sm:inline">{labels[choice].label}</span>
          </Button>
        );
      })}
    </div>
  );
}

function CenteredMessage({
  icon,
  children,
}: {
  icon?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="preview-canvas-status flex h-full flex-1 items-center justify-center">
      <div className="preview-canvas-status-muted flex flex-col items-center gap-3">
        {icon}
        <span className="text-sm">{children}</span>
      </div>
    </div>
  );
}

function RenderError({
  engine,
  stage,
  message,
  detail,
  t,
  compact = false,
}: {
  engine?: string;
  stage?: string | null;
  message: string;
  detail?: string | null;
  t: (key: string) => string;
  compact?: boolean;
}) {
  return (
    <div
      className="preview-canvas-status flex h-full flex-1 items-center justify-center p-6"
      data-merman-render-error="true"
      data-merman-error-engine={engine}
      data-merman-error-stage={stage ?? undefined}
      role="alert"
    >
      <div className={cn("text-center", compact ? "max-w-sm" : "max-w-md")}>
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-destructive/10">
          <AlertCircle className="size-6 text-destructive" />
        </div>
        <h3 className="mb-1 font-medium">
          {engine ? `${engine} · ${t("preview.error")}` : t("preview.error")}
        </h3>
        {stage && (
          <p className="preview-canvas-status-muted mb-2 font-mono text-xs">
            {stage}
          </p>
        )}
        <p className="preview-canvas-status-muted rounded-md bg-black/5 p-3 font-mono text-sm dark:bg-white/5">
          {message}
        </p>
        {detail && (
          <details className="preview-canvas-status-muted mt-3 text-left text-xs">
            <summary className="cursor-pointer select-none text-center">
              {t("preview.errorDetails")}
            </summary>
            <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted/50 p-3 font-mono">
              {detail}
            </pre>
          </details>
        )}
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
      className="preview-canvas-status flex h-full flex-1 items-center justify-center p-6"
      role="alert"
    >
      <div className="max-w-md text-center">
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-destructive/10">
          <AlertCircle className="size-6 text-destructive" />
        </div>
        <h3 className="mb-2 font-medium">
          {t("preview.error")}
        </h3>
        <p className="preview-canvas-status-muted mb-4 text-xs">{failure.stage}</p>
        <p className="preview-canvas-status-muted mb-4 rounded-md bg-black/5 p-3 font-mono text-sm dark:bg-white/5">
          {failure.message}
        </p>
        {failure.detail && (
          <details className="preview-canvas-status-muted mb-4 text-left text-xs">
            <summary className="cursor-pointer select-none text-center">
              {t("preview.errorDetails")}
            </summary>
            <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted/50 p-3 font-mono">
              {failure.detail}
            </pre>
          </details>
        )}
        <Button onClick={handleRecovery}>
          {t(failure.recovery === "reload" ? "wasm.reload" : "wasm.retry")}
        </Button>
      </div>
    </div>
  );
}

function successfulMerman(
  artifact: MermanRenderSuccess | EngineRenderFailure | null,
): MermanRenderSuccess | null {
  return artifact?.status === "success" ? artifact : null;
}

function successfulMermaid(
  artifact: MermaidRenderSuccess | EngineRenderFailure | null,
): MermaidRenderSuccess | null {
  return artifact?.status === "success" ? artifact : null;
}

function failedMessage(
  artifact:
    MermanRenderSuccess | MermaidRenderSuccess | EngineRenderFailure | null,
): string | null {
  return artifact?.status === "failure" ? artifact.message : null;
}

function failedDetail(
  artifact:
    MermanRenderSuccess | MermaidRenderSuccess | EngineRenderFailure | null,
): string | null {
  return artifact?.status === "failure" ? artifact.detail : null;
}

function failedStage(
  artifact:
    MermanRenderSuccess | MermaidRenderSuccess | EngineRenderFailure | null,
): string | null {
  return artifact?.status === "failure" ? artifact.stage : null;
}
