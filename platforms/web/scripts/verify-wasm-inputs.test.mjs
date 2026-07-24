import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { parseVerificationTargets } from "./wasm-build/verify-cli.mjs";

describe("WASM artifact freshness CLI", () => {
  it("selects every package-owned WASM artifact", () => {
    const targets = parseVerificationTargets(["--all-packages"]);
    assert.deepEqual(
      targets.map((target) => [target.descriptor.id, target.profile.name, target.outputDir.relative]),
      [
        ["full", "web-full", "pkg/full"],
        ["analysis", "web-analysis", "pkg/analysis"],
        ["render", "web-render", "pkg/render"],
        ["editor", "web-editor", "pkg/editor"],
        ["ascii", "web-ascii", "pkg/ascii"],
      ],
    );
  });

  it("selects the default full package and rejects ambiguous legacy selectors", () => {
    assert.deepEqual(
      parseVerificationTargets([]).map((target) => target.descriptor.id),
      ["full"],
    );
    assert.throws(
      () => parseVerificationTargets(["--all-packages", "--package", "full"]),
      /mutually exclusive/,
    );
    assert.throws(() => parseVerificationTargets(["--package", "missing"]), /Unknown browser package/);
    assert.throws(() => parseVerificationTargets(["--preset", "browser-full"]), /Unknown argument/);
  });
});
