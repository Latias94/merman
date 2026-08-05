import assert from "node:assert/strict";
import test from "node:test";

import {
  evaluateNpmAuditReport,
  NPM_AUDIT_POLICIES,
  npmAuditArguments,
} from "./verify-npm-audits.mjs";

const complete = NPM_AUDIT_POLICIES.find(
  (policy) => policy.id === "playground-complete",
);
const production = NPM_AUDIT_POLICIES.find(
  (policy) => policy.id === "playground-production",
);
assert(complete && production);

test("audit policy commands are explicit about production mode", () => {
  assert.deepEqual(npmAuditArguments(complete), [
    "audit",
    "--json",
    "--audit-level=moderate",
  ]);
  assert.deepEqual(npmAuditArguments(production), [
    "audit",
    "--json",
    "--audit-level=low",
    "--omit=dev",
  ]);
});

test("complete audit applies the configured moderate threshold", () => {
  const summary = evaluateNpmAuditReport(report({ low: 2, total: 2 }), complete);
  assert.equal(summary.total, 2);
  assert.throws(
    () =>
      evaluateNpmAuditReport(
        report({ moderate: 1, total: 1 }),
        complete,
      ),
    /threshold failed/u,
  );
});

test("production audit requires a zero-vulnerability graph", () => {
  assert.doesNotThrow(() =>
    evaluateNpmAuditReport(report({ total: 0 }), production),
  );
  assert.throws(
    () =>
      evaluateNpmAuditReport(report({ low: 1, total: 1 }), production),
    /threshold failed/u,
  );
});

test("audit reports without metadata fail closed", () => {
  assert.throws(
    () => evaluateNpmAuditReport({ error: { code: "EAUDIT" } }, complete),
    /no vulnerability metadata/u,
  );
});

function report(overrides) {
  return {
    metadata: {
      vulnerabilities: {
        info: 0,
        low: 0,
        moderate: 0,
        high: 0,
        critical: 0,
        total: 0,
        ...overrides,
      },
    },
  };
}
