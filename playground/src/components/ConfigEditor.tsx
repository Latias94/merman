import Editor from "@monaco-editor/react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  CheckCircle2,
  RotateCcw,
  WandSparkles,
} from "lucide-react";
import { useAppStore } from "@/src/store";
import {
  DEFAULT_MERMAID_CONFIG,
  formatMermaidConfigJson,
  parseMermaidConfigJson,
} from "@/src/lib/mermaid-config";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface ConfigEditorProps {
  className?: string;
}

export function ConfigEditor({ className }: ConfigEditorProps) {
  const { t } = useTranslation();
  const { mermaidConfig, setMermaidConfig, uiTheme } = useAppStore();
  const validation = useMemo(
    () => validateConfig(mermaidConfig, t),
    [mermaidConfig, t]
  );

  const editorTheme =
    uiTheme === "dark" ||
    (uiTheme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches)
      ? "vs-dark"
      : "light";

  return (
    <div className={cn("flex min-h-0 flex-col", className)}>
      <div className="flex h-10 shrink-0 items-center justify-between border-b bg-background px-3">
        <div
          className={cn(
            "flex min-w-0 items-center gap-2 text-xs",
            validation.valid ? "text-muted-foreground" : "text-destructive"
          )}
        >
          {validation.valid ? (
            <CheckCircle2 className="size-4 shrink-0" />
          ) : (
            <AlertCircle className="size-4 shrink-0" />
          )}
          <span className="truncate">
            {validation.valid ? t("config.valid") : validation.error}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                disabled={!validation.valid}
                onClick={() =>
                  setMermaidConfig(formatMermaidConfigJson(mermaidConfig))
                }
              >
                <WandSparkles className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("config.format")}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => setMermaidConfig(DEFAULT_MERMAID_CONFIG)}
              >
                <RotateCcw className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("config.reset")}</TooltipContent>
          </Tooltip>
        </div>
      </div>
      <div className="min-h-0 flex-1">
        <Editor
          height="100%"
          language="json"
          theme={editorTheme}
          value={mermaidConfig}
          onChange={(value) => setMermaidConfig(value || "")}
          loading={
            <div className="flex h-full items-center justify-center text-muted-foreground">
              {t("editor.loading")}
            </div>
          }
          options={{
            automaticLayout: true,
            minimap: { enabled: false },
            lineNumbers: "on",
            fontSize: 14,
            fontFamily: '"JetBrains Mono", "Fira Code", monospace',
            fontLigatures: true,
            wordWrap: "on",
            scrollBeyondLastLine: false,
            padding: { top: 16, bottom: 16 },
            renderLineHighlight: "line",
            cursorBlinking: "smooth",
            smoothScrolling: true,
            tabSize: 2,
          }}
        />
      </div>
    </div>
  );
}

function validateConfig(
  configJson: string,
  t: (key: string) => string
): { valid: true } | { valid: false; error: string } {
  try {
    parseMermaidConfigJson(configJson);
    return { valid: true };
  } catch (error) {
    const isSyntaxError = error instanceof SyntaxError;
    const detail = error instanceof Error ? error.message : String(error);
    return {
      valid: false,
      error: isSyntaxError
        ? `${t("config.invalidJson")}: ${detail}`
        : t("config.invalidObject"),
    };
  }
}
