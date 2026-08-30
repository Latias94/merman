export const ROOT_VIEWPORT_RESIDUAL_SCHEMA_VERSION = 1;
export const ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION =
  "browser-root-paint-containment-v10";

const SHA256 = /^[0-9a-f]{64}$/u;
const REASONS = new Set([
  "deterministic-text-measurement-out-of-domain-extrapolation",
]);

export type RootViewportResidualReceipt = {
  fixture: string;
  localSvgSha256: string;
  upstreamSvgSha256: string;
  reason: string;
};

export type RootViewportResidualCatalog = {
  schemaVersion: number;
  comparisonRevision: string;
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
  if (!Array.isArray(value.entries)) {
    throw new Error("Root viewport residual entries must be an array.");
  }

  let previousFixture: string | null = null;
  const entries = value.entries.map((entry): RootViewportResidualReceipt => {
    if (!isRecord(entry)) throw new Error("Root viewport residual entry must be an object.");
    const { fixture, localSvgSha256, upstreamSvgSha256, reason } = entry;
    if (typeof fixture !== "string" || fixture.length === 0) {
      throw new Error("Root viewport residual fixture must be non-empty.");
    }
    if (previousFixture !== null && previousFixture >= fixture) {
      throw new Error("Root viewport residual entries must be unique and sorted.");
    }
    if (
      typeof localSvgSha256 !== "string" ||
      !SHA256.test(localSvgSha256) ||
      typeof upstreamSvgSha256 !== "string" ||
      !SHA256.test(upstreamSvgSha256)
    ) {
      throw new Error(`Root viewport residual ${fixture} has an invalid SVG SHA-256.`);
    }
    if (typeof reason !== "string" || !REASONS.has(reason)) {
      throw new Error(`Root viewport residual ${fixture} has an unsupported reason.`);
    }
    previousFixture = fixture;
    return { fixture, localSvgSha256, upstreamSvgSha256, reason };
  });

  return {
    schemaVersion: ROOT_VIEWPORT_RESIDUAL_SCHEMA_VERSION,
    comparisonRevision: ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION,
    entries,
  };
}

export function matchingRootViewportResidual(
  catalog: RootViewportResidualCatalog,
  fixture: string,
  localSvgSha256: string,
  upstreamSvgSha256: string | null,
): RootViewportResidualReceipt | null {
  const receipt = catalog.entries.find((entry) => entry.fixture === fixture);
  if (
    receipt === undefined ||
    upstreamSvgSha256 === null ||
    receipt.localSvgSha256 !== localSvgSha256 ||
    receipt.upstreamSvgSha256 !== upstreamSvgSha256
  ) {
    return null;
  }
  return receipt;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
