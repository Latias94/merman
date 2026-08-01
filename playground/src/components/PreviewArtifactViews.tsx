import Editor from "@monaco-editor/react";
import { Maximize2, RotateCcw, ZoomIn, ZoomOut } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { SvgViewportController } from "@/src/components/SvgViewport";

export function ViewportControls({
  controller,
  t,
}: {
  controller: SvgViewportController;
  t: (key: string) => string;
}) {
  return (
    <div className="flex items-center gap-1">
      <ArtifactIconButton label={t("preview.zoomOut")} onClick={controller.zoomOut}>
        <ZoomOut className="size-4" />
      </ArtifactIconButton>
      <span className="w-16 text-center text-xs tabular-nums text-muted-foreground">
        {formatZoomPercent(controller.zoom)}
      </span>
      <ArtifactIconButton label={t("preview.zoomIn")} onClick={controller.zoomIn}>
        <ZoomIn className="size-4" />
      </ArtifactIconButton>
      <ArtifactIconButton label={t("preview.fitToView")} onClick={controller.fitToView}>
        <Maximize2 className="size-4" />
      </ArtifactIconButton>
      <ArtifactIconButton label={t("preview.reset")} onClick={controller.reset}>
        <RotateCcw className="size-4" />
      </ArtifactIconButton>
    </div>
  );
}

function formatZoomPercent(zoom: number): string {
  const percent = zoom * 100;
  if (percent >= 10) return `${Math.round(percent)}%`;
  if (percent >= 1) return `${Number(percent.toFixed(1))}%`;
  return `${Number(percent.toPrecision(2))}%`;
}

export function SvgSourceEditor({
  svg,
  isDarkMode,
}: {
  svg: string | null;
  isDarkMode: boolean;
}) {
  if (!svg) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        -
      </div>
    );
  }

  return (
    <Editor
      height="100%"
      language="xml"
      value={svg}
      theme={isDarkMode ? "vs-dark" : "light"}
      options={{
        readOnly: true,
        domReadOnly: true,
        minimap: { enabled: false },
        fontSize: 12,
        fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        renderLineHighlight: "none",
        selectionHighlight: false,
        occurrencesHighlight: "off",
        folding: true,
        automaticLayout: true,
        padding: { top: 16, bottom: 16 },
      }}
    />
  );
}

function ArtifactIconButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick(): void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onClick}
          aria-label={label}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
