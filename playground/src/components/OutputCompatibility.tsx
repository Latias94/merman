import { useTranslation } from "react-i18next";

import type { ArtifactEngine } from "@/src/runtime/artifact-actions";
import type { MermanSvgPipeline } from "@/src/runtime/merman-core";

export function OutputCompatibility({ svgPipeline, engine, raster = false }: {
  svgPipeline: MermanSvgPipeline | null;
  engine: ArtifactEngine;
  raster?: boolean;
}) {
  const { t } = useTranslation();
  if (!svgPipeline && !raster) return null;
  const heading = t(raster ? "workbench.output.sourceTitle" : "workbench.output.title");
  return <section aria-label={heading} data-testid="output-compatibility" className="space-y-2 rounded-md border p-3 text-xs">
    <h3 className="font-semibold">{heading}</h3>
    {svgPipeline && <p className="text-muted-foreground">
      {t("workbench.output.pipeline", { pipeline: t(`svgPipelines.${svgPipeline}`) })}
    </p>}
    {svgPipeline === "resvg-safe" && <p className="leading-relaxed text-muted-foreground">{t("workbench.export.resvgBoundary")}</p>}
    {svgPipeline === "readable" && <p className="leading-relaxed text-muted-foreground">{t("svgPipelineGuidance.browserWarning")}</p>}
    {raster && <p className="leading-relaxed text-muted-foreground">
      {t(engine === "merman" ? "workbench.output.mermanRaster" : "workbench.output.mermaidRaster")}
    </p>}
  </section>;
}
