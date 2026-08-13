import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  BUNDLED_ANALYSIS_CONFIG,
  assertAnalysisConfigCapability,
  negotiateAnalysisConfig,
  type AnalysisConfigSchemaProjection,
} from "../analysis-config-contract.js";
import { BUNDLED_ANALYSIS_CONFIG_PROJECTION } from "../generated/analysis-config-baseline.js";

function response(
  overrides: Partial<AnalysisConfigSchemaProjection> = {},
): AnalysisConfigSchemaProjection {
  return {
    version: 2,
    rule_catalog_method: "merman/ruleCatalog",
    accepted_roots: BUNDLED_ANALYSIS_CONFIG.acceptedRoots,
    profiles: BUNDLED_ANALYSIS_CONFIG.profiles,
    severities: BUNDLED_ANALYSIS_CONFIG.severities,
    configurable_rule_ids: ["merman.future.rule"],
    constraints: {
      version: 1,
      settings: bundledClientSettings(),
    },
    schema: { opaque: true },
    ...overrides,
  };
}

function bundledClientSettings(): unknown[] {
  return [...structuredClone(BUNDLED_ANALYSIS_CONFIG_PROJECTION.constraints.settings)];
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
  });

  it("accepts typed setting constraints while keeping the full schema opaque", () => {
    const contract = negotiateAnalysisConfig(response({
      schema: { allOf: [{ $ref: "#/$defs/futureAnalysisOptions" }] },
    }));

    assert.equal(contract.version, 2);
    assert.deepEqual(contract.acceptedRoots, BUNDLED_ANALYSIS_CONFIG.acceptedRoots);
    assert.deepEqual(contract.configurableRuleIds, ["merman.future.rule"]);
    assert.equal(contract.settings.length, 9);
    assert.equal(contract.settings[0]?.changeScope, "snapshot_affecting");
    assert.equal(contract.settings[5]?.changeScope, "diagnostics_only");
    assert.deepEqual(contract.settings[0]?.runtimeConstraints[1], {
      kind: "representable_local_midnight",
      offsetSettingPath: "fixed_local_offset_minutes",
    });
    assert.deepEqual(
      contract.settings.find((setting) =>
        setting.path === "resources.limits.max_source_bytes"
      )?.normalization,
      BUNDLED_ANALYSIS_CONFIG.settings.find((setting) =>
        setting.path === "resources.limits.max_source_bytes"
      )?.normalization,
    );
    assert.deepEqual(
      contract.settings.find((setting) => setting.path === "lint.rule_severities")
        ?.normalization,
      {
        kind: "rule_severity_overrides",
        fields: [
          {
            name: "rule_id",
            required: true,
            normalization: {
              kind: "string",
              pattern: undefined,
              matcher: undefined,
              values: "configurable_rule_ids",
            },
          },
          {
            name: "severity",
            required: true,
            normalization: {
              kind: "string",
              pattern: undefined,
              matcher: undefined,
              values: "severities",
            },
          },
        ],
      },
    );
  });

  it("loads the generated bundled baseline through the same DTO parser", () => {
    assert.equal(BUNDLED_ANALYSIS_CONFIG.settings.length, 9);
    assert.equal(
      BUNDLED_ANALYSIS_CONFIG.settings.find((setting) =>
        setting.path === "resources.limits.max_source_bytes"
      )?.changeScope,
      "snapshot_affecting",
    );
  });

  it("rejects unsupported or malformed server contracts", () => {
    assert.throws(
      () => negotiateAnalysisConfig(response({ version: 3 })),
      /Unsupported Merman analysis config schema version 3/,
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
      () => negotiateAnalysisConfig(response({ constraints: { version: 2 } })),
      /Unsupported Merman analysis client constraints version 2/,
    );
    assert.throws(
      () =>
        negotiateAnalysisConfig(response({
          constraints: {
            version: 1,
            settings: bundledClientSettings().map((setting, index) =>
              index === 0
                ? {
                    ...(setting as Record<string, unknown>),
                    normalization: { kind: "string", pattern: "[" },
                  }
                : setting
            ),
          },
        })),
      /invalid fixed_today pattern/,
    );
    assert.throws(
      () =>
        negotiateAnalysisConfig(response({
          constraints: {
            version: 1,
            settings: bundledClientSettings().map((setting, index) =>
              index === 0
                ? { ...(setting as Record<string, unknown>), change_scope: "sometimes" }
                : setting
            ),
          },
        })),
      /invalid change scope/,
    );
    assert.throws(
      () =>
        negotiateAnalysisConfig(response({
          constraints: {
            version: 1,
            settings: bundledClientSettings().map((setting, index) =>
              index === 8
                ? {
                    ...(setting as Record<string, unknown>),
                    normalization: {
                      kind: "rule_severity_overrides",
                      fields: [
                        {
                          name: "rule_id",
                          required: true,
                          normalization: { kind: "string" },
                        },
                        {
                          name: "rule_id",
                          required: false,
                          normalization: { kind: "string" },
                        },
                      ],
                    },
                  }
                : setting
            ),
          },
        })),
      /duplicate .*object fields/,
    );
  });
});
