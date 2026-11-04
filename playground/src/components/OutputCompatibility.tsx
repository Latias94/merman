import type { SvgRenderMetadata } from "@mermanjs/web";
import { useTranslation } from "react-i18next";

import { boundedThemeReasonCodes, readThemeExecutionEvidence } from "@/src/lib/theme-discovery";
import type { ArtifactEngine } from "@/src/runtime/artifact-actions";
import type { MermanSvgPipeline } from "@/src/runtime/merman-core";

export function OutputCompatibility({
  metadata,
  svgPipeline,
  engine,
  raster = false,
}: {
  metadata: SvgRenderMetadata | null;
  svgPipeline: MermanSvgPipeline | null;
  engine: ArtifactEngine;
  raster?: boolean;
}) {
  const { t } = useTranslation();
  const evidence = readThemeExecutionEvidence(metadata);
  const status = evidence?.target_status ?? "unknown";
  const reasons = boundedThemeReasonCodes(evidence?.target_reason_ids ?? []);
  const heading = t(raster ? "workbench.output.sourceTitle" : "workbench.output.title");

  return <section aria-label={heading} data-testid="output-compatibility"
    data-output-target-state={status}
    data-output-evidence-version={metadata?.theme_execution_evidence?.version}
    className="space-y-2 rounded-md border p-3 text-xs">
    <h3 className="font-semibold">{heading}</h3>
    <p className={status === "rejected" ? "leading-relaxed text-destructive" : "leading-relaxed text-muted-foreground"}>
      {t(`workbench.output.status.${status}`, {
        defaultValue: t("workbench.output.unknownStatus", { status }),
      })}
    </p>
    {svgPipeline && <p className="text-muted-foreground">
      {t("workbench.output.pipeline", { pipeline: t(`svgPipelines.${svgPipeline}`) })}
    </p>}
    {svgPipeline === "resvg-safe" && <p className="leading-relaxed text-muted-foreground">{t("workbench.export.resvgBoundary")}</p>}
    {svgPipeline === "readable" && <p className="leading-relaxed text-muted-foreground">{t("svgPipelineGuidance.browserWarning")}</p>}
    {raster && <p className="leading-relaxed text-muted-foreground">
      {t(engine === "merman" ? "workbench.output.mermanRaster" : "workbench.output.mermaidRaster")}
    </p>}
    {evidence && <details>
      <summary className="cursor-pointer font-medium">{t("workbench.output.evidence")}</summary>
      <dl className="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-muted-foreground">
        <dt>{t("workbench.output.target")}</dt><dd className="break-words [overflow-wrap:anywhere]">{evidence.target_kind}</dd>
        <dt>{t("workbench.output.runtimeStatus")}</dt><dd className="break-words [overflow-wrap:anywhere]">{evidence.target_status}</dd>
        <dt>{t("workbench.output.fontSource")}</dt><dd className="break-words [overflow-wrap:anywhere]">{evidence.font_source}</dd>
      </dl>
      {reasons.visible.length > 0 && <ul className="mt-2 list-disc space-y-1.5 pl-4 text-muted-foreground">
        {reasons.visible.map((code) => <li key={code} data-output-reason-code={code}
          className="break-words leading-relaxed [overflow-wrap:anywhere]">
          {t(`workbench.output.reasons.${code}`, { defaultValue: t("workbench.output.unknownReason", { code }) })}
        </li>)}
      </ul>}
      {reasons.remaining > 0 && <details className="mt-2 text-muted-foreground">
        <summary className="cursor-pointer">{t("workbench.output.moreReasons", { count: reasons.remaining })}</summary>
        <ul className="mt-2 list-disc space-y-1.5 pl-4">
          {[...new Set(evidence.target_reason_ids)].slice(reasons.visible.length).map((code) => <li key={code}
            data-output-reason-code={code} className="break-words [overflow-wrap:anywhere]">
            {t(`workbench.output.reasons.${code}`, { defaultValue: t("workbench.output.unknownReason", { code }) })}
          </li>)}
        </ul>
      </details>}
    </details>}
  </section>;
}
