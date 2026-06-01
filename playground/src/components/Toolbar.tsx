import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppStore, type Theme, type UITheme } from "@/src/store";
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
import { normalizeThemeName } from "@merman/web";
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
} from "lucide-react";

const UI_THEME_ICONS: Record<UITheme, React.ReactNode> = {
  light: <Sun className="size-4" />,
  dark: <Moon className="size-4" />,
  system: <Monitor className="size-4" />,
};

export function Toolbar() {
  const { t } = useTranslation();
  const {
    code,
    diagramTheme,
    setDiagramTheme,
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

  const UI_THEME_OPTIONS: { value: UITheme; label: string }[] = [
    { value: "light", label: t("themes.default") },
    { value: "dark", label: t("themes.dark") },
    { value: "system", label: "System" },
  ];

  // 获取当前 SVG
  const currentSvg = useMemo(() => {
    const result = render(code, diagramTheme);
    return result.svg;
  }, [code, diagramTheme, render]);

  // 导出 SVG
  const handleExportSVG = useCallback(() => {
    if (!currentSvg) {
      toast.error(t("export.title") + " failed");
      return;
    }
    exportSVG(currentSvg, "merman-diagram");
    toast.success(t("export.svg") + " - OK");
  }, [currentSvg, t]);

  // 导出 PNG
  const handleExportPNG = useCallback(async () => {
    if (!currentSvg) {
      toast.error(t("export.title") + " failed");
      return;
    }
    setIsExporting(true);
    try {
      await exportPNG(currentSvg, "merman-diagram", 2);
      toast.success(t("export.png") + " - OK");
    } catch {
      toast.error(t("export.title") + " failed");
    } finally {
      setIsExporting(false);
    }
  }, [currentSvg, t]);

  // 导出 ASCII
  const handleExportASCII = useCallback(() => {
    if (!isAsciiSupported(diagramType)) {
      toast.error(t("export.asciiNotSupported"));
      return;
    }
    const ascii = renderAscii(code);
    if (!ascii) {
      toast.error(t("export.asciiNotSupported"));
      return;
    }
    exportASCII(ascii, "merman-diagram");
    toast.success(t("export.ascii") + " - OK");
  }, [code, diagramType, renderAscii, t]);

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

  // 复制 SVG
  const handleCopySVG = useCallback(async () => {
    if (!currentSvg) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copySVGToClipboard(currentSvg);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [currentSvg, t]);

  // 分享
  const handleShare = useCallback(async () => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyShareUrl(code, diagramTheme);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [code, diagramTheme, copyShareUrl, t]);

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

  const asciiSupported = isAsciiSupported(diagramType);

  return (
    <>
      <Toaster position="bottom-right" richColors />
      <header className="flex h-14 items-center justify-between border-b px-4 bg-card">
        {/* 左侧：Logo 和功能按钮 */}
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <div className="size-8 rounded-lg bg-primary flex items-center justify-center">
              <span className="text-primary-foreground font-bold text-sm">M</span>
            </div>
            <div>
              <h1 className="text-sm font-semibold leading-none">Merman</h1>
              <p className="text-xs text-muted-foreground">Playground</p>
            </div>
          </div>

          <div className="h-6 w-px bg-border" />

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

          <BenchDialog />
        </div>

        {/* 右侧：主题、导出、分享 */}
        <div className="flex items-center gap-2">
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
                  <Button variant="outline" size="sm">
                    <Palette className="size-4" />
                    <span className="hidden sm:inline capitalize">{diagramTheme}</span>
                    <ChevronDown className="size-3 opacity-50" />
                  </Button>
                </DropdownMenuTrigger>
              </TooltipTrigger>
              <TooltipContent>{t("toolbar.theme")}</TooltipContent>
            </Tooltip>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>{t("toolbar.theme")}</DropdownMenuLabel>
              <DropdownMenuSeparator />
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
            </DropdownMenuContent>
          </DropdownMenu>

          {/* 导出 */}
          <DropdownMenu>
            <Tooltip>
              <TooltipTrigger asChild>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" size="sm" disabled={isExporting}>
                    <Download className="size-4" />
                    <span className="hidden sm:inline">{t("toolbar.export")}</span>
                    <ChevronDown className="size-3 opacity-50" />
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
              <DropdownMenuItem onClick={handleCopySVG}>
                <Copy className="size-4" />
                Copy SVG
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          {/* 分享 */}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline" size="sm" onClick={handleShare}>
                <Share2 className="size-4" />
                <span className="hidden sm:inline">{t("toolbar.share")}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("share.copyLink")}</TooltipContent>
          </Tooltip>

          <div className="h-6 w-px bg-border" />

          {/* 语言切换 */}
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

          {/* UI 主题切换 */}
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

          {/* GitHub 链接 */}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" asChild>
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
