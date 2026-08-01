import { useCallback, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  useAppStore,
  type HostThemePreset,
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
import { useShare } from "@/src/hooks/useShare";
import {
  exportSVG,
  exportPNG,
  exportASCII,
  copySVGToClipboard,
  copyCodeToClipboard,
} from "@/src/lib/export";
import { pngExportErrorMessage } from "@/src/components/png-export-feedback";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import { BenchDialog } from "@/src/components/BenchDialog";
import {
  asciiSupportDescription,
  asciiSupportLabelKey,
} from "@/src/lib/ascii-support";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { languages, changeLanguage, getCurrentLanguage } from "@/src/i18n";
import {
  createMarkdownImageLink,
  createMermaidLiveEditorUrl,
} from "@/src/lib/mermaid-live";
import {
  SUPPORTED_HOST_THEME_PRESETS,
  SUPPORTED_THEMES,
  normalizeHostThemePresetName,
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
  Github,
  Languages,
  FileText,
  Code,
  ExternalLink,
  Type,
} from "lucide-react";

const UI_THEME_ICONS: Record<UITheme, ReactNode> = {
  light: <Sun className="size-4" />,
  dark: <Moon className="size-4" />,
  system: <Monitor className="size-4" />,
};

const TEXT_MEASUREMENT_VALUES: readonly TextMeasurementMode[] = [
  "browser",
  "headless",
];

export function Toolbar() {
  const { t } = useTranslation();
  const {
    code,
    diagramTheme,
    hostThemePreset,
    mermaidConfig,
    setDiagramTheme,
    setHostThemePreset,
    textMeasurementMode,
    setTextMeasurementMode,
    diagramFont,
    setDiagramFont,
    uiTheme,
    setUITheme,
    toggleExamples,
    showExamples,
  } = useAppStore(
    useShallow((state) => ({
      code: state.code,
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
      hostThemePreset: state.hostThemePreset,
      mermaidConfig: state.mermaidConfig,
      setDiagramFont: state.setDiagramFont,
      setDiagramTheme: state.setDiagramTheme,
      setHostThemePreset: state.setHostThemePreset,
      setTextMeasurementMode: state.setTextMeasurementMode,
      setUITheme: state.setUITheme,
      showExamples: state.showExamples,
      textMeasurementMode: state.textMeasurementMode,
      toggleExamples: state.toggleExamples,
      uiTheme: state.uiTheme,
    }))
  );
  const diagramType = useRenderCoordinator(selectCurrentDiagramType);
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const currentBatch = useRenderCoordinator(selectCompletedRenderBatch);
  const { copyShareUrl } = useShare();
  const facade = useMermanRuntime(selectMermanFacade);
  const asciiSupport = useAsciiSupport();
  const [isExporting, setIsExporting] = useState(false);
  const currentLang = getCurrentLanguage();

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

  const hostThemeOptions: { value: HostThemePreset; label: string }[] = useMemo(
    () => [
      { value: "none", label: t("hostThemes.none") },
      ...SUPPORTED_HOST_THEME_PRESETS.map((preset) => ({
        value: preset,
        label: t(`hostThemes.${preset}`, { defaultValue: preset }),
      })),
    ],
    [t]
  );

  const renderThemeLabel =
    hostThemePreset === "none"
      ? t(`themes.${diagramTheme}`, { defaultValue: diagramTheme })
      : t(`hostThemes.${hostThemePreset}`, { defaultValue: hostThemePreset });
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
  const renderCurrentSvgArtifact = useCallback(
    (pipeline?: "resvg-safe") => {
      if (!currentBatch || !currentMerman) {
        throw new Error("Current Merman render is unavailable.");
      }
      if (!pipeline) return currentMerman.artifact;
      if (!facade) throw new Error("Merman runtime is not ready.");
      const snapshot = currentBatch.snapshot;
      const result = facade.render(
        snapshot.source,
        snapshot.theme,
        snapshot.configJson,
        { ...snapshot.options, pipeline }
      );
      if (result.status === "failure") {
        throw new Error(result.error.summary);
      }
      return result.artifact;
    },
    [currentBatch, currentMerman, facade]
  );

  // Export actions consume only the completed, current render batch.
  const handleExportSVG = useCallback(() => {
    try {
      exportSVG(renderCurrentSvgArtifact(), "merman-diagram");
      toast.success(t("export.svgSuccess"));
    } catch {
      toast.error(t("export.failed"));
    }
  }, [renderCurrentSvgArtifact, t]);

  const handleExportPNG = useCallback(async () => {
    setIsExporting(true);
    let notificationId: string | number | undefined;
    try {
      const plan = await exportPNG(
        renderCurrentSvgArtifact("resvg-safe"),
        "merman-diagram",
        2,
        {
          onPlan: ({ outputWidth, outputHeight }) => {
            notificationId = toast.loading(
              t("export.pngPreparing", {
                width: outputWidth,
                height: outputHeight,
              })
            );
          },
        }
      );
      toast.success(
        t("export.pngSuccess", {
          width: plan.outputWidth,
          height: plan.outputHeight,
        }),
        { id: notificationId }
      );
    } catch (error) {
      toast.error(pngExportErrorMessage(error, t), { id: notificationId });
    } finally {
      setIsExporting(false);
    }
  }, [renderCurrentSvgArtifact, t]);

  const handleExportASCII = useCallback(() => {
    const ascii = currentMerman?.ascii;
    if (!ascii) {
      toast.error(t("export.asciiNotSupported"));
      return;
    }
    exportASCII(ascii, "merman-diagram");
    toast.success(t("export.asciiSuccess"));
  }, [currentMerman, t]);

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
      await copySVGToClipboard(renderCurrentSvgArtifact());
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [renderCurrentSvgArtifact, t]);

  const handleShare = useCallback(async () => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyShareUrl(
        code,
        diagramTheme,
        mermaidConfig,
        hostThemePreset,
        textMeasurementMode,
        diagramFont
      );
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [
    code,
    copyShareUrl,
    diagramFont,
    diagramTheme,
    hostThemePreset,
    mermaidConfig,
    t,
    textMeasurementMode,
  ]);

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

  const normalizeHostThemeValue = useCallback((value: string): HostThemePreset => {
    if (value === "none") return "none";
    return normalizeHostThemePresetName(value) ?? "none";
  }, []);
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

  const handleLanguageChange = useCallback((lang: string) => {
    changeLanguage(lang as "en" | "zh");
  }, []);

  const renderThemeMenuContent = () => (
    <DropdownMenuContent align="end">
      <DropdownMenuLabel>{t("toolbar.theme")}</DropdownMenuLabel>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.mermaidTheme")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={hostThemePreset === "none" ? diagramTheme : ""}
        onValueChange={(v) => setDiagramTheme(normalizeThemeName(v))}
      >
        {themeOptions.map((option) => (
          <DropdownMenuRadioItem key={option.value} value={option.value}>
            {option.label}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.hostTheme")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={hostThemePreset}
        onValueChange={(v) => setHostThemePreset(normalizeHostThemeValue(v))}
      >
        {hostThemeOptions.map((option) => (
          <DropdownMenuRadioItem key={option.value} value={option.value}>
            {option.label}
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
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

          {/* 示例按钮 */}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                id="examples-trigger"
                variant={showExamples ? "secondary" : "ghost"}
                size="sm"
                onClick={toggleExamples}
                aria-label={t("toolbar.examples")}
              >
                <BookOpen className="size-4" />
                <span className="hidden sm:inline">{t("toolbar.examples")}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("toolbar.examples")}</TooltipContent>
          </Tooltip>

          <BenchDialog />

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
                  <Github className="size-4" />
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
