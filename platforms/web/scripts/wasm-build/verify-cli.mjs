import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

import {
  assertWasmRuntimeMatchesInput,
  wasmRuntimeFileRecords,
} from "../wasm-runtime-files.mjs";
import { hasHelpFlag } from "./arg-parse.mjs";
import {
  WASM_INPUT_MANIFEST_NAME,
  cargoMetadataForPreset,
  currentWasmBuildToolVersions,
  verifyWasmInputManifest,
} from "./input-manifest.mjs";
import { repositoryRoot, webPackageRoot } from "./paths.mjs";
import { assertWasmTargetEvidence, parseWasmTargets } from "./targets.mjs";

export function runVerifyWasmInputsCli(args = process.argv.slice(2)) {
  if (hasHelpFlag(args)) {
    printUsage();
    return;
  }

  let targets;
  try {
    targets = parseVerificationTargets(args);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printUsage();
    process.exitCode = 2;
    return;
  }

  const metadataByProfile = new Map();
  const assembled = args.includes("--assembled");
  let toolVersions;
  const failures = [];
  for (const target of targets) {
    const checked = verifyTarget(target, {
      assembled,
      getMetadata(profile) {
        if (!metadataByProfile.has(profile.name)) {
          metadataByProfile.set(
            profile.name,
            cargoMetadataForPreset({ preset: profile, repoRoot: repositoryRoot }),
          );
        }
        return metadataByProfile.get(profile.name);
      },
      getToolVersions() {
        toolVersions ??= currentWasmBuildToolVersions(repositoryRoot);
        return toolVersions;
      },
    });
    if (checked.ok) {
      console.log(
        `[merman-web] WASM ${assembled ? "inputs and assembly" : "inputs"} verified (${target.outputDir.relative}, ${checked.digest.slice(0, 12)}).`,
      );
    } else {
      failures.push({ target, reasons: checked.reasons });
    }
  }

  if (failures.length === 0) return;
  console.error("[merman-web] WASM artifact is stale or unverifiable.");
  for (const failure of failures) {
    console.error(`  ${failure.target.descriptor.id}:`);
    for (const reason of failure.reasons) console.error(`    - ${reason}`);
  }
  const localShoal = args.includes("--local-shoal");
  const singlePackage = targets.length === 1 ? targets[0].descriptor.id : null;
  const buildScript = localShoal
    ? "build:wasm:full -- --local-shoal"
    : singlePackage ? `build:wasm:${singlePackage}` : "build:wasm";
  console.error(`  Run \`npm --prefix platforms/web run ${buildScript}\` from the repository root if the inputs are stale.`);
  if (assembled) {
    const selectors = singlePackage ? ` -- --package ${singlePackage}${localShoal ? " --local-shoal" : ""}` : "";
    console.error(`  Run \`npm --prefix platforms/web run build:packages${selectors}\` to refresh the package.`);
  }
  process.exitCode = 1;
}

export function parseVerificationTargets(args) {
  return parseWasmTargets(args, ["--assembled"]);
}

function verifyTarget(target, { assembled, getMetadata, getToolVersions }) {
  const manifestPath = path.join(target.outputDir.absolute, WASM_INPUT_MANIFEST_NAME);
  let manifest = null;
  if (existsSync(manifestPath)) {
    try {
      manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    } catch (error) {
      return {
        ok: false,
        reasons: [`WASM input manifest is corrupt: ${error instanceof Error ? error.message : String(error)}`],
      };
    }
  }

  try {
    const result = verifyWasmInputManifest({
      manifest,
      metadata: manifest ? getMetadata(target.profile) : null,
      outputRoot: target.outputDir.absolute,
      preset: target.profile,
      repoRoot: repositoryRoot,
      toolVersions: getToolVersions(),
    });
    if (!result.ok) return result;
    assertWasmTargetEvidence(target, manifest);
    if (assembled) {
      const artifactRoot = path.join(webPackageRoot, target.descriptor.package_dir, "artifacts");
      assertAssembledWasm(target.profile, manifest, artifactRoot);
    }
    return { ...result, digest: manifest.input_digest };
  } catch (error) {
    return {
      ok: false,
      reasons: [error instanceof Error ? error.message : String(error)],
    };
  }
}

export function assertAssembledWasm(profile, input, artifactRoot) {
  const provenance = JSON.parse(readFileSync(path.join(artifactRoot, "provenance.json"), "utf8"));
  if (
    provenance.artifact_profile !== profile.name ||
    !isDeepStrictEqual(provenance.runtime_capability_ids, profile.runtime_capability_ids) ||
    !isDeepStrictEqual(provenance.outputs, profile.runtime_output_ids) ||
    provenance.wasm?.input_digest !== input.input_digest
  ) {
    throw new Error("Assembled WASM provenance does not match the selected build.");
  }
  const records = wasmRuntimeFileRecords(path.join(artifactRoot, "wasm"), { strictTopLevel: true });
  assertWasmRuntimeMatchesInput(records, input.artifacts);
}

function printUsage() {
  console.log("usage: node scripts/verify-wasm-inputs.mjs [--package <id> | --all-packages] [--local-shoal] [--assembled]");
}
