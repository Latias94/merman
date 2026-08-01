import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  wasmArtifactProfile,
  wasmArtifactProfileManifest,
} from "./wasm-build/build.mjs";
import { parseVerificationTargets } from "./wasm-build/verify-cli.mjs";
import { webPackageDescriptors } from "./wasm-build/web-surface-descriptor.mjs";

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

  it("carries descriptor runtime outputs into every WASM build evidence layer", () => {
    for (const descriptor of webPackageDescriptors) {
      const profile = wasmArtifactProfile(descriptor);
      assert.deepEqual(
        profile.runtime_output_ids,
        descriptor.artifact_profile.expected.outputs,
        descriptor.id,
      );
      assert.deepEqual(
        wasmArtifactProfileManifest(descriptor).runtime_output_ids,
        descriptor.artifact_profile.expected.outputs,
        descriptor.id,
      );
    }
  });
});
