import { useTranslation } from "react-i18next";
import { useAppStore } from "@/src/store";
import { cn } from "@/lib/utils";
import { useMerman } from "@/src/hooks/useMerman";

export function StatusBar() {
  const { t } = useTranslation();
  const {
    code,
    lastRenderTime,
    diagramTheme,
    hostThemePreset,
    diagramType,
    textMeasurementMode,
    diagramFont,
  } = useAppStore();
  const { ready, getBindingCapabilities, getRegistryProfile } = useMerman();
  const capabilities = getBindingCapabilities();
  const registryProfile = getRegistryProfile();

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
          {t("status.wasm")}: {ready ? t("status.ready") : t("status.loading")}
        </span>
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
