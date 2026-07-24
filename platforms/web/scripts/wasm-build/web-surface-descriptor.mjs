import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { repositoryRoot } from "./paths.mjs";

export const WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION = 1;

const RUNTIME_PROFILES = new Set([
  "analysis",
  "render",
  "ascii",
  "editor",
  "full",
]);
const VISIBILITIES = new Set(["candidate", "public"]);
const descriptorPath = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "web-surface-descriptor.json",
);
const artifactProfilesPath = path.join(
  repositoryRoot,
  "capabilities",
  "artifact-profiles-v1.json",
);

export function parseWebSurfaceDescriptor(value) {
  const descriptor = expectRecord(value, "Web package descriptor");
  assertExactKeys(
    descriptor,
    ["schema_version", "default_package", "packages"],
    "Web package descriptor",
  );
  if (descriptor.schema_version !== WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION) {
    throw new Error(
      `Web package descriptor schema must be ${WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION}.`,
    );
  }

  const packages = expectArray(descriptor.packages, "packages").map(parsePackage);
  const packageIds = assertUnique(packages, (item) => item.id, "package ID");
  assertUnique(packages, (item) => item.name, "package name");
  assertUnique(packages, (item) => item.package_dir, "package directory");
  assertUnique(packages, (item) => item.artifact_profile, "artifact profile");

  const defaultPackage = expectName(descriptor.default_package, "default_package");
  if (!packageIds.has(defaultPackage)) {
    throw new Error(`default_package references unknown package ${defaultPackage}.`);
  }
  const defaultDescriptor = packages.find((item) => item.id === defaultPackage);
  if (defaultDescriptor.visibility !== "public") {
    throw new Error("default_package must reference a public package.");
  }
  if (defaultDescriptor.name !== "@mermanjs/web") {
    throw new Error("default_package must own @mermanjs/web.");
  }

  return deepFreeze({
    schema_version: WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION,
    default_package: defaultPackage,
    packages,
  });
}

export function loadWebSurfaceDescriptor(file = descriptorPath) {
  return parseWebSurfaceDescriptor(readJson(file, "Web package descriptor"));
}

export function loadWebArtifactProfiles(file = artifactProfilesPath) {
  const descriptor = expectRecord(readJson(file, "Artifact profile descriptor"), "Artifact profile descriptor");
  if (!Array.isArray(descriptor.profiles)) {
    throw new Error("Artifact profile descriptor profiles must be an array.");
  }
  const profiles = descriptor.profiles
    .map((profile, index) => parseWebArtifactProfile(profile, index))
    .filter((profile) => profile !== null);
  assertUnique(profiles, (profile) => profile.id, "artifact profile ID");
  return new Map(profiles.map((profile) => [profile.id, profile]));
}

export function resolveWebPackages({
  descriptor = webSurfaceDescriptor,
  artifactProfiles = loadWebArtifactProfiles(),
} = {}) {
  const packages = descriptor.packages.map((item) => {
    const artifactProfile = artifactProfiles.get(item.artifact_profile);
    if (!artifactProfile) {
      throw new Error(
        `Web package ${item.id} references unknown artifact profile ${item.artifact_profile}.`,
      );
    }
    return deepFreeze({ ...item, artifact_profile: artifactProfile });
  });
  const ownedProfileIds = new Set(packages.map((item) => item.artifact_profile.id));
  const unownedProfileIds = [...artifactProfiles.keys()].filter(
    (id) => !ownedProfileIds.has(id),
  );
  if (unownedProfileIds.length > 0) {
    throw new Error(
      `Web artifact profiles must each be owned by one package descriptor; unowned: ${unownedProfileIds.join(", ")}.`,
    );
  }
  return packages;
}

export const webSurfaceDescriptor = loadWebSurfaceDescriptor();
export const webPackageDescriptors = resolveWebPackages();
export const publicWebPackageDescriptors = webPackageDescriptors.filter(
  (item) => item.visibility === "public",
);
export const defaultWebPackage = webPackageDescriptors.find(
  (item) => item.id === webSurfaceDescriptor.default_package,
);

function parsePackage(value, index) {
  const item = expectRecord(value, `packages[${index}]`);
  assertExactKeys(
    item,
    ["id", "name", "package_dir", "artifact_profile", "runtime_profile", "visibility"],
    `packages[${index}]`,
  );
  const id = expectName(item.id, `packages[${index}].id`);
  const name = expectPackageName(item.name, `packages[${index}].name`);
  const packageDir = expectPackageDir(item.package_dir, `packages[${index}].package_dir`);
  const artifactProfile = expectName(item.artifact_profile, `packages[${index}].artifact_profile`);
  const runtimeProfile = expectName(item.runtime_profile, `packages[${index}].runtime_profile`);
  const visibility = expectName(item.visibility, `packages[${index}].visibility`);

  if (packageDir !== `packages/${id}`) {
    throw new Error(`Web package ${id} package_dir must be packages/${id}.`);
  }
  if (artifactProfile !== `web-${id}`) {
    throw new Error(`Web package ${id} artifact_profile must be web-${id}.`);
  }
  if (!RUNTIME_PROFILES.has(runtimeProfile)) {
    throw new Error(`Web package ${id} has unknown runtime profile ${runtimeProfile}.`);
  }
  if (!VISIBILITIES.has(visibility)) {
    throw new Error(`Web package ${id} has unknown visibility ${visibility}.`);
  }
  if (visibility === "candidate" && runtimeProfile !== "render") {
    throw new Error(`Only the render package may be a candidate Web package.`);
  }

  return {
    id,
    name,
    package_dir: packageDir,
    artifact_profile: artifactProfile,
    runtime_profile: runtimeProfile,
    visibility,
  };
}

function parseWebArtifactProfile(value, index) {
  const profile = expectRecord(value, `artifact profiles[${index}]`);
  const id = expectName(profile.id, `artifact profiles[${index}].id`);
  const semanticTarget = expectName(
    profile.semantic_target,
    `Artifact profile ${id} semantic target`,
  );
  if (semanticTarget !== "web") {
    return null;
  }
  if (!id.startsWith("web-")) {
    throw new Error(`Web artifact profile ${id} must use the web-* namespace.`);
  }
  const cargo = expectRecord(profile.cargo, `Artifact profile ${id} cargo`);
  const expected = expectRecord(profile.expected, `Artifact profile ${id} expected`);
  if (cargo.package !== "merman-wasm") {
    throw new Error(`Artifact profile ${id} must build merman-wasm.`);
  }
  if (cargo.profile !== "wasm-size" || cargo.default_features !== false) {
    throw new Error(`Artifact profile ${id} must use the exact wasm-size no-default-features recipe.`);
  }
  const target = expectRecord(cargo.target, `Artifact profile ${id} cargo target`);
  if (target.name !== "merman_wasm") {
    throw new Error(`Artifact profile ${id} must target merman_wasm.`);
  }
  const buildTarget = expectRecord(cargo.build_target, `Artifact profile ${id} build target`);
  if (
    buildTarget.kind !== "target-set" ||
    !Array.isArray(buildTarget.triples) ||
    buildTarget.triples.length !== 1 ||
    buildTarget.triples[0] !== "wasm32-unknown-unknown"
  ) {
    throw new Error(`Artifact profile ${id} must target only wasm32-unknown-unknown.`);
  }
  const features = expectNameArray(cargo.features, `Artifact profile ${id} features`);
  const capabilities = expectNameArray(
    expected.capabilities,
    `Artifact profile ${id} capabilities`,
    true,
  );
  const runtimeIds = expectNameArray(expected.runtime_ids, `Artifact profile ${id} runtime IDs`);
  const outputs = expectNameArray(expected.outputs, `Artifact profile ${id} outputs`, true);
  return {
    id,
    cargo: { default_features: false, features },
    expected: { capabilities, runtime_ids: runtimeIds, outputs },
  };
}

function readJson(file, label) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(
      `Failed to read ${label} ${file}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

function expectRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
  return value;
}

function expectArray(value, label, allowEmpty = false) {
  if (!Array.isArray(value) || (!allowEmpty && value.length === 0)) {
    throw new Error(`${label} must be ${allowEmpty ? "an" : "a non-empty"} array.`);
  }
  return value;
}

function expectNameArray(value, label, allowEmpty = false) {
  const values = expectArray(value, label, allowEmpty).map((item, index) =>
    expectName(item, `${label}[${index}]`),
  );
  assertSortedUnique(values, label);
  return values;
}

function expectPackageDir(value, label) {
  if (typeof value !== "string" || !/^packages\/[a-z0-9][a-z0-9-]*$/.test(value)) {
    throw new Error(`${label} must be a package-relative directory.`);
  }
  return value;
}

function expectPackageName(value, label) {
  if (typeof value !== "string" || !/^@mermanjs\/web(?:-[a-z0-9][a-z0-9-]*)?$/.test(value)) {
    throw new Error(`${label} must be an @mermanjs/web package name.`);
  }
  return value;
}

function expectName(value, label) {
  if (typeof value !== "string" || !/^[a-z0-9][a-z0-9-]*$/.test(value)) {
    throw new Error(`${label} must be a lowercase kebab-case name.`);
  }
  return value;
}

function assertExactKeys(record, expected, label) {
  const actual = Object.keys(record).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((key, index) => key !== wanted[index])
  ) {
    throw new Error(`${label} keys must be exactly: ${wanted.join(", ")}.`);
  }
}

function assertUnique(values, keyFor, label) {
  const keys = new Set();
  for (const value of values) {
    const key = keyFor(value);
    if (keys.has(key)) throw new Error(`Duplicate ${label}: ${key}.`);
    keys.add(key);
  }
  return keys;
}

function assertSortedUnique(values, label) {
  for (let index = 1; index < values.length; index += 1) {
    if (values[index - 1] >= values[index]) {
      throw new Error(`${label} must be sorted and unique.`);
    }
  }
}

function deepFreeze(value) {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value)) deepFreeze(child);
  }
  return value;
}
