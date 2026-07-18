import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { useAppStore } from "@/src/store";
import {
  selectCurrentDiagramType,
  selectCurrentMermanRenderTime,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import { cn } from "@/lib/utils";
import {
  selectMermanFacade,
  selectMermanFailure,
  selectMermanStatus,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";

export function StatusBar() {
  const { t } = useTranslation();
  const {
    code,
    diagramTheme,
    hostThemePreset,
    textMeasurementMode,
    diagramFont,
  } = useAppStore(
    useShallow((state) => ({
      code: state.code,
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
      hostThemePreset: state.hostThemePreset,
      textMeasurementMode: state.textMeasurementMode,
    }))
  );
  const diagramType = useRenderCoordinator(selectCurrentDiagramType);
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const runtimeStatus = useMermanRuntime(selectMermanStatus);
  const facade = useMermanRuntime(selectMermanFacade);
  const runtimeFailure = useMermanRuntime(selectMermanFailure);
  const runtimeMetadata = useMemo(
    () => ({
      capabilities: facade?.bindingCapabilities() ?? null,
      registryProfile: facade?.registryProfile() ?? null,
    }),
    [facade]
  );
  const { capabilities, registryProfile } = runtimeMetadata;
  const runtimeLabel = facade
    ? `${t("status.ready")} ${facade.packageVersion}`
    : t(runtimeStatus === "error" ? "status.error" : "status.loading");

  const lineCount = code.split("\n").length;
  const charCount = code.length;

  // 获取图表类型的翻译
  const getDiagramTypeLabel = () => {
    const typeKey = `diagramTypes.${diagramType}`;
    return t(typeKey, { defaultValue: diagramType });
  };

  return (
    <footer className="h-7 overflow-hidden border-t bg-card px-3 sm:px-4 flex items-center justify-between text-xs text-muted-foreground">
      <div className="flex min-w-0 items-center gap-3 sm:gap-4">
        <span className="flex items-center gap-1.5">
          <span
            className={cn(
              "size-2 rounded-full",
              diagramType !== "unknown" ? "bg-green-500" : "bg-yellow-500"
            )}
          />
          {getDiagramTypeLabel()}
        </span>
        <span>{lineCount} {t("status.lines")}</span>
        <span className="hidden sm:inline">{charCount} {t("status.chars")}</span>
      </div>
      <div className="hidden items-center gap-4 sm:flex">
        <span>
          {t("status.wasm")}: {runtimeLabel}
        </span>
        {runtimeFailure && (
          <span
            className="max-w-52 truncate text-destructive"
            title={runtimeFailure.message}
          >
            {runtimeFailure.stage}: {runtimeFailure.message}
          </span>
        )}
        {capabilities && (
          <span>
            {t("status.editorLanguage")}:{" "}
            {capabilities.editor_language ? t("status.enabled") : t("status.disabled")}
          </span>
        )}
        {registryProfile && (
          <span className="hidden xl:inline">
            {t("status.registryProfile")}: {registryProfile}
          </span>
        )}
        {lastRenderTime > 0 && (
          <span>{t("status.renderTime")}: {lastRenderTime.toFixed(1)}ms</span>
        )}
        <span>
          {t("status.theme")}:{" "}
          {hostThemePreset === "none"
            ? t(`themes.${diagramTheme}`)
            : t(`hostThemes.${hostThemePreset}`)}
        </span>
        <span className="hidden xl:inline">
          {t("status.measurement")}:{" "}
          {t(`textMeasurement.${textMeasurementMode}`)}
        </span>
        <span className="hidden xl:inline">
          {t("status.font")}: {t(`diagramFonts.${diagramFont}`)}
        </span>
        <span className="hidden lg:inline">{t("app.title")}</span>
      </div>
    </footer>
  );
}
