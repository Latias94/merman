import assert from "node:assert/strict";
import test from "node:test";

import {
  benchmarkPhasePath,
  type BenchmarkEngine,
  type BenchmarkSampleMode,
  type BenchmarkTraceMark,
} from "./phase-contract.ts";

for (const [engine, mode] of [
  ["merman", "realm-cold"],
  ["mermaid", "realm-cold"],
  ["merman", "warm"],
  ["mermaid", "warm"],
] as const satisfies readonly (readonly [BenchmarkEngine, BenchmarkSampleMode])[]) {
  test(`${mode} ${engine} exposes one immutable complete phase path`, () => {
    const path = benchmarkPhasePath(engine, mode);
    const seen = new Set<BenchmarkTraceMark>();
    for (const event of path.canonicalSuccessEvents) {
      path.assertNext(seen, event);
      seen.add(event);
    }
    path.assertSuccess(seen);
    assert.equal(Object.isFrozen(path), true);
    assert.equal(Object.isFrozen(path.applicableEvents), true);
    assert.equal(Object.isFrozen(path.canonicalSuccessEvents), true);
    assert.equal(Object.isFrozen(path.spans), true);
    assert.equal(Object.isFrozen(path.timedPhases), true);
    for (const event of path.applicableEvents) {
      const rule = path.rule(event);
      assert.ok(rule);
      assert.equal(Object.isFrozen(rule), true);
      assert.equal(Object.isFrozen(rule.predecessors), true);
      assert.equal(Object.isFrozen(rule.watchdog), true);
    }
  });
}

test("phase paths reject missing predecessors, duplicates, and inapplicable events", () => {
  const path = benchmarkPhasePath("merman", "realm-cold");
  const seen = new Set<BenchmarkTraceMark>();
  assert.throws(() => path.assertNext(seen, "render_start"), /requires/);
  path.assertNext(seen, "fonts_wait_start");
  seen.add("fonts_wait_start");
  assert.throws(() => path.assertNext(seen, "fonts_wait_start"), /twice/);
  assert.throws(() => path.assertNext(seen, "register_start"), /forbidden/);
});

test("watchdog transitions and publication boundary derive from the same path", () => {
  const path = benchmarkPhasePath("mermaid", "realm-cold");
  assert.deepEqual(path.rule("render_start")?.watchdog, {
    complete: null,
    start: "render",
  });
  assert.deepEqual(path.rule("budgeted_svg_ready")?.watchdog, {
    complete: "render",
    start: "presentation",
  });
  assert.equal(
    path.rule("isolated_presentation_ready")?.publicationBoundary,
    true
  );
  assert.equal(path.rule("resource_acquire_start"), null);
  assert.deepEqual(path.boundary("render"), {
    start: "render_start",
    end: "budgeted_svg_ready",
  });
  assert.equal(
    path.dependsOn("isolated_presentation_ready", "budgeted_svg_ready"),
    true
  );
});
