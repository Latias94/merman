import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  ChevronDown,
  Code,
  Copy,
  Download,
  ExternalLink,
  FileCode,
  FileText,
  ImageIcon,
  Share2,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { pngExportErrorMessage } from "@/src/components/png-export-feedback";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import {
  asciiSupportDescription,
  asciiSupportLabelKey,
} from "@/src/lib/ascii-support";
import { copyCodeToClipboard } from "@/src/lib/export";
import {
  createMarkdownImageLink,
  createMermaidLiveEditorUrl,
} from "@/src/lib/mermaid-live";
import { copyShareUrl } from "@/src/lib/share";
import { executeArtifactAction } from "@/src/runtime/artifact-actions-browser";
import {
  selectCompletedRenderBatch,
  selectCurrentDiagramType,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";

export function useToolbarArtifactActions() {
  const { t } = useTranslation();
  const { code, diagramTheme, mermaidConfig } = useAppStore(
    useShallow((state) => ({
      code: state.code,
      diagramTheme: state.diagramTheme,
      mermaidConfig: state.mermaidConfig,
    })),
  );
  const diagramType = useRenderCoordinator(selectCurrentDiagramType);
  const currentBatch = useRenderCoordinator(selectCompletedRenderBatch);
  const asciiSupport = useAsciiSupport();
  const [isExporting, setIsExporting] = useState(false);
  const asciiCapability = asciiSupport.capabilityFor(diagramType);
  const asciiSupported = asciiSupport.isSupported(diagramType);
  const asciiSupportLabel = t(asciiSupportLabelKey(asciiCapability));
  const asciiSupportLimit = asciiSupportDescription(asciiCapability);
  const asciiExportDescription = asciiSupported
    ? [asciiSupportLabel, asciiSupportLimit].filter(Boolean).join(" · ")
    : t("export.asciiNotSupported");
  const currentMerman =
    currentBatch?.merman.status === "success" ? currentBatch.merman : null;
  const artifactActionsEnabled = currentMerman !== null;
  const asciiAvailable = currentBatch?.ascii.status === "success";

  const handleExportSVG = useCallback(async () => {
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      await executeArtifactAction({
        action: "download-svg",
        engine: "merman",
        publicationId: currentBatch.snapshot.publicationId,
      });
      toast.success(t("export.svgSuccess"));
    } catch {
      toast.error(t("export.failed"));
    }
  }, [currentBatch, t]);

  const handleExportPNG = useCallback(async () => {
    setIsExporting(true);
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      const plan = await executeArtifactAction({
        action: "download-png",
        engine: "merman",
        publicationId: currentBatch.snapshot.publicationId,
        scale: 2,
      });
      toast.success(
        t("export.pngSuccess", {
          width: plan.outputWidth,
          height: plan.outputHeight,
        }),
      );
    } catch (error) {
      toast.error(pngExportErrorMessage(error, t));
    } finally {
      setIsExporting(false);
    }
  }, [currentBatch, t]);

  const handleExportASCII = useCallback(async () => {
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      await executeArtifactAction({
        action: "download-ascii",
        publicationId: currentBatch.snapshot.publicationId,
      });
      toast.success(t("export.asciiSuccess"));
    } catch {
      toast.error(t("export.asciiNotSupported"));
    }
  }, [currentBatch, t]);

  const handleCopyCode = useCallback(async () => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyCodeToClipboard(code);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [code, t]);

  const handleCopyMarkdown = useCallback(async () => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await navigator.clipboard.writeText(
        createMarkdownImageLink(code, diagramTheme, mermaidConfig),
      );
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [code, diagramTheme, mermaidConfig, t]);

  const handleCopySVG = useCallback(async () => {
    try {
      if (!currentBatch) throw new Error("Current render is unavailable.");
      await executeArtifactAction({
        action: "copy-svg",
        engine: "merman",
        publicationId: currentBatch.snapshot.publicationId,
      });
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [currentBatch, t]);

  const handleShare = useCallback(async () => {
    const snapshot = selectWorkspaceSnapshot(useAppStore.getState());
    if (!snapshot.code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyShareUrl(snapshot);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [t]);

  const handleOpenMermaidLive = useCallback(() => {
    if (!code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    window.open(
      createMermaidLiveEditorUrl(code, diagramTheme, mermaidConfig),
      "_blank",
      "noopener,noreferrer",
    );
  }, [code, diagramTheme, mermaidConfig, t]);

  return {
    artifactActionsEnabled,
    asciiExportDescription,
    asciiAvailable,
    handleCopyCode,
    handleCopyMarkdown,
    handleCopySVG,
    handleExportASCII,
    handleExportPNG,
    handleExportSVG,
    handleOpenMermaidLive,
    handleShare,
    isExporting,
  };
}

type ToolbarArtifactActionsOwner = ReturnType<
  typeof useToolbarArtifactActions
>;

export function ToolbarArtifactActions({
  compact,
  owner,
}: {
  compact: boolean;
  owner: ToolbarArtifactActionsOwner;
}) {
  const { t } = useTranslation();
  return (
    <>
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size={compact ? "icon-sm" : "sm"}
                className={compact ? undefined : "w-8 px-0 sm:w-auto sm:px-2.5"}
                disabled={owner.isExporting}
                aria-label={t("toolbar.export")}
              >
                <Download className="size-4" />
                {!compact && (
                  <>
                    <span className="hidden sm:inline">{t("toolbar.export")}</span>
                    <ChevronDown className="hidden size-3 opacity-50 sm:block" />
                  </>
                )}
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>{t("toolbar.export")}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end">
          <DropdownMenuLabel>{t("export.title")}</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onClick={owner.handleExportSVG}
            disabled={!owner.artifactActionsEnabled}
          >
            <FileCode className="size-4" />
            {t("export.svg")}
            {!compact && (
              <span className="ml-auto text-xs text-muted-foreground">
                {t("export.svgDesc")}
              </span>
            )}
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={owner.handleExportPNG}
            disabled={!owner.artifactActionsEnabled}
          >
            <ImageIcon className="size-4" />
            {t("export.png")}
            {!compact && (
              <span className="ml-auto text-xs text-muted-foreground">
                {t("export.pngDesc")}
              </span>
            )}
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={owner.handleExportASCII}
            disabled={!owner.asciiAvailable}
          >
            <FileText className="size-4" />
            {t("export.ascii")}
            <span
              className={
                compact
                  ? "ml-auto max-w-44 truncate text-xs text-muted-foreground"
                  : "ml-auto text-xs text-muted-foreground"
              }
            >
              {owner.asciiExportDescription}
            </span>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={owner.handleCopyCode}>
            <Code className="size-4" />
            {t("export.copyCode")}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={owner.handleCopyMarkdown}>
            <FileText className="size-4" />
            {t("export.copyMarkdown")}
            {!compact && (
              <span className="ml-auto text-xs text-muted-foreground">
                {t("export.copyMarkdownDesc")}
              </span>
            )}
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={owner.handleCopySVG}
            disabled={!owner.artifactActionsEnabled}
          >
            <Copy className="size-4" />
            {t("export.copySvg")}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={owner.handleOpenMermaidLive}>
            <ExternalLink className="size-4" />
            {t("share.openMermaidLive")}
            {!compact && (
              <span className="ml-auto text-xs text-muted-foreground">
                {t("share.openMermaidLiveDesc")}
              </span>
            )}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="outline"
            size={compact ? "icon-sm" : "sm"}
            className={compact ? undefined : "w-8 px-0 sm:w-auto sm:px-2.5"}
            onClick={owner.handleShare}
            aria-label={t("share.copyLink")}
          >
            <Share2 className="size-4" />
            {!compact && (
              <span className="hidden sm:inline">{t("toolbar.share")}</span>
            )}
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("share.copyLink")}</TooltipContent>
      </Tooltip>
    </>
  );
}
