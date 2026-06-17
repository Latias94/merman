import { useCallback, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  useAppStore,
  type HostThemePreset,
  type TextMeasurementMode,
  type Theme,
  type UITheme,
} from "@/src/store";
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
  isAsciiSupported,
} from "@/src/lib/export";
import { useMerman } from "@/src/hooks/useMerman";
import { BenchDialog } from "@/src/components/BenchDialog";
import { languages, changeLanguage, getCurrentLanguage } from "@/src/i18n";
import {
  createMarkdownImageLink,
  createMermaidLiveEditorUrl,
} from "@/src/lib/mermaid-live";
import {
  SUPPORTED_HOST_THEME_PRESETS,
  normalizeHostThemePresetName,
  normalizeThemeName,
  type HostThemePresetName,
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
    lastRenderTime,
    diagramType,
  } = useAppStore();
  const { copyShareUrl } = useShare();
  const { render, renderAscii, getThemes } = useMerman();
  const [isExporting, setIsExporting] = useState(false);
  const currentLang = getCurrentLanguage();

  const themeOptions: { value: Theme; label: string }[] = useMemo(() => {
    const seen = new Set<Theme>();
    return getThemes()
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
  }, [getThemes, t]);

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

  const activeHostThemePreset: HostThemePresetName | undefined =
    hostThemePreset === "none" ? undefined : hostThemePreset;
  const renderOptions = useMemo(
    () => ({
      hostThemePreset: activeHostThemePreset,
      textMeasurementMode,
      diagramFont,
    }),
    [activeHostThemePreset, diagramFont, textMeasurementMode]
  );
  const renderThemeLabel =
    hostThemePreset === "none"
      ? t(`themes.${diagramTheme}`, { defaultValue: diagramTheme })
      : t(`hostThemes.${hostThemePreset}`, { defaultValue: hostThemePreset });
  const renderSettingsLabel = t("toolbar.renderSettings");

  const UI_THEME_OPTIONS: { value: UITheme; label: string }[] = [
    { value: "light", label: t("uiThemes.light") },
    { value: "dark", label: t("uiThemes.dark") },
    { value: "system", label: t("uiThemes.system") },
  ];

  const renderCurrentSvg = useCallback((pipeline?: "resvg-safe") => {
    const result = render(code, diagramTheme, mermaidConfig, {
      ...renderOptions,
      ...(pipeline ? { pipeline } : {}),
    });
    if (!result.svg) {
      throw new Error(result.error ?? "Failed to render SVG");
    }
    return result.svg;
  }, [code, diagramTheme, mermaidConfig, render, renderOptions]);

  // 导出 SVG
  const handleExportSVG = useCallback(() => {
    try {
      exportSVG(renderCurrentSvg(), "merman-diagram");
      toast.success(t("export.svgSuccess"));
    } catch {
      toast.error(t("export.failed"));
    }
  }, [renderCurrentSvg, t]);

  // 导出 PNG
  const handleExportPNG = useCallback(async () => {
    setIsExporting(true);
    try {
      await exportPNG(renderCurrentSvg("resvg-safe"), "merman-diagram", 2);
      toast.success(t("export.pngSuccess"));
    } catch {
      toast.error(t("export.failed"));
    } finally {
      setIsExporting(false);
    }
  }, [renderCurrentSvg, t]);

  // 导出 ASCII
  const handleExportASCII = useCallback(() => {
    if (!isAsciiSupported(diagramType)) {
      toast.error(t("export.asciiNotSupported"));
      return;
    }
    const ascii = renderAscii(code, diagramTheme, mermaidConfig);
    if (!ascii) {
      toast.error(t("export.asciiNotSupported"));
      return;
    }
    exportASCII(ascii, "merman-diagram");
    toast.success(t("export.asciiSuccess"));
  }, [code, diagramType, diagramTheme, mermaidConfig, renderAscii, t]);

  // 复制代码
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

  // 复制 SVG
  const handleCopySVG = useCallback(async () => {
    try {
      await copySVGToClipboard(renderCurrentSvg());
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [renderCurrentSvg, t]);

  // 分享
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

  // 应用 UI 主题到 HTML
  const handleUIThemeChange = useCallback(
    (theme: UITheme) => {
      setUITheme(theme);
      const root = document.documentElement;
      if (theme === "dark") {
        root.classList.add("dark");
      } else if (theme === "light") {
        root.classList.remove("dark");
      } else {
        // system
        if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
          root.classList.add("dark");
        } else {
          root.classList.remove("dark");
        }
      }
    },
    [setUITheme]
  );

  // 切换语言
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
    </DropdownMenuContent>
  );

  const asciiSupported = isAsciiSupported(diagramType);

  return (
    <>
      <Toaster position="bottom-right" richColors />
      <header className="relative flex h-14 items-center gap-2 overflow-hidden border-b bg-card px-3 sm:px-4">
        {/* 左侧：Logo 和功能按钮 */}
        <div className="flex min-w-0 shrink-0 items-center gap-2 sm:gap-4">
          <div className="flex items-center gap-2">
            <div className="size-8 rounded-lg bg-primary flex items-center justify-center">
              <span className="text-primary-foreground font-bold text-sm">M</span>
            </div>
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
                variant={showExamples ? "secondary" : "ghost"}
                size="sm"
                onClick={toggleExamples}
              >
                <BookOpen className="size-4" />
                <span className="hidden sm:inline">{t("toolbar.examples")}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("toolbar.examples")}</TooltipContent>
          </Tooltip>

          <div className="hidden sm:block">
            <BenchDialog />
          </div>
        </div>

        <div className="absolute right-3 top-1/2 flex -translate-y-1/2 items-center gap-1 sm:hidden">
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" size="icon-sm">
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
                  <Button variant="outline" size="icon-sm">
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
                  <Button variant="outline" size="icon-sm" disabled={isExporting}>
                    <Download className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{t("toolbar.export")}</TooltipContent>
            </Tooltip>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>{t("export.title")}</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleExportSVG}>
                <FileCode className="size-4" />
                {t("export.svg")}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleExportPNG}>
                <ImageIcon className="size-4" />
                {t("export.png")}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleExportASCII} disabled={!asciiSupported}>
                <FileText className="size-4" />
                {t("export.ascii")}
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
              <DropdownMenuItem onClick={handleCopySVG}>
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
              <Button variant="outline" size="icon-sm" onClick={handleShare}>
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
              <DropdownMenuItem onClick={handleExportSVG}>
                <FileCode className="size-4" />
                {t("export.svg")}
                <span className="ml-auto text-xs text-muted-foreground">{t("export.svgDesc")}</span>
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleExportPNG}>
                <ImageIcon className="size-4" />
                {t("export.png")}
                <span className="ml-auto text-xs text-muted-foreground">{t("export.pngDesc")}</span>
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleExportASCII} disabled={!asciiSupported}>
                <FileText className="size-4" />
                {t("export.ascii")}
                <span className="ml-auto text-xs text-muted-foreground">
                  {asciiSupported ? t("export.asciiDesc") : t("export.asciiNotSupported")}
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
              <DropdownMenuItem onClick={handleCopySVG}>
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
                    <Button variant="ghost" size="icon-sm">
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
                    <Button variant="ghost" size="icon-sm">
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
                  onValueChange={(v) => handleUIThemeChange(v as UITheme)}
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
