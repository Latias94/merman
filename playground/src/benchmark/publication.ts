import { REALM_BUDGETS, RealmProtocolError } from "../runtime/realm/channel-protocol.ts";

export const BENCHMARK_PUBLICATION_CLOCK_BOUNDARY =
  "parent-sample-dispatch-to-strict-svg" as const;

export interface BenchmarkParentPublicationEvidence {
  readonly clockBoundary: typeof BENCHMARK_PUBLICATION_CLOCK_BOUNDARY;
  readonly isolatedPresentationReceiptMs: number;
  readonly responseEnvelopeValidationMs: number;
  readonly responseDeliveryMs: number;
  readonly strictSvgValidationMs: number;
  readonly totalMs: number;
}

export interface BenchmarkParentPublicationTimestamps {
  readonly dispatchedAt: number;
  readonly envelopeValidatedAt: number;
  readonly isolatedPresentationReceivedAt: number;
  readonly responseReceivedAt: number;
  readonly strictSvgValidatedAt: number;
}

export function deriveBenchmarkParentPublicationEvidence(
  timestamps: BenchmarkParentPublicationTimestamps
): BenchmarkParentPublicationEvidence {
  const ordered = [
    ["sample dispatch", timestamps.dispatchedAt],
    [
      "isolated presentation receipt",
      timestamps.isolatedPresentationReceivedAt,
    ],
    ["response receipt", timestamps.responseReceivedAt],
    ["response envelope validation", timestamps.envelopeValidatedAt],
    ["strict SVG validation", timestamps.strictSvgValidatedAt],
  ] as const;
  for (let index = 0; index < ordered.length; index += 1) {
    const [name, value] = ordered[index];
    if (!Number.isFinite(value) || value < 0) {
      throw new RealmProtocolError(
        `Benchmark parent clock for ${name} is invalid.`
      );
    }
    if (index > 0 && value < ordered[index - 1][1]) {
      throw new RealmProtocolError(
        `Benchmark parent clock moved backwards before ${name}.`
      );
    }
  }

  const evidence = Object.freeze({
    clockBoundary: BENCHMARK_PUBLICATION_CLOCK_BOUNDARY,
    isolatedPresentationReceiptMs:
      timestamps.isolatedPresentationReceivedAt - timestamps.dispatchedAt,
    responseDeliveryMs:
      timestamps.responseReceivedAt -
      timestamps.isolatedPresentationReceivedAt,
    responseEnvelopeValidationMs:
      timestamps.envelopeValidatedAt - timestamps.responseReceivedAt,
    strictSvgValidationMs:
      timestamps.strictSvgValidatedAt - timestamps.envelopeValidatedAt,
    totalMs: timestamps.strictSvgValidatedAt - timestamps.dispatchedAt,
  });
  for (const [name, value] of Object.entries(evidence)) {
    if (
      name !== "clockBoundary" &&
      (typeof value !== "number" ||
        value < 0 ||
        value > REALM_BUDGETS.runTimeoutMs)
    ) {
      throw new RealmProtocolError(
        `Benchmark parent publication evidence ${name} is invalid.`
      );
    }
  }

  const componentTotal =
    evidence.isolatedPresentationReceiptMs +
    evidence.responseDeliveryMs +
    evidence.responseEnvelopeValidationMs +
    evidence.strictSvgValidationMs;
  if (Math.abs(componentTotal - evidence.totalMs) > Number.EPSILON * 8) {
    throw new RealmProtocolError(
      "Benchmark parent publication evidence total is inconsistent."
    );
  }
  return evidence;
}
