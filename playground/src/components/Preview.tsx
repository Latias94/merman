import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { assertSafeSvgForDom } from "@mermanjs/web";
import { toast } from "sonner";
import {
  mermanRuntimeErrorI18nKey,
  useMerman,
} from "@/src/hooks/useMerman";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import {
  asciiSupportDescription,
  asciiSupportLabelKey,
  type AsciiCapability,
} from "@/src/lib/ascii-support";
import { detectDiagramType } from "@/src/lib/diagram-detection";
import {
  bindRenderArtifact,
  createPreviewRenderKey,
  freshRenderArtifactValue,
  type RenderArtifact,
} from "@/src/lib/render-artifacts";
import { prewarmWasmRenderer } from "@/src/lib/wasm-loader";
import { useAppStore } from "@/src/store";
import {
  getMermaidLoadSource,
  isMermaidLoaded,
  mermaidRuntimeErrorI18nKey,
  prewarmMermaidRenderer,
  renderMermaidSvg,
  MERMAID_JS_VERSION,
} from "@/src/lib/mermaid-renderer";
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
type MermaidStatus = "idle" | "preparing" | "rendering";

interface CompareArtifact {
  key: EngineKey;
  artifactKey: string;
  renderKey: string;
  title: string;
  version: string;
  svg: string | null;
  renderArtifact: SvgRenderArtifact | null;
  error: string | null;
  renderTime: number | null;
  loading: boolean;
  loadingLabel: string | null;
  unavailableLabel: string | null;
}

interface DiagnosticArtifact {
  json: string | null;
  error: string | null;
  elapsedMs: number | null;
}

interface MermanRenderArtifact {
  svg: string | null;
  ascii: string | null;
  error: string | null;
  renderTime: number | null;
}

interface MermaidRenderArtifact {
  svg: string | null;
  error: string | null;
  renderTime: number | null;
}

type SvgRenderArtifact = RenderArtifact<MermanRenderArtifact | MermaidRenderArtifact>;

const EMPTY_DIAGNOSTICS: Record<DiagnosticKey, DiagnosticArtifact> = {
  parse: { json: null, error: null, elapsedMs: null },
  layout: { json: null, error: null, elapsedMs: null },
};

export function Preview({ className }: PreviewProps) {
  const { t } = useTranslation();
  const {
    code,
    diagramTheme,
    hostThemePreset,
    mermaidConfig,
    textMeasurementMode,
    diagramFont,
    setLastRenderTime,
    setDiagramType,
    isDarkMode,
  } = useAppStore();
  const { ready, loading, render, renderAscii, parseJson, layoutJson } = useMerman();
  const asciiSupport = useAsciiSupport();
  const [mermanRenderArtifact, setMermanRenderArtifact] =
    useState<RenderArtifact<MermanRenderArtifact> | null>(null);
  const [mermaidRenderArtifact, setMermaidRenderArtifact] =
    useState<RenderArtifact<MermaidRenderArtifact> | null>(null);
  const [previewMode, setPreviewMode] = useState<PreviewMode>("svg");
  const [svgDisplayMode, setSvgDisplayMode] =
    useState<SvgDisplayMode>("visual");
  const [copiedAsciiKey, setCopiedAsciiKey] = useState<string | null>(null);
  const [copiedDiagnostic, setCopiedDiagnostic] = useState<DiagnosticKey | null>(null);
  const [copiedSvgKey, setCopiedSvgKey] = useState<string | null>(null);
  const [exportingSvgKeys, setExportingSvgKeys] = useState<Set<string>>(
    () => new Set()
  );
  const [diagnosticTab, setDiagnosticTab] = useState<DiagnosticKey>("parse");
  const [diagnostics, setDiagnostics] =
    useState<Record<DiagnosticKey, DiagnosticArtifact>>(EMPTY_DIAGNOSTICS);
  const [diagnosticsLoading, setDiagnosticsLoading] = useState(false);
  const [mermaidStatus, setMermaidStatus] = useState<MermaidStatus>("idle");
  const [refreshNonce, setRefreshNonce] = useState(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const diagnosticsDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const asciiCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const diagnosticCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const activeHostThemePreset =
    hostThemePreset === "none" ? undefined : hostThemePreset;
  const renderOptions = useMemo(
    () => ({
      hostThemePreset: activeHostThemePreset,
      textMeasurementMode,
      diagramFont,
    }),
    [activeHostThemePreset, diagramFont, textMeasurementMode]
  );

  const detectedDiagramType = useMemo(
    () => (code.trim() ? detectDiagramType(code) : "flowchart"),
    [code]
  );
  const previewRenderKey = useMemo(
    () =>
      createPreviewRenderKey({
        code,
        diagramTheme,
        mermaidConfig,
        hostThemePreset: activeHostThemePreset ?? null,
        textMeasurementMode,
        diagramFont,
        refreshNonce,
      }),
    [
      activeHostThemePreset,
      code,
      diagramFont,
      diagramTheme,
      mermaidConfig,
      refreshNonce,
      textMeasurementMode,
    ]
  );
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
  const freshMermanArtifact = freshRenderArtifactValue(
    mermanRenderArtifact,
    previewRenderKey
  );
  const freshMermaidArtifact = freshRenderArtifactValue(
    mermaidRenderArtifact,
    previewRenderKey
  );
  const svg = freshMermanArtifact?.svg ?? null;
  const ascii = freshMermanArtifact?.ascii ?? null;
  const error = freshMermanArtifact?.error ?? null;
  const mermanRenderTime = freshMermanArtifact?.renderTime ?? null;
  const mermaidSvg = freshMermaidArtifact?.svg ?? null;
  const mermaidError = freshMermaidArtifact?.error ?? null;
  const mermaidRenderTime = freshMermaidArtifact?.renderTime ?? null;
  const mermanRendering = Boolean(code.trim() && ready && !svg && !error);
  const isAsciiSupported = asciiSupport.isSupported(detectedDiagramType);
  const asciiCapability = asciiSupport.capabilityFor(detectedDiagramType);
  const asciiSupportLabel = t(asciiSupportLabelKey(asciiCapability));
  const asciiSupportLimit = asciiSupportDescription(asciiCapability);
  const svgViewport = useSvgViewport({
    svg,
    enabled: previewMode === "svg",
  });
  const mermanCompareViewport = useSvgViewport({
    svg,
    enabled: previewMode === "compare",
  });
  const mermaidCompareViewport = useSvgViewport({
    svg: mermaidSvg,
    enabled: previewMode === "compare",
  });
  const localizeMermanError = useCallback(
    (message: string | null): string | null => {
      if (!message) return null;
      const key = mermanRuntimeErrorI18nKey(message);
      return key ? t(key) : message;
    },
    [t]
  );
  const localizeMermaidError = useCallback(
    (message: string | null): string | null => {
      if (!message) return null;
      const key = mermaidRuntimeErrorI18nKey(message);
      return key ? t(key) : message;
    },
    [t]
  );
  const warmCompareRenderer = useCallback(() => {
    void prewarmMermaidRenderer(diagramTheme, mermaidConfig, {
      diagramFont,
    });
  }, [diagramFont, diagramTheme, mermaidConfig]);
  useEffect(() => {
    let cancelled = false;

    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    debounceRef.current = setTimeout(() => {
      if (ready && code.trim()) {
        const diagramType = detectedDiagramType;
        setDiagramType(diagramType);

        void (async () => {
          await prewarmWasmRenderer(
            diagramTheme,
            mermaidConfig,
            renderOptions
          ).catch(() => undefined);
          if (cancelled) return;

          const result = render(code, diagramTheme, mermaidConfig, renderOptions);
          if (cancelled) return;

          const renderedAscii = asciiSupport.isSupported(diagramType)
            ? renderAscii(code, diagramTheme, mermaidConfig)
            : null;
          setMermanRenderArtifact(
            bindRenderArtifact(previewRenderKey, {
              svg: result.svg,
              ascii: renderedAscii,
              error: localizeMermanError(result.error),
              renderTime: result.error ? null : result.renderTime,
            })
          );
          setLastRenderTime(result.renderTime);

        })();
      } else if (!code.trim()) {
        setMermanRenderArtifact(null);
      }
    }, 300);

    return () => {
      cancelled = true;
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [
    code,
    asciiSupport,
    detectedDiagramType,
    diagramTheme,
    localizeMermanError,
    mermaidConfig,
    previewRenderKey,
    ready,
    render,
    renderOptions,
    renderAscii,
    setDiagramType,
    setLastRenderTime,
  ]);

  useEffect(() => {
    if (previewMode === "ascii" && !isAsciiSupported) {
      setPreviewMode("svg");
    }
  }, [isAsciiSupported, previewMode]);

  useEffect(() => {
    if (previewMode !== "compare" || !code.trim()) {
      setMermaidStatus("idle");
      if (!code.trim()) {
        setMermaidRenderArtifact(null);
      }
      return;
    }

    let cancelled = false;
    setMermaidStatus(isMermaidLoaded() ? "rendering" : "preparing");
    const timeout = setTimeout(() => {
      void (async () => {
        await prewarmMermaidRenderer(diagramTheme, mermaidConfig, {
          diagramFont,
        });
        if (cancelled) return;
        setMermaidStatus("rendering");
        const result = await renderMermaidSvg(code, diagramTheme, mermaidConfig, {
          diagramFont,
        });
        if (cancelled) return;
        setMermaidRenderArtifact(
          bindRenderArtifact(previewRenderKey, {
            svg: result.svg,
            error: localizeMermaidError(result.error),
            renderTime: result.renderTime,
          })
        );
        setMermaidStatus("idle");
      })();
    }, 300);

    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, [
    code,
    diagramFont,
    diagramTheme,
    localizeMermaidError,
    mermaidConfig,
    previewRenderKey,
    previewMode,
  ]);

  useEffect(() => {
    if (diagnosticsDebounceRef.current) {
      clearTimeout(diagnosticsDebounceRef.current);
    }

    setDiagnostics(EMPTY_DIAGNOSTICS);

    if (previewMode !== "diagnostics") {
      setDiagnosticsLoading(false);
      return;
    }

    if (!code.trim()) {
      setDiagnostics(EMPTY_DIAGNOSTICS);
      setDiagnosticsLoading(false);
      return;
    }

    if (!ready) {
      setDiagnostics(
        diagnosticsError(
          loading ? t("preview.loading") : t("preview.diagnosticsUnavailable")
        )
      );
      setDiagnosticsLoading(false);
      return;
    }

    setDiagnosticsLoading(true);
    diagnosticsDebounceRef.current = setTimeout(() => {
      setDiagnostics({
        parse: collectDiagnostic(
          () =>
            parseJson(
              code,
              diagramTheme,
              mermaidConfig,
              renderOptions
            ),
          localizeMermanError
        ),
        layout: collectDiagnostic(
          () =>
            layoutJson(
              code,
              diagramTheme,
              mermaidConfig,
              renderOptions
            ),
          localizeMermanError
        ),
      });
      setDiagnosticsLoading(false);
    }, 300);

    return () => {
      if (diagnosticsDebounceRef.current) {
        clearTimeout(diagnosticsDebounceRef.current);
      }
    };
  }, [
    code,
    diagramTheme,
    layoutJson,
    loading,
    localizeMermanError,
    mermaidConfig,
    parseJson,
    previewMode,
    ready,
    renderOptions,
    t,
  ]);

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
      const artifact = requireFreshRenderArtifact(
        mermanRenderArtifact,
        previewRenderKey
      );
      if (!artifact.ascii) {
        throw new Error("Current ASCII artifact is unavailable.");
      }
      await navigator.clipboard.writeText(artifact.ascii);
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
  }, [asciiActionKey, mermanRenderArtifact, previewRenderKey, t]);

  const handleCopySvg = useCallback(async (
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string,
    actionKey: string
  ) => {
    try {
      const safeSvg = requireFreshSvgArtifact(artifact, expectedRenderKey);
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
  }, [t]);

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

  const handleExportSvg = useCallback((
    engine: EngineKey,
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string
  ) => {
    try {
      const safeSvg = requireFreshSvgArtifact(artifact, expectedRenderKey);
      exportSVG(safeSvg, `merman-compare-${engine}`);
      toast.success(t("export.svgSuccess"));
    } catch {
      toast.error(t("export.failed"));
    }
  }, [t]);

  const handleExportPng = useCallback(async (
    engine: EngineKey,
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string,
    actionKey: string
  ) => {
    setExportingSvgKeys((keys) => new Set(keys).add(actionKey));
    try {
      const safeValue = requireFreshSvgArtifact(artifact, expectedRenderKey);
      let exportSvg = safeValue;
      if (engine === "merman") {
        const pngResult = render(code, diagramTheme, mermaidConfig, {
          ...renderOptions,
          pipeline: "resvg-safe",
        });
        if (!pngResult.svg) {
          throw new Error(pngResult.error ?? "Failed to render PNG SVG");
        }
        exportSvg = requireSafeSvgString(pngResult.svg);
      }

      await exportPNG(exportSvg, `merman-compare-${engine}`, 2);
      toast.success(t("export.pngSuccess"));
    } catch {
      toast.error(t("export.failed"));
    } finally {
      setExportingSvgKeys((keys) => {
        const next = new Set(keys);
        next.delete(actionKey);
        return next;
      });
    }
  }, [code, diagramTheme, mermaidConfig, render, renderOptions, t]);

  const handleRefreshCompare = useCallback(() => {
    setRefreshNonce((value) => value + 1);
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
      (mermaidStatus !== "idle" || (!mermaidSvg && !mermaidError))
  );
  const mermaidLoadingLabel =
    mermaidStatus === "preparing"
      ? t("preview.preparingMermaid")
      : mermaidStatus === "rendering"
        ? t("preview.renderingMermaid")
        : mermaidRendering
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
      onCompareWarmup={warmCompareRenderer}
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
        <CenteredMessage icon={<Loader2 className="size-8 animate-spin" />}>
          {t("preview.loading")}
        </CenteredMessage>
      </div>
    );
  }

  if (!code.trim()) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <div className="flex-1 flex items-center justify-center">
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
        <RenderError message={error} t={t} />
      </div>
    );
  }

  const mermanArtifact: CompareArtifact = {
    key: "merman",
    artifactKey: mermanSvgActionKey,
    renderKey: previewRenderKey,
    title: t("preview.mermanEngine"),
    version: "WASM",
    svg,
    renderArtifact: mermanRenderArtifact,
    error,
    renderTime: svg ? mermanRenderTime : null,
    loading: mermanRendering,
    loadingLabel: mermanRendering ? t("preview.renderingCurrent") : null,
    unavailableLabel: mermanSvgUnavailableLabel,
  };
  const mermaidLoadSource = getMermaidLoadSource();
  const mermaidArtifact: CompareArtifact = {
    key: "mermaid",
    artifactKey: mermaidSvgActionKey,
    renderKey: previewRenderKey,
    title: t("preview.mermaidEngine"),
    version:
      mermaidLoadSource === "cdn"
        ? t("preview.mermaidVersionCdn", { version: MERMAID_JS_VERSION })
        : MERMAID_JS_VERSION,
    svg: mermaidSvg,
    renderArtifact: mermaidRenderArtifact,
    error: mermaidError,
    renderTime: mermaidRenderTime,
    loading: mermaidRendering,
    loadingLabel: mermaidLoadingLabel,
    unavailableLabel: mermaidSvgUnavailableLabel,
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
                  handleCopySvg(
                    mermanRenderArtifact,
                    previewRenderKey,
                    mermanSvgActionKey
                  )
                }
                disabled={Boolean(mermanSvgUnavailableLabel)}
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
                <Button variant="ghost" size="icon-sm" onClick={handleCopyAscii}>
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

      <div className="flex-1 min-h-0 relative overflow-hidden">
        {previewMode === "svg" && (
          svgDisplayMode === "source" ? (
            <SvgSourceEditor svg={svg} isDarkMode={isDarkMode} />
          ) : (
            <SvgViewport svg={svg} controller={svgViewport} />
          )
        )}

        {previewMode === "compare" && (
          <CompareView
            mermanArtifact={mermanArtifact}
            mermaidArtifact={mermaidArtifact}
            mermanController={mermanCompareViewport}
            mermaidController={mermaidCompareViewport}
            copiedSvgKey={copiedSvgKey}
            exportingSvgKeys={exportingSvgKeys}
            isDarkMode={isDarkMode}
            onCopySvg={handleCopySvg}
            onExportSvg={handleExportSvg}
            onExportPng={handleExportPng}
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
  onCompareWarmup: () => void;
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
  onCompareWarmup,
  isAsciiSupported,
  asciiCapability,
  asciiSupportLabel,
  asciiSupportLimit,
  t,
  rightContent,
}: TabBarProps) {
  return (
    <div className="flex h-10 shrink-0 items-center justify-between gap-2 overflow-hidden border-b bg-muted/30 px-2">
      <div className="scrollbar-thin flex min-w-0 items-center gap-1 overflow-x-auto">
        <TabButton active={mode === "svg"} onClick={() => onModeChange("svg")}>
          SVG
        </TabButton>
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              onClick={() => isAsciiSupported && onModeChange("ascii")}
              disabled={!isAsciiSupported}
              className={cn(
                "shrink-0 px-3 py-1.5 text-sm rounded-md transition-colors",
                mode === "ascii"
                  ? "bg-background text-foreground shadow-sm font-medium"
                  : "text-muted-foreground hover:text-foreground hover:bg-background/50",
                !isAsciiSupported &&
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
          active={mode === "compare"}
          onClick={() => onModeChange("compare")}
          onFocus={onCompareWarmup}
          onPointerEnter={onCompareWarmup}
        >
          {t("preview.compareMode")}
        </TabButton>
        <TabButton
          active={mode === "diagnostics"}
          onClick={() => onModeChange("diagnostics")}
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
  active: boolean;
  onClick(): void;
  onFocus?: () => void;
  onPointerEnter?: () => void;
  children: ReactNode;
}

function TabButton({
  active,
  onClick,
  onFocus,
  onPointerEnter,
  children,
}: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      onFocus={onFocus}
      onPointerEnter={onPointerEnter}
      className={cn(
        "shrink-0 px-3 py-1.5 text-sm rounded-md transition-colors",
        active
          ? "bg-background text-foreground shadow-sm font-medium"
          : "text-muted-foreground hover:text-foreground hover:bg-background/50"
      )}
    >
      {children}
    </button>
  );
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
    <div className="flex h-full flex-col bg-background">
      <div className="flex min-h-10 items-center justify-between gap-2 border-b bg-muted/20 px-3 py-2">
        <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
          <TabButton
            active={activeTab === "parse"}
            onClick={() => onActiveTabChange("parse")}
          >
            {t("preview.parseJson")}
          </TabButton>
          <TabButton
            active={activeTab === "layout"}
            onClick={() => onActiveTabChange("layout")}
          >
            {t("preview.layoutJson")}
          </TabButton>
        </div>
        <p className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {loading
            ? t("preview.runningDiagnostics")
            : current.elapsedMs !== null
              ? `${current.elapsedMs.toFixed(1)}ms`
              : "-"}
        </p>
      </div>

      <div className="min-h-0 flex-1">
        {loading ? (
          <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
            {t("preview.runningDiagnostics")}
          </CenteredMessage>
        ) : current.error ? (
          <RenderError message={current.error} t={t} compact />
        ) : current.json ? (
          <Editor
            height="100%"
            language="json"
            value={current.json}
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
        ) : (
          <CenteredMessage icon={<FileCode className="size-8" />}>
            {t("preview.diagnosticsEmpty")}
          </CenteredMessage>
        )}
      </div>
    </div>
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
  mermanArtifact,
  mermaidArtifact,
  mermanController,
  mermaidController,
  copiedSvgKey,
  exportingSvgKeys,
  isDarkMode,
  onCopySvg,
  onExportSvg,
  onExportPng,
  t,
}: {
  mermanArtifact: CompareArtifact;
  mermaidArtifact: CompareArtifact;
  mermanController: SvgViewportController;
  mermaidController: SvgViewportController;
  copiedSvgKey: string | null;
  exportingSvgKeys: ReadonlySet<string>;
  isDarkMode: boolean;
  onCopySvg(
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string,
    actionKey: string
  ): void;
  onExportSvg(
    engine: EngineKey,
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string
  ): void;
  onExportPng(
    engine: EngineKey,
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string,
    actionKey: string
  ): void;
  t: (key: string) => string;
}) {
  return (
    <div className="h-full overflow-auto p-3">
      <div className="grid min-h-full grid-cols-1 gap-3 xl:grid-cols-2">
        <ComparePane
          artifact={mermanArtifact}
          controller={mermanController}
          copied={copiedSvgKey === mermanArtifact.artifactKey}
          exporting={exportingSvgKeys.has(mermanArtifact.artifactKey)}
          isDarkMode={isDarkMode}
          onCopySvg={onCopySvg}
          onExportSvg={onExportSvg}
          onExportPng={onExportPng}
          t={t}
        />
        <ComparePane
          artifact={mermaidArtifact}
          controller={mermaidController}
          copied={copiedSvgKey === mermaidArtifact.artifactKey}
          exporting={exportingSvgKeys.has(mermaidArtifact.artifactKey)}
          isDarkMode={isDarkMode}
          onCopySvg={onCopySvg}
          onExportSvg={onExportSvg}
          onExportPng={onExportPng}
          t={t}
        />
      </div>
    </div>
  );
}

function ComparePane({
  artifact,
  controller,
  copied,
  exporting,
  isDarkMode,
  onCopySvg,
  onExportSvg,
  onExportPng,
  t,
}: {
  artifact: CompareArtifact;
  controller: SvgViewportController;
  copied: boolean;
  exporting: boolean;
  isDarkMode: boolean;
  onCopySvg(
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string,
    actionKey: string
  ): void;
  onExportSvg(
    engine: EngineKey,
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string
  ): void;
  onExportPng(
    engine: EngineKey,
    artifact: SvgRenderArtifact | null,
    expectedRenderKey: string,
    actionKey: string
  ): void;
  t: (key: string) => string;
}) {
  const hasSvg = Boolean(artifact.svg);
  const actionsDisabled = Boolean(artifact.unavailableLabel);
  const [svgDisplayMode, setSvgDisplayMode] =
    useState<SvgDisplayMode>("visual");

  return (
    <section className="flex min-h-[320px] flex-col overflow-hidden rounded-md border bg-background xl:min-h-0">
      <div className="border-b bg-muted/30 px-3 py-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate text-sm font-medium">{artifact.title}</span>
            <span className="shrink-0 rounded-sm bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
              {artifact.version}
            </span>
          </div>
          <p className="shrink-0 text-xs text-muted-foreground">
            {artifact.loading
              ? (artifact.loadingLabel ?? t("preview.renderingMermaid"))
              : artifact.renderTime !== null
                ? `${artifact.renderTime.toFixed(1)}ms`
                : "-"}
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
                onCopySvg(
                  artifact.renderArtifact,
                  artifact.renderKey,
                  artifact.artifactKey
                )
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
                onExportSvg(
                  artifact.key,
                  artifact.renderArtifact,
                  artifact.renderKey
                )
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
                onExportPng(
                  artifact.key,
                  artifact.renderArtifact,
                  artifact.renderKey,
                  artifact.artifactKey
                )
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
      <div className="min-h-0 flex-1">
        {artifact.loading ? (
          <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
            {artifact.loadingLabel ?? t("preview.renderingMermaid")}
          </CenteredMessage>
        ) : artifact.error ? (
          <RenderError message={artifact.error} t={t} compact />
        ) : svgDisplayMode === "source" ? (
          <SvgSourceEditor svg={artifact.svg} isDarkMode={isDarkMode} />
        ) : (
          <SvgViewport
            svg={artifact.svg}
            controller={controller}
            empty={
              <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                {t("preview.empty")}
              </div>
            }
          />
        )}
      </div>
    </section>
  );
}

function artifactActionKey(kind: string, renderKey: string): string {
  return JSON.stringify([kind, renderKey]);
}

function requireFreshRenderArtifact<T>(
  artifact: RenderArtifact<T> | null,
  expectedKey: string
): T {
  if (!artifact || artifact.key !== expectedKey) {
    throw new Error("Rendered artifact is not current.");
  }
  return artifact.value;
}

function requireFreshSvgArtifact(
  artifact: SvgRenderArtifact | null,
  expectedKey: string
): string {
  const value = requireFreshRenderArtifact(artifact, expectedKey);
  if (!value.svg) {
    throw new Error("Current SVG artifact is unavailable.");
  }
  return requireSafeSvgString(value.svg);
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

function collectDiagnostic(
  createJson: () => string,
  localizeError: (message: string | null) => string | null
): DiagnosticArtifact {
  const start = performance.now();

  try {
    const json = formatDiagnosticJson(createJson());
    return {
      json,
      error: null,
      elapsedMs: performance.now() - start,
    };
  } catch (err) {
    return {
      json: null,
      error: localizeError(err instanceof Error ? err.message : String(err)),
      elapsedMs: null,
    };
  }
}

function diagnosticsError(message: string): Record<DiagnosticKey, DiagnosticArtifact> {
  return {
    parse: { json: null, error: message, elapsedMs: null },
    layout: { json: null, error: message, elapsedMs: null },
  };
}

function formatDiagnosticJson(rawJson: string): string {
  try {
    return `${JSON.stringify(JSON.parse(rawJson), null, 2)}\n`;
  } catch {
    return rawJson;
  }
}
