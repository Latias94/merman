import { spawnSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";

const playgroundRoot = path.resolve(import.meta.dirname, "..");

export const NPM_AUDIT_POLICIES = Object.freeze([
  Object.freeze({
    id: "playground-complete",
    auditLevel: "moderate",
    cwd: playgroundRoot,
    omitDev: false,
  }),
  Object.freeze({
    id: "playground-production",
    auditLevel: "low",
    cwd: playgroundRoot,
    omitDev: true,
  }),
  Object.freeze({
    id: "playground-tests-complete",
    auditLevel: "moderate",
    cwd: path.join(playgroundRoot, "tests"),
    omitDev: false,
  }),
]);

const SEVERITIES = Object.freeze([
  "info",
  "low",
  "moderate",
  "high",
  "critical",
]);

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  runAudits();
}

export function npmAuditArguments(policy) {
  return [
    "audit",
    "--json",
    `--audit-level=${policy.auditLevel}`,
    ...(policy.omitDev ? ["--omit=dev"] : []),
  ];
}

export function evaluateNpmAuditReport(report, policy) {
  const vulnerabilities = report?.metadata?.vulnerabilities;
  if (!isRecord(vulnerabilities)) {
    throw new Error(
      `${policy.id}: npm audit report has no vulnerability metadata.`,
    );
  }
  const counts = Object.fromEntries(
    SEVERITIES.map((severity) => [
      severity,
      nonNegativeCount(vulnerabilities[severity], severity),
    ]),
  );
  const total = nonNegativeCount(vulnerabilities.total, "total");
  const blocking = policy.omitDev
    ? total
    : SEVERITIES.slice(SEVERITIES.indexOf(policy.auditLevel)).reduce(
        (sum, severity) => sum + counts[severity],
        0,
      );
  if (blocking > 0) {
    const detail = SEVERITIES.filter((severity) => counts[severity] > 0)
      .map((severity) => `${severity}=${counts[severity]}`)
      .join(", ");
    throw new Error(
      `${policy.id}: audit threshold failed (${detail || `total=${total}`}).`,
    );
  }
  return Object.freeze({
    counts: Object.freeze(counts),
    id: policy.id,
    total,
  });
}

function runAudits() {
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  for (const policy of NPM_AUDIT_POLICIES) {
    const result = spawnSync(npm, npmAuditArguments(policy), {
      cwd: policy.cwd,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
    if (result.error) throw result.error;
    let report;
    try {
      report = JSON.parse(result.stdout);
    } catch (error) {
      throw new Error(
        `${policy.id}: npm audit did not return JSON: ${String(error)}\n${result.stderr}`,
        { cause: error },
      );
    }
    const summary = evaluateNpmAuditReport(report, policy);
    if (result.status !== 0) {
      throw new Error(`${policy.id}: npm exited with status ${result.status}.`);
    }
    const threshold = policy.omitDev ? "production-zero" : `${policy.auditLevel}+`;
    console.log(
      `[npm-audit] ${summary.id}: ${summary.total} vulnerabilities (threshold ${threshold}).`,
    );
  }
}

function nonNegativeCount(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`npm audit vulnerability count ${label} is invalid.`);
  }
  return value;
}

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
