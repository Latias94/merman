import { useTranslation } from "react-i18next";
import { useAppStore, selectThemeDraftChanged } from "@/src/store";

export function ThemeRenderStatus() {
  const { t } = useTranslation();
  const session = useAppStore((state) => state.themeEditSession);
  const dirty = useAppStore(selectThemeDraftChanged);
  if (!session) return null;
  return <div className="shrink-0 border-b bg-muted/20 px-3 py-1.5 text-xs" data-testid="theme-render-status">
    <p role="status" className="text-muted-foreground">
      {session.validation === "invalid" ? t("themePanel.invalidPreview") : dirty ? t("themePanel.draftPreview") : t("themePanel.appliedPreview")}
      {dirty && <> {t("themePanel.applyFirst")}</>}
    </p>
  </div>;
}
