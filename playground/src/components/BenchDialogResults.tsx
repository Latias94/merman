import { useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, CheckCircle2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { BenchmarkControllerState } from "@/src/benchmark/controller";
import type {
  BenchmarkIntervalName,
  BenchmarkRecordedFailure,
  BenchmarkRecordedSample,
  BenchmarkReport,
} from "@/src/benchmark/report";
import {
  calculateBenchmarkStatistics,
  type BenchmarkStatistics,
} from "@/src/benchmark/statistics";

const COLD_METRICS = [
  "adapterImportMs",
  "engineImportMs",
  "resourceAcquisitionMs",
  "registrationMs",
  "initializationMs",
  "firstBudgetedSvgMs",
  "firstIsolatedPresentationMs",
  "isolatedPresentationReceiptMs",
  "responseDeliveryMs",
  "responseEnvelopeValidationMs",
  "strictSvgValidationMs",
  "firstPublishableSvgMs",
] as const satisfies readonly BenchmarkIntervalName[];
const WARM_METRICS = [
  "warmBudgetedSvgMs",
  "warmIsolatedPresentationMs",
  "isolatedPresentationReceiptMs",
  "responseDeliveryMs",
  "responseEnvelopeValidationMs",
  "strictSvgValidationMs",
  "warmPublishableSvgMs",
] as const satisfies readonly BenchmarkIntervalName[];
const SETUP_METRICS = [
  "artifactAcquisitionMs",
  "realmBootstrapMs",
  "totalMs",
] as const;
type SetupMetric = (typeof SETUP_METRICS)[number];

export function BenchmarkRunningView({
  state,
  elapsedMs,
  headingRef,
}: {
  state: Extract<BenchmarkControllerState, { status: "running" }>;
  elapsedMs: number;
  headingRef: RefObject<HTMLHeadingElement | null>;
}) {
  const { t } = useTranslation();
  const progress = state.progress;
  const percentage =
    progress.total === 0 ? 0 : (progress.completed / progress.total) * 100;
  return (
    <section className="space-y-5 py-4" aria-labelledby="benchmark-running-title">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3
            ref={headingRef}
            id="benchmark-running-title"
            tabIndex={-1}
            className="text-base font-semibold outline-none"
          >
            {t(`bench.stages.${progress.stage}`)}
          </h3>
          <p className="text-muted-foreground mt-1 text-sm">
            {progress.engine
              ? `${engineLabel(progress.engine)} · ${progress.purpose ? t(`bench.purposes.${progress.purpose}`) : ""}`
              : t("bench.preparing")}
          </p>
        </div>
        <span className="font-mono text-sm tabular-nums">
          {formatDuration(elapsedMs)}
        </span>
      </div>
      <div
        role="progressbar"
        aria-label={t("bench.progress")}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(percentage)}
        className="bg-primary/20 h-2 w-full overflow-hidden rounded-full"
      >
        <div
          className="bg-primary h-full transition-[width]"
          style={{ width: `${percentage}%` }}
        />
      </div>
      <div className="flex justify-between text-xs text-muted-foreground">
        <span>
          {t("bench.sampleProgress", {
            completed: progress.completed,
            total: progress.total,
          })}
        </span>
        {progress.blockIndex !== null && (
          <span>{t("bench.block", { block: progress.blockIndex + 1 })}</span>
        )}
      </div>
      {state.stale && (
        <div className="border-amber-500/40 bg-amber-500/10 rounded-md border px-3 py-2 text-sm">
          {t("bench.staleDescription")}
        </div>
      )}
    </section>
  );
}

export function BenchmarkReportView({
  report,
  headingRef,
}: {
  report: BenchmarkReport;
  headingRef: RefObject<HTMLHeadingElement | null>;
}) {
  const { t } = useTranslation();
  const metric: BenchmarkIntervalName =
    report.plan.mode === "realm-cold"
      ? "firstPublishableSvgMs"
      : "warmPublishableSvgMs";
  const metrics =
    report.plan.mode === "realm-cold" ? COLD_METRICS : WARM_METRICS;
  const merman = report.aggregates?.engines.merman[metric] ?? null;
  const mermaid = report.aggregates?.engines.mermaid[metric] ?? null;
  const ratio = report.aggregates?.ratios[metric] ?? null;
  const failures = report.samples.filter(
    (sample): sample is BenchmarkRecordedFailure => sample.outcome === "failure",
  );

  return (
    <>
      <section className="space-y-3">
        <div className="flex items-start gap-3">
          {report.terminalStatus === "success" ? (
            <CheckCircle2 className="mt-0.5 size-5 text-emerald-600" />
          ) : (
            <AlertTriangle className="mt-0.5 size-5 text-amber-600" />
          )}
          <div>
            <h3
              ref={headingRef}
              tabIndex={-1}
              className="text-base font-semibold outline-none"
            >
              {t(`bench.states.${report.terminalStatus}`)}
            </h3>
            <p className="text-muted-foreground mt-1 text-sm">
              {t("bench.completedSummary", {
                samples: report.samples.length,
                errors: failures.length,
                duration: formatDuration(report.run.durationMs),
              })}
            </p>
          </div>
        </div>
        {report.terminalError && (
          <BenchmarkFailureNotice
            detail={report.terminalError.detail}
            engine="Benchmark"
            message={report.terminalError.message}
            stage={report.terminalError.stage}
          />
        )}
      </section>

      {report.aggregates && (
        <>
          <Separator />
          <section className="space-y-3">
            <div>
              <h3 className="text-sm font-semibold">
                {t("bench.presentationReady")}
              </h3>
              <p className="text-muted-foreground mt-1 text-xs">
                {t(`bench.metricDescriptions.${metric}`)}
              </p>
            </div>
            <div className="grid gap-px overflow-hidden rounded-md border bg-border sm:grid-cols-3">
              <MetricSummary engine="Merman" statistics={merman} />
              <MetricSummary engine="Mermaid JS" statistics={mermaid} />
              <div className="bg-background px-4 py-3">
                <div className="text-muted-foreground text-xs">
                  {t("bench.ratio")}
                </div>
                <div className="mt-1 text-xl font-semibold tabular-nums">
                  {ratio === null
                    ? t("bench.unavailable")
                    : `${ratio.toFixed(2)}×`}
                </div>
              </div>
            </div>
          </section>

          <section className="space-y-2">
            <h3 className="text-sm font-semibold">{t("bench.statistics")}</h3>
            <StatisticsTable report={report} metrics={metrics} />
          </section>

          <section className="space-y-2">
            <div>
              <h3 className="text-sm font-semibold">
                {t("bench.setupEvidence")}
              </h3>
              <p className="text-muted-foreground mt-1 text-xs">
                {t("bench.setupEvidenceDescription")}
              </p>
            </div>
            <SetupEvidenceTable report={report} />
          </section>
        </>
      )}

      <Separator />

      {failures.length > 0 && (
        <section className="space-y-2" aria-labelledby="benchmark-failures-title">
          <div>
            <h3 id="benchmark-failures-title" className="text-sm font-semibold">
              {t("bench.failureEvidence")}
            </h3>
            <p className="mt-1 text-xs text-muted-foreground">
              {t("bench.failureEvidenceDescription")}
            </p>
          </div>
          <div className="space-y-2">
            {failures.map((sample) => (
              <BenchmarkFailureNotice
                key={sample.requestId}
                detail={sample.failure.detail}
                engine={engineLabel(sample.engine)}
                message={sample.failure.message}
                stage={sample.failure.stage}
              />
            ))}
          </div>
        </section>
      )}

      {failures.length > 0 && <Separator />}

      <section className="grid gap-3 text-xs sm:grid-cols-4">
        <EvidenceFact label={t("bench.runId")} value={report.run.id} />
        <EvidenceFact label={t("bench.seed")} value={String(report.plan.seed)} />
        <EvidenceFact
          label={t("bench.order")}
          value={report.plan.blocks
            .map((block) => (block.order[0] === "merman" ? "AB" : "BA"))
            .join(" · ")}
        />
        <EvidenceFact
          label={t("bench.versions")}
          value={`${report.versions.expected.merman} / ${report.versions.expected.mermaid}`}
        />
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-semibold">{t("bench.rawEvidence")}</h3>
        <p className="text-muted-foreground text-xs">
          {t("bench.rawEvidenceDescription")}
        </p>
        <div className="divide-y rounded-md border">
          {report.samples.map((sample, index) => (
            <RawSampleRow key={`${sample.requestId}-${index}`} sample={sample} />
          ))}
          {report.samples.length === 0 && (
            <div className="text-muted-foreground px-3 py-4 text-sm">
              {t("bench.noSamples")}
            </div>
          )}
        </div>
      </section>
    </>
  );
}

function SetupEvidenceTable({ report }: { report: BenchmarkReport }) {
  const { t } = useTranslation();
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{t("bench.metric")}</TableHead>
          <TableHead>{t("bench.engine")}</TableHead>
          <TableHead className="text-right">{t("bench.median")}</TableHead>
          <TableHead className="text-right">{t("bench.p95")}</TableHead>
          <TableHead className="text-right">{t("bench.mean")}</TableHead>
          <TableHead className="text-right">{t("bench.range")}</TableHead>
          <TableHead className="text-right">CV</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {SETUP_METRICS.flatMap((metric) =>
          (["merman", "mermaid"] as const).map((engine) => (
            <TableRow key={`${metric}-${engine}`}>
              <TableCell>{t(`bench.setupMetrics.${metric}`)}</TableCell>
              <TableCell>{engineLabel(engine)}</TableCell>
              <StatisticCells
                statistics={setupStatistics(report, engine, metric)}
              />
            </TableRow>
          )),
        )}
      </TableBody>
    </Table>
  );
}

function setupStatistics(
  report: BenchmarkReport,
  engine: "merman" | "mermaid",
  metric: SetupMetric,
): BenchmarkStatistics | null {
  const values = report.samples
    .filter(
      (sample) => sample.engine === engine && sample.realmCreation !== null,
    )
    .map((sample) => sample.realmCreation?.[metric])
    .filter((value): value is number => value !== undefined);
  return values.length === 0 ? null : calculateBenchmarkStatistics(values);
}

function StatisticsTable({
  report,
  metrics,
}: {
  report: BenchmarkReport;
  metrics: readonly BenchmarkIntervalName[];
}) {
  const { t } = useTranslation();
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{t("bench.metric")}</TableHead>
          <TableHead>{t("bench.engine")}</TableHead>
          <TableHead className="text-right">{t("bench.median")}</TableHead>
          <TableHead className="text-right">{t("bench.p95")}</TableHead>
          <TableHead className="text-right">{t("bench.mean")}</TableHead>
          <TableHead className="text-right">{t("bench.range")}</TableHead>
          <TableHead className="text-right">CV</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {metrics.flatMap((metric) =>
          (["merman", "mermaid"] as const).map((engine) => {
            const value = report.aggregates?.engines[engine][metric] ?? null;
            return (
              <TableRow key={`${metric}-${engine}`}>
                <TableCell>{t(`bench.metrics.${metric}`)}</TableCell>
                <TableCell>{engineLabel(engine)}</TableCell>
                <StatisticCells statistics={value} />
              </TableRow>
            );
          }),
        )}
      </TableBody>
    </Table>
  );
}

function StatisticCells({
  statistics,
}: {
  statistics: BenchmarkStatistics | null;
}) {
  if (!statistics) {
    return (
      <TableCell colSpan={5} className="text-muted-foreground text-right">
        —
      </TableCell>
    );
  }
  return (
    <>
      <TableCell className="text-right tabular-nums">
        {formatMilliseconds(statistics.median)}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {formatMilliseconds(statistics.p95)}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {formatMilliseconds(statistics.mean)}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {formatMilliseconds(statistics.min)}–{formatMilliseconds(statistics.max)}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {(statistics.coefficientOfVariation * 100).toFixed(1)}%
      </TableCell>
    </>
  );
}

function MetricSummary({
  engine,
  statistics,
}: {
  engine: string;
  statistics: BenchmarkStatistics | null;
}) {
  const { t } = useTranslation();
  return (
    <div className="bg-background px-4 py-3">
      <div className="text-muted-foreground text-xs">{engine}</div>
      <div className="mt-1 text-xl font-semibold tabular-nums">
        {statistics
          ? formatMilliseconds(statistics.median)
          : t("bench.unavailable")}
      </div>
      {statistics && (
        <div className="text-muted-foreground mt-1 text-xs">
          {t("bench.samples", { count: statistics.count })}
        </div>
      )}
    </div>
  );
}

function RawSampleRow({ sample }: { sample: BenchmarkRecordedSample }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <details
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}
      className="group"
    >
      <summary className="focus-visible:ring-ring flex cursor-pointer list-none items-center justify-between gap-3 px-3 py-2 text-sm outline-none focus-visible:ring-2">
        <span className="min-w-0 truncate">
          {engineLabel(sample.engine)} · {t(`bench.purposes.${sample.purpose}`)} ·{" "}
          {sample.requestId}
        </span>
        <Badge
          variant={sample.outcome === "success" ? "secondary" : "destructive"}
        >
          {t(`bench.sampleOutcomes.${sample.outcome}`)}
        </Badge>
      </summary>
      {open && (
        <pre className="bg-muted/30 border-t p-3 font-mono text-[11px] whitespace-pre-wrap break-all">
          {JSON.stringify(sample, null, 2)}
        </pre>
      )}
    </details>
  );
}

export function BenchmarkFailureNotice({
  detail,
  engine,
  message,
  stage,
}: {
  detail: string | null;
  engine: string;
  message: string;
  stage: string;
}) {
  const { t } = useTranslation();
  return (
    <div
      role="alert"
      className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm"
      data-merman-benchmark-error-engine={engine}
      data-merman-benchmark-error-stage={stage}
    >
      <p className="font-medium text-destructive">
        {engine} · <span className="font-mono text-xs">{stage}</span>
      </p>
      <p className="mt-1 break-words font-mono text-xs text-foreground">
        {message}
      </p>
      {detail && (
        <details className="mt-2 text-xs text-muted-foreground">
          <summary className="cursor-pointer select-none">
            {t("preview.errorDetails")}
          </summary>
          <pre className="mt-2 whitespace-pre-wrap break-words rounded bg-muted/50 p-2 font-mono">
            {detail}
          </pre>
        </details>
      )}
    </div>
  );
}

export function BenchmarkStatusBadge({
  status,
}: {
  status: "running" | BenchmarkReport["terminalStatus"] | null;
}) {
  const { t } = useTranslation();
  if (!status) return null;
  const destructive = status === "failed" || status === "invalidated";
  return (
    <Badge variant={destructive ? "destructive" : "secondary"}>
      {status === "running"
        ? t("bench.running")
        : t(`bench.states.${status}`)}
    </Badge>
  );
}

function EvidenceFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-muted-foreground">{label}</div>
      <div className="mt-1 truncate font-medium" title={value}>
        {value}
      </div>
    </div>
  );
}

function engineLabel(engine: "merman" | "mermaid"): string {
  return engine === "merman" ? "Merman" : "Mermaid JS";
}

function formatMilliseconds(value: number): string {
  return `${value.toFixed(value < 10 ? 2 : 1)} ms`;
}

function formatDuration(value: number): string {
  return value < 1_000
    ? `${Math.round(value)} ms`
    : `${(value / 1_000).toFixed(1)} s`;
}
