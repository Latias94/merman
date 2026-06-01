import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Gauge, Loader2, Play, Square } from "lucide-react";
import { useAppStore } from "@/src/store";
import { useMerman } from "@/src/hooks/useMerman";
import {
  runLocalRenderBench,
  type BenchEngine,
  type BenchEngineStats,
  type BenchRunResult,
} from "@/src/lib/bench-runner";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

const DEFAULT_WARMUP = 5;
const DEFAULT_MEASURE = 30;

export function BenchDialog() {
  const { t } = useTranslation();
  const { code, diagramTheme, mermaidConfig } = useAppStore();
  const { ready, loading, render } = useMerman();
  const [open, setOpen] = useState(false);
  const [includeMerman, setIncludeMerman] = useState(true);
  const [includeMermaid, setIncludeMermaid] = useState(true);
  const [warmupIterations, setWarmupIterations] = useState(DEFAULT_WARMUP);
  const [measureIterations, setMeasureIterations] = useState(DEFAULT_MEASURE);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<BenchRunResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const engines = useMemo(() => {
    const selected: BenchEngine[] = [];
    if (includeMerman) selected.push("merman");
    if (includeMermaid) selected.push("mermaid");
    return selected;
  }, [includeMermaid, includeMerman]);

  const disabledReason = useMemo(() => {
    if (running) return null;
    if (loading || !ready) return t("bench.notReady");
    if (!code.trim()) return t("bench.empty");
    if (engines.length === 0) return t("bench.noEngine");
    return null;
  }, [code, engines.length, loading, ready, running, t]);

  const handleRun = useCallback(async () => {
    if (disabledReason || running) return;

    const controller = new AbortController();
    abortRef.current = controller;
    setRunning(true);
    setError(null);
    setResult(null);

    try {
      const nextResult = await runLocalRenderBench({
        source: code,
        theme: diagramTheme,
        configJson: mermaidConfig,
        engines,
        warmupIterations,
        measureIterations,
        renderMerman: (source, theme, configJson) =>
          render(source, theme, configJson),
        signal: controller.signal,
      });
      setResult(nextResult);
    } catch (err) {
      setError(
        err instanceof DOMException && err.name === "AbortError"
          ? t("bench.cancelled")
          : err instanceof Error
            ? err.message
            : String(err)
      );
    } finally {
      abortRef.current = null;
      setRunning(false);
    }
  }, [
    code,
    diagramTheme,
    mermaidConfig,
    disabledReason,
    engines,
    measureIterations,
    render,
    running,
    t,
    warmupIterations,
  ]);

  const handleCancel = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <DialogTrigger asChild>
            <Button variant="ghost" size="sm">
              <Gauge className="size-4" />
              <span className="hidden sm:inline">{t("toolbar.bench")}</span>
            </Button>
          </DialogTrigger>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.bench")}</TooltipContent>
      </Tooltip>
      <DialogContent className="max-h-[85vh] overflow-hidden sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>{t("bench.title")}</DialogTitle>
          <DialogDescription>{t("bench.description")}</DialogDescription>
        </DialogHeader>

        <div className="grid gap-4 overflow-y-auto pr-1">
          <div className="grid gap-3 rounded-md border bg-muted/20 p-3 md:grid-cols-[1.2fr_0.8fr_0.8fr_auto] md:items-end">
            <fieldset className="grid gap-2">
              <legend className="mb-1 text-sm font-medium">{t("bench.engines")}</legend>
              <label className="flex items-center gap-2 text-sm">
                <Checkbox
                  checked={includeMerman}
                  onCheckedChange={(checked: boolean | "indeterminate") =>
                    setIncludeMerman(checked === true)
                  }
                />
                {t("bench.merman")}
              </label>
              <label className="flex items-center gap-2 text-sm">
                <Checkbox
                  checked={includeMermaid}
                  onCheckedChange={(checked: boolean | "indeterminate") =>
                    setIncludeMermaid(checked === true)
                  }
                />
                {t("bench.mermaid")}
              </label>
            </fieldset>

            <NumberField
              label={t("bench.warmup")}
              value={warmupIterations}
              min={0}
              max={200}
              disabled={running}
              onChange={setWarmupIterations}
            />
            <NumberField
              label={t("bench.measure")}
              value={measureIterations}
              min={1}
              max={1000}
              disabled={running}
              onChange={setMeasureIterations}
            />

            <div className="flex items-center gap-2">
              <Button
                onClick={handleRun}
                disabled={Boolean(disabledReason) || running}
                className="min-w-24"
              >
                {running ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <Play className="size-4" />
                )}
                {running ? t("bench.running") : t("bench.run")}
              </Button>
              {running && (
                <Button variant="outline" size="icon" onClick={handleCancel}>
                  <Square className="size-4" />
                </Button>
              )}
            </div>
          </div>

          {disabledReason && !running && (
            <p className="text-sm text-muted-foreground">{disabledReason}</p>
          )}
          {error && <p className="text-sm text-destructive">{error}</p>}

          {result ? (
            <BenchResults result={result} t={t} />
          ) : (
            <div className="flex min-h-40 items-center justify-center rounded-md border border-dashed text-sm text-muted-foreground">
              {t("bench.noResults")}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  disabled,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  disabled: boolean;
  onChange(value: number): void;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <Input
        type="number"
        min={min}
        max={max}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          const nextValue = clampInteger(Number(event.target.value), min, max);
          onChange(nextValue);
        }}
      />
    </label>
  );
}

function BenchResults({
  result,
  t,
}: {
  result: BenchRunResult;
  t: (key: string) => string;
}) {
  const merman = result.results.find((entry) => entry.engine === "merman");
  const mermaid = result.results.find((entry) => entry.engine === "mermaid");
  const ratio =
    merman?.medianMs && mermaid?.medianMs ? mermaid.medianMs / merman.medianMs : null;

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
        <span>
          {t("bench.warmup")}: {result.warmupIterations}
        </span>
        <span>
          {t("bench.measure")}: {result.measureIterations}
        </span>
        {ratio !== null && (
          <span>
            {t("bench.ratio")}: {ratio.toFixed(2)}x
          </span>
        )}
      </div>

      <div className="overflow-x-auto rounded-md border">
        <table className="w-full min-w-[720px] text-sm">
          <thead className="bg-muted/50 text-xs text-muted-foreground">
            <tr>
              <HeaderCell>{t("bench.engine")}</HeaderCell>
              <HeaderCell>{t("bench.samples")}</HeaderCell>
              <HeaderCell>{t("bench.median")}</HeaderCell>
              <HeaderCell>{t("bench.p95")}</HeaderCell>
              <HeaderCell>{t("bench.mean")}</HeaderCell>
              <HeaderCell>{t("bench.min")}</HeaderCell>
              <HeaderCell>{t("bench.max")}</HeaderCell>
              <HeaderCell>{t("bench.errors")}</HeaderCell>
            </tr>
          </thead>
          <tbody>
            {result.results.map((entry) => (
              <ResultRow key={entry.engine} entry={entry} t={t} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function HeaderCell({ children }: { children: ReactNode }) {
  return <th className="px-3 py-2 text-left font-medium">{children}</th>;
}

function ResultRow({
  entry,
  t,
}: {
  entry: BenchEngineStats;
  t: (key: string) => string;
}) {
  return (
    <tr className="border-t">
      <td className="px-3 py-2 font-medium">
        {entry.engine === "merman" ? t("bench.merman") : t("bench.mermaid")}
      </td>
      <td className="px-3 py-2 tabular-nums">{entry.samples.length}</td>
      <td className="px-3 py-2 tabular-nums">{formatMs(entry.medianMs)}</td>
      <td className="px-3 py-2 tabular-nums">{formatMs(entry.p95Ms)}</td>
      <td className="px-3 py-2 tabular-nums">{formatMs(entry.meanMs)}</td>
      <td className="px-3 py-2 tabular-nums">{formatMs(entry.minMs)}</td>
      <td className="px-3 py-2 tabular-nums">{formatMs(entry.maxMs)}</td>
      <td
        className={cn(
          "px-3 py-2 tabular-nums",
          entry.errorCount > 0 && "text-destructive"
        )}
      >
        {entry.errorCount}
      </td>
    </tr>
  );
}

function formatMs(value: number | null): string {
  if (value === null || !Number.isFinite(value)) {
    return "-";
  }
  return `${value < 10 ? value.toFixed(2) : value.toFixed(1)}ms`;
}

function clampInteger(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.max(min, Math.min(max, Math.trunc(value)));
}
