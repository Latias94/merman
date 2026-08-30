import { createHash } from "node:crypto";

import {
  exactRootViewportResidualEvidenceIsEligible,
  ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX,
  ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX,
  ROOT_VIEWPORT_ORACLE_REVISION,
  ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
  rootViewportAuditFingerprintFacts,
  type RootViewportAudit,
  type RootViewportContainmentClassification,
} from "./root-viewport-oracle.ts";

export const ROOT_VIEWPORT_RESIDUAL_SCHEMA_VERSION = 2;
export const ROOT_VIEWPORT_AUDIT_FINGERPRINT_VERSION = 1;
export const ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION = ROOT_VIEWPORT_ORACLE_REVISION;

const SHA256 = /^[0-9a-f]{64}$/u;
const ZERO_SHA256 = /^0{64}$/u;
const REASONS = new Set([
  "deterministic-text-measurement-out-of-domain-extrapolation",
]);

export type RootViewportResidualReceipt = {
  fixture: string;
  localSvgSha256: string;
  upstreamSvgSha256: string;
  auditEvidenceSha256: string;
  reason: string;
};

export type RootViewportResidualCatalog = {
  schemaVersion: number;
  comparisonRevision: string;
  auditFingerprintVersion: number;
  entries: RootViewportResidualReceipt[];
};

export function parseRootViewportResidualCatalog(
  source: string,
): RootViewportResidualCatalog {
  const value: unknown = JSON.parse(source);
  if (!isRecord(value)) throw new Error("Root viewport residual catalog must be an object.");
  if (value.schemaVersion !== ROOT_VIEWPORT_RESIDUAL_SCHEMA_VERSION) {
    throw new Error(
      `Unsupported root viewport residual schema ${String(value.schemaVersion)}.`,
    );
  }
  if (value.comparisonRevision !== ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION) {
    throw new Error("Root viewport residual comparison revision drifted.");
  }
  if (value.auditFingerprintVersion !== ROOT_VIEWPORT_AUDIT_FINGERPRINT_VERSION) {
    throw new Error("Root viewport residual audit fingerprint version drifted.");
  }
  if (!Array.isArray(value.entries)) {
    throw new Error("Root viewport residual entries must be an array.");
  }

  let previousFixture: string | null = null;
  const entries = value.entries.map((entry): RootViewportResidualReceipt => {
    if (!isRecord(entry)) throw new Error("Root viewport residual entry must be an object.");
    const { fixture, localSvgSha256, upstreamSvgSha256, auditEvidenceSha256, reason } =
      entry;
    if (typeof fixture !== "string" || fixture.length === 0) {
      throw new Error("Root viewport residual fixture must be non-empty.");
    }
    if (previousFixture !== null && previousFixture >= fixture) {
      throw new Error("Root viewport residual entries must be unique and sorted.");
    }
    if (
      !isNonZeroSha256(localSvgSha256) ||
      !isNonZeroSha256(upstreamSvgSha256) ||
      !isNonZeroSha256(auditEvidenceSha256)
    ) {
      throw new Error(`Root viewport residual ${fixture} has an invalid evidence SHA-256.`);
    }
    if (typeof reason !== "string" || !REASONS.has(reason)) {
      throw new Error(`Root viewport residual ${fixture} has an unsupported reason.`);
    }
    previousFixture = fixture;
    return { fixture, localSvgSha256, upstreamSvgSha256, auditEvidenceSha256, reason };
  });

  return {
    schemaVersion: ROOT_VIEWPORT_RESIDUAL_SCHEMA_VERSION,
    comparisonRevision: ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION,
    auditFingerprintVersion: ROOT_VIEWPORT_AUDIT_FINGERPRINT_VERSION,
    entries,
  };
}

export function matchingRootViewportResidual(
  catalog: RootViewportResidualCatalog,
  fixture: string,
  localSvgSha256: string,
  upstreamSvgSha256: string | null,
  auditEvidenceSha256: string,
): RootViewportResidualReceipt | null {
  const receipt = catalog.entries.find((entry) => entry.fixture === fixture);
  if (
    receipt === undefined ||
    upstreamSvgSha256 === null ||
    receipt.localSvgSha256 !== localSvgSha256 ||
    receipt.upstreamSvgSha256 !== upstreamSvgSha256 ||
    receipt.auditEvidenceSha256 !== auditEvidenceSha256
  ) {
    return null;
  }
  return receipt;
}

export function rootViewportResidualAuditEvidenceSha256(
  local: RootViewportAudit,
  upstream: RootViewportAudit | null,
  baseContainmentClassification: RootViewportContainmentClassification,
): string {
  const canonicalEvidence = {
    fingerprintVersion: ROOT_VIEWPORT_AUDIT_FINGERPRINT_VERSION,
    oracleRevision: ROOT_VIEWPORT_ORACLE_REVISION,
    quantizationEpsilonCssPx: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
    maxCaptureDimensionCssPx: ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX,
    maxCaptureAreaCssPx: ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX,
    baseContainmentClassification,
    exactResidualEligible: exactRootViewportResidualEvidenceIsEligible(local, upstream),
    local: rootViewportAuditFingerprintFacts(local),
    upstream: upstream === null ? null : rootViewportAuditFingerprintFacts(upstream),
  };
  return sha256(JSON.stringify(canonicalEvidence));
}

export function unusedRootViewportResidualFixtures(
  catalog: RootViewportResidualCatalog,
  usedFixtures: ReadonlySet<string>,
): string[] {
  return catalog.entries
    .map((entry) => entry.fixture)
    .filter((fixture) => !usedFixtures.has(fixture));
}

function isNonZeroSha256(value: unknown): value is string {
  return typeof value === "string" && SHA256.test(value) && !ZERO_SHA256.test(value);
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
