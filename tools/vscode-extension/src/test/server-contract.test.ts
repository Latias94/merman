import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  assertAnalysisConfigCapability,
  negotiateAnalysisConfig,
  type AnalysisConfigSchemaProjection,
} from "../analysis-config-contract.js";

function response(
  overrides: Partial<AnalysisConfigSchemaProjection> = {},
): AnalysisConfigSchemaProjection {
  return {
    version: 1,
    rule_catalog_method: "merman/ruleCatalog",
    accepted_roots: ["analysis", "options"],
    configurable_rule_ids: ["merman.future.rule"],
    ...overrides,
  };
}

describe("analysis config negotiation", () => {
  it("requires the initialized server to advertise the config-schema request", () => {
    assert.doesNotThrow(() =>
      assertAnalysisConfigCapability({
        capabilities: {
          experimental: {
            merman: {
              requests: { configSchema: "merman/configSchema" },
            },
          },
        },
      })
    );
    assert.throws(
      () => assertAnalysisConfigCapability({ capabilities: {} }),
      /does not advertise merman\/configSchema/,
    );
    assert.throws(
      () =>
        assertAnalysisConfigCapability({
          capabilities: {
            experimental: {
              merman: { requests: { configSchema: "other/configSchema" } },
            },
          },
        }),
      /does not advertise merman\/configSchema/,
    );
  });

  it("accepts schema 1 and preserves future server rule ids", () => {
    assert.deepEqual(negotiateAnalysisConfig(response()), {
      version: 1,
      configurableRuleIds: ["merman.future.rule"],
    });
  });

  it("rejects unsupported or malformed server contracts", () => {
    assert.throws(
      () => negotiateAnalysisConfig(response({ version: 2 })),
      /Unsupported Merman analysis config schema version 2/,
    );
    assert.throws(
      () =>
        negotiateAnalysisConfig(response({ configurable_rule_ids: ["ok", 7] })),
      /invalid configurable rule IDs/,
    );
    assert.throws(
      () => negotiateAnalysisConfig(response({ configurable_rule_ids: ["dup", "dup"] })),
      /invalid configurable rule IDs/,
    );
    assert.throws(
      () => negotiateAnalysisConfig(response({ rule_catalog_method: "other/catalog" })),
      /incompatible rule catalog method/,
    );
    assert.throws(
      () => negotiateAnalysisConfig(response({ accepted_roots: ["options"] })),
      /does not accept the analysis root/,
    );
    assert.throws(
      () => negotiateAnalysisConfig(response({ configurable_rule_ids: [" padded "] })),
      /invalid configurable rule IDs/,
    );
  });
});
