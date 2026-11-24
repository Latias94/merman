import { readFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

import { assertKnownArgs, parseArgValue, resolvePackageSubdir } from "./arg-parse.mjs";
import { WASM_ARTIFACT_PROFILE_NAME } from "./input-manifest.mjs";
import { webPackageRoot } from "./paths.mjs";
import { defaultWebPackage, webPackageDescriptors } from "./web-surface-descriptor.mjs";

const packagesById = new Map(webPackageDescriptors.map((descriptor) => [descriptor.id, descriptor]));

export function parseWasmTargets(args, additionalFlags = []) {
  assertKnownArgs(args, {
    valueArgs: ["--package"],
    booleanArgs: ["--all-packages", "--local-shoal", "--help", "-h", ...additionalFlags],
  });
  const allPackages = args.includes("--all-packages");
  const packageArg = parseArgValue(args, "--package");
  const id = packageArg ?? defaultWebPackage.id;
  const localShoal = args.includes("--local-shoal");
  if (allPackages && packageArg !== null) {
    throw new Error("--all-packages and --package are mutually exclusive.");
  }
  if (localShoal && (allPackages || id !== "full")) {
    throw new Error("--local-shoal requires the full package.");
  }
  if (!allPackages && !packagesById.has(id)) {
    throw new Error(`Unknown browser package ${id}.`);
  }
  const descriptors = allPackages ? webPackageDescriptors : [packagesById.get(id)];
  return descriptors.map((descriptor) => {
    const directory = localShoal ? "full-local-shoal" : descriptor.id;
    return {
      descriptor,
      profile: wasmArtifactProfile(descriptor, localShoal),
      outputDir: resolvePackageSubdir(webPackageRoot, `pkg/${directory}`, `package ${descriptor.id} output`),
    };
  });
}

export function wasmArtifactProfile(descriptor, localShoal = false) {
  if (!descriptor || typeof descriptor !== "object" || !descriptor.artifact_profile) {
    throw new Error("Web package descriptor is invalid.");
  }
  const profile = descriptor.artifact_profile;
  if (localShoal && (descriptor.id !== "full" || profile.id !== "web-full")) {
    throw new Error("--local-shoal requires the full package.");
  }
  const selectedIds = (ids) => localShoal ? [...new Set([...ids, "layout-shoal"])].sort() : [...ids];
  return {
    name: profile.id,
    surface: "web",
    default_features: profile.cargo.default_features,
    features: selectedIds(profile.cargo.features),
    runtime_capability_ids: selectedIds(profile.expected.runtime_ids),
    runtime_output_ids: [...profile.expected.outputs],
  };
}

export function wasmArtifactProfileManifest(descriptor, profile = wasmArtifactProfile(descriptor)) {
  return {
    schema_version: 1,
    package: descriptor.name,
    package_id: descriptor.id,
    artifact_profile: profile.name,
    default_features: profile.default_features,
    features: profile.features,
    runtime_capability_ids: profile.runtime_capability_ids,
    runtime_output_ids: profile.runtime_output_ids,
  };
}

export function assertWasmTargetEvidence({ descriptor, profile, outputDir }, inputManifest) {
  if (!isDeepStrictEqual(inputManifest?.preset, profile)) {
    throw new Error(`WASM preset or feature configuration changed for ${outputDir.relative}.`);
  }
  const file = path.join(outputDir.absolute, WASM_ARTIFACT_PROFILE_NAME);
  const actual = JSON.parse(readFileSync(file, "utf8"));
  if (!isDeepStrictEqual(actual, wasmArtifactProfileManifest(descriptor, profile))) {
    throw new Error(`WASM artifact profile does not match the selected features in ${file}.`);
  }
}
