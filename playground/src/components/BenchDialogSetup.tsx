import { type ReactNode, type RefObject } from "react";
import { useTranslation } from "react-i18next";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { MERMAID_JS_VERSION } from "@/src/runtime/mermaid-requirements";
import { REALM_BUDGETS } from "@/src/runtime/realm/channel-protocol";

const ITERATION_OPTIONS = [2, 4, 6, 10, 20] as const;
const WARMUP_OPTIONS = [0, 1, 2, 3, 5] as const;

export function BenchmarkSetupView({
  code,
  facadeVersion,
  headingRef,
  iterations,
  mode,
  setIterations,
  setMode,
  setWarmups,
  warmups,
}: {
  code: string;
  facadeVersion: string | null;
  headingRef: RefObject<HTMLHeadingElement | null>;
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
          <h3
            ref={headingRef}
            id="benchmark-mode-label"
            tabIndex={-1}
            className="text-sm font-semibold outline-none"
          >
            {t("bench.mode")}
          </h3>
          <p className="text-muted-foreground mt-1 text-xs">
            {mode === "realm-cold"
              ? t("bench.realmColdDescription")
              : t("bench.warmDescription")}
          </p>
        </div>
        <div
          role="group"
          aria-label={t("bench.mode")}
          className="grid w-full grid-cols-2 rounded-md border bg-muted/30 p-1 sm:w-fit"
        >
          {(["realm-cold", "warm"] as const).map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={mode === value}
              onClick={() => setMode(value)}
              className={`min-h-10 rounded px-3 py-1.5 text-sm font-medium whitespace-normal transition-colors ${
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
            <SelectTrigger id="bench-iterations" className="h-10 w-full">
              <SelectValue>{iterations}</SelectValue>
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
            <SelectTrigger id="bench-warmups" className="h-10 w-full">
              <SelectValue>{warmups}</SelectValue>
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
        <h3 className="text-sm font-semibold">{t("bench.currentSource")}</h3>
        <pre className="bg-muted/40 rounded-md border p-3 font-mono text-xs whitespace-pre-wrap break-words">
          {code || t("bench.empty")}
        </pre>
      </section>
    </>
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

function formatBytes(value: number): string {
  return `${Math.round(value / (1024 * 1024))} MiB`;
}
