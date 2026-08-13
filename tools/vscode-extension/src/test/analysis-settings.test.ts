import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  bootstrapAnalysisSettings,
  normalizeAnalysisSettings,
  projectAnalysisSettings,
  type RawAnalysisSettings,
} from "../analysis-settings.js";
import {
  BUNDLED_ANALYSIS_CONFIG,
  type AnalysisConfigClientSetting,
  type NegotiatedAnalysisConfig,
} from "../analysis-config-contract.js";

describe("analysis settings normalization", () => {
  it("bootstraps only generated snapshot-affecting settings before documents open", () => {
    assert.deepEqual(bootstrapAnalysisSettings(rawSettings({
      fixed_today: "2026-08-12",
      "resources.limits.max_source_bytes": 8 * 1024 * 1024,
      "resources.limits.max_document_diagrams": 512,
      "lint.profile": "recommended",
      "lint.enable_rules": ["merman.parse.no_diagram"],
    })), {
      fixed_today: "2026-08-12",
      resources: {
        limits: {
          max_source_bytes: 8 * 1024 * 1024,
          max_document_diagrams: 512,
        },
      },
    });
  });

  it("rejects invalid bootstrap dates and numeric ranges before initialize", () => {
    for (const fixedToday of ["2026-02-29", "2026-13-01", "20260812"]) {
      assert.deepEqual(bootstrapAnalysisSettings(rawSettings({
        fixed_today: fixedToday,
      })), {});
    }

    assert.deepEqual(bootstrapAnalysisSettings(rawSettings({
      fixed_today: "-2147483648-01-01",
      fixed_local_offset_minutes: 1439,
      "resources.limits.max_source_bytes": 0,
      "resources.limits.max_document_diagrams": 0x1_0000_0000,
    })), {
      fixed_local_offset_minutes: 1439,
    });

    assert.deepEqual(bootstrapAnalysisSettings(rawSettings({
      fixed_local_offset_minutes: 1440,
      "resources.limits.max_source_bytes": 4096.5,
      "resources.limits.max_document_diagrams": -1,
    })), {});
  });

  it("uses the same bundled constraints for negotiated normalization", () => {
    assert.deepEqual(normalizeAnalysisSettings(rawSettings({
      fixed_today: "2024-02-29",
      fixed_local_offset_minutes: -1439,
      site_config: {
        theme: "dark",
        flowchart: { htmlLabels: false },
      },
      "resources.limits.max_source_bytes": 1024,
      "resources.limits.max_document_diagrams": 256,
    }), BUNDLED_ANALYSIS_CONFIG), {
      fixed_today: "2024-02-29",
      fixed_local_offset_minutes: -1439,
      site_config: {
        theme: "dark",
        flowchart: { htmlLabels: false },
      },
      resources: {
        limits: {
          max_source_bytes: 1024,
          max_document_diagrams: 256,
        },
      },
      lint: { profile: "core" },
    });
  });

  it("keeps representable wide dates and drops unrepresentable signed-year boundaries", () => {
    for (const fixedToday of ["+10000-01-01", "-10000-01-01"]) {
      assert.equal(normalizeAnalysisSettings(rawSettings({
        fixed_today: fixedToday,
      }), BUNDLED_ANALYSIS_CONFIG).fixed_today, fixedToday);
    }

    for (const fixedToday of ["+2147483647-12-31", "-2147483648-01-01"]) {
      assert.equal(normalizeAnalysisSettings(rawSettings({
        fixed_today: fixedToday,
      }), BUNDLED_ANALYSIS_CONFIG).fixed_today, undefined);
    }
  });

  it("omits invalid object values and preserves the owner-defined zero diagram limit", () => {
    for (const siteConfig of [null, [], "dark"]) {
      assert.deepEqual(normalizeAnalysisSettings(rawSettings({
        site_config: siteConfig,
        "resources.limits.max_document_diagrams": 0,
      }), BUNDLED_ANALYSIS_CONFIG), {
        resources: { limits: { max_document_diagrams: 0 } },
        lint: { profile: "core" },
      });
    }
  });

  it("omits unset values so the connected server owns defaults", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      fixed_today: undefined,
      fixed_local_offset_minutes: undefined,
      site_config: undefined,
      "resources.limits.max_source_bytes": undefined,
      "resources.limits.max_document_diagrams": undefined,
      "lint.profile": undefined,
      "lint.enable_rules": undefined,
      "lint.disable_rules": undefined,
      "lint.rule_severities": undefined,
    }, BUNDLED_ANALYSIS_CONFIG), {});
  });

  it("accepts additive catalogs and per-setting ranges from a connected server", () => {
    const contract: NegotiatedAnalysisConfig = {
      ...BUNDLED_ANALYSIS_CONFIG,
      profiles: [...BUNDLED_ANALYSIS_CONFIG.profiles, "pedantic"],
      severities: [...BUNDLED_ANALYSIS_CONFIG.severities, "notice"],
      settings: [
        ...replaceSetting(
          BUNDLED_ANALYSIS_CONFIG.settings,
          "resources.limits.max_source_bytes",
          (setting) => ({
            ...setting,
            normalization: { kind: "integer", minimum: 8, maximum: 2048 },
          }),
        ),
        {
          path: "lint.future_config",
          changeScope: "diagnostics_only",
          runtimeConstraints: [],
          normalization: { kind: "object" },
        },
      ],
    };

    assert.deepEqual(normalizeAnalysisSettings(rawSettings({
      "resources.limits.max_source_bytes": 8,
      "lint.profile": "pedantic",
      "lint.future_config": { mode: "fast" },
      "lint.rule_severities": [
        { rule_id: "merman.future.rule", severity: "notice" },
      ],
    }), contract), {
      resources: { limits: { max_source_bytes: 8 } },
      lint: {
        profile: "pedantic",
        future_config: { mode: "fast" },
        rule_severities: [{ rule_id: "merman.future.rule", severity: "notice" }],
      },
    });
  });

  it("rejects surrounding whitespace instead of rewriting contract strings", () => {
    assert.deepEqual(normalizeAnalysisSettings(rawSettings({
      fixed_today: " 2026-08-12",
      "lint.profile": "core ",
      "lint.enable_rules": [" merman.parse.no_diagram"],
      "lint.disable_rules": ["merman.parse.no_diagram "],
      "lint.rule_severities": [
        { rule_id: " merman.parse.no_diagram", severity: "warning" },
        { rule_id: "merman.parse.no_diagram", severity: " warning" },
        { rule_id: "merman.parse.no_diagram", severity: "warning" },
      ],
    }), BUNDLED_ANALYSIS_CONFIG), {
      lint: {
        rule_severities: [
          { rule_id: "merman.parse.no_diagram", severity: "warning" },
        ],
      },
    });

    assert.deepEqual(bootstrapAnalysisSettings(rawSettings({
      fixed_today: "2026-08-12 ",
    })), {});
  });

  it("projects rule settings through the connected server catalog", () => {
    const settings = normalizeAnalysisSettings(rawSettings({
      "resources.limits.max_document_diagrams": 12,
      "lint.enable_rules": ["merman.parse.no_diagram", "merman.unsupported.rule"],
      "lint.disable_rules": ["merman.config.invalid_theme_color", "merman.parse.no_diagram"],
      "lint.rule_severities": [
        { rule_id: "merman.unsupported.rule", severity: "warning" },
        { rule_id: "merman.parse.no_diagram", severity: "hint" },
      ],
    }), BUNDLED_ANALYSIS_CONFIG);

    assert.deepEqual(projectAnalysisSettings(settings, ["merman.parse.no_diagram"]), {
      settings: {
        resources: { limits: { max_document_diagrams: 12 } },
        lint: {
          profile: "core",
          enable_rules: ["merman.parse.no_diagram"],
          disable_rules: ["merman.parse.no_diagram"],
          rule_severities: [{ rule_id: "merman.parse.no_diagram", severity: "hint" }],
        },
      },
      unsupportedRuleIds: [
        "merman.unsupported.rule",
        "merman.config.invalid_theme_color",
      ],
    });
  });
});

function rawSettings(overrides: RawAnalysisSettings = {}): RawAnalysisSettings {
  return {
    fixed_today: "",
    fixed_local_offset_minutes: null,
    site_config: {},
    "resources.limits.max_source_bytes": null,
    "resources.limits.max_document_diagrams": null,
    "lint.profile": "core",
    "lint.enable_rules": [],
    "lint.disable_rules": [],
    "lint.rule_severities": [],
    ...overrides,
  };
}

function replaceSetting(
  settings: readonly AnalysisConfigClientSetting[],
  path: string,
  replace: (setting: AnalysisConfigClientSetting) => AnalysisConfigClientSetting,
): AnalysisConfigClientSetting[] {
  return settings.map((setting) => setting.path === path ? replace(setting) : setting);
}
