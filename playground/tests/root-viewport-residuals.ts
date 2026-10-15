import { createHash } from "node:crypto";

import {
  exactRootViewportResidualEvidenceIsEligible,
  ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX,
  ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX,
  ROOT_VIEWPORT_ORACLE_REVISION,
  ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
  rootViewportAuditFingerprintFacts,
  type PaintedPixelViolation,
  type RectSnapshot,
  type RootViewportAudit,
  type RootViewportContainmentClassification,
} from "./root-viewport-oracle.ts";

export const ROOT_VIEWPORT_RESIDUAL_SCHEMA_VERSION = 2;
export const ROOT_VIEWPORT_AUDIT_FINGERPRINT_VERSION = 1;
export const ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION = ROOT_VIEWPORT_ORACLE_REVISION;

const SHA256 = /^[0-9a-f]{64}$/u;
const ZERO_SHA256 = /^0{64}$/u;
export const FILTERED_TITLE_FONT_RESIDUAL_REASON =
  "deterministic-title-display-font-with-reviewed-filter-paint";
const REASONS = new Set([
  FILTERED_TITLE_FONT_RESIDUAL_REASON,
  "deterministic-text-measurement-out-of-domain-extrapolation",
]);

export type RootViewportResidualReceipt = {
  fixture: string;
  localSvgSha256: string;
  upstreamSvgSha256: string;
  auditEvidenceSha256: string;
  reason: string;
  auditSha256?: string;
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
    const { fixture, localSvgSha256, upstreamSvgSha256, auditEvidenceSha256, reason, auditSha256 } = entry;
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
    if (reason === FILTERED_TITLE_FONT_RESIDUAL_REASON) {
      if (!isNonZeroSha256(auditSha256)) {
        throw new Error(`Root viewport residual ${fixture} requires an exact audit SHA-256.`);
      }
    } else if (auditSha256 !== undefined) {
      throw new Error(`Root viewport residual ${fixture} does not support an audit SHA-256.`);
    }
    previousFixture = fixture;
    return {
      fixture, localSvgSha256, upstreamSvgSha256, auditEvidenceSha256, reason,
      ...(auditSha256 === undefined ? {} : { auditSha256 }),
    };
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
  auditSha256?: string,
): RootViewportResidualReceipt | null {
  const receipt = catalog.entries.find((entry) => entry.fixture === fixture);
  if (
    receipt === undefined ||
    upstreamSvgSha256 === null ||
    receipt.localSvgSha256 !== localSvgSha256 ||
    receipt.upstreamSvgSha256 !== upstreamSvgSha256 ||
    receipt.auditEvidenceSha256 !== auditEvidenceSha256 ||
    (receipt.reason === FILTERED_TITLE_FONT_RESIDUAL_REASON
      ? auditSha256 === undefined || receipt.auditSha256 !== auditSha256
      : auditSha256 !== undefined)
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

export type RootViewportReportedAudit = Omit<RootViewportAudit, "structuralPixelKeys"> & {
  paintedPixelCount: number;
  structuralPaintedPixelCount: number;
  structuralPixelSha256: string;
};

export type RootViewportAuditEnvironment = {
  playwright: string;
  browser: string;
  platform: string;
  locale: string;
  timezone: string;
  localPaintAudit: string;
  upstreamPaintAudit: string;
};

// This receipt records one reviewed display-font boundary, not arbitrary indeterminate paint.
export function filteredTitleFontAuditSha256(
  environment: RootViewportAuditEnvironment,
  local: RootViewportReportedAudit,
  upstream: RootViewportReportedAudit | null,
): string | null {
  if (upstream === null || !eligibleFilteredAudit(local) || !eligibleFilteredAudit(upstream)) {
    return null;
  }
  const environmentFields = [
    environment.playwright, environment.browser, environment.platform,
    environment.locale, environment.timezone,
    environment.localPaintAudit, environment.upstreamPaintAudit,
  ];
  if (environmentFields.some((field) => typeof field !== "string" || field.length === 0)) {
    return null;
  }
  // Fixed fields also accept the saved CI report without depending on object key order.
  return createHash("sha256").update(JSON.stringify([
    "filtered-title-font-audit-v1", ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION,
    environmentFields, auditFields(local), auditFields(upstream),
  ])).digest("hex");
}

function eligibleFilteredAudit(audit: RootViewportReportedAudit): boolean {
  const root = audit.root;
  const paint = audit.paintAudit;
  return root !== null && rectFields(root)!.every(Number.isFinite) &&
    root.width > 0 && root.height > 0 && root.right > root.left && root.bottom > root.top &&
    paint.status === "indeterminate" &&
    paint.indeterminateReasons.length === 1 && paint.indeterminateReasons[0] === "active-filter" &&
    Number.isFinite(paint.guardCssPx) && paint.guardCssPx > 0 &&
    paint.captureWidthCssPx !== null && Number.isFinite(paint.captureWidthCssPx) &&
    paint.captureWidthCssPx >= root.width &&
    paint.captureHeightCssPx !== null && Number.isFinite(paint.captureHeightCssPx) &&
    paint.captureHeightCssPx >= root.height &&
    SHA256.test(audit.structuralPixelSha256) &&
    [...audit.violations, ...audit.structuralViolations].every(
      (violation) => violation.reachesAuditBoundary === false,
    );
}

function rectFields(rect: RectSnapshot | null): number[] | null {
  return rect === null ? null : [rect.left, rect.top, rect.right, rect.bottom, rect.width, rect.height];
}

function violationFields(violation: PaintedPixelViolation): unknown[] {
  return [violation.edge, violation.paintedPixelCount, rectFields(violation.rect),
    violation.reachesAuditBoundary];
}

function auditFields(audit: RootViewportReportedAudit): unknown[] {
  return [
    rectFields(audit.root), rectFields(audit.geometryUnion), audit.paintedElementCount,
    audit.paintAudit.status, audit.paintAudit.guardCssPx, audit.paintAudit.captureWidthCssPx,
    audit.paintAudit.captureHeightCssPx, audit.paintAudit.indeterminateReasons,
    audit.violations.map(violationFields), audit.structuralViolations.map(violationFields),
    audit.paintedPixelCount, audit.structuralPaintedPixelCount, audit.structuralPixelSha256,
  ];
}
