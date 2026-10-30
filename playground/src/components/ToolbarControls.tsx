import { useCallback, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { GitFork, Languages, Monitor, Moon, Palette, Sun, Type } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuRadioGroup,
  DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { selectRenderInputSnapshot, useAppStore, type UITheme } from "@/src/store";
import { DIAGRAM_FONT_VALUES, isDiagramFont } from "@/src/lib/diagram-font";
import { isMermanSvgPipeline, MERMAN_SVG_PIPELINES } from "@/src/runtime/merman-core";
import { selectCurrentMermanRenderTime, useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
import { languages, changeLanguage, getCurrentLanguage } from "@/src/i18n";
import { ToolbarArtifactActions, useToolbarArtifactActions } from "./ToolbarArtifactActions";

const UI_THEME_ICONS: Record<UITheme, ReactNode> = {
  light: <Sun className="size-4" />,
  dark: <Moon className="size-4" />,
  system: <Monitor className="size-4" />,
};

export function ToolbarControls() {
  const { t } = useTranslation();
  const state = useAppStore(useShallow((current) => ({
    diagramFont: selectRenderInputSnapshot(current).diagramFont,
    textMeasurementMode: current.textMeasurementMode, setTextMeasurementMode: current.setTextMeasurementMode,
    svgPipeline: current.svgPipeline, setSvgPipeline: current.setSvgPipeline,
    uiTheme: current.uiTheme, setUITheme: current.setUITheme,
    themeEditSession: current.themeEditSession,
  })));
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const artifactActions = useToolbarArtifactActions();
  const currentLang = getCurrentLanguage();
  const handleLanguageChange = useCallback((language: string) => changeLanguage(language as "en" | "zh"), []);

  function openThemePanel() {
    const store = useAppStore.getState();
    if (store.themeEditSession) {
      document.querySelector<HTMLElement>("#theme-panel h2")?.focus();
      return;
    }
    store.beginThemeEdit();
    store.setWorkspacePane("preview");
  }

  function themeLauncher(compact: boolean) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="outline" size={compact ? "icon-sm" : "sm"}
            aria-label={t("toolbar.theme")} aria-expanded={state.themeEditSession !== null}
            aria-controls="theme-panel" data-theme-launcher onClick={openThemePanel}>
            <Palette className="size-4" />
            {!compact && <span>{t("toolbar.theme")}</span>}
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.theme")}</TooltipContent>
      </Tooltip>
    );
  }

  function renderSettings(compact: boolean) {
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size={compact ? "icon-sm" : "sm"} aria-label={t("toolbar.renderSettings")}>
            <Type className="size-4" />{!compact && <span>{t("toolbar.renderSettings")}</span>}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="max-h-[80dvh] overflow-y-auto">
          <DropdownMenuLabel>{t("toolbar.font")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={state.diagramFont} onValueChange={(value) => {
            if (!isDiagramFont(value)) return;
            const store = useAppStore.getState();
            if (store.themeEditSession) store.previewThemeSettings({ diagramFont: value });
            else store.setDiagramFont(value);
          }}>
            {DIAGRAM_FONT_VALUES.map((font) => <DropdownMenuRadioItem key={font} value={font}>{t(`diagramFonts.${font}`)}</DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t("toolbar.textMeasurement")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={state.textMeasurementMode} onValueChange={(value) => state.setTextMeasurementMode(value === "headless" ? "headless" : "browser")}>
            {["browser", "headless"].map((mode) => <DropdownMenuRadioItem key={mode} value={mode}>{t(`textMeasurement.${mode}`)}</DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t("toolbar.svgOutput")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={state.svgPipeline} onValueChange={(value) => state.setSvgPipeline(isMermanSvgPipeline(value) ? value : "parity")}>
            {MERMAN_SVG_PIPELINES.map((pipeline) => <DropdownMenuRadioItem key={pipeline} value={pipeline} aria-description={t(`svgPipelineDescriptions.${pipeline}`)}>{t(`svgPipelines.${pipeline}`)}</DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t("toolbar.toggleTheme")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={state.uiTheme} onValueChange={(value) => state.setUITheme(value as UITheme)}>
            {(["light", "dark", "system"] as const).map((theme) => <DropdownMenuRadioItem key={theme} value={theme}>{UI_THEME_ICONS[theme]}{t(`uiThemes.${theme}`)}</DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t("toolbar.language")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={currentLang} onValueChange={handleLanguageChange}>
            {languages.map((language) => <DropdownMenuRadioItem key={language.code} value={language.code}>{language.flag} {language.name}</DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }

  return <>
    <div className="absolute right-3 top-1/2 flex -translate-y-1/2 items-center gap-1 xl:hidden">
      {themeLauncher(true)}{renderSettings(true)}
      <ToolbarArtifactActions compact owner={artifactActions} />
      <Button variant="ghost" size="icon-sm" className="hidden sm:inline-flex" asChild><a href="https://github.com/Latias94/merman" target="_blank" rel="noopener noreferrer" aria-label={t("toolbar.viewSource")}><GitFork className="size-4" /></a></Button>
    </div>
    <div className="ml-auto hidden min-w-0 items-center gap-2 xl:flex">
      {lastRenderTime > 0 && <span className="text-xs text-muted-foreground">{lastRenderTime.toFixed(1)}ms</span>}
      {themeLauncher(false)}{renderSettings(false)}
      <ToolbarArtifactActions compact={false} owner={artifactActions} />
      <DropdownMenu>
        <DropdownMenuTrigger asChild><Button variant="ghost" size="icon-sm" aria-label={t("toolbar.language")}><Languages className="size-4" /></Button></DropdownMenuTrigger>
        <DropdownMenuContent align="end"><DropdownMenuRadioGroup value={currentLang} onValueChange={handleLanguageChange}>
          {languages.map((language) => <DropdownMenuRadioItem key={language.code} value={language.code}>{language.flag} {language.name}</DropdownMenuRadioItem>)}
        </DropdownMenuRadioGroup></DropdownMenuContent>
      </DropdownMenu>
      <DropdownMenu>
        <DropdownMenuTrigger asChild><Button variant="ghost" size="icon-sm" aria-label={t("toolbar.toggleTheme")}>{UI_THEME_ICONS[state.uiTheme]}</Button></DropdownMenuTrigger>
        <DropdownMenuContent align="end"><DropdownMenuRadioGroup value={state.uiTheme} onValueChange={(value) => state.setUITheme(value as UITheme)}>
          {(["light", "dark", "system"] as const).map((theme) => <DropdownMenuRadioItem key={theme} value={theme}>{UI_THEME_ICONS[theme]}{t(`uiThemes.${theme}`)}</DropdownMenuRadioItem>)}
        </DropdownMenuRadioGroup></DropdownMenuContent>
      </DropdownMenu>
      <Button variant="ghost" size="icon-sm" asChild><a href="https://github.com/Latias94/merman" target="_blank" rel="noopener noreferrer" aria-label={t("toolbar.viewSource")}><GitFork className="size-4" /></a></Button>
    </div>
  </>;
}
