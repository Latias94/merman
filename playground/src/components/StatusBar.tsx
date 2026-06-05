import { useTranslation } from "react-i18next";
import { useAppStore } from "@/src/store";
import { cn } from "@/lib/utils";

export function StatusBar() {
  const { t } = useTranslation();
  const { code, lastRenderTime, diagramTheme, diagramType } = useAppStore();

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
        {lastRenderTime > 0 && (
          <span>{t("status.renderTime")}: {lastRenderTime.toFixed(1)}ms</span>
        )}
        <span className="capitalize">{t("status.theme")}: {t(`themes.${diagramTheme}`)}</span>
        <span className="hidden lg:inline">{t("app.title")}</span>
      </div>
    </footer>
  );
}
