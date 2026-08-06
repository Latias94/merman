import assert from "node:assert/strict";
import test from "node:test";

import {
  createBenchmarkDialogState,
  reduceBenchmarkDialogState,
} from "./dialog-state.ts";

test("benchmark dialog moves between configuration, running, and retained results", () => {
  let state = createBenchmarkDialogState("idle");
  assert.deepEqual(state, {
    activeRunId: null,
    phase: "configure",
    reportId: null,
    draft: { mode: "realm-cold", iterations: 6, warmups: 2 },
  });

  state = reduceBenchmarkDialogState(state, {
    type: "update-draft",
    draft: { mode: "warm", iterations: 2, warmups: 0 },
  });
  state = reduceBenchmarkDialogState(state, {
    type: "run-started",
    runId: "run-1",
    retainedReportId: null,
  });
  assert.equal(state.phase, "running");
  assert.equal(state.reportId, null);

  state = reduceBenchmarkDialogState(state, {
    type: "run-settled",
    runId: "run-1",
    reportId: "run-1",
  });
  assert.equal(state.phase, "report");
  assert.equal(state.reportId, "run-1");

  state = reduceBenchmarkDialogState(state, { type: "change-settings" });
  assert.deepEqual(state, {
    activeRunId: null,
    phase: "configure",
    reportId: "run-1",
    draft: { mode: "warm", iterations: 2, warmups: 0 },
  });

  state = reduceBenchmarkDialogState(state, {
    type: "back-to-report",
    reportId: "run-1",
  });
  assert.equal(state.phase, "report");
});

test("a rerun retains the old report and ignores completion from an older run", () => {
  const report = createBenchmarkDialogState("success", {
    id: "report-old",
    draft: { mode: "warm", iterations: 2, warmups: 0 },
  });
  const rejected = reduceBenchmarkDialogState(report, {
    type: "run-rejected",
    runId: "rejected-before-start",
  });
  assert.strictEqual(rejected, report);

  const running = reduceBenchmarkDialogState(report, {
    type: "run-started",
    runId: "run-new",
    retainedReportId: "report-old",
  });
  assert.equal(running.phase, "running");
  assert.equal(running.reportId, "report-old");

  assert.strictEqual(
    reduceBenchmarkDialogState(running, {
      type: "run-settled",
      runId: "run-old",
      reportId: "report-old",
    }),
    running,
  );
  assert.strictEqual(
    reduceBenchmarkDialogState(running, {
      type: "run-rejected",
      runId: "run-old",
    }),
    running,
  );

  const settled = reduceBenchmarkDialogState(running, {
    type: "run-settled",
    runId: "run-new",
    reportId: "report-new",
  });
  assert.equal(settled.phase, "report");
  assert.equal(settled.activeRunId, null);
  assert.equal(settled.reportId, "report-new");
});

test("configuration is immutable while a run is active", () => {
  const configured = reduceBenchmarkDialogState(
    createBenchmarkDialogState("idle"),
    {
      type: "update-draft",
      draft: { mode: "warm", iterations: 10, warmups: 3 },
    },
  );
  const running = reduceBenchmarkDialogState(configured, {
    type: "run-started",
    runId: "run-1",
    retainedReportId: null,
  });

  assert.strictEqual(
    reduceBenchmarkDialogState(running, {
      type: "update-draft",
      draft: { iterations: 20 },
    }),
    running,
  );
  assert.strictEqual(
    reduceBenchmarkDialogState(running, { type: "change-settings" }),
    running,
  );
  assert.strictEqual(
    reduceBenchmarkDialogState(running, {
      type: "back-to-report",
      reportId: "report-old",
    }),
    running,
  );
});
