import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import {
  assertKnownArgs,
  hasHelpFlag,
  parseArgValue,
  resolvePackageSubdir,
} from "./arg-parse.mjs";
import {
  WASM_INPUT_MANIFEST_NAME,
  cargoMetadataForPreset,
  currentWasmBuildToolVersions,
  verifyWasmInputManifest,
} from "./input-manifest.mjs";
import { repositoryRoot, webPackageRoot } from "./paths.mjs";
import {
  defaultWebPackage,
  webPackageDescriptors,
} from "./web-surface-descriptor.mjs";
import { wasmArtifactProfile } from "./build.mjs";

const packagesById = new Map(webPackageDescriptors.map((item) => [item.id, item]));

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
  let toolVersions;
  const failures = [];
  for (const target of targets) {
    const checked = verifyTarget(target, {
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
        `[merman-web] WASM inputs verified (${target.descriptor.id}, ${checked.digest.slice(0, 12)}).`,
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
  console.error("  Run `npm --prefix platforms/web run build:wasm` from the repository root.");
  process.exitCode = 1;
}

export function parseVerificationTargets(args) {
  assertKnownArgs(args, {
    valueArgs: ["--package"],
    booleanArgs: ["--all-packages", "--help", "-h"],
  });
  const allPackages = args.includes("--all-packages");
  const id = parseArgValue(args, "--package");
  if (allPackages && id !== null) {
    throw new Error("--all-packages and --package are mutually exclusive.");
  }
  const descriptors = allPackages
    ? webPackageDescriptors
    : [packagesById.get(id ?? defaultWebPackage.id)].filter(Boolean);
  if (descriptors.length === 0) {
    throw new Error(`Unknown browser package ${id}.`);
  }
  return descriptors.map((descriptor) => ({
    descriptor,
    profile: wasmArtifactProfile(descriptor),
    outputDir: resolvePackageSubdir(
      webPackageRoot,
      `pkg/${descriptor.id}`,
      `package ${descriptor.id} output`,
    ),
  }));
}

function verifyTarget(target, { getMetadata, getToolVersions }) {
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
    return { ...result, digest: result.ok ? manifest.input_digest : null };
  } catch (error) {
    return {
      ok: false,
      reasons: [error instanceof Error ? error.message : String(error)],
    };
  }
}

function printUsage() {
  console.log("usage: node scripts/verify-wasm-inputs.mjs [--package <id> | --all-packages]");
}
