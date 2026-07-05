import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { normalizeAnalysisSettings, type RawAnalysisSettings } from "../analysis-settings.js";

describe("analysis settings normalization", () => {
  it("keeps only integer analysis values accepted by the LSP parser", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      fixedToday: "2024-02-29",
      fixedLocalOffsetMinutes: -1439,
      maxSourceBytes: 1024,
    }), {
      fixed_today: "2024-02-29",
      fixed_local_offset_minutes: -1439,
      resources: {
        max_source_bytes: 1024,
      },
      lint: {
        profile: "core",
      },
    });
  });

  it("drops invalid fixed_today strings before sending LSP settings", () => {
    for (const fixedToday of ["2026-02-29", "2026-13-01", "20260705"]) {
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

  it("drops fractional and out-of-range numeric values before sending LSP settings", () => {
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      fixedLocalOffsetMinutes: 1439.5,
      maxSourceBytes: 4096.25,
    }), {
      lint: {
        profile: "core",
      },
    });
    assert.deepEqual(normalizeAnalysisSettings({
      ...defaultRawAnalysisSettings(),
      fixedLocalOffsetMinutes: 1440,
      maxSourceBytes: -1,
    }), {
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
});

function defaultRawAnalysisSettings(): RawAnalysisSettings {
  return {
    fixedToday: "",
    fixedLocalOffsetMinutes: null,
    suppressErrors: false,
    maxSourceBytes: 0,
    lintProfile: "core",
    enableRules: [],
    disableRules: [],
    ruleSeverities: [],
  };
}
