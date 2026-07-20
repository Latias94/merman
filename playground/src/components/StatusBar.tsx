import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { useAppStore } from "@/src/store";
import {
  selectCurrentDetectionValidity,
  selectCurrentDiagramType,
  selectCurrentMermanRenderFailure,
  selectCurrentMermanRenderTime,
  selectCurrentMermaidRenderFailure,
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
  const detectionValidity = useRenderCoordinator(selectCurrentDetectionValidity);
  const lastRenderTime = useRenderCoordinator(selectCurrentMermanRenderTime);
  const mermanRenderFailure = useRenderCoordinator(
    selectCurrentMermanRenderFailure
  );
  const mermaidRenderFailure = useRenderCoordinator(
    selectCurrentMermaidRenderFailure
  );
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

  const getDiagramTypeLabel = () => {
    const typeKey = `diagramTypes.${diagramType}`;
    return t(typeKey, { defaultValue: diagramType });
  };

  return (
    <footer className="flex h-8 shrink-0 items-center justify-between gap-3 overflow-hidden border-t bg-card px-3 text-xs text-muted-foreground sm:px-4">
      <div className="flex min-w-0 shrink items-center gap-3 sm:gap-4">
        <span className="flex min-w-0 items-center gap-1.5">
          <span
            aria-hidden="true"
            className={cn(
              "size-2 rounded-full",
              detectionValidity === "valid"
                ? "bg-green-500"
                : detectionValidity === "recoverable-invalid"
                  ? "bg-yellow-500"
                  : "bg-muted-foreground"
            )}
          />
          <span className="truncate">
            {getDiagramTypeLabel()}
            {detectionValidity === "recoverable-invalid" &&
              ` · ${t("status.syntaxIssues")}`}
          </span>
        </span>
        <span className="shrink-0">
          {lineCount} {t("status.lines")}
        </span>
        <span className="hidden shrink-0 sm:inline">
          {charCount} {t("status.chars")}
        </span>
      </div>
      <div className="scrollbar-thin flex min-w-0 items-center gap-3 overflow-x-auto sm:gap-4">
        <span className="shrink-0 whitespace-nowrap" aria-live="polite">
          {t("status.wasm")}: {runtimeLabel}
        </span>
        {runtimeFailure && (
          <span
            className="hidden max-w-52 truncate text-destructive sm:inline"
            title={runtimeFailure.message}
          >
            {runtimeFailure.stage}: {runtimeFailure.message}
          </span>
        )}
        {[mermanRenderFailure, mermaidRenderFailure].map((failure) => {
          if (!failure) return null;
          const engine = failure.engine === "merman" ? "Merman" : "Mermaid JS";
          const label = `${engine} · ${failure.stage}: ${failure.message}`;
          return (
            <span
              key={failure.engine}
              className="hidden max-w-64 truncate text-destructive md:inline"
              data-merman-status-error-engine={failure.engine}
              title={label}
            >
              {label}
            </span>
          );
        })}
        {capabilities && (
          <span className="hidden shrink-0 md:inline">
            {t("status.editorLanguage")}:{" "}
            {capabilities.editor_language
              ? t("status.enabled")
              : t("status.disabled")}
          </span>
        )}
        {registryProfile && (
          <span className="hidden xl:inline">
            {t("status.registryProfile")}: {registryProfile}
          </span>
        )}
        {lastRenderTime > 0 && (
          <span className="hidden shrink-0 whitespace-nowrap sm:inline">
            {t("status.renderTime")}: {lastRenderTime.toFixed(1)}ms
          </span>
        )}
        <span className="hidden shrink-0 lg:inline">
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
      </div>
    </footer>
  );
}
