import { useCallback, useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import type { ThemeAppearance } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { SvgViewport, useSvgViewportController } from "@/src/components/SvgViewport";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "@/src/lib/workspace-snapshot";
import { projectError } from "@/src/runtime/error-projection";
import type { MermanDomainFacade, MermanRenderResult } from "@/src/runtime/merman-core";
import { captureMermanRuntimeSnapshot, configuredMermanOperationInput } from "@/src/runtime/merman-operation-input";
import { captureRenderViewport } from "@/src/runtime/render-viewport";
import { useAppStore } from "@/src/store";

const SAMPLES = ["flowchart", "sequence", "pie", "current"] as const;
type Sample = (typeof SAMPLES)[number];
type PreviewResult = Pick<MermanRenderResult, "artifact" | "error">;

const PAGE_SHARES = [["News", 40], ["Features", 30], ["Reviews", 20], ["Letters", 10]] as const;

const SOURCES = {
  flowchart: `%%{init: {"flowchart":{"rankSpacing":24}}}%%
flowchart TD
    accTitle: Article review
    accDescr: The editor can send the draft back for revisions or approve it for publication.
    draft([Write draft]) --> review[Copy edit]
    review -.->|Revise| draft
    review -->|Approved| publish([Publish])`,
  sequence: `%%{init: {"sequence":{"width":84,"actorMargin":36,"diagramMarginX":12,"mirrorActors":false}}}%%
sequenceDiagram
    accTitle: Editing a draft
    accDescr: A writer sends a draft to an editor. The editor requests edits and the writer sends a revision.
    participant Writer
    participant Editor
    Writer->>+Editor: Send draft
    Editor-->>-Writer: Request edits
    Writer->>Editor: Send revision`,
  pie: `pie title Pages by section
    accTitle: Pages by section
    accDescr: ${PAGE_SHARES.map(([label, share]) => `${label} ${share}%`).join(", ")}.
${PAGE_SHARES.map(([label, share]) => `    "${label}" : ${share}`).join("\n")}`,
} satisfies Record<Exclude<Sample, "current">, string>;

export function ThemeBrowserPreview({ facade, presetId, appearance }: {
  readonly facade: MermanDomainFacade;
  readonly presetId: string;
  readonly appearance: ThemeAppearance;
}) {
  const { t } = useTranslation();
  const id = useId();
  const controller = useSvgViewportController();
  const [sample, setSample] = useState<Sample>("flowchart");
  const [attempt, setAttempt] = useState(0);
  const settings = useAppStore(useShallow((state) => {
    const workspace = sample === "current" ? state : DEFAULT_WORKSPACE_SNAPSHOT;
    return {
      source: sample === "current" ? state.code : SOURCES[sample],
      config: workspace.mermaidConfig,
      diagramFont: workspace.diagramFont,
      textMeasurementMode: workspace.textMeasurementMode,
      svgPipeline: workspace.svgPipeline,
    };
  }));
  const hasSource = Boolean(settings.source.trim());
  const render = useCallback((): PreviewResult => {
    try {
      const input = configuredMermanOperationInput(settings.source, "auto", settings.config, {
        diagramFont: settings.diagramFont,
        textMeasurementMode: settings.textMeasurementMode,
        svgPipeline: settings.svgPipeline,
        layoutEnvironment: captureRenderViewport().layoutEnvironment,
        runtimeSnapshot: sample === "current" ? captureMermanRuntimeSnapshot() : undefined,
        themePresetId: presetId,
        themeAppearance: appearance,
      });
      const { artifact, error } = facade.render(input);
      return { artifact, error };
    } catch (cause) {
      return { artifact: null, error: projectError(cause) };
    }
  }, [appearance, facade, presetId, sample, settings]);
  const [completed, setCompleted] = useState<{ render: typeof render; result: PreviewResult } | null>(null);
  const result = completed?.render === render ? completed.result : null;

  useEffect(() => {
    if (!hasSource) {
      setCompleted(null);
      return;
    }
    const frame = requestAnimationFrame(() => setCompleted({ render, result: render() }));
    return () => cancelAnimationFrame(frame);
  }, [attempt, hasSource, render]);

  function renderError(message: string) {
    return <div role="alert" className="flex h-full cursor-auto flex-col items-center justify-center gap-2 p-4 text-center">
      <p className="text-sm font-semibold">{t("themeBrowser.previewError")}</p>
      <p className="preview-canvas-status-muted max-h-24 max-w-full select-text overflow-auto text-xs [overflow-wrap:anywhere]">{message}</p>
      <Button type="button" size="sm" variant="outline" className="text-foreground" onClick={() => {
        setCompleted(null);
        setAttempt((n) => n + 1);
      }}>
        {t("preview.retryCompare")}
      </Button>
    </div>;
  }

  return <section aria-labelledby={`${id}-heading`} data-testid="theme-browser-preview" data-sample={sample}
    className="@container w-full min-w-0 space-y-2">
    <div className="flex flex-wrap items-center justify-between gap-2">
      <h3 id={`${id}-heading`} className="text-sm font-semibold">{t("themeBrowser.preview")}</h3>
      <div role="group" aria-label={t("themeBrowser.sample")} className="grid w-full grid-cols-2 gap-1 rounded-lg bg-muted p-1 @md:flex @md:w-auto @md:flex-wrap">
        {SAMPLES.map((choice) => <Button key={choice} type="button" size="sm" variant="ghost"
          data-browser-sample={choice} aria-pressed={choice === sample} aria-controls={id}
          className="px-2.5 text-xs aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm"
          onClick={() => setSample(choice)}>
          {t(choice === "current" ? "themeBrowser.currentDiagram" : `diagramTypes.${choice}`)}
        </Button>)}
      </div>
    </div>
    <div id={id} aria-busy={hasSource && result === null} className="h-80 w-full overflow-hidden rounded-lg border @lg:h-[28rem]">
      <SvgViewport artifact={result?.artifact ?? null} canvasTone={appearance} controller={controller}
        presentationKey={null} presentationMode="viewbox" interactionMode="embedded" navigationEnabled={false} showSvgBounds={false}
        renderMountError={(error) => renderError(error.message)}
        empty={result?.error ? renderError(result.error.summary) :
          <p role="status" className="preview-canvas-status-muted grid h-full place-items-center p-4 text-center text-sm">
            {t(hasSource ? "preview.renderingCurrent" : "preview.empty")}
          </p>} />
    </div>
    {sample === "pie" && result?.artifact && <dl lang="en" className="grid grid-cols-2 gap-x-5 gap-y-1 text-xs @md:hidden">
      {PAGE_SHARES.map(([label, share]) => <div key={label} className="flex justify-between gap-2">
        <dt>{label}</dt><dd className="font-medium tabular-nums">{share}%</dd>
      </div>)}
    </dl>}
  </section>;
}
