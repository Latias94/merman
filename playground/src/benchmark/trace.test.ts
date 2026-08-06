import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_TRACE_EVENT_NAMES,
  BENCHMARK_TRACE_SCHEMA_VERSION,
  BenchmarkTraceError,
  createBenchmarkTraceRecorder,
  deriveBenchmarkIntervals,
  type BenchmarkRawTrace,
  type BenchmarkTraceContract,
  validateBenchmarkRawTrace,
} from "./trace.ts";

const COLD_MERMAN = {
  engine: "merman",
  mode: "realm-cold",
  outcome: "success",
} as const satisfies BenchmarkTraceContract;

const COLD_MERMAID = {
  engine: "mermaid",
  mode: "realm-cold",
  outcome: "success",
} as const satisfies BenchmarkTraceContract;

const WARM_MERMAN = {
  engine: "merman",
  mode: "warm",
  outcome: "success",
} as const satisfies BenchmarkTraceContract;

test("the versioned wire schema contains exactly the 19 event fields", () => {
  assert.equal(BENCHMARK_TRACE_SCHEMA_VERSION, 1);
  assert.equal(BENCHMARK_TRACE_EVENT_NAMES.length, 19);
  assert.deepEqual(
    Object.keys(coldMermanTrace()).sort(),
    [...BENCHMARK_TRACE_EVENT_NAMES].sort()
  );
});

test("validates and derives a realm-cold Merman trace without summing overlaps", () => {
  const trace = validateBenchmarkRawTrace(coldMermanTrace(), COLD_MERMAN);

  assert(Object.isFrozen(trace));
  assert.deepEqual(deriveBenchmarkIntervals(trace, COLD_MERMAN), {
    adapterImportMs: 2,
    engineImportMs: 3,
    resourceAcquisitionMs: 3.5,
    registrationMs: null,
    initializationMs: 1,
    firstBudgetedSvgMs: 10,
    firstIsolatedPresentationMs: 12,
    warmBudgetedSvgMs: null,
    warmIsolatedPresentationMs: null,
  });
});

test("validates Mermaid registration while keeping resource acquisition null", () => {
  const trace = validateBenchmarkRawTrace(coldMermaidTrace(), COLD_MERMAID);

  assert.deepEqual(deriveBenchmarkIntervals(trace, COLD_MERMAID), {
    adapterImportMs: 1.5,
    engineImportMs: 2,
    resourceAcquisitionMs: null,
    registrationMs: 1,
    initializationMs: 1,
    firstBudgetedSvgMs: 8,
    firstIsolatedPresentationMs: 10,
    warmBudgetedSvgMs: null,
    warmIsolatedPresentationMs: null,
  });
});

test("warm traces exclude every acquisition and initialization phase", () => {
  const trace = validateBenchmarkRawTrace(warmTrace(), WARM_MERMAN);

  assert.deepEqual(deriveBenchmarkIntervals(trace, WARM_MERMAN), {
    adapterImportMs: null,
    engineImportMs: null,
    resourceAcquisitionMs: null,
    registrationMs: null,
    initializationMs: null,
    firstBudgetedSvgMs: null,
    firstIsolatedPresentationMs: null,
    warmBudgetedSvgMs: 2,
    warmIsolatedPresentationMs: 4,
  });
});

test("accepts equal adjacent offsets from a coarse monotonic clock", () => {
  const trace = Object.fromEntries(
    Object.entries(coldMermanTrace()).map(([event, value]) => [
      event,
      value === null ? null : 0,
    ])
  );

  assert.doesNotThrow(() => validateBenchmarkRawTrace(trace, COLD_MERMAN));
});

test("rejects missing and extra serialized fields, including adapter totals", () => {
  const missing = { ...coldMermanTrace() } as Record<string, unknown>;
  delete missing.isolated_layout_box_ready;

  for (const invalid of [
    missing,
    { ...coldMermanTrace(), totalMs: 12 },
    { ...coldMermanTrace(), renderTimeMs: 4 },
  ]) {
    assert.throws(
      () => validateBenchmarkRawTrace(invalid, COLD_MERMAN),
      /unexpected fields/
    );
  }
});

test("rejects negative and non-finite offsets", () => {
  for (const invalid of [
    coldMermanTrace({ render_start: -1 }),
    coldMermanTrace({ budgeted_svg_ready: Number.NaN }),
    coldMermanTrace({ sample_end: Number.POSITIVE_INFINITY }),
  ]) {
    assert.throws(
      () => validateBenchmarkRawTrace(invalid, COLD_MERMAN),
      BenchmarkTraceError
    );
  }
});

test("rejects unpaired spans", () => {
  assert.throws(
    () =>
      validateBenchmarkRawTrace(
        coldMermanTrace({ engine_import_end: null }),
        COLD_MERMAN
      ),
    /complete phase path|engine_import_end/
  );
});

test("rejects illegal pair, dependency, overlap, and render-tail order", () => {
  for (const invalid of [
    coldMermanTrace({ adapter_import_start: 3 }),
    coldMermanTrace({ engine_import_start: 1.5 }),
    coldMermanTrace({ resource_acquire_start: 5.5 }),
    coldMermanTrace({ initialize_start: 5.5 }),
    coldMermanTrace({ render_start: 6.5 }),
    coldMermanTrace({ isolated_dom_inserted: 9.5 }),
    coldMermanTrace({ isolated_presentation_ready: 13 }),
  ]) {
    assert.throws(
      () => validateBenchmarkRawTrace(invalid, COLD_MERMAN),
      BenchmarkTraceError
    );
  }
});

test("successful traces require the complete presentation path", () => {
  assert.throws(
    () =>
      validateBenchmarkRawTrace(
        coldMermanTrace({ isolated_presentation_ready: null }),
        COLD_MERMAN
      ),
    /requires isolated_presentation_ready/
  );
});

test("rejects fields forbidden by engine and sample mode", () => {
  assert.throws(
    () =>
      validateBenchmarkRawTrace(
        coldMermaidTrace({
          resource_acquire_start: 3,
          resource_acquire_end: 4,
        }),
        COLD_MERMAID
      ),
    /resource_acquire_start is forbidden/
  );
  assert.throws(
    () =>
      validateBenchmarkRawTrace(
        coldMermanTrace({ register_start: 5, register_end: 6 }),
        COLD_MERMAN
      ),
    /register_start is forbidden/
  );
  assert.throws(
    () =>
      validateBenchmarkRawTrace(
        warmTrace({ adapter_import_start: 0, adapter_import_end: 1 }),
        WARM_MERMAN
      ),
    /adapter_import_start is forbidden/
  );
});

test("failure traces retain a completed prefix and a final sample_end", () => {
  const contract = {
    ...COLD_MERMAN,
    outcome: "failure",
  } as const satisfies BenchmarkTraceContract;
  const trace = validateBenchmarkRawTrace(
    coldMermanTrace({
      budgeted_svg_ready: null,
      isolated_dom_inserted: null,
      isolated_layout_box_ready: null,
      isolated_presentation_ready: null,
      sample_end: 9,
    }),
    contract
  );

  assert.equal(trace.render_start, 8);
  assert.equal(trace.budgeted_svg_ready, null);
  assert.equal(trace.sample_end, 9);
  assert.deepEqual(deriveBenchmarkIntervals(trace, contract), {
    adapterImportMs: 2,
    engineImportMs: 3,
    resourceAcquisitionMs: 3.5,
    registrationMs: null,
    initializationMs: 1,
    firstBudgetedSvgMs: null,
    firstIsolatedPresentationMs: null,
    warmBudgetedSvgMs: null,
    warmIsolatedPresentationMs: null,
  });
});

test("recorder freezes a realm-local failure prefix and finishes once", () => {
  const times = [
    100, 100, 100, 102, 102, 102.5, 104, 105, 105, 106, 107, 107, 108,
  ];
  let index = 0;
  const recorder = createBenchmarkTraceRecorder(() => times[index++] ?? 108);

  recorder.mark("fonts_wait_start");
  recorder.mark("adapter_import_start");
  recorder.mark("adapter_import_end");
  recorder.mark("fonts_wait_end");
  recorder.mark("engine_import_start");
  recorder.mark("resource_acquire_start");
  recorder.mark("engine_import_end");
  recorder.mark("resource_acquire_end");
  recorder.mark("initialize_start");
  recorder.mark("initialize_end");
  recorder.mark("render_start");
  const first = recorder.finish();

  assert(Object.isFrozen(first));
  assert.equal(first.sample_start, 0);
  assert.equal(first.sample_end, 8);
  assert.equal(first.budgeted_svg_ready, null);
  assert.strictEqual(recorder.finish(), first);
  assert.throws(() => recorder.mark("budgeted_svg_ready"), /already finished/);
  assert.doesNotThrow(() =>
    validateBenchmarkRawTrace(first, {
      ...COLD_MERMAN,
      outcome: "failure",
    })
  );
});

test("recorder rejects duplicate events and a regressing clock", () => {
  const duplicate = createBenchmarkTraceRecorder(() => 1);
  duplicate.mark("fonts_wait_start");
  assert.throws(
    () => duplicate.mark("fonts_wait_start"),
    /recorded twice/
  );

  const times = [10, 12, 11];
  let index = 0;
  const regressing = createBenchmarkTraceRecorder(() => times[index++] ?? 11);
  regressing.mark("fonts_wait_start");
  assert.throws(
    () => regressing.mark("adapter_import_start"),
    /moved backwards/
  );
});

test("failure finish preserves a half-open phase without fabricating its end", () => {
  const times = [100, 100, 101, 102, 103];
  let index = 0;
  const recorder = createBenchmarkTraceRecorder(() => times[index++] ?? 103);

  recorder.mark("fonts_wait_start");
  recorder.mark("adapter_import_start");
  recorder.mark("fonts_wait_end");
  const trace = recorder.finishFailure();

  assert.equal(trace.fonts_wait_start, 0);
  assert.equal(trace.fonts_wait_end, 2);
  assert.equal(trace.adapter_import_start, 1);
  assert.equal(trace.adapter_import_end, null);
  assert.equal(trace.sample_end, 3);
  assert.doesNotThrow(() =>
    validateBenchmarkRawTrace(trace, {
      engine: "merman",
      mode: "realm-cold",
      outcome: "failure",
    })
  );
});

test("failure trace permits one completed concurrent Merman acquisition", () => {
  const trace = coldMermanTrace({
    engine_import_start: 2,
    engine_import_end: null,
    initialize_start: null,
    initialize_end: null,
    render_start: null,
    budgeted_svg_ready: null,
    isolated_dom_inserted: null,
    isolated_layout_box_ready: null,
    isolated_presentation_ready: null,
    sample_end: 8,
  });
  assert.doesNotThrow(() =>
    validateBenchmarkRawTrace(trace, {
      engine: "merman",
      mode: "realm-cold",
      outcome: "failure",
    })
  );
});

function coldMermanTrace(
  overrides: Partial<BenchmarkRawTrace> = {}
): BenchmarkRawTrace {
  return {
    sample_start: 0,
    fonts_wait_start: 0,
    fonts_wait_end: 2,
    adapter_import_start: 0,
    adapter_import_end: 2,
    engine_import_start: 2,
    engine_import_end: 5,
    resource_acquire_start: 2.5,
    resource_acquire_end: 6,
    register_start: null,
    register_end: null,
    initialize_start: 6,
    initialize_end: 7,
    render_start: 8,
    budgeted_svg_ready: 10,
    isolated_dom_inserted: 11,
    isolated_layout_box_ready: 11,
    isolated_presentation_ready: 12,
    sample_end: 12,
    ...overrides,
  };
}

function coldMermaidTrace(
  overrides: Partial<BenchmarkRawTrace> = {}
): BenchmarkRawTrace {
  return {
    sample_start: 0,
    fonts_wait_start: 0,
    fonts_wait_end: 1.5,
    adapter_import_start: 0,
    adapter_import_end: 1.5,
    engine_import_start: 1.5,
    engine_import_end: 3.5,
    resource_acquire_start: null,
    resource_acquire_end: null,
    register_start: 3.5,
    register_end: 4.5,
    initialize_start: 4.5,
    initialize_end: 5.5,
    render_start: 6,
    budgeted_svg_ready: 8,
    isolated_dom_inserted: 9,
    isolated_layout_box_ready: 9,
    isolated_presentation_ready: 10,
    sample_end: 10,
    ...overrides,
  };
}

function warmTrace(
  overrides: Partial<BenchmarkRawTrace> = {}
): BenchmarkRawTrace {
  return {
    sample_start: 0,
    fonts_wait_start: 0,
    fonts_wait_end: 1,
    adapter_import_start: null,
    adapter_import_end: null,
    engine_import_start: null,
    engine_import_end: null,
    resource_acquire_start: null,
    resource_acquire_end: null,
    register_start: null,
    register_end: null,
    initialize_start: null,
    initialize_end: null,
    render_start: 1,
    budgeted_svg_ready: 3,
    isolated_dom_inserted: 4,
    isolated_layout_box_ready: 4,
    isolated_presentation_ready: 5,
    sample_end: 5,
    ...overrides,
  };
}
