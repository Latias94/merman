import { useTranslation } from "react-i18next";
import { boundedThemeReasonCodes, readThemeExecutionEvidence, themeEvidenceOutcome, themeTargetOutcome, type CurrentThemeRender } from "@/src/lib/theme-discovery";

function ThemeEvidenceReasons({ codes, kind }: { codes: readonly string[]; kind: "theme" | "target" }) {
  const { t } = useTranslation();
  const reasons = boundedThemeReasonCodes(codes);
  if (reasons.visible.length === 0) return null;
  const prefix = kind === "theme" ? "diagnosticReasons" : "targetReasons";
  return <div className="space-y-1.5" data-testid={kind === "theme" ? "theme-diagnostic-reasons" : "theme-target-reasons"}>
    <h4 className="font-medium">{t(`themePanel.${prefix}Title`)}</h4>
    <ul className="list-disc space-y-1.5 pl-4 text-muted-foreground">
      {reasons.visible.map((code) => <li key={code} className="break-words leading-relaxed [overflow-wrap:anywhere]" data-theme-reason-code={code}>
        {t(`themePanel.${prefix}.${code}`, { defaultValue: t(`themePanel.${prefix}Unknown`, { code }) })}
      </li>)}
    </ul>
    {reasons.remaining > 0 && <p className="text-[11px] text-muted-foreground">{t("themePanel.moreReasons", { count: reasons.remaining })}</p>}
  </div>;
}

export function ThemeRenderEvidence({ render }: { render: CurrentThemeRender }) {
  const { t } = useTranslation();
  const evidence = render.status === "ready" && render.batch.merman.status === "success"
    ? readThemeExecutionEvidence(render.batch.merman.metadata) : null;
  const outcome = render.status === "ready" ? themeEvidenceOutcome(evidence) : render.status;
  const target = themeTargetOutcome(evidence);
  return <section className="space-y-2 rounded-md border p-3 text-xs" aria-label={t("themePanel.renderOutcome")}
    data-testid="theme-render-evidence" data-theme-evidence-state={outcome} aria-live="polite" aria-atomic="true">
    <h3 className="font-semibold">{t("themePanel.renderOutcome")}</h3>
    <p data-testid="theme-outcome" className="leading-relaxed">
      {render.status === "waiting" ? t(`themePanel.waiting.${render.reason}`) : t(`themePanel.outcome.${outcome}`)}
    </p>
    {render.status === "failed" && render.batch.merman.status === "failure" &&
      <p className="break-words text-destructive">{render.batch.merman.message}</p>}
    {evidence && <ThemeEvidenceReasons codes={evidence.diagnostics?.map((diagnostic) => diagnostic.code) ?? []} kind="theme" />}
    {render.status === "ready" && <div className="space-y-2 border-t pt-2" data-theme-target-state={target}>
      {(target === "rejected" || target === "host_dependent") && <p className={target === "rejected" ? "leading-relaxed text-destructive" : "leading-relaxed text-muted-foreground"}
        role={target === "rejected" ? "alert" : undefined} data-testid="theme-target-notice">{t(`themePanel.targetNotice.${target}`)}</p>}
      <details data-testid="theme-target-details" className="space-y-2">
        <summary className="cursor-pointer font-medium">{t("themePanel.targetBoundary")}</summary>
        <p className="mt-2 leading-relaxed text-muted-foreground">{t(`themePanel.target.${target}`)}</p>
        {evidence && <p className="text-muted-foreground">{t("themePanel.observedTarget", { target: evidence.target_kind })}</p>}
        {evidence && <ThemeEvidenceReasons codes={evidence.target_reason_ids} kind="target" />}
      </details>
    </div>}
  </section>;
}
