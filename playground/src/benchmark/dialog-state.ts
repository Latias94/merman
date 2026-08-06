import type { BenchmarkControllerState } from "./controller.ts";

export type BenchmarkDialogPhase = "configure" | "running" | "report";

export interface BenchmarkDialogDraft {
  readonly iterations: number;
  readonly mode: "realm-cold" | "warm";
  readonly warmups: number;
}

export interface BenchmarkDialogState {
  readonly activeRunId: string | null;
  readonly draft: BenchmarkDialogDraft;
  readonly phase: BenchmarkDialogPhase;
  readonly reportId: string | null;
}

export interface BenchmarkRetainedDialogSnapshot {
  readonly draft: BenchmarkDialogDraft;
  readonly id: string;
}

export type BenchmarkDialogAction =
  | {
      readonly draft: Partial<BenchmarkDialogDraft>;
      readonly type: "update-draft";
    }
  | {
      readonly retainedReportId: string | null;
      readonly runId: string;
      readonly type: "run-started";
    }
  | {
      readonly reportId: string;
      readonly runId: string;
      readonly type: "run-settled";
    }
  | { readonly runId: string; readonly type: "run-rejected" }
  | { readonly type: "change-settings" }
  | { readonly reportId: string; readonly type: "back-to-report" };

const DEFAULT_DRAFT: BenchmarkDialogDraft = Object.freeze({
  mode: "realm-cold",
  iterations: 6,
  warmups: 2,
});

export function createBenchmarkDialogState(
  status: BenchmarkControllerState["status"],
  retained: BenchmarkRetainedDialogSnapshot | null = null,
  activeRunId: string | null = null,
): BenchmarkDialogState {
  const draft = retained?.draft ?? DEFAULT_DRAFT;
  if (status === "running") {
    return freezeState("running", retained?.id ?? null, activeRunId, draft);
  }
  if (status !== "idle" && retained) {
    return freezeState("report", retained.id, null, draft);
  }
  return freezeState("configure", null, null, draft);
}

export function reduceBenchmarkDialogState(
  state: BenchmarkDialogState,
  action: BenchmarkDialogAction,
): BenchmarkDialogState {
  switch (action.type) {
    case "update-draft": {
      if (state.phase !== "configure") return state;
      const draft = Object.freeze({ ...state.draft, ...action.draft });
      if (
        draft.mode === state.draft.mode &&
        draft.iterations === state.draft.iterations &&
        draft.warmups === state.draft.warmups
      ) {
        return state;
      }
      return freezeState(
        state.phase,
        state.reportId,
        state.activeRunId,
        draft,
      );
    }
    case "run-started":
      return freezeState(
        "running",
        action.retainedReportId,
        action.runId,
        state.draft,
      );
    case "run-settled":
      return state.activeRunId === action.runId
        ? freezeState("report", action.reportId, null, state.draft)
        : state;
    case "run-rejected":
      return state.activeRunId === action.runId
        ? freezeState(
            state.reportId ? "report" : "configure",
            state.reportId,
            null,
            state.draft,
          )
        : state;
    case "change-settings":
      return state.phase === "report" && state.reportId
        ? freezeState("configure", state.reportId, null, state.draft)
        : state;
    case "back-to-report":
      return state.phase === "configure" && state.reportId === action.reportId
        ? freezeState("report", state.reportId, null, state.draft)
        : state;
  }
}

function freezeState(
  phase: BenchmarkDialogPhase,
  reportId: string | null,
  activeRunId: string | null,
  draft: BenchmarkDialogDraft,
): BenchmarkDialogState {
  return Object.freeze({ activeRunId, draft, phase, reportId });
}
