import { useTranslation } from "react-i18next";
import { useAppStore, selectThemeDraftChanged } from "@/src/store";
import { selectVisibleRenderBatch, useRenderCoordinator } from "@/src/runtime/use-render-coordinator";

export function ThemeRenderStatus() {
  const { t } = useTranslation();
  const session = useAppStore((state) => state.themeEditSession);
  const dirty = useAppStore(selectThemeDraftChanged);
  const unavailablePreset = useRenderCoordinator((state) => selectVisibleRenderBatch(state)?.snapshot.operation.unavailableThemePresetId ?? null);
  if (!session && !unavailablePreset) return null;
  return <div className="shrink-0 border-b bg-muted/20 px-3 py-1.5 text-xs" data-testid="theme-render-status">
    {unavailablePreset && <p role="status" className="break-words text-muted-foreground" data-testid="theme-preset-fallback">{t("themePanel.unavailablePresetPreview", { id: unavailablePreset })}</p>}
    {session && (!unavailablePreset || session.importing || session.validation !== "valid" || dirty) && <p role="status" className="text-muted-foreground">
      {session.importing ? t("customTheme.importing") : session.validation === "pending" ? t("themePanel.validating")
        : session.validation === "invalid" ? t("themePanel.invalidPreview") : dirty ? t("themePanel.draftPreview") : t("themePanel.appliedPreview")}
      {dirty && <> {t("themePanel.applyFirst")}</>}
    </p>}
  </div>;
}
