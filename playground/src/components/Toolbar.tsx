import {
  lazy,
  useCallback,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  useAppStore,
  selectWorkspaceSnapshot,
  type SvgPipeline,
  type TextMeasurementMode,
  type Theme,
  type UITheme,
} from "@/src/store";
import {
  selectCompletedRenderBatch,
  selectCurrentDiagramType,
  selectCurrentMermanRenderTime,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import {
  DIAGRAM_FONT_VALUES,
  isDiagramFont,
  type DiagramFont,
} from "@/src/lib/diagram-font";
import { copyShareUrl } from "@/src/lib/share";
import { copyCodeToClipboard } from "@/src/lib/export";
import { executeArtifactAction } from "@/src/runtime/artifact-actions-browser";
import { pngExportErrorMessage } from "@/src/components/png-export-feedback";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { pauseRenderCoordinator } from "@/src/runtime/render-coordinator-browser";
import {
  asciiSupportDescription,
  asciiSupportLabelKey,
} from "@/src/lib/ascii-support";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { presentationProfileStatus } from "@/src/runtime/presentation-status";
import {
  isMermanSvgPipeline,
  MERMAN_SVG_PIPELINES,
} from "@/src/runtime/merman-core";
import { languages, changeLanguage, getCurrentLanguage } from "@/src/i18n";
import {
  createMarkdownImageLink,
  createMermaidLiveEditorUrl,
} from "@/src/lib/mermaid-live";
import {
  SUPPORTED_THEMES,
  normalizeThemeName,
} from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { toast, Toaster } from "sonner";
import {
  Download,
  Share2,
  BookOpen,
  Palette,
  Sun,
  Moon,
  Monitor,
  Copy,
  ImageIcon,
  FileCode,
  ChevronDown,
  GitFork,
  Languages,
  FileText,
  Code,
  ExternalLink,
  Gauge,
  Type,
} from "lucide-react";

const BenchWorkbench = lazy(() =>
  import("@/src/components/BenchWorkbench").then((module) => ({
    default: module.BenchWorkbench,
  })),
);
const ExampleGallery = lazy(() =>
  import("@/src/components/ExampleGallery").then((module) => ({
    default: module.ExampleGallery,
  })),
);

const UI_THEME_ICONS: Record<UITheme, ReactNode> = {
  light: <Sun className="size-4" />,
  dark: <Moon className="size-4" />,
  system: <Monitor className="size-4" />,
};

const TEXT_MEASUREMENT_VALUES: readonly TextMeasurementMode[] = [
  "browser",
  "headless",
];
const NO_PRESENTATION_SELECTION = "__none__";
const PRESENTATION_STATUS_ID = "presentation-profile-status";

function openIdOptions(
  ids: readonly string[],
  selectedId: string | null,
  labelFor: (id: string) => string,
): { value: string; label: string }[] {
  const values = [...new Set(ids)];
  if (selectedId && !values.includes(selectedId)) values.push(selectedId);
  return values.map((value) => ({ value, label: labelFor(value) }));
}

function BenchLauncher() {
  const { t } = useTranslation();
  const [activated, setActivated] = useState(false);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useCallback(() => triggerRef.current?.focus(), []);

  const openBench = () => {
    setActivated(true);
    setOpen(true);
  };

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            ref={triggerRef}
            variant={open ? "secondary" : "ghost"}
            size="sm"
            aria-label={t("toolbar.bench")}
            className="size-10 px-0 lg:h-8 lg:w-auto lg:px-2.5"
            onClick={openBench}
          >
            <Gauge className="size-4" />
            <span className="hidden lg:inline">{t("toolbar.bench")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.bench")}</TooltipContent>
      </Tooltip>

      {activated && (
        <LazyFeatureBoundary
          feature={t("toolbar.bench")}
          presentation={{
            kind: "dialog",
            open,
            onOpenChange: setOpen,
            restoreFocus,
          }}
        >
          <BenchWorkbench
            open={open}
            onOpenChange={setOpen}
            pauseCoordinator={pauseRenderCoordinator}
            restoreFocus={restoreFocus}
          />
        </LazyFeatureBoundary>
      )}
    </>
  );
}

function ExamplesLauncher() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useCallback(() => triggerRef.current?.focus(), []);

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            ref={triggerRef}
            variant={open ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setOpen(true)}
            aria-label={t("toolbar.examples")}
            className="size-10 px-0 sm:h-8 sm:w-auto sm:px-2.5"
          >
            <BookOpen className="size-4" />
            <span className="hidden sm:inline">{t("toolbar.examples")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.examples")}</TooltipContent>
      </Tooltip>

      {open && (
        <LazyFeatureBoundary
          feature={t("toolbar.examples")}
          presentation={{
            kind: "dialog",
            open,
            onOpenChange: setOpen,
            restoreFocus,
          }}
        >
          <ExampleGallery
            open={open}
            onOpenChange={setOpen}
            restoreFocus={restoreFocus}
          />
        </LazyFeatureBoundary>
      )}
    </>
  );
}

export function Toolbar() {
  const { t } = useTranslation();
  const {
    code,
    diagramTheme,
    mermaidConfig,
    presentationProfileId,
    presentationThemePresetId,
    setDiagramTheme,
    setPresentationProfileId,
    setPresentationThemePresetId,
    setSvgPipeline,
    svgPipeline,
    textMeasurementMode,
    setTextMeasurementMode,
    diagramFont,
    setDiagramFont,
    uiTheme,
    setUITheme,
  } = useAppStore(
    useShallow((state) => ({
      code: state.code,
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
      mermaidConfig: state.mermaidConfig,
      presentationProfileId: state.presentationProfileId,
      presentationThemePresetId: state.presentationThemePresetId,
      setDiagramFont: state.setDiagramFont,
      setDiagramTheme: state.setDiagramTheme,
      setPresentationProfileId: state.setPresentationProfileId,
      setPresentationThemePresetId: state.setPresentationThemePresetId,
      setSvgPipeline: state.setSvgPipeline,
      setTextMeasurementMode: state.setTextMeasurementMode,
      setUITheme: state.setUITheme,
      svgPipeline: state.svgPipeline,
      textMeasurementMode: state.textMeasurementMode,
      uiTheme: state.uiTheme,
    }))
  );
  const diagramType = useRenderCoordinator(selectCurrentDiagramType);
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const currentBatch = useRenderCoordinator(selectCompletedRenderBatch);
  const facade = useMermanRuntime(selectMermanFacade);
  const asciiSupport = useAsciiSupport();
  const [isExporting, setIsExporting] = useState(false);
  const currentLang = getCurrentLanguage();
  const presentationCatalog = useMemo(() => {
    try {
      return facade?.presentationCatalog() ?? null;
    } catch {
      return null;
    }
  }, [facade]);

  const themeOptions: { value: Theme; label: string }[] = useMemo(() => {
    const seen = new Set<Theme>();
    return (facade?.getThemes() ?? SUPPORTED_THEMES)
      .map(normalizeThemeName)
      .filter((theme) => {
        if (seen.has(theme)) return false;
        seen.add(theme);
        return true;
      })
      .map((theme) => ({
        value: theme,
        label: t(`themes.${theme}`, { defaultValue: theme }),
      }));
  }, [facade, t]);

  const presentationThemeOptions = useMemo(
    () =>
      openIdOptions(
        presentationCatalog?.theme_presets.map((preset) => preset.id) ?? [],
        presentationThemePresetId,
        (id) => t(`presentationThemes.${id}`, { defaultValue: id }),
      ),
    [presentationCatalog, presentationThemePresetId, t],
  );
  const presentationProfileOptions = useMemo(
    () =>
      openIdOptions(
        presentationCatalog?.profiles.map((profile) => profile.id) ?? [],
        presentationProfileId,
        (id) => t(`presentationProfiles.${id}`, { defaultValue: id }),
      ),
    [presentationCatalog, presentationProfileId, t],
  );
  const currentProfileStatus = useMemo(() => {
    if (!presentationCatalog || !presentationProfileId) return null;
    const catalogProfile = presentationCatalog.profiles.find(
      (profile) => profile.id === presentationProfileId,
    );
    if (!catalogProfile?.fully_available && !currentBatch) return null;
    return presentationProfileStatus({
      catalog: presentationCatalog,
      detection:
        currentBatch?.detection ?? {
          status: "unavailable",
          validity: "unknown",
          diagramType: null,
          syntaxId: null,
          effectiveLayoutId: null,
        },
      plan: currentBatch?.svgPlan ?? null,
      selectedProfileId: presentationProfileId,
    });
  }, [currentBatch, presentationCatalog, presentationProfileId]);
  const presentationStatusText = !presentationProfileId
    ? t("presentationStatus.none")
    : currentProfileStatus
      ? t(`presentationStatus.${currentProfileStatus.kind}`, {
          capabilities:
            currentProfileStatus.missingCapabilityIds.join(", ") || "—",
          profile: presentationProfileId,
        })
      : t("presentationStatus.pending", { profile: presentationProfileId });
  const renderThemeLabel = t("toolbar.presentation");
  const renderSettingsLabel = t("toolbar.renderSettings");
  const asciiCapability = asciiSupport.capabilityFor(diagramType);
  const asciiSupported = asciiSupport.isSupported(diagramType);
  const asciiSupportLabel = t(asciiSupportLabelKey(asciiCapability));
  const asciiSupportLimit = asciiSupportDescription(asciiCapability);
  const asciiExportDescription = asciiSupported
    ? [asciiSupportLabel, asciiSupportLimit].filter(Boolean).join(" · ")
    : t("export.asciiNotSupported");

  const UI_THEME_OPTIONS: { value: UITheme; label: string }[] = [
    { value: "light", label: t("uiThemes.light") },
    { value: "dark", label: t("uiThemes.dark") },
    { value: "system", label: t("uiThemes.system") },
  ];

  const currentMerman =
    currentBatch?.merman.status === "success" ? currentBatch.merman : null;
  const artifactActionsEnabled = currentMerman !== null;

  const handleExportSVG = useCallback(async () => {
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      await executeArtifactAction({
        action: "download-svg",
        engine: "merman",
        publicationId: currentBatch.snapshot.publicationId,
      });
      toast.success(t("export.svgSuccess"));
    } catch {
      toast.error(t("export.failed"));
    }
  }, [currentBatch, t]);

  const handleExportPNG = useCallback(async () => {
    setIsExporting(true);
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      const plan = await executeArtifactAction({
        action: "download-png",
        engine: "merman",
        publicationId: currentBatch.snapshot.publicationId,
        scale: 2,
      });
      toast.success(
        t("export.pngSuccess", {
          width: plan.outputWidth,
          height: plan.outputHeight,
        })
      );
    } catch (error) {
      toast.error(pngExportErrorMessage(error, t));
    } finally {
      setIsExporting(false);
    }
  }, [currentBatch, t]);

  const handleExportASCII = useCallback(async () => {
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      await executeArtifactAction({
        action: "download-ascii",
        publicationId: currentBatch.snapshot.publicationId,
      });
      toast.success(t("export.asciiSuccess"));
    } catch {
      toast.error(t("export.asciiNotSupported"));
    }
  }, [currentBatch, t]);

  const handleCopyCode = useCallback(async () => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyCodeToClipboard(code);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [code, t]);

  const handleCopyMarkdown = useCallback(async () => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await navigator.clipboard.writeText(
        createMarkdownImageLink(code, diagramTheme, mermaidConfig)
      );
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [code, diagramTheme, mermaidConfig, t]);

  const handleCopySVG = useCallback(async () => {
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      await executeArtifactAction({
        action: "copy-svg",
        engine: "merman",
        publicationId: currentBatch.snapshot.publicationId,
      });
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [currentBatch, t]);

  const handleShare = useCallback(async () => {
    const snapshot = selectWorkspaceSnapshot(useAppStore.getState());
    if (!snapshot.code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyShareUrl(snapshot);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [t]);

  const handleOpenMermaidLive = useCallback(() => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    window.open(
      createMermaidLiveEditorUrl(code, diagramTheme, mermaidConfig),
      "_blank",
      "noopener,noreferrer"
    );
  }, [code, diagramTheme, mermaidConfig, t]);

  const normalizePresentationId = useCallback(
    (value: string): string | null =>
      value === NO_PRESENTATION_SELECTION ? null : value,
    [],
  );
  const normalizeTextMeasurementValue = useCallback(
    (value: string): TextMeasurementMode =>
      value === "headless" ? "headless" : "browser",
    []
  );
  const normalizeDiagramFontValue = useCallback(
    (value: string): DiagramFont =>
      isDiagramFont(value) ? value : "trebuchet",
    []
  );
  const normalizeSvgPipeline = useCallback(
    (value: string): SvgPipeline =>
      isMermanSvgPipeline(value) ? value : "parity",
    [],
  );

  const handleLanguageChange = useCallback((lang: string) => {
    changeLanguage(lang as "en" | "zh");
  }, []);

  const renderThemeMenuContent = () => (
    <DropdownMenuContent align="end">
      <DropdownMenuLabel>{t("toolbar.theme")}</DropdownMenuLabel>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.mermaidTheme")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={diagramTheme}
        onValueChange={(v) => setDiagramTheme(normalizeThemeName(v))}
      >
        {themeOptions.map((option) => (
          <DropdownMenuRadioItem key={option.value} value={option.value}>
            {option.label}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.presentationTheme")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={presentationThemePresetId ?? NO_PRESENTATION_SELECTION}
        onValueChange={(value) =>
          setPresentationThemePresetId(normalizePresentationId(value))
        }
      >
        <DropdownMenuRadioItem value={NO_PRESENTATION_SELECTION}>
          {t("presentationThemes.none")}
        </DropdownMenuRadioItem>
        {presentationThemeOptions.map((option) => (
          <DropdownMenuRadioItem key={option.value} value={option.value}>
            {option.label}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.presentationProfile")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        aria-describedby={PRESENTATION_STATUS_ID}
        value={presentationProfileId ?? NO_PRESENTATION_SELECTION}
        onValueChange={(value) =>
          setPresentationProfileId(normalizePresentationId(value))
        }
      >
        <DropdownMenuRadioItem value={NO_PRESENTATION_SELECTION}>
          {t("presentationProfiles.none")}
        </DropdownMenuRadioItem>
        {presentationProfileOptions.map((option) => (
          <DropdownMenuRadioItem key={option.value} value={option.value}>
            {option.label}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      <p className="max-w-72 px-2 py-1 text-xs text-muted-foreground">
        {presentationStatusText}
      </p>
    </DropdownMenuContent>
  );

  const renderRenderSettingsMenuContent = () => (
    <DropdownMenuContent align="end">
      <DropdownMenuLabel>{renderSettingsLabel}</DropdownMenuLabel>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.font")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={diagramFont}
        onValueChange={(v) => setDiagramFont(normalizeDiagramFontValue(v))}
      >
        {DIAGRAM_FONT_VALUES.map((font) => (
          <DropdownMenuRadioItem key={font} value={font}>
            {t(`diagramFonts.${font}`)}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.textMeasurement")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={textMeasurementMode}
        onValueChange={(v) =>
          setTextMeasurementMode(normalizeTextMeasurementValue(v))
        }
      >
        {TEXT_MEASUREMENT_VALUES.map((mode) => (
          <DropdownMenuRadioItem key={mode} value={mode}>
            {t(`textMeasurement.${mode}`)}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.svgOutput")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={svgPipeline}
        onValueChange={(value) => setSvgPipeline(normalizeSvgPipeline(value))}
      >
        {MERMAN_SVG_PIPELINES.map((pipeline) => (
          <DropdownMenuRadioItem key={pipeline} value={pipeline}>
            {t(`svgPipelines.${pipeline}`)}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <div className="sm:hidden">
        <DropdownMenuSeparator />
        <DropdownMenuLabel>{t("toolbar.toggleTheme")}</DropdownMenuLabel>
        <DropdownMenuRadioGroup
          value={uiTheme}
          onValueChange={(value) => setUITheme(value as UITheme)}
        >
          {UI_THEME_OPTIONS.map((option) => (
            <DropdownMenuRadioItem key={option.value} value={option.value}>
              {UI_THEME_ICONS[option.value]}
              {option.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>{t("toolbar.language")}</DropdownMenuLabel>
        <DropdownMenuRadioGroup
          value={currentLang}
          onValueChange={handleLanguageChange}
        >
          {languages.map((lang) => (
            <DropdownMenuRadioItem key={lang.code} value={lang.code}>
              <span className="mr-2">{lang.flag}</span>
              {lang.name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </div>
    </DropdownMenuContent>
  );

  return (
    <>
      <Toaster position="bottom-right" richColors />
      <header className="relative flex h-14 shrink-0 items-center gap-2 overflow-hidden border-b bg-card px-3 sm:px-4">
        <span
          id={PRESENTATION_STATUS_ID}
          className="sr-only"
          aria-live="polite"
        >
          {presentationStatusText}
        </span>
        {/* 左侧：Logo 和功能按钮 */}
        <div className="flex min-w-0 shrink-0 items-center gap-2 sm:gap-4">
          <div className="flex items-center gap-2">
            <img
              src={`${import.meta.env.BASE_URL}icon.svg`}
              alt=""
              aria-hidden="true"
              className="size-8 rounded-md"
            />
            <div className="hidden sm:block">
              <h1 className="text-sm font-semibold leading-none">Merman</h1>
              <p className="text-xs text-muted-foreground">{t("app.playground")}</p>
            </div>
          </div>

          <div className="hidden h-6 w-px bg-border sm:block" />

          <ExamplesLauncher />
          <BenchLauncher />

        </div>

        <div className="absolute right-3 top-1/2 flex -translate-y-1/2 items-center gap-1 sm:hidden">
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon-sm"
                    aria-label={t("toolbar.theme")}
                    aria-describedby={PRESENTATION_STATUS_ID}
                  >
                    <Palette className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{t("toolbar.theme")}</TooltipContent>
            </Tooltip>
            {renderThemeMenuContent()}
          </DropdownMenu>

          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon-sm"
                    aria-label={renderSettingsLabel}
                  >
                    <Type className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{renderSettingsLabel}</TooltipContent>
            </Tooltip>
            {renderRenderSettingsMenuContent()}
          </DropdownMenu>

          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon-sm"
                    disabled={isExporting}
                    aria-label={t("toolbar.export")}
                  >
                    <Download className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{t("toolbar.export")}</TooltipContent>
            </Tooltip>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>{t("export.title")}</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={handleExportSVG}
                disabled={!artifactActionsEnabled}
              >
                <FileCode className="size-4" />
                {t("export.svg")}
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleExportPNG}
                disabled={!artifactActionsEnabled}
              >
                <ImageIcon className="size-4" />
                {t("export.png")}
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleExportASCII}
                disabled={!currentMerman?.ascii}
              >
                <FileText className="size-4" />
                {t("export.ascii")}
                <span className="ml-auto max-w-44 truncate text-xs text-muted-foreground">
                  {asciiExportDescription}
                </span>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleCopyCode}>
                <Code className="size-4" />
                {t("export.copyCode")}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleCopyMarkdown}>
                <FileText className="size-4" />
                {t("export.copyMarkdown")}
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleCopySVG}
                disabled={!artifactActionsEnabled}
              >
                <Copy className="size-4" />
                {t("export.copySvg")}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleOpenMermaidLive}>
                <ExternalLink className="size-4" />
                {t("share.openMermaidLive")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="icon-sm"
                onClick={handleShare}
                aria-label={t("share.copyLink")}
              >
                <Share2 className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("share.copyLink")}</TooltipContent>
          </Tooltip>
        </div>

        {/* 右侧：主题、导出、分享 */}
        <div className="hidden min-w-0 items-center gap-2 sm:ml-auto sm:flex">
          {/* 渲染时间 */}
          {lastRenderTime > 0 && (
            <span className="text-xs text-muted-foreground hidden md:inline">
              {lastRenderTime.toFixed(1)}ms
            </span>
          )}

          {/* 图表主题 */}
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="w-8 px-0 sm:w-auto sm:px-2.5"
                    aria-label={t("toolbar.theme")}
                    aria-describedby={PRESENTATION_STATUS_ID}
                  >
                    <Palette className="size-4" />
                    <span className="hidden sm:inline">{renderThemeLabel}</span>
                    <ChevronDown className="hidden size-3 opacity-50 sm:block" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{t("toolbar.theme")}</TooltipContent>
            </Tooltip>
            {renderThemeMenuContent()}
          </DropdownMenu>

          {/* 渲染设置 */}
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="w-8 px-0 sm:w-auto sm:px-2.5"
                    aria-label={renderSettingsLabel}
                  >
                    <Type className="size-4" />
                    <span className="hidden sm:inline">{renderSettingsLabel}</span>
                    <ChevronDown className="hidden size-3 opacity-50 sm:block" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{renderSettingsLabel}</TooltipContent>
            </Tooltip>
            {renderRenderSettingsMenuContent()}
          </DropdownMenu>

          {/* 导出 */}
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="w-8 px-0 sm:w-auto sm:px-2.5"
                    disabled={isExporting}
                    aria-label={t("toolbar.export")}
                  >
                    <Download className="size-4" />
                    <span className="hidden sm:inline">{t("toolbar.export")}</span>
                    <ChevronDown className="hidden size-3 opacity-50 sm:block" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{t("toolbar.export")}</TooltipContent>
            </Tooltip>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>{t("export.title")}</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={handleExportSVG}
                disabled={!artifactActionsEnabled}
              >
                <FileCode className="size-4" />
                {t("export.svg")}
                <span className="ml-auto text-xs text-muted-foreground">{t("export.svgDesc")}</span>
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleExportPNG}
                disabled={!artifactActionsEnabled}
              >
                <ImageIcon className="size-4" />
                {t("export.png")}
                <span className="ml-auto text-xs text-muted-foreground">{t("export.pngDesc")}</span>
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleExportASCII}
                disabled={!currentMerman?.ascii}
              >
                <FileText className="size-4" />
                {t("export.ascii")}
                <span className="ml-auto text-xs text-muted-foreground">
                  {asciiExportDescription}
                </span>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleCopyCode}>
                <Code className="size-4" />
                {t("export.copyCode")}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleCopyMarkdown}>
                <FileText className="size-4" />
                {t("export.copyMarkdown")}
                <span className="ml-auto text-xs text-muted-foreground">
                  {t("export.copyMarkdownDesc")}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleCopySVG}
                disabled={!artifactActionsEnabled}
              >
                <Copy className="size-4" />
                {t("export.copySvg")}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleOpenMermaidLive}>
                <ExternalLink className="size-4" />
                {t("share.openMermaidLive")}
                <span className="ml-auto text-xs text-muted-foreground">
                  {t("share.openMermaidLiveDesc")}
                </span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          {/* 分享 */}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="w-8 px-0 sm:w-auto sm:px-2.5"
                onClick={handleShare}
                aria-label={t("share.copyLink")}
              >
                <Share2 className="size-4" />
                <span className="hidden sm:inline">{t("toolbar.share")}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("share.copyLink")}</TooltipContent>
          </Tooltip>

          <div className="hidden h-6 w-px shrink-0 bg-border sm:block" />

          {/* 语言切换 */}
          <div className="hidden sm:block">
            <DropdownMenu>
              <Tooltip>
                <TooltipTrigger asChild>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("toolbar.language")}
                    >
                      <Languages className="size-4" />
                    </Button>
                  </DropdownMenuTrigger>
                </TooltipTrigger>
                <TooltipContent>{t("toolbar.language")}</TooltipContent>
              </Tooltip>
              <DropdownMenuContent align="end">
                <DropdownMenuLabel>{t("toolbar.language")}</DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuRadioGroup
                  value={currentLang}
                  onValueChange={handleLanguageChange}
                >
                  {languages.map((lang) => (
                    <DropdownMenuRadioItem key={lang.code} value={lang.code}>
                      <span className="mr-2">{lang.flag}</span>
                      {lang.name}
                    </DropdownMenuRadioItem>
                  ))}
                </DropdownMenuRadioGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          {/* UI 主题切换 */}
          <div className="hidden sm:block">
            <DropdownMenu>
              <Tooltip>
                <TooltipTrigger asChild>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("toolbar.toggleTheme")}
                    >
                      {UI_THEME_ICONS[uiTheme]}
                    </Button>
                  </DropdownMenuTrigger>
                </TooltipTrigger>
                <TooltipContent>{t("toolbar.toggleTheme")}</TooltipContent>
              </Tooltip>
              <DropdownMenuContent align="end">
                <DropdownMenuLabel>{t("toolbar.toggleTheme")}</DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuRadioGroup
                  value={uiTheme}
                  onValueChange={(v) => setUITheme(v as UITheme)}
                >
                  {UI_THEME_OPTIONS.map((option) => (
                    <DropdownMenuRadioItem key={option.value} value={option.value}>
                      {UI_THEME_ICONS[option.value]}
                      {option.label}
                    </DropdownMenuRadioItem>
                  ))}
                </DropdownMenuRadioGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          {/* GitHub 链接 */}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                className="hidden sm:inline-flex"
                asChild
              >
                <a
                  href="https://github.com/Latias94/merman"
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label={t("toolbar.viewSource")}
                >
                  <GitFork className="size-4" />
                </a>
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("toolbar.viewSource")}</TooltipContent>
          </Tooltip>
        </div>
      </header>
    </>
  );
}
