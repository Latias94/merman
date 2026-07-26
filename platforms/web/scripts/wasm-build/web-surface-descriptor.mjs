import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { repositoryRoot } from "./paths.mjs";

const descriptorPath = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "web-surface-descriptor.json",
);
const descriptorSchemaPath = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "web-surface-descriptor.schema.json",
);
const artifactProfilesPath = path.join(
  repositoryRoot,
  "capabilities",
  "artifact-profiles-v1.json",
);

export function loadWebSurfaceDescriptorSchema(file = descriptorSchemaPath) {
  const schema = readJson(file, "Web package descriptor schema");
  validateDescriptorSchema(schema);
  return deepFreeze(schema);
}

export const webSurfaceDescriptorSchema = loadWebSurfaceDescriptorSchema();
export const WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION =
  webSurfaceDescriptorSchema.properties.schema_version.const;

export function parseWebSurfaceDescriptor(
  value,
  { schema = webSurfaceDescriptorSchema } = {},
) {
  const descriptor = expectRecord(value, "Web package descriptor");
  validateDescriptorSchema(schema);
  validateJsonSchema(descriptor, schema, schema, "Web package descriptor");
  validateDescriptorInvariants(descriptor, schema["x-merman-invariants"]);
  const packages = descriptor.packages.map((item) => ({ ...item }));

  return deepFreeze({
    schema_version: descriptor.schema_version,
    default_package: descriptor.default_package,
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

function validateDescriptorSchema(schema) {
  const descriptorSchema = expectRecord(schema, "Web package descriptor schema");
  assertExactKeys(
    descriptorSchema,
    [
      "$schema",
      "$id",
      "title",
      "type",
      "additionalProperties",
      "required",
      "properties",
      "$defs",
      "x-merman-invariants",
    ],
    "Web package descriptor schema",
  );
  if (descriptorSchema.$schema !== "https://json-schema.org/draft/2020-12/schema") {
    throw new Error("Web package descriptor schema must use JSON Schema draft 2020-12.");
  }
  if (descriptorSchema.type !== "object") {
    throw new Error("Web package descriptor schema root must be an object.");
  }
  validateSchemaNode(descriptorSchema, "Web package descriptor schema", { root: true });

  const invariants = expectRecord(
    descriptorSchema["x-merman-invariants"],
    "Web package descriptor schema invariants",
  );
  assertExactKeys(
    invariants,
    ["uniqueBy", "derivedFields", "conditionalPackages", "defaultPackage"],
    "Web package descriptor schema invariants",
  );
  if (!Array.isArray(invariants.uniqueBy) || !invariants.uniqueBy.every(isString)) {
    throw new Error("Web package descriptor schema uniqueBy must be a string array.");
  }
  if (
    !isRecord(invariants.derivedFields) ||
    !Object.entries(invariants.derivedFields).every(
      ([field, template]) => isString(field) && isString(template),
    )
  ) {
    throw new Error("Web package descriptor schema derivedFields must be string templates.");
  }
  if (!Array.isArray(invariants.conditionalPackages)) {
    throw new Error("Web package descriptor schema conditionalPackages must be an array.");
  }
  invariants.conditionalPackages.forEach((rule, index) => {
    const conditionalRule = expectRecord(
      rule,
      `Web package descriptor conditionalPackages[${index}]`,
    );
    assertExactKeys(
      conditionalRule,
      ["when", "allowed"],
      `Web package descriptor conditionalPackages[${index}]`,
    );
    if (!isRecord(conditionalRule.when) || Object.keys(conditionalRule.when).length === 0) {
      throw new Error(
        `Web package descriptor conditionalPackages[${index}].when must be an object.`,
      );
    }
    if (
      !Array.isArray(conditionalRule.allowed) ||
      !conditionalRule.allowed.every(
        (candidate) => isRecord(candidate) && Object.keys(candidate).length > 0,
      )
    ) {
      throw new Error(
        `Web package descriptor conditionalPackages[${index}].allowed must be an object array.`,
      );
    }
  });
  const defaultRule = expectRecord(
    invariants.defaultPackage,
    "Web package descriptor schema defaultPackage",
  );
  assertExactKeys(
    defaultRule,
    ["referenceField", "targetField", "requiredFields"],
    "Web package descriptor schema defaultPackage",
  );
  if (
    !isString(defaultRule.referenceField) ||
    !isString(defaultRule.targetField) ||
    !isRecord(defaultRule.requiredFields)
  ) {
    throw new Error("Web package descriptor schema defaultPackage is invalid.");
  }
}

function validateSchemaNode(schema, label, { root = false } = {}) {
  const supported = new Set([
    "$ref",
    "type",
    "const",
    "enum",
    "pattern",
    "additionalProperties",
    "required",
    "properties",
    "minItems",
    "items",
  ]);
  if (root) {
    for (const keyword of ["$schema", "$id", "title", "$defs", "x-merman-invariants"]) {
      supported.add(keyword);
    }
  }
  const unknown = Object.keys(schema).filter((keyword) => !supported.has(keyword));
  if (unknown.length > 0) {
    throw new Error(`${label} has unsupported schema keywords: ${unknown.sort().join(", ")}.`);
  }
  if (schema.properties !== undefined) {
    const properties = expectRecord(schema.properties, `${label} properties`);
    for (const [field, child] of Object.entries(properties)) {
      validateSchemaNode(
        expectRecord(child, `${label}.properties.${field}`),
        `${label}.properties.${field}`,
      );
    }
  }
  if (schema.items !== undefined) {
    validateSchemaNode(expectRecord(schema.items, `${label} items`), `${label}.items`);
  }
  if (schema.$defs !== undefined) {
    const definitions = expectRecord(schema.$defs, `${label} $defs`);
    for (const [name, child] of Object.entries(definitions)) {
      validateSchemaNode(expectRecord(child, `${label}.$defs.${name}`), `${label}.$defs.${name}`);
    }
  }
}

function validateJsonSchema(value, schema, root, label) {
  if (schema.$ref !== undefined) {
    const prefix = "#/$defs/";
    if (!isString(schema.$ref) || !schema.$ref.startsWith(prefix)) {
      throw new Error(`${label} has unsupported schema reference ${String(schema.$ref)}.`);
    }
    const definition = root.$defs[schema.$ref.slice(prefix.length)];
    validateJsonSchema(
      value,
      expectRecord(definition, `${label} schema reference ${schema.$ref}`),
      root,
      label,
    );
    return;
  }

  if (Object.hasOwn(schema, "const") && !jsonEqual(value, schema.const)) {
    throw new Error(`${label} must be ${JSON.stringify(schema.const)}.`);
  }
  if (schema.enum !== undefined) {
    if (!Array.isArray(schema.enum) || !schema.enum.some((item) => jsonEqual(value, item))) {
      throw new Error(`${label} must be one of ${JSON.stringify(schema.enum)}.`);
    }
  }

  if (schema.type === "object") {
    const record = expectRecord(value, label);
    const properties = schema.properties ?? {};
    if (!Array.isArray(schema.required) || !schema.required.every(isString)) {
      throw new Error(`${label} schema required must be a string array.`);
    }
    const missing = schema.required.filter((field) => !Object.hasOwn(record, field));
    const unknown = Object.keys(record).filter((field) => !Object.hasOwn(properties, field));
    if (missing.length > 0 || (schema.additionalProperties === false && unknown.length > 0)) {
      const details = [];
      if (missing.length > 0) details.push(`missing ${missing.sort().join(", ")}`);
      if (unknown.length > 0) details.push(`unknown ${unknown.sort().join(", ")}`);
      throw new Error(`${label} keys must be exact (${details.join("; ")}).`);
    }
    for (const [field, child] of Object.entries(properties)) {
      if (Object.hasOwn(record, field)) {
        validateJsonSchema(record[field], child, root, `${label}.${field}`);
      }
    }
  } else if (schema.type === "array") {
    if (!Array.isArray(value)) throw new Error(`${label} must be an array.`);
    const minimum = schema.minItems ?? 0;
    if (!Number.isSafeInteger(minimum) || value.length < minimum) {
      throw new Error(`${label} must contain at least ${String(minimum)} items.`);
    }
    if (schema.items !== undefined) {
      value.forEach((item, index) =>
        validateJsonSchema(item, schema.items, root, `${label}[${index}]`),
      );
    }
  } else if (schema.type === "string") {
    if (!isString(value)) throw new Error(`${label} must be a string.`);
    if (schema.pattern !== undefined) {
      if (!isString(schema.pattern) || !new RegExp(schema.pattern, "u").test(value)) {
        throw new Error(`${label} does not match ${JSON.stringify(schema.pattern)}.`);
      }
    }
  } else if (schema.type !== undefined) {
    throw new Error(`${label} has unsupported schema type ${String(schema.type)}.`);
  }
}

function validateDescriptorInvariants(descriptor, invariants) {
  descriptor.packages.forEach((item, index) => {
    const label = `Web package descriptor packages[${index}]`;
    for (const [field, template] of Object.entries(invariants.derivedFields)) {
      const expected = template.replace(/\{([a-zA-Z0-9_]+)\}/gu, (_match, sourceField) => {
        if (!Object.hasOwn(item, sourceField)) {
          throw new Error(
            `Web package descriptor schema derivedFields references unknown field ${sourceField}.`,
          );
        }
        return String(item[sourceField]);
      });
      if (item[field] !== expected) {
        throw new Error(`${label} ${field} must be ${expected}.`);
      }
    }
    for (const rule of invariants.conditionalPackages) {
      const matches = Object.entries(rule.when).every(
        ([field, expected]) => item[field] === expected,
      );
      const admitted = rule.allowed.some((candidate) =>
        Object.entries(candidate).every(([field, expected]) => item[field] === expected),
      );
      if (matches && !admitted) {
        throw new Error(`${label} package mapping is not admitted by the schema.`);
      }
    }
  });

  for (const field of invariants.uniqueBy) {
    assertUnique(descriptor.packages, (item) => item[field], `package ${field}`);
  }

  const defaultRule = invariants.defaultPackage;
  const reference = descriptor[defaultRule.referenceField];
  const defaultPackage = descriptor.packages.find(
    (item) => item[defaultRule.targetField] === reference,
  );
  if (!defaultPackage) {
    throw new Error(`default_package references unknown package ${reference}.`);
  }
  for (const [field, expected] of Object.entries(defaultRule.requiredFields)) {
    if (defaultPackage[field] !== expected) {
      throw new Error(`default_package ${field} must be ${JSON.stringify(expected)}.`);
    }
  }
}

function isRecord(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function isString(value) {
  return typeof value === "string";
}

function jsonEqual(left, right) {
  return typeof left === typeof right && JSON.stringify(left) === JSON.stringify(right);
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
