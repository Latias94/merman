import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import Editor from "@monaco-editor/react";
import {
  AlertCircle,
  Check,
  Code2,
  Copy,
  Download,
  ImageIcon,
  Loader2,
  RefreshCw,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { ViewportControls } from "@/src/components/PreviewArtifactViews";
import {
  SvgViewport,
  useSvgViewportController,
  type SvgViewportController,
} from "@/src/components/SvgViewport";
import type { MermaidCanvasTone } from "@/src/lib/mermaid-canvas-tone";
import type { SvgPresentationMode } from "@/src/lib/svg-presentation";
import type { NavigableInlineSvg } from "@/src/runtime/render-artifact";
import type { RenderPublicationId } from "@/src/runtime/render-coordinator";
import { cn } from "@/lib/utils";

export type CompareEngineKey = "merman" | "mermaid";
type SvgDisplayMode = "visual" | "source";

export interface CompareArtifact {
  key: CompareEngineKey;
  publicationId: RenderPublicationId | null;
  title: string;
  version: string;
  svgArtifact: NavigableInlineSvg | null;
  error: string | null;
  errorDetail: string | null;
  errorStage: string | null;
  renderTime: number | null;
  loading: boolean;
  loadingLabel: string | null;
  unavailableLabel: string | null;
  stale: boolean;
}

export interface ComparePaneModel {
  artifact: CompareArtifact;
}

export interface ComparePaneActions {
  copiedSvgTarget: {
    readonly engine: CompareEngineKey;
    readonly publicationId: RenderPublicationId;
  } | null;
  onCopySvg(engine: CompareEngineKey, publicationId: RenderPublicationId): void;
  onExport(engine: CompareEngineKey, publicationId: RenderPublicationId): void;
  onPresentationReady(engine: CompareEngineKey, at: number): void;
  onRetry(): void;
}

export function CompareView({
  merman,
  mermaid,
  actions,
  canvasTone,
  isDarkMode,
  showSvgBounds,
  presentationMode,
  t,
}: {
  merman: ComparePaneModel;
  mermaid: ComparePaneModel;
  actions: ComparePaneActions;
  canvasTone: MermaidCanvasTone;
  isDarkMode: boolean;
  showSvgBounds: boolean;
  presentationMode: SvgPresentationMode;
  t: (key: string) => string;
}) {
  return (
    <div
      className="preview-canvas h-full overflow-auto overscroll-contain p-2 sm:p-3"
      data-merman-compare-scroll-owner="true"
      data-preview-canvas-tone={canvasTone}
      data-svg-presentation-mode={presentationMode}
    >
      <div className="grid min-h-full grid-cols-1 gap-3 xl:grid-cols-2">
        {[merman, mermaid].map((pane) => (
          <ComparePane
            key={pane.artifact.key}
            model={pane}
            actions={actions}
            canvasTone={canvasTone}
            isDarkMode={isDarkMode}
            showSvgBounds={showSvgBounds}
            presentationMode={presentationMode}
            t={t}
          />
        ))}
      </div>
    </div>
  );
}

function ComparePane({
  model,
  actions,
  canvasTone,
  isDarkMode,
  showSvgBounds,
  presentationMode,
  t,
}: {
  model: ComparePaneModel;
  actions: ComparePaneActions;
  canvasTone: MermaidCanvasTone;
  isDarkMode: boolean;
  showSvgBounds: boolean;
  presentationMode: SvgPresentationMode;
  t: (key: string) => string;
}) {
  const { artifact } = model;
  const copied =
    actions.copiedSvgTarget?.engine === artifact.key &&
    actions.copiedSvgTarget.publicationId === artifact.publicationId;
  const hasSvg = Boolean(artifact.svgArtifact);
  const actionsDisabled =
    artifact.publicationId === null ||
    artifact.stale ||
    Boolean(artifact.unavailableLabel);
  const [svgDisplayMode, setSvgDisplayMode] =
    useState<SvgDisplayMode>("visual");
  const controller = useSvgViewportController();
  const statusId = useId();
  const paneRef = useRef<HTMLElement>(null);
  const ownedFocus = useRef(false);
  const status = compareArtifactStatus(artifact, t);
  const replacementKey = [
    artifact.publicationId,
    status.state,
    hasSvg ? "svg" : "empty",
  ].join(":");

  useEffect(() => {
    const pane = paneRef.current;
    if (!pane || !ownedFocus.current) return;
    const active = document.activeElement;
    if (!(active instanceof Node) || !pane.contains(active)) {
      pane.focus({ preventScroll: true });
    }
  }, [replacementKey]);

  return (
    <section
      ref={paneRef}
      aria-busy={artifact.loading || artifact.stale}
      aria-describedby={statusId}
      aria-label={artifact.title}
      data-merman-artifact-state={status.state}
      data-merman-compare-engine={artifact.key}
      tabIndex={-1}
      onFocusCapture={() => {
        ownedFocus.current = true;
      }}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          ownedFocus.current = false;
        }
      }}
      className="preview-canvas-frame flex min-h-[320px] min-w-0 flex-col overflow-hidden rounded-md border outline-none xl:min-h-0"
    >
      <div className="preview-canvas-toolbar border-b px-3 py-2">
        <div className="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <span className="max-w-full truncate text-sm font-medium">
              {artifact.title}
            </span>
            <span className="shrink-0 rounded-sm bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
              {artifact.version}
            </span>
            {artifact.stale && (
              <span className="shrink-0 rounded-sm bg-amber-500/15 px-1.5 py-0.5 text-[11px] text-amber-700 dark:text-amber-300">
                {t("preview.updatingStale")}
              </span>
            )}
          </div>
          <p
            id={statusId}
            aria-live="polite"
            className="shrink-0 text-xs text-muted-foreground"
            role="status"
          >
            {status.label}
          </p>
        </div>
        <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
          {hasSvg && svgDisplayMode === "visual" ? (
            <ViewportControls controller={controller} t={t} />
          ) : (
            <div />
          )}
          <div className="flex items-center gap-1">
            <CompareIconButton
              label={
                copied
                  ? t("preview.copied")
                  : (artifact.unavailableLabel ?? t("preview.copySvg"))
              }
              onClick={() =>
                artifact.publicationId &&
                actions.onCopySvg(artifact.key, artifact.publicationId)
              }
              disabled={actionsDisabled}
            >
              {copied ? (
                <Check className="size-4 text-green-500" />
              ) : (
                <Copy className="size-4" />
              )}
            </CompareIconButton>
            <CompareIconButton
              label={
                artifact.unavailableLabel ?? t("export.workbenchTitle")
              }
              onClick={() =>
                artifact.publicationId &&
                actions.onExport(artifact.key, artifact.publicationId)
              }
              disabled={actionsDisabled}
            >
              <Download className="size-4" />
            </CompareIconButton>
            <CompareIconButton
              label={
                svgDisplayMode === "visual"
                  ? t("preview.viewSvgSource")
                  : t("preview.viewSvgPreview")
              }
              onClick={() =>
                setSvgDisplayMode((value) =>
                  value === "visual" ? "source" : "visual"
                )
              }
              disabled={!hasSvg}
            >
              {svgDisplayMode === "visual" ? (
                <Code2 className="size-4" />
              ) : (
                <ImageIcon className="size-4" />
              )}
            </CompareIconButton>
          </div>
        </div>
      </div>
      <div
        className={cn(
          "min-h-0 flex-1 overflow-hidden transition-opacity",
          artifact.stale && "opacity-60"
        )}
      >
        <ComparePaneBody
          artifact={artifact}
          canvasTone={canvasTone}
          controller={controller}
          displayMode={svgDisplayMode}
          isDarkMode={isDarkMode}
          onPresentationReady={(at) =>
            actions.onPresentationReady(artifact.key, at)
          }
          onRetry={() => {
            paneRef.current?.focus({ preventScroll: true });
            actions.onRetry();
          }}
          showSvgBounds={showSvgBounds}
          presentationMode={presentationMode}
          t={t}
        />
      </div>
    </section>
  );
}

function compareArtifactStatus(
  artifact: CompareArtifact,
  t: (key: string) => string
): { label: string; state: "empty" | "ready" | "rejected" | "updating" } {
  if (artifact.loading || artifact.stale) {
    return {
      label: artifact.loadingLabel ?? t("preview.renderingMermaid"),
      state: "updating",
    };
  }
  if (artifact.error) {
    return { label: t("preview.currentRenderFailed"), state: "rejected" };
  }
  if (artifact.svgArtifact) {
    const timing =
      artifact.renderTime === null ? "" : ` · ${artifact.renderTime.toFixed(1)}ms`;
    return { label: `${t("preview.artifactReady")}${timing}`, state: "ready" };
  }
  return { label: t("preview.noCurrentArtifact"), state: "empty" };
}

function ComparePaneBody({
  artifact,
  canvasTone,
  controller,
  displayMode,
  isDarkMode,
  onPresentationReady,
  onRetry,
  showSvgBounds,
  presentationMode,
  t,
}: {
  artifact: CompareArtifact;
  canvasTone: MermaidCanvasTone;
  controller: SvgViewportController;
  displayMode: SvgDisplayMode;
  isDarkMode: boolean;
  onPresentationReady(at: number): void;
  onRetry(): void;
  showSvgBounds: boolean;
  presentationMode: SvgPresentationMode;
  t: (key: string) => string;
}) {
  if (artifact.loading && !artifact.svgArtifact) {
    return (
      <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
        {artifact.loadingLabel ?? t("preview.renderingMermaid")}
      </CenteredMessage>
    );
  }
  if (artifact.error) {
    return (
      <CompareFailure
        detail={artifact.errorDetail}
        engine={artifact.title}
        message={artifact.error}
        stage={artifact.errorStage}
        onRetry={onRetry}
        t={t}
      />
    );
  }
  if (displayMode === "source") {
    return (
      <CompareSvgSource
        svg={artifact.svgArtifact?.svg ?? null}
        isDarkMode={isDarkMode}
      />
    );
  }
  return (
    <SvgViewport
      artifact={artifact.svgArtifact}
      canvasTone={canvasTone}
      presentationKey={artifact.publicationId}
      controller={controller}
      navigationEnabled={!artifact.loading && !artifact.stale}
      onPresentationReady={onPresentationReady}
      showSvgBounds={showSvgBounds}
      presentationMode={presentationMode}
      renderMountError={(mountError) => (
        <CompareFailure
          detail={mountError.stack ?? null}
          engine={artifact.title}
          message={mountError.message}
          stage="svg-mount"
          onRetry={onRetry}
          t={t}
        />
      )}
      empty={
        <div className="preview-canvas-status-muted flex h-full items-center justify-center text-sm">
          {t("preview.empty")}
        </div>
      }
    />
  );
}

function CompareFailure({
  detail,
  engine,
  message,
  stage,
  onRetry,
  t,
}: {
  detail: string | null;
  engine: string;
  message: string;
  stage: string | null;
  onRetry(): void;
  t: (key: string) => string;
}) {
  return (
    <div
      className="preview-canvas-status flex h-full items-center justify-center p-5"
      data-merman-render-error="true"
      data-merman-error-engine={engine}
      data-merman-error-stage={stage ?? undefined}
      role="alert"
    >
      <div className="max-w-sm text-center">
        <div className="mx-auto mb-3 flex size-10 items-center justify-center rounded-full bg-destructive/10">
          <AlertCircle className="size-5 text-destructive" />
        </div>
        <h3 className="mb-1 font-medium">
          {engine} · {t("preview.error")}
        </h3>
        {stage && (
          <p className="preview-canvas-status-muted mb-2 font-mono text-xs">{stage}</p>
        )}
        <p className="preview-canvas-status-muted rounded-md bg-black/5 p-3 font-mono text-sm dark:bg-white/5">
          {message}
        </p>
        {detail && (
          <details className="preview-canvas-status-muted mt-3 text-left text-xs">
            <summary className="cursor-pointer select-none text-center">
              {t("preview.errorDetails")}
            </summary>
            <pre className="mt-2 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted/50 p-3 font-mono">
              {detail}
            </pre>
          </details>
        )}
        <Button className="mt-4" size="sm" variant="outline" onClick={onRetry}>
          <RefreshCw className="size-4" />
          {t("preview.retryCompare")}
        </Button>
      </div>
    </div>
  );
}

function CompareSvgSource({
  svg,
  isDarkMode,
}: {
  svg: string | null;
  isDarkMode: boolean;
}) {
  if (!svg) return null;
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
        scrollBeyondLastLine: false,
        wordWrap: "on",
        automaticLayout: true,
        padding: { top: 12, bottom: 12 },
      }}
    />
  );
}

function CompareIconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick(): void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function CenteredMessage({
  icon,
  children,
}: {
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="preview-canvas-status flex h-full flex-1 items-center justify-center">
      <div className="preview-canvas-status-muted flex flex-col items-center gap-3">
        {icon}
        <span className="text-sm">{children}</span>
      </div>
    </div>
  );
}
