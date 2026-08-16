import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
} from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "zustand";
import { useShallow } from "zustand/react/shallow";
import {
  ArrowLeft,
  Download,
  Play,
  RotateCcw,
  Settings2,
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
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useAppStore } from "@/src/store";
import type {
  BenchmarkDialogAction,
  BenchmarkDialogState,
} from "@/src/benchmark/dialog-state";
import { downloadBenchmarkReport } from "@/src/benchmark/report";
import type {
  BenchmarkController,
  BenchmarkControllerState,
  BenchmarkRunRequest,
} from "@/src/benchmark/controller";
import type { BenchmarkDocumentLifecycle } from "@/src/benchmark/document-lifecycle";
import {
  MERMAID_JS_VERSION,
  mermaidExternalRequirementsFor,
} from "@/src/runtime/mermaid-requirements";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { CANONICAL_RENDER_VIEWPORT } from "@/src/runtime/render-viewport";
import {
  projectError,
  type ErrorProjection,
} from "@/src/runtime/error-projection";
import { configuredMermanOperationInput } from "@/src/runtime/merman-operation-input";
import {
  BenchmarkFailureNotice,
  BenchmarkReportView,
  BenchmarkRunningView,
  BenchmarkStatusBadge,
} from "@/src/components/BenchDialogResults";
import { BenchmarkSetupView } from "@/src/components/BenchDialogSetup";

export function BenchDialog({
  benchmarkController,
  benchmarkDocumentLifecycle,
  dialogState,
  dispatchDialog,
  open,
  onOpenChange,
  restoreFocus,
  runFingerprint,
  setRunFingerprint,
}: {
  readonly benchmarkController: BenchmarkController;
  readonly benchmarkDocumentLifecycle: BenchmarkDocumentLifecycle;
  readonly dialogState: BenchmarkDialogState;
  readonly dispatchDialog: Dispatch<BenchmarkDialogAction>;
  readonly open: boolean;
  readonly runFingerprint: string | null;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
  setRunFingerprint(fingerprint: string): void;
}) {
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
  const [visible, setVisible] = useState(
    () => benchmarkDocumentLifecycle.getVisibilityState() === "visible"
  );
  const [runError, setRunError] = useState<ErrorProjection | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const phaseHeadingRef = useRef<HTMLHeadingElement>(null);

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
  const report = state.retained?.report ?? null;
  const reportId = report?.run.id ?? null;
  const phase = running
    ? "running"
    : dialogState.phase === "running"
      ? report
        ? "report"
        : "configure"
      : dialogState.phase === "report" && !report
        ? "configure"
        : dialogState.phase;
  const { iterations, mode, warmups } = dialogState.draft;
  const canRun = Boolean(facade && code.trim() && visible && !running);

  useEffect(() => {
    return benchmarkDocumentLifecycle.subscribe((signal) => {
      setVisible(signal.visibilityState === "visible");
    });
  }, [benchmarkDocumentLifecycle]);

  useEffect(() => {
    if (!runFingerprint || fingerprint === runFingerprint) return;
    if (state.status !== "idle") benchmarkController.markStale();
  }, [benchmarkController, fingerprint, runFingerprint, state.status]);

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

  useEffect(() => {
    if (!open) return;
    const frame = window.requestAnimationFrame(() => {
      phaseHeadingRef.current?.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, phase]);

  useEffect(
    () => () => {
      if (benchmarkController.store.getState().status === "running") {
        benchmarkController.cancel("dialog-unmounted");
      }
    },
    [benchmarkController],
  );

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (
        !nextOpen &&
        benchmarkController.store.getState().status === "running"
      ) {
        benchmarkController.cancel("dialog-closed");
      }
      onOpenChange(nextOpen);
    },
    [benchmarkController, onOpenChange],
  );

  const handleRun = useCallback(() => {
    if (!facade || !code.trim() || !visible) return;
    setRunError(null);
    setElapsedMs(0);
    const rejectRun = (error: unknown) => {
      setRunError(projectError(error));
    };
    try {
      const options = {
        diagramFont,
        textMeasurementMode,
      } as const;
      const detection = facade.detectDiagram(
        configuredMermanOperationInput(
          code,
          diagramTheme,
          mermaidConfig,
          options,
        ),
      );
      const commonRequest = {
        iterations,
        payload: {
          source: code,
          configJson: mermaidConfig,
          theme: diagramTheme,
          diagramFont,
          externalRequirements: mermaidExternalRequirementsFor(detection),
          screenAvailableWidth: window.screen.availWidth,
          viewport: CANONICAL_RENDER_VIEWPORT,
        },
        detection,
        versions: {
          merman: facade.packageVersion,
          mermaid: MERMAID_JS_VERSION,
        },
      } satisfies Omit<BenchmarkRunRequest, "mode" | "warmups">;
      const request: BenchmarkRunRequest =
        mode === "warm"
          ? { ...commonRequest, mode: "warm", warmups }
          : { ...commonRequest, mode: "realm-cold", warmups: 0 };
      const run = benchmarkController.start(request);
      const startedState = benchmarkController.store.getState();
      setRunFingerprint(fingerprint);
      dispatchDialog({
        type: "run-started",
        runId: run.runId,
        retainedReportId:
          startedState.status === "running"
            ? (startedState.retained?.report.run.id ?? null)
            : null,
      });
      void run.completion.then(
        () => {
          const settledState = benchmarkController.store.getState();
          const retainedReportId = settledState.retained?.report.run.id;
          if (!retainedReportId) {
            dispatchDialog({ type: "run-rejected", runId: run.runId });
            return;
          }
          dispatchDialog({
            type: "run-settled",
            reportId: retainedReportId,
            runId: run.runId,
          });
        },
        (error: unknown) => {
          rejectRun(error);
          dispatchDialog({ type: "run-rejected", runId: run.runId });
        },
      );
    } catch (error) {
      rejectRun(error);
    }
  }, [
    code,
    benchmarkController,
    diagramFont,
    diagramTheme,
    dispatchDialog,
    facade,
    fingerprint,
    iterations,
    mermaidConfig,
    mode,
    setRunFingerprint,
    textMeasurementMode,
    visible,
    warmups,
  ]);

  const cancellation =
    state.status !== "idle" && state.status !== "running"
      ? state.cancellation
      : null;
  const liveAnnouncement = running
    ? t(`bench.stages.${state.progress.stage}`)
    : cancellation
      ? t("bench.cancelledShowingPrevious")
      : report
        ? t(`bench.states.${report.terminalStatus}`)
        : "";

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        showCloseButton={false}
        className="grid grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          restoreFocus();
        }}
        style={{
          maxHeight: "min(56rem, 100%)",
          width: "min(56rem, 100%)",
          maxWidth: "none",
        }}
      >
        <DialogHeader className="relative border-b px-5 py-3 pr-14 text-left sm:px-6 sm:py-4">
          <div className="flex flex-wrap items-center gap-2">
            <DialogTitle>{t("bench.title")}</DialogTitle>
            <BenchmarkStatusBadge
              status={running ? "running" : (report?.terminalStatus ?? null)}
            />
            {state.stale && (
              <Badge variant="outline">{t("bench.stale")}</Badge>
            )}
          </div>
          <DialogClose asChild>
            <Button
              variant="ghost"
              size="icon"
              className="absolute top-2 right-3 size-10 sm:top-2.5 sm:right-4"
              aria-label={t("bench.close")}
            >
              <X className="size-4" />
            </Button>
          </DialogClose>
        </DialogHeader>

        <ScrollArea className="min-h-0 overscroll-contain">
          <div className="space-y-6 px-4 py-5 sm:px-6">
            <div aria-live="polite" className="sr-only">
              {liveAnnouncement}
            </div>

            <DialogDescription className="text-left">
              {t("bench.description")}
            </DialogDescription>

            {cancellation && (
              <div
                role="status"
                className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-sm"
              >
                {t("bench.cancelledShowingPrevious")}
              </div>
            )}

            {phase === "running" && running ? (
              <BenchmarkRunningView
                state={state}
                elapsedMs={elapsedMs}
                headingRef={phaseHeadingRef}
              />
            ) : phase === "report" && report ? (
              <BenchmarkReportView
                report={report}
                headingRef={phaseHeadingRef}
              />
            ) : (
              <BenchmarkSetupView
                code={code}
                facadeVersion={facade?.packageVersion ?? null}
                iterations={iterations}
                headingRef={phaseHeadingRef}
                mode={mode}
                setIterations={(value) =>
                  dispatchDialog({
                    type: "update-draft",
                    draft: { iterations: value },
                  })
                }
                setMode={(value) =>
                  dispatchDialog({
                    type: "update-draft",
                    draft: { mode: value },
                  })
                }
                setWarmups={(value) =>
                  dispatchDialog({
                    type: "update-draft",
                    draft: { warmups: value },
                  })
                }
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

        <DialogFooter className="flex-col border-t bg-muted/20 px-4 pt-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] min-[30rem]:flex-row min-[30rem]:flex-wrap min-[30rem]:justify-end sm:px-6">
          {phase === "running" && running ? (
            <Button
              variant="destructive"
              className="min-h-10 w-full min-[30rem]:w-auto"
              onClick={() => benchmarkController.cancel("user")}
            >
              <Square className="size-4" />
              {t("bench.cancel")}
            </Button>
          ) : (
            <>
              {phase === "configure" && report && (
                <Button
                  variant="outline"
                  className="min-h-10 w-full min-[30rem]:w-auto"
                  onClick={() => {
                    if (reportId) {
                      dispatchDialog({
                        type: "back-to-report",
                        reportId,
                      });
                    }
                  }}
                >
                  <ArrowLeft className="size-4" />
                  {t("bench.backToResult")}
                </Button>
              )}
              {phase === "report" && report && (
                <Button
                  variant="outline"
                  className="min-h-10 w-full min-[30rem]:w-auto"
                  onClick={() => dispatchDialog({ type: "change-settings" })}
                >
                  <Settings2 className="size-4" />
                  {t("bench.changeSettings")}
                </Button>
              )}
              {phase === "report" && report && (
                <Button
                  variant="outline"
                  className="min-h-10 w-full min-[30rem]:w-auto"
                  onClick={() => downloadBenchmarkReport(report)}
                >
                  <Download className="size-4" />
                  {t("bench.download")}
                </Button>
              )}
              <Button
                variant="secondary"
                className="min-h-10 w-full min-[30rem]:w-auto"
                onClick={handleRun}
                disabled={!canRun}
              >
                {phase === "report" && report ? (
                  <RotateCcw className="size-4" />
                ) : (
                  <Play className="size-4" />
                )}
                {phase === "report" && report
                  ? t("bench.runAgain")
                  : t("bench.run")}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
