import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "zustand";
import { useShallow } from "zustand/react/shallow";
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  Gauge,
  Play,
  RotateCcw,
  Square,
  X,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useAppStore } from "@/src/store";
import { benchmarkController } from "@/src/benchmark/browser";
import { downloadBenchmarkReport } from "@/src/benchmark/report";
import type {
  BenchmarkControllerState,
  BenchmarkRunRequest,
} from "@/src/benchmark/controller";
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
import {
  MERMAID_JS_VERSION,
  mermaidExternalRequirementsFor,
} from "@/src/runtime/mermaid-requirements";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { REALM_BUDGETS } from "@/src/runtime/realm/channel-protocol";
import { PLAYGROUND_RENDER_VIEWPORT } from "@/src/runtime/render-viewport";
import {
  projectError,
  type ErrorProjection,
} from "@/src/runtime/error-projection";

const ITERATION_OPTIONS = [2, 4, 6, 10, 20] as const;
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
const WARMUP_OPTIONS = [0, 1, 2, 3, 5] as const;

export function BenchDialog() {
  const { t } = useTranslation();
  const state = useStore(
    benchmarkController.store,
    (current: BenchmarkControllerState) => current
  );
  const {
    code,
    diagramFont,
    diagramTheme,
    mermaidConfig,
    textMeasurementMode,
  } = useAppStore(
    useShallow((current) => ({
      code: current.code,
      diagramFont: current.diagramFont,
      diagramTheme: current.diagramTheme,
      mermaidConfig: current.mermaidConfig,
      textMeasurementMode: current.textMeasurementMode,
    }))
  );
  const facade = useMermanRuntime(selectMermanFacade);
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<"realm-cold" | "warm">("realm-cold");
  const [iterations, setIterations] = useState(6);
  const [warmups, setWarmups] = useState(2);
  const [visible, setVisible] = useState(
    () => document.visibilityState === "visible"
  );
  const [runFingerprint, setRunFingerprint] = useState<string | null>(null);
  const [runError, setRunError] = useState<ErrorProjection | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);

  const fingerprint = useMemo(
    () =>
      JSON.stringify([
        code,
        diagramTheme,
        mermaidConfig,
        diagramFont,
        textMeasurementMode,
      ]),
    [
      code,
      diagramFont,
      diagramTheme,
      mermaidConfig,
      textMeasurementMode,
    ]
  );
  const running = state.status === "running";
  const report = state.status === "idle" || running ? null : state.report;
  const canRun = Boolean(facade && code.trim() && visible && !running);

  useEffect(() => {
    const updateVisibility = () =>
      setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", updateVisibility);
    return () =>
      document.removeEventListener("visibilitychange", updateVisibility);
  }, []);

  useEffect(() => {
    if (!runFingerprint || fingerprint === runFingerprint) return;
    if (state.status !== "idle") benchmarkController.markStale();
  }, [fingerprint, runFingerprint, state.status]);

  useEffect(() => {
    if (!running) return;
    const startedAt = Date.now();
    setElapsedMs(0);
    const timer = window.setInterval(
      () => setElapsedMs(Date.now() - startedAt),
      250
    );
    return () => window.clearInterval(timer);
  }, [running]);

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    if (!nextOpen && benchmarkController.store.getState().status === "running") {
      benchmarkController.cancel("dialog-closed");
    }
    setOpen(nextOpen);
  }, []);

  const handleRun = useCallback(() => {
    if (!facade || !code.trim() || !visible) return;
    setRunError(null);
    setRunFingerprint(fingerprint);
    setElapsedMs(0);
    const options = {
      diagramFont,
      textMeasurementMode,
    } as const;
    const detection = facade.detectDiagram(
      code,
      diagramTheme,
      mermaidConfig,
      options
    );
    const request: BenchmarkRunRequest = {
      mode,
      iterations,
      warmups: mode === "warm" ? warmups : 0,
      payload: {
        source: code,
        configJson: mermaidConfig,
        theme: diagramTheme,
        diagramFont,
        externalRequirements: mermaidExternalRequirementsFor(detection),
        viewport: PLAYGROUND_RENDER_VIEWPORT,
      },
      detection,
      versions: {
        merman: facade.packageVersion,
        mermaid: MERMAID_JS_VERSION,
      },
    };
    void benchmarkController.run(request).catch((error: unknown) => {
      setRunError(projectError(error));
    });
  }, [
    code,
    diagramFont,
    diagramTheme,
    facade,
    fingerprint,
    iterations,
    mermaidConfig,
    mode,
    textMeasurementMode,
    visible,
    warmups,
  ]);

  const liveAnnouncement = running
    ? t(`bench.stages.${state.progress.stage}`)
    : state.status === "idle"
      ? ""
      : t(`bench.states.${state.status}`);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <Tooltip>
        <TooltipTrigger asChild>
          <DialogTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              aria-label={t("toolbar.bench")}
              className="w-8 px-0 lg:w-auto lg:px-2.5"
            >
              <Gauge className="size-4" />
              <span className="hidden lg:inline">{t("toolbar.bench")}</span>
            </Button>
          </DialogTrigger>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.bench")}</TooltipContent>
      </Tooltip>

      <DialogContent
        showCloseButton={false}
        className="grid max-h-[min(90dvh,56rem)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0"
        style={{
          width: "min(56rem, calc(100vw - 1.5rem))",
          maxWidth: "none",
        }}
      >
        <DialogHeader className="relative border-b px-5 py-4 pr-14 text-left sm:px-6">
          <div className="flex flex-wrap items-center gap-2">
            <DialogTitle>{t("bench.title")}</DialogTitle>
            <StatusBadge state={state} />
            {state.stale && (
              <Badge variant="outline">{t("bench.stale")}</Badge>
            )}
          </div>
          <DialogDescription>{t("bench.description")}</DialogDescription>
          <DialogClose asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              className="absolute right-4 top-4"
              aria-label={t("bench.close")}
            >
              <X className="size-4" />
            </Button>
          </DialogClose>
        </DialogHeader>

        <ScrollArea className="min-h-0">
          <div className="space-y-6 px-5 py-5 sm:px-6">
            <div aria-live="polite" className="sr-only">
              {liveAnnouncement}
            </div>

            {running ? (
              <RunningView state={state} elapsedMs={elapsedMs} />
            ) : report ? (
              <ReportView report={report} />
            ) : (
              <PreRunView
                code={code}
                facadeVersion={facade?.packageVersion ?? null}
                iterations={iterations}
                mode={mode}
                setIterations={setIterations}
                setMode={setMode}
                setWarmups={setWarmups}
                warmups={warmups}
              />
            )}

            {runError && (
              <BenchmarkFailureNotice
                detail={runError.detail}
                engine="Benchmark"
                message={runError.summary}
                stage="controller"
              />
            )}
          </div>
        </ScrollArea>

        <DialogFooter className="border-t bg-muted/20 px-5 py-3 sm:px-6">
          {running ? (
            <Button
              variant="destructive"
              onClick={() => benchmarkController.cancel("user")}
            >
              <Square className="size-4" />
              {t("bench.cancel")}
            </Button>
          ) : (
            <>
              {report && (
                <Button
                  variant="outline"
                  onClick={() => downloadBenchmarkReport(report)}
                >
                  <Download className="size-4" />
                  {t("bench.download")}
                </Button>
              )}
              <Button
                variant="secondary"
                onClick={handleRun}
                disabled={!canRun}
              >
                {report ? (
                  <RotateCcw className="size-4" />
                ) : (
                  <Play className="size-4" />
                )}
                {report ? t("bench.runAgain") : t("bench.run")}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PreRunView({
  code,
  facadeVersion,
  iterations,
  mode,
  setIterations,
  setMode,
  setWarmups,
  warmups,
}: {
  code: string;
  facadeVersion: string | null;
  iterations: number;
  mode: "realm-cold" | "warm";
  setIterations(value: number): void;
  setMode(value: "realm-cold" | "warm"): void;
  setWarmups(value: number): void;
  warmups: number;
}) {
  const { t } = useTranslation();
  return (
    <>
      <section className="space-y-3" aria-labelledby="benchmark-mode-label">
        <div>
          <h3 id="benchmark-mode-label" className="text-sm font-semibold">
            {t("bench.mode")}
          </h3>
          <p className="text-muted-foreground mt-1 text-xs">
            {mode === "realm-cold"
              ? t("bench.realmColdDescription")
              : t("bench.warmDescription")}
          </p>
        </div>
        <div className="inline-flex rounded-md border bg-muted/30 p-1">
          {(["realm-cold", "warm"] as const).map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={mode === value}
              onClick={() => setMode(value)}
              className={`rounded px-3 py-1.5 text-sm font-medium transition-colors ${
                mode === value
                  ? "bg-foreground text-background shadow-sm"
                  : "text-foreground hover:bg-background/70"
              }`}
            >
              {t(`bench.modes.${value}`)}
            </button>
          ))}
        </div>
      </section>

      <Separator />

      <section className="grid gap-4 sm:grid-cols-2">
        <ControlField label={t("bench.iterations")} htmlFor="bench-iterations">
          <Select
            value={String(iterations)}
            onValueChange={(value) => setIterations(Number(value))}
          >
            <SelectTrigger id="bench-iterations" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {ITERATION_OPTIONS.map((value) => (
                <SelectItem key={value} value={String(value)}>
                  {value}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </ControlField>
        <ControlField label={t("bench.warmups")} htmlFor="bench-warmups">
          <Select
            value={String(warmups)}
            disabled={mode !== "warm"}
            onValueChange={(value) => setWarmups(Number(value))}
          >
            <SelectTrigger id="bench-warmups" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {WARMUP_OPTIONS.map((value) => (
                <SelectItem key={value} value={String(value)}>
                  {value}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </ControlField>
      </section>

      <section className="grid gap-3 border-y bg-muted/20 px-4 py-3 text-xs sm:grid-cols-3">
        <EvidenceFact label={t("bench.engines")} value="Merman / Mermaid JS" />
        <EvidenceFact
          label={t("bench.versions")}
          value={`${facadeVersion ?? t("bench.notReady")} / ${MERMAID_JS_VERSION}`}
        />
        <EvidenceFact
          label={t("bench.resourceBounds")}
          value={`${formatBytes(REALM_BUDGETS.sourceBytes)} / ${formatBytes(REALM_BUDGETS.svgBytes)}`}
        />
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-semibold">{t("bench.frozenSource")}</h3>
        <pre className="bg-muted/40 max-h-36 overflow-auto rounded-md border p-3 font-mono text-xs whitespace-pre-wrap break-words">
          {code || t("bench.empty")}
        </pre>
      </section>
    </>
  );
}

function RunningView({
  state,
  elapsedMs,
}: {
  state: Extract<BenchmarkControllerState, { status: "running" }>;
  elapsedMs: number;
}) {
  const { t } = useTranslation();
  const progress = state.progress;
  const percentage =
    progress.total === 0 ? 0 : (progress.completed / progress.total) * 100;
  return (
    <section className="space-y-5 py-4" aria-labelledby="benchmark-running-title">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 id="benchmark-running-title" className="text-base font-semibold">
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

function ReportView({ report }: { report: BenchmarkReport }) {
  const { t } = useTranslation();
  const metric: BenchmarkIntervalName =
    report.run.mode === "realm-cold"
      ? "firstPublishableSvgMs"
      : "warmPublishableSvgMs";
  const metrics =
    report.run.mode === "realm-cold" ? COLD_METRICS : WARM_METRICS;
  const merman = report.aggregates?.engines.merman[metric] ?? null;
  const mermaid = report.aggregates?.engines.mermaid[metric] ?? null;
  const ratio = report.aggregates?.ratios[metric] ?? null;
  const failures = report.samples.filter(
    (sample): sample is BenchmarkRecordedFailure => sample.outcome === "failure"
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
            <h3 className="text-base font-semibold">
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
                  {ratio === null ? t("bench.unavailable") : `${ratio.toFixed(2)}×`}
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
        <EvidenceFact label={t("bench.seed")} value={String(report.run.seed)} />
        <EvidenceFact
          label={t("bench.order")}
          value={report.schedule.blocks
            .map((block) =>
              block.order[0] === "merman" ? "AB" : "BA"
            )
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
          ))
        )}
      </TableBody>
    </Table>
  );
}

function setupStatistics(
  report: BenchmarkReport,
  engine: "merman" | "mermaid",
  metric: SetupMetric
): BenchmarkStatistics | null {
  const values = report.samples
    .filter(
      (sample) => sample.engine === engine && sample.realmCreation !== null
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
          })
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
          {engineLabel(sample.engine)} · {t(`bench.purposes.${sample.purpose}`)} · {sample.requestId}
        </span>
        <Badge variant={sample.outcome === "success" ? "secondary" : "destructive"}>
          {t(`bench.sampleOutcomes.${sample.outcome}`)}
        </Badge>
      </summary>
      {open && (
        <pre className="bg-muted/30 max-h-72 overflow-auto border-t p-3 font-mono text-[11px] whitespace-pre-wrap break-all">
          {JSON.stringify(sample, null, 2)}
        </pre>
      )}
    </details>
  );
}

function BenchmarkFailureNotice({
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
          <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded bg-muted/50 p-2 font-mono">
            {detail}
          </pre>
        </details>
      )}
    </div>
  );
}

function StatusBadge({ state }: { state: BenchmarkControllerState }) {
  const { t } = useTranslation();
  if (state.status === "idle") return null;
  const destructive =
    state.status === "failed" || state.status === "invalidated";
  return (
    <Badge variant={destructive ? "destructive" : "secondary"}>
      {state.status === "running"
        ? t("bench.running")
        : t(`bench.states.${state.status}`)}
    </Badge>
  );
}

function ControlField({
  children,
  htmlFor,
  label,
}: {
  children: ReactNode;
  htmlFor: string;
  label: string;
}) {
  return (
    <div className="space-y-2">
      <label htmlFor={htmlFor} className="text-sm font-medium">
        {label}
      </label>
      {children}
    </div>
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

function formatBytes(value: number): string {
  return `${Math.round(value / (1024 * 1024))} MiB`;
}
