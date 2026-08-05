import { useReducer, useState } from "react";

import {
  createBenchmarkDialogState,
  reduceBenchmarkDialogState,
} from "@/src/benchmark/dialog-state";
import { createBrowserBenchmarkRuntime } from "@/src/benchmark/browser";
import { BenchDialog } from "./BenchDialog";

export function BenchWorkbench({
  open,
  onOpenChange,
  pauseCoordinator,
  restoreFocus,
}: {
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  pauseCoordinator(): Promise<() => void>;
  restoreFocus(): void;
}) {
  const [dialogState, dispatchDialog] = useReducer(
    reduceBenchmarkDialogState,
    "idle",
    createBenchmarkDialogState,
  );
  const [runFingerprint, setRunFingerprint] = useState<string | null>(null);
  const [runtime] = useState(() =>
    createBrowserBenchmarkRuntime(pauseCoordinator),
  );

  if (!open) return null;

  return (
    <BenchDialog
      dialogState={dialogState}
      dispatchDialog={dispatchDialog}
      benchmarkController={runtime.controller}
      benchmarkDocumentLifecycle={runtime.lifecycle}
      open={open}
      onOpenChange={onOpenChange}
      restoreFocus={restoreFocus}
      runFingerprint={runFingerprint}
      setRunFingerprint={setRunFingerprint}
    />
  );
}
