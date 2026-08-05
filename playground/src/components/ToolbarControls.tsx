import {
  useCallback,
  useMemo,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  useAppStore,
  type SvgPipeline,
  type TextMeasurementMode,
  type Theme,
  type UITheme,
} from "@/src/store";
import {
  selectCompletedRenderBatch,
  selectCurrentMermanRenderTime,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import {
  DIAGRAM_FONT_VALUES,
  isDiagramFont,
  type DiagramFont,
} from "@/src/lib/diagram-font";
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
  SUPPORTED_THEMES,
  normalizeThemeName,
} from "@mermanjs/web";
import {
  ToolbarArtifactActions,
  useToolbarArtifactActions,
} from "@/src/components/ToolbarArtifactActions";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
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
import {
  Palette,
  Sun,
  Moon,
  Monitor,
  ChevronDown,
  GitFork,
  Languages,
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

export function ToolbarControls() {
  const { t } = useTranslation();
  const {
    diagramTheme,
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
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
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
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const currentBatch = useRenderCoordinator(selectCompletedRenderBatch);
  const facade = useMermanRuntime(selectMermanFacade);
  const artifactActions = useToolbarArtifactActions();
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
  const UI_THEME_OPTIONS: { value: UITheme; label: string }[] = [
    { value: "light", label: t("uiThemes.light") },
    { value: "dark", label: t("uiThemes.dark") },
    { value: "system", label: t("uiThemes.system") },
  ];

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
      <div className="xl:hidden">
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

  const renderRepositoryLink = () => (
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
  );

  return (
    <>
      <span
        id={PRESENTATION_STATUS_ID}
        className="sr-only"
        aria-live="polite"
      >
        {presentationStatusText}
      </span>

        <div className="absolute right-3 top-1/2 flex -translate-y-1/2 items-center gap-1 xl:hidden">
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

          <ToolbarArtifactActions compact owner={artifactActions} />
          {renderRepositoryLink()}
        </div>

        {/* Desktop presentation and artifact controls. */}
        <div className="ml-auto hidden min-w-0 items-center gap-2 xl:flex">
          {/* Latest completed Merman render duration. */}
          {lastRenderTime > 0 && (
            <span className="text-xs text-muted-foreground hidden md:inline">
              {lastRenderTime.toFixed(1)}ms
            </span>
          )}

          {/* Diagram and presentation themes. */}
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

          {/* Render settings. */}
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

          <ToolbarArtifactActions compact={false} owner={artifactActions} />

          <div className="hidden h-6 w-px shrink-0 bg-border sm:block" />

          {/* Language selection. */}
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

          {/* Application theme selection. */}
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

          {/* Repository link. */}
          {renderRepositoryLink()}
        </div>
    </>
  );
}
