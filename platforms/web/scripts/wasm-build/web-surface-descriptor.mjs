import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION = 1;

const CAPABILITY_NAMES = Object.freeze([
  "render",
  "analysis",
  "ascii",
  "core_host",
  "cytoscape_layout",
  "elk_layout",
  "ratex_math",
  "editor_language",
]);
const RUNTIME_PROFILES = new Set([
  "core",
  "render",
  "render-only",
  "ascii",
  "editor",
  "full",
]);
const descriptorPath = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "web-surface-descriptor.json",
);

export function parseWebSurfaceDescriptor(value) {
  const descriptor = expectRecord(value, "Web surface descriptor");
  assertExactKeys(descriptor, [
    "schema_version",
    "default_preset",
    "presets",
    "public_surfaces",
  ], "Web surface descriptor");
  if (descriptor.schema_version !== WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION) {
    throw new Error(
      `Web surface descriptor schema must be ${WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION}.`,
    );
  }

  const presets = expectArray(descriptor.presets, "presets").map(parsePreset);
  const presetNames = assertUnique(presets, (preset) => preset.name, "preset name");
  const defaultPreset = expectName(descriptor.default_preset, "default_preset");
  if (!presetNames.has(defaultPreset)) {
    throw new Error(`default_preset references unknown preset ${defaultPreset}.`);
  }

  const publicSurfaces = expectArray(
    descriptor.public_surfaces,
    "public_surfaces",
  ).map((surface, index) => parsePublicSurface(surface, index, presetNames));
  assertUnique(publicSurfaces, (surface) => surface.entry, "public surface entry");
  assertUnique(publicSurfaces, (surface) => surface.preset, "public surface preset");
  assertUnique(publicSurfaces, (surface) => surface.pkg_dir_rel, "public package directory");

  return deepFreeze({
    schema_version: WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION,
    default_preset: defaultPreset,
    presets,
    public_surfaces: publicSurfaces,
  });
}

export function loadWebSurfaceDescriptor(file = descriptorPath) {
  let value;
  try {
    value = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(
      `Failed to read Web surface descriptor ${file}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return parseWebSurfaceDescriptor(value);
}

export const webSurfaceDescriptor = loadWebSurfaceDescriptor();
export const webPresetDescriptors = webSurfaceDescriptor.presets;
export const publicWebSurfaceDescriptors =
  webSurfaceDescriptor.public_surfaces;
export const defaultWebPresetName = webSurfaceDescriptor.default_preset;

function parsePreset(value, index) {
  const preset = expectRecord(value, `presets[${index}]`);
  assertExactKeys(preset, [
    "name",
    "surface",
    "default_features",
    "features",
    "capabilities",
  ], `presets[${index}]`);

  const name = expectName(preset.name, `presets[${index}].name`);
  if (preset.surface !== "browser") {
    throw new Error(`Preset ${name} must declare surface browser.`);
  }
  if (typeof preset.default_features !== "boolean") {
    throw new Error(`Preset ${name} default_features must be boolean.`);
  }
  const features = expectArray(
    preset.features,
    `Preset ${name} features`,
    true,
  ).map((feature, featureIndex) =>
    expectName(feature, `Preset ${name} features[${featureIndex}]`)
  );
  assertUnique(features, (feature) => feature, `Preset ${name} feature`);

  const capabilities = expectRecord(
    preset.capabilities,
    `Preset ${name} capabilities`,
  );
  assertExactKeys(
    capabilities,
    CAPABILITY_NAMES,
    `Preset ${name} capabilities`,
  );
  for (const capability of CAPABILITY_NAMES) {
    if (typeof capabilities[capability] !== "boolean") {
      throw new Error(
        `Preset ${name} capability ${capability} must be boolean.`,
      );
    }
  }

  return {
    name,
    surface: "browser",
    default_features: preset.default_features,
    features,
    capabilities: { ...capabilities },
  };
}

function parsePublicSurface(value, index, presetNames) {
  const surface = expectRecord(value, `public_surfaces[${index}]`);
  assertExactKeys(surface, [
    "entry",
    "preset",
    "pkg_dir_rel",
    "runtime_profile",
  ], `public_surfaces[${index}]`);
  const entry = expectName(surface.entry, `public_surfaces[${index}].entry`);
  const preset = expectName(surface.preset, `Public surface ${entry} preset`);
  const pkgDirRel = expectPackageDir(
    surface.pkg_dir_rel,
    `Public surface ${entry} pkg_dir_rel`,
  );
  const runtimeProfile = expectName(
    surface.runtime_profile,
    `Public surface ${entry} runtime_profile`,
  );
  if (!presetNames.has(preset)) {
    throw new Error(`Public surface ${entry} references unknown preset ${preset}.`);
  }
  if (pkgDirRel !== `pkg/${entry}`) {
    throw new Error(
      `Public surface ${entry} pkg_dir_rel must be pkg/${entry}.`,
    );
  }
  if (!RUNTIME_PROFILES.has(runtimeProfile)) {
    throw new Error(
      `Public surface ${entry} has unknown runtime profile ${runtimeProfile}.`,
    );
  }
  return {
    entry,
    preset,
    pkg_dir_rel: pkgDirRel,
    runtime_profile: runtimeProfile,
  };
}

function expectRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
  return value;
}

function expectArray(value, label, allowEmpty = false) {
  if (!Array.isArray(value) || (!allowEmpty && value.length === 0)) {
    throw new Error(
      `${label} must be ${allowEmpty ? "an" : "a non-empty"} array.`,
    );
  }
  return value;
}

function expectPackageDir(value, label) {
  if (typeof value !== "string" || !/^pkg\/[a-z0-9][a-z0-9-]*$/.test(value)) {
    throw new Error(`${label} must be a package-relative directory.`);
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
    throw new Error(
      `${label} keys must be exactly: ${wanted.join(", ")}.`,
    );
  }
}

function assertUnique(values, keyFor, label) {
  const keys = new Set();
  for (const value of values) {
    const key = keyFor(value);
    if (keys.has(key)) {
      throw new Error(`Duplicate ${label}: ${key}.`);
    }
    keys.add(key);
  }
  return keys;
}

function deepFreeze(value) {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value)) {
      deepFreeze(child);
    }
  }
  return value;
}
