import { useTranslation } from "react-i18next";
import { selectVisibleRenderBatch, useRenderCoordinator } from "@/src/runtime/use-render-coordinator";

export function ThemeRenderStatus() {
  const { t } = useTranslation();
  const unavailablePreset = useRenderCoordinator((state) => selectVisibleRenderBatch(state)?.snapshot.operation.unavailableThemePresetId ?? null);
  if (!unavailablePreset) return null;
  return <div className="shrink-0 border-b bg-muted/20 px-3 py-1.5 text-xs" data-testid="theme-render-status">
    <p role="status" className="break-words text-muted-foreground" data-testid="theme-preset-fallback">{t("themePanel.unavailablePresetPreview", { id: unavailablePreset })}</p>
  </div>;
}
