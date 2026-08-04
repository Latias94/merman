import { useReducer, useState } from "react";

import {
  createBenchmarkDialogState,
  reduceBenchmarkDialogState,
} from "@/src/benchmark/dialog-state";
import { BenchDialog } from "./BenchDialog";

export function BenchWorkbench({
  open,
  onOpenChange,
  restoreFocus,
}: {
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
}) {
  const [dialogState, dispatchDialog] = useReducer(
    reduceBenchmarkDialogState,
    "idle",
    createBenchmarkDialogState,
  );
  const [runFingerprint, setRunFingerprint] = useState<string | null>(null);

  if (!open) return null;

  return (
    <BenchDialog
      dialogState={dialogState}
      dispatchDialog={dispatchDialog}
      open={open}
      onOpenChange={onOpenChange}
      restoreFocus={restoreFocus}
      runFingerprint={runFingerprint}
      setRunFingerprint={setRunFingerprint}
    />
  );
}
