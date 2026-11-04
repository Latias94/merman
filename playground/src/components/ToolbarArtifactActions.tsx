import { useCallback, useId, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  ChevronDown,
  Code,
  Copy,
  Download,
  ExternalLink,
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
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useExportWorkbench } from "@/src/components/ExportDialog";
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
import { copyWorkspaceShareUrl } from "@/src/lib/share";
import { copyIssueShareUrl } from "@/src/lib/share-view";
import { currentThemeRender } from "@/src/lib/theme-discovery";
import { executeArtifactAction } from "@/src/runtime/artifact-actions-browser";
import { isMermanSvgPipeline, MERMAN_SVG_PIPELINES } from "@/src/runtime/merman-core";
import { isAsciiExportAvailable } from "@/src/components/toolbar-artifact-availability";
import {
  selectCompletedRenderBatch,
  selectCurrentDiagramType,
  useRenderCoordinator,
} from "@/src/runtime/use-render-coordinator";
import {
  selectWorkspaceSnapshot,
  useAppStore,
} from "@/src/store";

export function useToolbarArtifactActions() {
  const { t } = useTranslation();
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const { code, diagramTheme, mermaidConfig, svgPipeline } = workspace;
  const diagramType = useRenderCoordinator(selectCurrentDiagramType);
  const renderState = useRenderCoordinator((state) => state);
  const render = currentThemeRender(renderState, workspace);
  const currentBatch = render.status === "waiting" ? null : selectCompletedRenderBatch(renderState);
  const asciiSupport = useAsciiSupport();
  const { openExport } = useExportWorkbench();
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
  const asciiAvailable = isAsciiExportAvailable(asciiSupported, currentBatch);

  const handleOpenExport = useCallback((restoreFocus?: HTMLElement | null) => {
    if (!currentBatch) return;
    openExport("merman", currentBatch.snapshot.publicationId, restoreFocus);
  }, [currentBatch, openExport]);

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

  const handleCopyWorkspaceLink = useCallback(async () => {
    const snapshot = selectWorkspaceSnapshot(useAppStore.getState());
    if (!snapshot.code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyWorkspaceShareUrl(snapshot);
      toast.success(t("share.copied"));
    } catch {
      toast.error(t("share.copyFailed"));
    }
  }, [t]);

  const handleCopyIssueLink = useCallback(async () => {
    const state = useAppStore.getState();
    const snapshot = selectWorkspaceSnapshot(state);
    if (!snapshot.code.trim()) {
      toast.error(t("share.copyFailed"));
      return;
    }
    try {
      await copyIssueShareUrl(snapshot, {
        workspacePane: state.workspacePane,
        editorMode: state.editorMode,
        previewMode: state.previewMode,
        showSvgBounds: state.showSvgBounds,
        svgPresentationMode: state.svgPresentationMode,
      });
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
    svgPipeline,
    artifactActionsEnabled,
    asciiExportDescription,
    asciiAvailable,
    handleCopyCode,
    handleCopyIssueLink,
    handleCopyMarkdown,
    handleCopySVG,
    handleCopyWorkspaceLink,
    handleExportASCII,
    handleOpenExport,
    handleOpenMermaidLive,
  };
}

type ToolbarArtifactActionsOwner = ReturnType<
  typeof useToolbarArtifactActions
>;

export function ToolbarArtifactActions({
  compact,
  owner,
  children,
}: {
  compact: boolean;
  owner: ToolbarArtifactActionsOwner;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  const exportTriggerRef = useRef<HTMLButtonElement>(null);
  const sourceDescriptionId = useId();
  const externalDescriptionId = useId();
  return (
    <>
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                ref={exportTriggerRef}
                variant="outline"
                size={compact ? "icon-sm" : "sm"}
                className={compact ? undefined : "w-8 px-0 xl:w-auto xl:px-2.5"}
                aria-label={t("toolbar.export")}
              >
                <Download className="size-4" />
                {!compact && (
                  <>
                    <span className="hidden xl:inline">{t("toolbar.export")}</span>
                    <ChevronDown className="hidden size-3 opacity-50 xl:block" />
                  </>
                )}
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>{t("toolbar.export")}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end" className="w-80 max-w-[calc(100vw-1rem)]">
          <DropdownMenuLabel>{t("workbench.export.currentArtifact")}</DropdownMenuLabel>
          <DropdownMenuItem
            onClick={() => owner.handleOpenExport(exportTriggerRef.current)}
            disabled={!owner.artifactActionsEnabled}
          >
            <ImageIcon className="size-4" />
            {t("export.image")}
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={owner.handleExportASCII}
            disabled={!owner.asciiAvailable}
            aria-description={owner.asciiExportDescription}
          >
            <FileText className="size-4" />
            {t("export.ascii")}
            <span className="ml-auto max-w-40 truncate text-xs text-muted-foreground">
              {owner.asciiExportDescription}
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={owner.handleCopySVG}
            disabled={!owner.artifactActionsEnabled}
          >
            <Copy className="size-4" />
            {t("export.copySvg")}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t("toolbar.svgOutput")}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={owner.svgPipeline} aria-label={t("toolbar.svgOutput")}
            onValueChange={(value) => {
              if (isMermanSvgPipeline(value)) useAppStore.getState().setSvgPipeline(value);
            }}>
            {MERMAN_SVG_PIPELINES.map((pipeline) => <DropdownMenuRadioItem key={pipeline} value={pipeline}
              aria-description={t(`svgPipelineDescriptions.${pipeline}`)}
              onSelect={(event) => event.preventDefault()}>
              {t(`svgPipelines.${pipeline}`)}
            </DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
          {owner.svgPipeline === "readable" && <p className="px-2 py-1.5 text-xs text-muted-foreground">
            {t("svgPipelineGuidance.browserWarning")}
          </p>}
          {owner.svgPipeline === "resvg-safe" && <p className="px-2 py-1.5 text-xs text-muted-foreground">
            {t("workbench.export.resvgBoundary")}
          </p>}
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={owner.handleCopyCode} className="items-start py-2"
            aria-label={t("workbench.export.copySource")} aria-describedby={sourceDescriptionId}>
            <Code className="mt-0.5 size-4" />
            <span>
              <span className="block">{t("workbench.export.copySource")}</span>
              <span id={sourceDescriptionId} className="mt-0.5 block text-xs leading-snug text-muted-foreground">{t("workbench.export.sourceDescription")}</span>
            </span>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>{t("workbench.export.externalRendering")}</DropdownMenuLabel>
          <p id={externalDescriptionId} className="px-2 pb-1.5 text-xs leading-snug text-muted-foreground">{t("workbench.export.externalDescription")}</p>
          <DropdownMenuItem onClick={owner.handleCopyMarkdown} aria-describedby={externalDescriptionId}>
            <FileText className="size-4" />
            {t("export.copyMarkdown")}
            <span className="ml-auto text-xs text-muted-foreground">Mermaid.ink</span>
          </DropdownMenuItem>
          <DropdownMenuItem onClick={owner.handleOpenMermaidLive} aria-describedby={externalDescriptionId}>
            <ExternalLink className="size-4" />
            {t("share.openMermaidLive")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {children}
      <ShareMenu compact={compact} owner={owner} />
    </>
  );
}

function ShareMenu({
  compact,
  owner,
}: {
  compact: boolean;
  owner: ToolbarArtifactActionsOwner;
}) {
  const { t } = useTranslation();
  const workspaceDescriptionId = useId();
  const issueDescriptionId = useId();

  return (
    <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size={compact ? "icon-sm" : "sm"}
                className={
                  compact ? undefined : "w-8 px-0 xl:w-auto xl:px-2.5"
                }
                aria-label={t("toolbar.share")}
              >
                <Share2 className="size-4" />
                {!compact && (
                  <>
                    <span className="hidden xl:inline">{t("toolbar.share")}</span>
                    <ChevronDown className="hidden size-3 opacity-50 xl:block" />
                  </>
                )}
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>{t("toolbar.share")}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent
          align="end"
          className="w-80 max-w-[calc(100vw-2rem)]"
        >
          <DropdownMenuLabel>{t("share.title")}</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            aria-label={t("share.workspaceLink")}
            aria-describedby={workspaceDescriptionId}
            className="items-start py-2"
            onClick={owner.handleCopyWorkspaceLink}
          >
            <Share2 className="mt-0.5 size-4" />
            <span className="min-w-0">
              <span className="block font-medium">
                {t("share.workspaceLink")}
              </span>
              <span
                id={workspaceDescriptionId}
                className="mt-0.5 block text-xs leading-snug text-muted-foreground"
              >
                {t("share.workspaceLinkDesc")}
              </span>
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem
            aria-label={t("share.issueLink")}
            aria-describedby={issueDescriptionId}
            className="items-start py-2"
            onClick={owner.handleCopyIssueLink}
          >
            <Copy className="mt-0.5 size-4" />
            <span className="min-w-0">
              <span className="block font-medium">{t("share.issueLink")}</span>
              <span
                id={issueDescriptionId}
                className="mt-0.5 block text-xs leading-snug text-muted-foreground"
              >
                {t("share.issueLinkDesc")}
              </span>
            </span>
          </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
