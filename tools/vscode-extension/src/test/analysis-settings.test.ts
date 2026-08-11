import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { normalizeAnalysisSettings, type RawAnalysisSettings } from "../analysis-settings.js";

describe("analysis settings normalization", () => {
  it("keeps only integer analysis values accepted by the LSP parser", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      fixedToday: "2024-02-29",
      fixedLocalOffsetMinutes: -1439,
      siteConfig: {
        theme: "dark",
        flowchart: {
          htmlLabels: false,
        },
      },
      maxSourceBytes: 1024,
      maxDocumentDiagrams: 256,
    }), {
      fixed_today: "2024-02-29",
      fixed_local_offset_minutes: -1439,
      site_config: {
        theme: "dark",
        flowchart: {
          htmlLabels: false,
        },
      },
      resources: {
        limits: {
          max_source_bytes: 1024,
          max_document_diagrams: 256,
        },
      },
      lint: {
        profile: "core",
      },
    });
  });

  it("drops non-object site_config values before sending LSP settings", () => {
    for (const siteConfig of [null, [], "dark"]) {
      assert.deepEqual(normalizeAnalysisSettings({
        ...defaultRawAnalysisSettings(),
        siteConfig,
      }), {
        lint: {
          profile: "core",
        },
      });
    }
  });

  it("keeps a document-diagram limit without requiring a source-byte override", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      maxDocumentDiagrams: 128,
    }), {
      resources: {
        limits: {
          max_document_diagrams: 128,
        },
      },
      lint: {
        profile: "core",
      },
    });
  });

  it("drops invalid fixed_today strings before sending LSP settings", () => {
    for (const fixedToday of [
      "2026-02-29",
      "2026-13-01",
      "20260705",
      "+2026-08-03",
      "+010000-01-01",
      "-0000-01-01",
      "-010000-01-01",
      "+2147483648-01-01",
      "-2147483649-01-01",
    ]) {
      assert.deepEqual(normalizeAnalysisSettings({
        ...defaultRawAnalysisSettings(),
        fixedToday,
      }), {
        lint: {
          profile: "core",
        },
      });
    }
  });

  it("keeps canonical signed 32-bit civil-year boundaries", () => {
    for (const fixedToday of [
      "+10000-01-01",
      "-10000-01-01",
      "+2147483647-12-31",
      "-2147483648-01-01",
    ]) {
      assert.equal(normalizeAnalysisSettings({
        ...defaultRawAnalysisSettings(),
        fixedToday,
      }).fixed_today, fixedToday);
    }
  });

  it("drops fractional and out-of-range numeric values before sending LSP settings", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      fixedLocalOffsetMinutes: 1439.5,
      maxSourceBytes: 4096.25,
      maxDocumentDiagrams: 256.5,
    }), {
      lint: {
        profile: "core",
      },
    });
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      fixedLocalOffsetMinutes: 1440,
      maxSourceBytes: -1,
      maxDocumentDiagrams: -1,
    }), {
      lint: {
        profile: "core",
      },
    });
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      maxSourceBytes: 0x1_0000_0000,
      maxDocumentDiagrams: 0x1_0000_0000,
    }), {
      lint: {
        profile: "core",
      },
    });
  });

  it("preserves the analysis-owned zero document limit", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      maxDocumentDiagrams: 0,
    }), {
      resources: {
        limits: {
          max_document_diagrams: 0,
        },
      },
      lint: {
        profile: "core",
      },
    });
  });

  it("keeps recommended authoring diagnostics as an explicit opt-in", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      lintProfile: "recommended",
    }), {
      lint: {
        profile: "recommended",
      },
    });
  });

  it("drops rule ids outside the analysis-owned configurable catalog", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      enableRules: [
        "merman.authoring.flowchart.explicit_direction",
        "merman.unknown.rule",
      ],
      disableRules: ["merman.resource.source_bytes_exceeded"],
      ruleSeverities: [
        {
          rule_id: "merman.config.invalid_theme_color",
          severity: "hint",
        },
        {
          rule_id: "merman.internal.panic",
          severity: "warning",
        },
      ],
    }), {
      lint: {
        profile: "core",
        enable_rules: ["merman.authoring.flowchart.explicit_direction"],
        rule_severities: [
          {
            rule_id: "merman.config.invalid_theme_color",
            severity: "hint",
          },
        ],
      },
    });
  });
});

function defaultRawAnalysisSettings(): RawAnalysisSettings {
  return {
    fixedToday: "",
    fixedLocalOffsetMinutes: null,
    siteConfig: {},
    maxSourceBytes: null,
    maxDocumentDiagrams: null,
    lintProfile: "core",
    enableRules: [],
    disableRules: [],
    ruleSeverities: [],
  };
}
