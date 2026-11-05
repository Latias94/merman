import { useTranslation } from "react-i18next";
import { boundedThemeReasonCodes, readThemeExecutionEvidence, themeEvidenceOutcome, type CurrentThemeRender } from "@/src/lib/theme-discovery";

function ThemeEvidenceReasons({ codes }: { codes: readonly string[] }) {
  const { t } = useTranslation();
  const reasons = boundedThemeReasonCodes(codes);
  if (reasons.visible.length === 0) return null;
  return <div className="space-y-1.5" data-testid="theme-diagnostic-reasons">
    <h4 className="font-medium">{t("themePanel.diagnosticReasonsTitle")}</h4>
    <ul className="list-disc space-y-1.5 pl-4 text-muted-foreground">
      {reasons.visible.map((code) => <li key={code} className="break-words leading-relaxed [overflow-wrap:anywhere]" data-theme-reason-code={code}>
        {t(`themePanel.diagnosticReasons.${code}`, { defaultValue: t("themePanel.diagnosticReasonsUnknown", { code }) })}
      </li>)}
    </ul>
    {reasons.remaining > 0 && <p className="text-[11px] text-muted-foreground">{t("themePanel.moreReasons", { count: reasons.remaining })}</p>}
  </div>;
}

export function ThemeRenderEvidence({ render, unavailablePresetId }: { render: CurrentThemeRender; unavailablePresetId: string | null }) {
  const { t } = useTranslation();
  const evidence = render.status === "ready" && render.batch.merman.status === "success"
    ? readThemeExecutionEvidence(render.batch.merman.metadata) : null;
  const unavailablePreset = unavailablePresetId ?? render.batch?.snapshot.operation.unavailableThemePresetId;
  const outcome = unavailablePreset ? "unavailable" : render.status === "ready" ? themeEvidenceOutcome(evidence) : render.status;
  return <section className="space-y-2 rounded-md border p-3 text-xs" aria-label={t("themePanel.renderOutcome")}
    data-testid="theme-render-evidence" data-theme-evidence-state={outcome} aria-live="polite" aria-atomic="true">
    <h3 className="font-semibold">{t("themePanel.renderOutcome")}</h3>
    <p data-testid="theme-outcome" className="leading-relaxed">
      {unavailablePreset ? t("themePanel.unavailablePresetPreview", { id: unavailablePreset })
        : render.status === "waiting" ? t(`themePanel.waiting.${render.reason}`) : t(`themePanel.outcome.${outcome}`)}
    </p>
    {render.status === "failed" && render.batch.merman.status === "failure" &&
      <p className="break-words text-destructive">{render.batch.merman.message}</p>}
    {evidence && <ThemeEvidenceReasons codes={evidence.diagnostics?.map((diagnostic) => diagnostic.code) ?? []} />}
  </section>;
}
