import { useTranslation } from "react-i18next";
import { useHistory } from "@/src/hooks/useHistory";
import { useAppStore } from "@/src/store";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { X, Trash2, Clock, FileCode } from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { zhCN, enUS } from "date-fns/locale";
import { getCurrentLanguage } from "@/src/i18n";
import { normalizeThemeName } from "@merman/web";

export function HistoryPanel() {
  const { t } = useTranslation();
  const { showHistory, toggleHistory, setCode, setDiagramTheme } = useAppStore();
  const { history, removeFromHistory, clearHistory } = useHistory();

  if (!showHistory) return null;

  const handleSelectHistory = (item: (typeof history)[0]) => {
    setCode(item.code);
    setDiagramTheme(normalizeThemeName(item.theme));
    toggleHistory();
  };

  const formatTime = (timestamp: number) => {
    const locale = getCurrentLanguage() === "zh" ? zhCN : enUS;
    return formatDistanceToNow(new Date(timestamp), {
      addSuffix: true,
      locale,
    });
  };

  return (
    <div className="absolute inset-0 z-20 bg-background/95 backdrop-blur-sm flex flex-col">
      {/* 头部 */}
      <div className="flex items-center justify-between p-4 border-b">
        <div>
          <h2 className="text-lg font-semibold">{t("history.title")}</h2>
          <p className="text-sm text-muted-foreground">
            {t("history.description")}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {history.length > 0 && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={clearHistory}
                  className="text-destructive hover:text-destructive"
                >
                  <Trash2 className="size-4" />
                  <span className="hidden sm:inline">{t("history.clear")}</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("history.confirmClear")}</TooltipContent>
            </Tooltip>
          )}
          <Button variant="ghost" size="icon" onClick={toggleHistory}>
            <X className="size-5" />
          </Button>
        </div>
      </div>

      {/* 历史列表 */}
      <ScrollArea className="flex-1">
        {history.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-muted-foreground">
            <Clock className="size-12 mb-4 opacity-50" />
            <p className="text-sm">{t("history.empty")}</p>
            <p className="text-xs mt-1">{t("history.emptyDesc")}</p>
          </div>
        ) : (
          <div className="p-4 space-y-3">
            {history.map((item) => (
              <div
                key={item.id}
                className={cn(
                  "group relative p-4 border rounded-lg bg-card",
                  "hover:border-primary/50 hover:shadow-sm transition-all"
                )}
              >
                <button
                  onClick={() => handleSelectHistory(item)}
                  className="w-full text-left"
                >
                  <div className="flex items-start justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <FileCode className="size-4 text-muted-foreground" />
                      <span className="text-sm font-medium">
                        {item.name || t("history.title")}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      <span className="capitalize bg-muted px-2 py-0.5 rounded">
                        {item.theme}
                      </span>
                      <span>{formatTime(item.timestamp)}</span>
                    </div>
                  </div>
                  <pre className="text-xs text-muted-foreground bg-muted/50 p-2 rounded overflow-hidden max-h-20 font-mono">
                    {item.code.slice(0, 150)}
                    {item.code.length > 150 && "..."}
                  </pre>
                </button>

                {/* 删除按钮 */}
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        removeFromHistory(item.id);
                      }}
                      className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-destructive"
                    >
                      <X className="size-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("history.delete")}</TooltipContent>
                </Tooltip>
              </div>
            ))}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}
