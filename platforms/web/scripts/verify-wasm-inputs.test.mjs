import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  wasmArtifactProfile,
  wasmArtifactProfileManifest,
} from "./wasm-build/build.mjs";
import { collectArtifactEntries, WASM_ARTIFACT_PROFILE_NAME } from "./wasm-build/input-manifest.mjs";
import { assertWasmTargetEvidence } from "./wasm-build/targets.mjs";
import { assertAssembledWasm, parseVerificationTargets } from "./wasm-build/verify-cli.mjs";
import { webPackageDescriptors } from "./wasm-build/web-surface-descriptor.mjs";
import { WASM_RUNTIME_TOP_LEVEL_FILES } from "./wasm-runtime-files.mjs";

describe("WASM artifact freshness CLI", () => {
  it("selects every package-owned WASM artifact", () => {
    const targets = parseVerificationTargets(["--all-packages"]);
    assert.deepEqual(
      targets.map((target) => [
        target.descriptor.id,
        target.profile.name,
        target.outputDir.relative.replaceAll("\\", "/"),
      ]),
      [
        ["full", "web-full", path.posix.join("pkg", "full")],
        ["analysis", "web-analysis", path.posix.join("pkg", "analysis")],
        ["render", "web-render", path.posix.join("pkg", "render")],
        ["editor", "web-editor", path.posix.join("pkg", "editor")],
        ["ascii", "web-ascii", path.posix.join("pkg", "ascii")],
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

  it("selects one isolated full WASM with the exact extra Shoal feature", () => {
    const [standard] = parseVerificationTargets(["--package", "full", "--assembled"]);
    const [local] = parseVerificationTargets(["--package", "full", "--local-shoal", "--assembled"]);
    assert.equal(standard.outputDir.relative, path.join("pkg", "full"));
    assert.equal(local.outputDir.relative, path.join("pkg", "full-local-shoal"));
    assert.deepEqual(local.profile, {
      ...standard.profile,
      features: [...standard.profile.features, "layout-shoal"].sort(),
      runtime_capability_ids: [...standard.profile.runtime_capability_ids, "layout-shoal"].sort(),
    });
    const artifact = wasmArtifactProfileManifest(local.descriptor, local.profile);
    assert.deepEqual(artifact.features, local.profile.features);
    assert.deepEqual(artifact.runtime_capability_ids, local.profile.runtime_capability_ids);
    assert.equal(artifact.artifact_profile, "web-full");
    assert.equal(standard.profile.features.includes("layout-shoal"), false);
  });

  it("rejects Shoal requests for slim packages or the release group before building", () => {
    for (const selection of [["--package", "render"], ["--all-packages"]]) {
      const args = [...selection, "--local-shoal"];
      assert.throws(() => parseVerificationTargets(args), /--local-shoal requires the full package/);
      for (const [script, prefix] of [
        ["./build-wasm.mjs", []],
        ["./build-surface-packages.mjs", ["--assemble"]],
      ]) {
        const result = spawnSync(
          process.execPath,
          [fileURLToPath(new URL(script, import.meta.url)), ...prefix, ...args],
          { encoding: "utf8" },
        );
        assert.equal(result.error, undefined);
        assert.equal(result.status, 2);
        assert.match(result.stderr, /--local-shoal requires the full package/);
      }
    }
  });

  it("checks both manifests and rejects assembled bytes from a different build", (context) => {
    const workspace = mkdtempSync(path.join(os.tmpdir(), "merman-web-shoal-"));
    context.after(() => rmSync(workspace, { recursive: true, force: true }));
    const raw = path.join(workspace, "raw");
    const assembled = path.join(workspace, "artifacts");
    const runtime = path.join(assembled, "wasm");
    mkdirSync(raw);
    mkdirSync(runtime, { recursive: true });
    for (const file of WASM_RUNTIME_TOP_LEVEL_FILES) {
      writeFileSync(path.join(raw, file), file);
      writeFileSync(path.join(runtime, file), file);
    }
    const [standard] = parseVerificationTargets([]);
    const [local] = parseVerificationTargets(["--local-shoal"]);
    const { descriptor, profile } = local;
    const profileFile = path.join(raw, WASM_ARTIFACT_PROFILE_NAME);
    writeFileSync(path.join(raw, "package.json"), "{}");
    const localArtifact = wasmArtifactProfileManifest(descriptor, profile);
    writeFileSync(profileFile, JSON.stringify(localArtifact));
    const input = {
      preset: profile,
      input_digest: "local-build-digest",
      artifacts: collectArtifactEntries(raw),
    };
    writeFileSync(path.join(assembled, "provenance.json"), JSON.stringify({
      artifact_profile: "web-full",
      runtime_capability_ids: localArtifact.runtime_capability_ids,
      outputs: localArtifact.runtime_output_ids,
      wasm: { input_digest: input.input_digest },
    }));

    const outputDir = { absolute: raw, relative: "pkg/full-local-shoal" };
    assert.doesNotThrow(() => assertWasmTargetEvidence({ ...local, outputDir }, input));
    assert.throws(
      () => assertWasmTargetEvidence({ ...standard, outputDir }, input),
      /WASM preset or feature configuration changed/,
    );
    assert.doesNotThrow(() => assertAssembledWasm(profile, input, assembled));
    assert.throws(
      () => assertAssembledWasm(standard.profile, input, assembled),
      /Assembled WASM provenance does not match/,
    );

    writeFileSync(profileFile, JSON.stringify(wasmArtifactProfileManifest(descriptor)));
    assert.throws(
      () => assertWasmTargetEvidence({ ...local, outputDir }, input),
      /WASM artifact profile does not match/,
    );
    writeFileSync(path.join(runtime, "merman_wasm_bg.wasm"), "stale assembly");
    assert.throws(
      () => assertAssembledWasm(profile, input, assembled),
      /Assembled WASM files do not match/,
    );
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
