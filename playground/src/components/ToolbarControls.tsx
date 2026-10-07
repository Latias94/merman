import {
  useCallback,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
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
  selectCurrentMermanRenderTime,
  selectCurrentDiagramType,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import { normalizeMermaidThemeSelection } from "@/src/lib/mermaid-theme-name";
import {
  DIAGRAM_FONT_VALUES,
  isDiagramFont,
  type DiagramFont,
} from "@/src/lib/diagram-font";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import {
  isMermanSvgPipeline,
  MERMAN_SVG_PIPELINES,
} from "@/src/runtime/merman-core";
import { themeFamilyTreatment } from "@/src/lib/theme-design";
import { languages, changeLanguage, getCurrentLanguage } from "@/src/i18n";
import { SUPPORTED_THEMES, normalizeThemeName } from "@mermanjs/web";
import {
  ToolbarArtifactActions,
  useToolbarArtifactActions,
} from "@/src/components/ToolbarArtifactActions";
import { ThemeEditorDialog } from "@/src/components/ThemeEditorDialog";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
  DropdownMenuLabel,
  DropdownMenuItem,
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
  FileJson,
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
const NO_THEME_PRESET = "__none__";

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
  const [themeEditorOpen, setThemeEditorOpen] = useState(false);
  const desktopThemeTrigger = useRef<HTMLButtonElement>(null);
  const compactThemeTrigger = useRef<HTMLButtonElement>(null);
  const themeEditorRestoreFocus = useRef<HTMLButtonElement | null>(null);
  const {
    diagramTheme,
    setDiagramTheme,
    setThemePresetId,
    setSvgPipeline,
    svgPipeline,
    textMeasurementMode,
    setTextMeasurementMode,
    diagramFont,
    setDiagramFont,
    uiTheme,
    setUITheme,
    themePresetId,
    themeRecipeJson,
  } = useAppStore(
    useShallow((state) => ({
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
      setDiagramFont: state.setDiagramFont,
      setDiagramTheme: state.setDiagramTheme,
      setThemePresetId: state.setThemePresetId,
      setSvgPipeline: state.setSvgPipeline,
      setTextMeasurementMode: state.setTextMeasurementMode,
      setUITheme: state.setUITheme,
      svgPipeline: state.svgPipeline,
      textMeasurementMode: state.textMeasurementMode,
      themePresetId: state.themePresetId,
      themeRecipeJson: state.themeRecipeJson,
      uiTheme: state.uiTheme,
    })),
  );
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const visibleDiagramType = useRenderCoordinator(selectCurrentDiagramType);
  const facade = useMermanRuntime(selectMermanFacade);
  const artifactActions = useToolbarArtifactActions();
  const currentLang = getCurrentLanguage();
  const themeCatalog = useMemo(() => {
    try {
      return facade?.themeCatalog() ?? null;
    } catch {
      return null;
    }
  }, [facade]);

  const familyId = useMemo(() => {
    try {
      return facade?.diagramFamilyCapabilities()
        .find((family) => family.diagram_type === visibleDiagramType)?.family_id;
    } catch {
      return undefined;
    }
  }, [facade, visibleDiagramType]);
  const selectedPreset = themeCatalog?.presets.find((preset) => preset.id === themePresetId);
  const selectedTreatment = themeFamilyTreatment(selectedPreset, familyId);

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

  const themePresetOptions = useMemo(
    () =>
      openIdOptions(
        themeCatalog?.presets.map((preset) => preset.id) ?? [],
        themePresetId,
        (id) => t(`themePresets.${id}`, {
          defaultValue: themeCatalog?.presets.find((preset) => preset.id === id)?.display_name ?? id,
        }),
      ),
    [themeCatalog, themePresetId, t],
  );
  const renderThemeLabel = t("toolbar.theme");
  const renderSettingsLabel = t("toolbar.renderSettings");
  const UI_THEME_OPTIONS: { value: UITheme; label: string }[] = [
    { value: "light", label: t("uiThemes.light") },
    { value: "dark", label: t("uiThemes.dark") },
    { value: "system", label: t("uiThemes.system") },
  ];

  const normalizeThemePresetId = useCallback(
    (value: string): string | null =>
      value === NO_THEME_PRESET ? null : value,
    [],
  );
  const normalizeTextMeasurementValue = useCallback(
    (value: string): TextMeasurementMode =>
      value === "headless" ? "headless" : "browser",
    [],
  );
  const normalizeDiagramFontValue = useCallback(
    (value: string): DiagramFont =>
      isDiagramFont(value) ? value : "trebuchet",
    [],
  );
  const normalizeSvgPipeline = useCallback(
    (value: string): SvgPipeline =>
      isMermanSvgPipeline(value) ? value : "parity",
    [],
  );

  const handleLanguageChange = useCallback((lang: string) => {
    changeLanguage(lang as "en" | "zh");
  }, []);

  const renderThemeMenuContent = (trigger: RefObject<HTMLButtonElement | null>) => (
    <DropdownMenuContent
      align="end"
      className="max-h-[min(80vh,42rem)] max-w-[calc(100vw-2rem)] overflow-y-auto"
      onCloseAutoFocus={(event) => {
        if (themeEditorOpen) event.preventDefault();
      }}
    >
      <DropdownMenuLabel>{t("toolbar.theme")}</DropdownMenuLabel>
      {themePresetId && (
        <div
          className="max-w-72 break-words px-2 py-2 text-xs text-muted-foreground"
          role="status"
          data-testid="theme-design-scope"
        >
          <p className="font-medium text-foreground">
            {themePresetOptions.find((option) => option.value === themePresetId)?.label}
          </p>
          <p>{t("themeDesign.visibleDiagram", {
            family: familyId
              ? t(`diagramTypes.${familyId}`, { defaultValue: familyId })
              : t("themeDesign.unknownFamily"),
          })}</p>
          <p>{t(`themeDesign.${selectedTreatment}`)}</p>
          <p>{t("themeDesign.explanation")}</p>
          {!selectedPreset?.available && <p>{t("themeDesign.unavailable")}</p>}
        </div>
      )}
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.themePreset")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={themeRecipeJson ? "__custom__" : themePresetId ?? NO_THEME_PRESET}
        onValueChange={(value) =>
          setThemePresetId(normalizeThemePresetId(value))
        }
      >
        <DropdownMenuRadioItem value={NO_THEME_PRESET}>
          {t("themePresets.none")}
        </DropdownMenuRadioItem>
        {themePresetOptions.map((option) => {
          const preset = themeCatalog?.presets.find((entry) => entry.id === option.value);
          return (
            <DropdownMenuRadioItem
              key={option.value}
              value={option.value}
              disabled={!preset?.available}
            >
              <span className="flex w-full items-center justify-between gap-4">
                <span>{option.label}</span>
                <span className="text-right text-xs text-muted-foreground">
                  {preset?.available
                    ? t(`themeDesign.${themeFamilyTreatment(preset, familyId)}`)
                    : t("themeDesign.unavailable")}
                </span>
              </span>
            </DropdownMenuRadioItem>
          );
        })}
      </DropdownMenuRadioGroup>

      <DropdownMenuSeparator />
      <DropdownMenuItem
        onSelect={() => {
          themeEditorRestoreFocus.current = trigger.current;
          setThemeEditorOpen(true);
        }}
      >
        <FileJson className="size-4" />
        {t(themeRecipeJson ? "customTheme.edit" : "customTheme.open")}
        {themeRecipeJson && (
          <span className="ml-auto text-xs text-muted-foreground">{t("customTheme.active")}</span>
        )}
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.mermaidTheme")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={diagramTheme}
        onValueChange={(v) => setDiagramTheme(normalizeMermaidThemeSelection(v))}
      >
        <DropdownMenuRadioItem value="auto">
          {t("themes.auto")}
        </DropdownMenuRadioItem>
        {themeOptions.map((option) => (
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
      <DropdownMenuSeparator />
      <DropdownMenuLabel>{t("toolbar.svgOutput")}</DropdownMenuLabel>
      <DropdownMenuRadioGroup
        value={svgPipeline}
        onValueChange={(value) => setSvgPipeline(normalizeSvgPipeline(value))}
      >
        {MERMAN_SVG_PIPELINES.map((pipeline) => (
          <DropdownMenuRadioItem
            key={pipeline}
            value={pipeline}
            aria-description={t(`svgPipelineDescriptions.${pipeline}`)}
          >
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
      {themeEditorOpen && (
        <ThemeEditorDialog
          onClose={() => setThemeEditorOpen(false)}
          restoreFocus={() => themeEditorRestoreFocus.current?.focus({ preventScroll: true })}
        />
      )}
      <div className="absolute right-3 top-1/2 flex -translate-y-1/2 items-center gap-1 xl:hidden">
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="outline"
                  size="icon-sm"
                  aria-label={t("toolbar.theme")}
                  ref={compactThemeTrigger}
                >
                  <Palette className="size-4" />
                </Button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent>{t("toolbar.theme")}</TooltipContent>
          </Tooltip>
          {renderThemeMenuContent(compactThemeTrigger)}
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

      {/* Desktop theme and artifact controls. */}
      <div className="ml-auto hidden min-w-0 items-center gap-2 xl:flex">
        {/* Latest completed Merman render duration. */}
        {lastRenderTime > 0 && (
          <span className="text-xs text-muted-foreground hidden md:inline">
            {lastRenderTime.toFixed(1)}ms
          </span>
        )}

        {/* Mermaid and compiled diagram themes. */}
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  className="w-8 px-0 sm:w-auto sm:px-2.5"
                  aria-label={t("toolbar.theme")}
                  ref={desktopThemeTrigger}
                >
                  <Palette className="size-4" />
                  <span className="hidden sm:inline">{renderThemeLabel}</span>
                  <ChevronDown className="hidden size-3 opacity-50 sm:block" />
                </Button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent>{t("toolbar.theme")}</TooltipContent>
          </Tooltip>
          {renderThemeMenuContent(desktopThemeTrigger)}
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
                  <span className="hidden sm:inline">
                    {renderSettingsLabel}
                  </span>
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
                  <DropdownMenuRadioItem
                    key={option.value}
                    value={option.value}
                  >
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
