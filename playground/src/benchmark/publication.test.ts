import assert from "node:assert/strict";
import test from "node:test";

import { REALM_BUDGETS } from "../runtime/realm/channel-protocol.ts";
import {
  BENCHMARK_PUBLICATION_CLOCK_BOUNDARY,
  deriveBenchmarkParentPublicationEvidence,
} from "./publication.ts";

test("parent publication evidence uses one additive clock boundary", () => {
  const evidence = deriveBenchmarkParentPublicationEvidence({
    dispatchedAt: 10,
    isolatedPresentationReceivedAt: 22,
    responseReceivedAt: 23,
    envelopeValidatedAt: 25,
    strictSvgValidatedAt: 28,
  });

  assert.deepEqual(evidence, {
    clockBoundary: BENCHMARK_PUBLICATION_CLOCK_BOUNDARY,
    isolatedPresentationReceiptMs: 12,
    responseDeliveryMs: 1,
    responseEnvelopeValidationMs: 2,
    strictSvgValidationMs: 3,
    totalMs: 18,
  });
  assert(Object.isFrozen(evidence));
});

test("parent publication evidence rejects regressing and over-budget clocks", () => {
  assert.throws(
    () =>
      deriveBenchmarkParentPublicationEvidence({
        dispatchedAt: 10,
        isolatedPresentationReceivedAt: 9,
        responseReceivedAt: 11,
        envelopeValidatedAt: 12,
        strictSvgValidatedAt: 13,
      }),
    /moved backwards/
  );
  assert.throws(
    () =>
      deriveBenchmarkParentPublicationEvidence({
        dispatchedAt: 0,
        isolatedPresentationReceivedAt: REALM_BUDGETS.runTimeoutMs + 1,
        responseReceivedAt: REALM_BUDGETS.runTimeoutMs + 1,
        envelopeValidatedAt: REALM_BUDGETS.runTimeoutMs + 1,
        strictSvgValidatedAt: REALM_BUDGETS.runTimeoutMs + 1,
      }),
    /is invalid/
  );
});
