import { useTranslation } from "react-i18next";
import { useAppStore, selectThemeDraftChanged } from "@/src/store";
import { selectVisibleMermanRenderMetadata, selectVisibleRenderBatch, useRenderCoordinator } from "@/src/runtime/use-render-coordinator";

export function ThemeRenderStatus() {
  const { t } = useTranslation();
  const session = useAppStore((state) => state.themeEditSession);
  const dirty = useAppStore(selectThemeDraftChanged);
  const renderStatus = useRenderCoordinator((state) => state.status);
  const batch = useRenderCoordinator(selectVisibleRenderBatch);
  const metadata = useRenderCoordinator(selectVisibleMermanRenderMetadata);
  const evidence = metadata?.theme_execution_evidence ?? null;
  const operation = batch?.snapshot.operation;
  const themed = Boolean(session || operation?.themePresetId || operation?.themeRecipeJson);
  if (!session && !themed) return null;
  return <div className="shrink-0 border-b bg-muted/20 px-3 py-1.5 text-xs" data-testid="theme-render-status">
    {session && <p role="status" className="text-muted-foreground">
      {session.validation === "invalid" ? t("themeWorkbench.invalidPreview") : dirty ? t("themeWorkbench.draftPreview") : t("themeWorkbench.appliedPreview")}
      {dirty && <> {t("themeWorkbench.applyFirst")}</>}
    </p>}
    <details className="mt-1">
      <summary className="cursor-pointer text-muted-foreground">{t("themeWorkbench.renderOutcome")}{renderStatus === "updating" && ` · ${t("preview.updatingStale")}`}</summary>
      <div className="mt-2 space-y-1 break-words [overflow-wrap:anywhere]">
        {!evidence ? <p>{t("themeWorkbench.evidenceUnavailable")}</p> : evidence.version !== 1 ? <p>{t("themeWorkbench.evidenceUnknown")}</p> : <>
          <p>{t("themeWorkbench.executionStatus")}: <code>{String(evidence.theme_status)}</code></p>
          <p>{t("themeWorkbench.targetStatus")}: <code>{String(evidence.target_kind)} · {String(evidence.target_status)}</code></p>
          <p>{t("themeWorkbench.fontSource")}: <code>{String(evidence.font_source)}</code></p>
          <p>{t("themeWorkbench.reasonIds")}: <code>{Array.isArray(evidence.target_reason_ids) && evidence.target_reason_ids.length ? evidence.target_reason_ids.join(", ") : t("themeWorkbench.noneReported")}</code></p>
        </>}
        <p className="text-muted-foreground">{t("themeWorkbench.evidenceExplanation")}</p>
        {evidence && <pre className="max-h-40 overflow-auto whitespace-pre-wrap text-[11px]">{JSON.stringify(evidence, null, 2)}</pre>}
      </div>
    </details>
  </div>;
}
