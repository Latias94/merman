import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { GitFork, Monitor, Moon, Palette, Settings, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { languages, changeLanguage, getCurrentLanguage } from "@/src/i18n";
import { useAppStore } from "@/src/store";
import { ProjectUseLauncher } from "./ProjectUseLauncher";
import { ToolbarArtifactActions, useToolbarArtifactActions } from "./ToolbarArtifactActions";
import { ToolsMenu } from "./ToolsMenu";

const UI_THEME_ICONS = {
  light: Sun,
  dark: Moon,
  system: Monitor,
};

export function ToolbarControls() {
  const { t } = useTranslation();
  const themePanelOpen = useAppStore((state) => state.themePanelOpen);
  const artifactActions = useToolbarArtifactActions();

  function openAppearance() {
    const store = useAppStore.getState();
    store.setWorkspacePane("preview");
    store.openThemePanel();
    requestAnimationFrame(() => {
      document.querySelector<HTMLElement>("#theme-panel h2")?.focus();
    });
  }

  return (
    <div className="ml-auto flex shrink-0 items-center gap-1 xl:gap-2">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            className="w-8 px-0 xl:w-auto xl:px-2.5"
            aria-label={t("workbench.appearance")}
            aria-expanded={themePanelOpen}
            aria-controls="theme-panel"
            data-theme-launcher
            onClick={openAppearance}
          >
            <Palette className="size-4" />
            <span className="hidden xl:inline">{t("workbench.appearance")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("workbench.appearance")}</TooltipContent>
      </Tooltip>

      <ToolbarArtifactActions compact={false} owner={artifactActions}>
        <div className="max-xl:[&>button]:w-8 max-xl:[&>button]:px-0 max-xl:[&>button>span]:hidden">
          <ProjectUseLauncher compact={false} />
        </div>
      </ToolbarArtifactActions>
      <ToolsMenu />
      <AppSettings />
    </div>
  );
}

function AppSettings() {
  const { t } = useTranslation();
  const { uiTheme, setUITheme } = useAppStore(useShallow((state) => ({
    uiTheme: state.uiTheme,
    setUITheme: state.setUITheme,
  })));

  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label={t("workbench.appSettings")}>
              <Settings className="size-4" />
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>{t("workbench.appSettings")}</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end" className="w-56 max-w-[calc(100vw-1rem)]">
        <DropdownMenuLabel>{t("workbench.appTheme")}</DropdownMenuLabel>
        <DropdownMenuRadioGroup value={uiTheme} onValueChange={(value) => {
          if (value === "light" || value === "dark" || value === "system") setUITheme(value);
        }}>
          {(["light", "dark", "system"] as const).map((theme) => {
            const Icon = UI_THEME_ICONS[theme];
            return <DropdownMenuRadioItem key={theme} value={theme}>
              <Icon className="size-4" />{t(`uiThemes.${theme}`)}
            </DropdownMenuRadioItem>;
          })}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>{t("toolbar.language")}</DropdownMenuLabel>
        <DropdownMenuRadioGroup value={getCurrentLanguage()} onValueChange={(value) => {
          if (value === "en" || value === "zh") changeLanguage(value);
        }}>
          {languages.map((language) => <DropdownMenuRadioItem key={language.code} value={language.code}>
            {language.flag} {language.name}
          </DropdownMenuRadioItem>)}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <a href="https://github.com/Latias94/merman" target="_blank" rel="noopener noreferrer">
            <GitFork className="size-4" />{t("toolbar.viewSource")}
          </a>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
