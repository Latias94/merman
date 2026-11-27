import { readFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

import { assertKnownArgs, parseArgValue, resolvePackageSubdir } from "./arg-parse.mjs";
import { WASM_ARTIFACT_PROFILE_NAME } from "./input-manifest.mjs";
import { webPackageRoot } from "./paths.mjs";
import { defaultWebPackage, webPackageDescriptors } from "./web-surface-descriptor.mjs";

const packagesById = new Map(webPackageDescriptors.map((descriptor) => [descriptor.id, descriptor]));
const LOCAL_LAYOUTS = ["grid", "layered"];
const LOCAL_LAYOUT_FLAG = "--local-layout";
const LOCAL_SHOAL_FLAG = "--local-shoal";

export const LOCAL_WASM_LAYOUT_USAGE = "[--local-shoal | --local-layout <grid|layered>]";

export function parseWasmTargets(args, additionalFlags = []) {
  assertKnownArgs(args, {
    valueArgs: ["--package", LOCAL_LAYOUT_FLAG],
    booleanArgs: ["--all-packages", LOCAL_SHOAL_FLAG, "--help", "-h", ...additionalFlags],
  });
  const allPackages = args.includes("--all-packages");
  const packageArg = parseArgValue(args, "--package");
  const id = packageArg ?? defaultWebPackage.id;
  const localShoal = args.includes(LOCAL_SHOAL_FLAG);
  const localLayout = parseArgValue(args, LOCAL_LAYOUT_FLAG);
  if (localLayout !== null) {
    if (!LOCAL_LAYOUTS.includes(localLayout)) {
      throw new Error(`${LOCAL_LAYOUT_FLAG} must be grid or layered.`);
    }
    if (localShoal) {
      throw new Error(`${LOCAL_LAYOUT_FLAG} and ${LOCAL_SHOAL_FLAG} are mutually exclusive.`);
    }
    if (countValueArgs(args, LOCAL_LAYOUT_FLAG) !== 1 || countValueArgs(args, "--package") > 1) {
      throw new Error(`${LOCAL_LAYOUT_FLAG} requires a single layout and package selection.`);
    }
  }
  if (allPackages && packageArg !== null) {
    throw new Error("--all-packages and --package are mutually exclusive.");
  }
  const local = localShoal ? "shoal" : localLayout;
  const localArgs = localSelectorArgs(local);
  if (local !== null && (allPackages || id !== "full")) {
    throw new Error(`${localArgs[0]} requires the full package.`);
  }
  if (!allPackages && !packagesById.has(id)) {
    throw new Error(`Unknown browser package ${id}.`);
  }
  const descriptors = allPackages ? webPackageDescriptors : [packagesById.get(id)];
  return descriptors.map((descriptor) => {
    const directory = local === null ? descriptor.id : `full-local-${local}`;
    return {
      descriptor,
      profile: wasmArtifactProfile(descriptor, local),
      outputDir: resolvePackageSubdir(webPackageRoot, `pkg/${directory}`, `package ${descriptor.id} output`),
      localArgs,
    };
  });
}

export function parseFullWasmTarget(args) {
  assertKnownArgs(args, { valueArgs: [LOCAL_LAYOUT_FLAG], booleanArgs: [LOCAL_SHOAL_FLAG] });
  return parseWasmTargets(["--package", "full", ...args])[0];
}

export function fullWasmTargetForProvenance(provenance) {
  const full = packagesById.get("full");
  const local = [null, "shoal", ...LOCAL_LAYOUTS].find((layout) => {
    const profile = wasmArtifactProfile(full, layout);
    return provenance?.artifact_profile === profile.name &&
      isDeepStrictEqual(provenance.runtime_capability_ids, profile.runtime_capability_ids) &&
      isDeepStrictEqual(provenance.outputs, profile.runtime_output_ids);
  });
  if (local === undefined) {
    throw new Error("Assembled full Web provenance does not match a known WASM target.");
  }
  return parseFullWasmTarget(localSelectorArgs(local));
}

function localSelectorArgs(local) {
  if (local === null) return [];
  if (local === "shoal") return [LOCAL_SHOAL_FLAG];
  return [LOCAL_LAYOUT_FLAG, local];
}

function countValueArgs(args, flag) {
  return args.filter((arg) => arg === flag || arg.startsWith(`${flag}=`)).length;
}

export function wasmArtifactProfile(descriptor, local = null) {
  if (!descriptor || typeof descriptor !== "object" || !descriptor.artifact_profile) {
    throw new Error("Web package descriptor is invalid.");
  }
  const profile = descriptor.artifact_profile;
  if (local !== null && local !== "shoal" && !LOCAL_LAYOUTS.includes(local)) {
    throw new Error(`${LOCAL_LAYOUT_FLAG} must be grid or layered.`);
  }
  if (local !== null && (descriptor.id !== "full" || profile.id !== "web-full")) {
    throw new Error(`${localSelectorArgs(local)[0]} requires the full package.`);
  }
  const selectedIds = (ids) => local === null ? [...ids] : [...new Set([...ids, `layout-${local}`])].sort();
  return {
    name: local === null || local === "shoal" ? profile.id : `${profile.id}-local-${local}`,
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
