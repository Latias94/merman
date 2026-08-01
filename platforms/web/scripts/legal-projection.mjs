import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.join(webRoot, "..", "..");
const componentInventoryPath = path.join(
  repositoryRoot,
  "docs",
  "release",
  "THIRD_PARTY_COMPONENTS.json",
);
const canonicalLegalRoot = path.join(repositoryRoot, "THIRD_PARTY_LICENSES");
const legalPathPrefix = "THIRD_PARTY_LICENSES/";
const scopedLegalPathPrefix = "platforms/web/legal/rust-cargo-dependencies/";
const artifactProfilesPath = path.join(
  repositoryRoot,
  "capabilities",
  "artifact-profiles-v1.json",
);
const cargoLockPath = path.join(repositoryRoot, "Cargo.lock");
const cargoAboutConfigurationPath = path.join(repositoryRoot, "about.toml");

export function legalProjectionForArtifactProfile(artifactProfileId) {
  if (
    typeof artifactProfileId !== "string" ||
    !/^[a-z0-9][a-z0-9-]*$/.test(artifactProfileId)
  ) {
    throw new Error("Web package legal projection requires an artifact profile ID.");
  }

  const inventory = loadComponentInventory();
  const scope = inventory.scopes.get(artifactProfileId);
  if (!scope) {
    throw new Error(
      `Web artifact profile ${artifactProfileId} has no third-party legal scope.`,
    );
  }
  const componentIds = resolveScopeComponents(scope.id, inventory.scopes);
  const components = componentIds.map((id) => {
    const component = inventory.components.get(id);
    if (!component) {
      throw new Error(
        `Third-party legal scope ${scope.id} references missing component ${id}.`,
      );
    }
    return component;
  });
  const scopedMaterials = inventory.scopedExternalMaterials.filter(
    (material) => material.artifactScope === scope.id,
  );
  if (scopedMaterials.length !== 1) {
    throw new Error(
      `Web artifact profile ${scope.id} must own exactly one Rust dependency report.`,
    );
  }
  const scopedMaterial = scopedMaterials[0];
  const externalFiles = [{
    relative: scopedMaterial.relative,
    role: "exact generated Rust dependency inventory",
    sha256: null,
    source: scopedMaterial.source,
  }];
  const files = [...legalFilesForComponents(components), ...externalFiles].sort(
    (left, right) => compareStrings(left.relative, right.relative),
  );

  return {
    componentIds,
    excludedExternalMaterials: [],
    files,
    notice: scopedNotice(scope, components, scopedMaterial),
    scopeId: scope.id,
  };
}

function loadComponentInventory() {
  const inventory = readJson(componentInventoryPath);
  if (
    inventory?.schema_version !== 3 ||
    !Array.isArray(inventory?.artifact_scopes) ||
    !Array.isArray(inventory?.components) ||
    !Array.isArray(inventory?.externally_managed_files) ||
    !Array.isArray(inventory?.scoped_external_materials)
  ) {
    throw new Error("Third-party component inventory has no scopes or components.");
  }

  const components = new Map();
  for (const raw of inventory.components) {
    const component = parseComponent(raw);
    if (components.has(component.id)) {
      throw new Error(`Third-party component inventory repeats ${component.id}.`);
    }
    components.set(component.id, component);
  }

  const scopes = new Map();
  for (const raw of inventory.artifact_scopes) {
    const scope = parseScope(raw);
    if (scopes.has(scope.id)) {
      throw new Error(`Third-party component inventory repeats scope ${scope.id}.`);
    }
    scopes.set(scope.id, scope);
  }
  const artifactProfiles = loadWebArtifactProfileRecipes();
  const scopedExternalMaterials = inventory.scoped_external_materials.map((raw) =>
    parseScopedExternalMaterial(raw, artifactProfiles),
  );
  const mappedScopes = new Set();
  for (const material of scopedExternalMaterials) {
    if (!scopes.has(material.artifactScope)) {
      throw new Error(
        `Scoped legal material ${material.source} references unknown scope ${material.artifactScope}.`,
      );
    }
    if (mappedScopes.has(material.artifactScope)) {
      throw new Error(
        `Scoped legal material repeats artifact scope ${material.artifactScope}.`,
      );
    }
    mappedScopes.add(material.artifactScope);
  }
  const missingProfiles = [...artifactProfiles.keys()].filter(
    (profileId) => !mappedScopes.has(profileId),
  );
  if (missingProfiles.length > 0 || mappedScopes.size !== artifactProfiles.size) {
    throw new Error(
      `Scoped legal materials must exactly cover Web profiles; missing: ${missingProfiles.join(", ")}.`,
    );
  }
  return { components, scopedExternalMaterials, scopes };
}

function parseScopedExternalMaterial(raw, artifactProfiles) {
  const material = expectRecord(raw, "Scoped externally managed legal material");
  assertExactKeys(
    material,
    ["artifact_scope", "path", "projection_path", "owner", "required", "format"],
    "Scoped externally managed legal material",
  );
  const artifactScope = expectSlug(
    material.artifact_scope,
    "Scoped externally managed legal material artifact scope",
  );
  const declaredPath = expectString(
    material.path,
    `Scoped legal material ${artifactScope} path`,
  );
  if (
    declaredPath !== `${scopedLegalPathPrefix}${artifactScope}.json` ||
    declaredPath.includes("\\") ||
    declaredPath.split("/").includes("..")
  ) {
    throw new Error(
      `Scoped legal material ${artifactScope} must remain in ${scopedLegalPathPrefix}.`,
    );
  }
  const projectionPath = expectString(
    material.projection_path,
    `Scoped legal material ${artifactScope} projection path`,
  );
  if (projectionPath !== `${legalPathPrefix}rust-cargo-dependencies.json`) {
    throw new Error(
      `Scoped legal material ${artifactScope} must project rust-cargo-dependencies.json.`,
    );
  }
  if (material.required !== true || material.format !== "json") {
    throw new Error(`Scoped legal material ${artifactScope} must be required JSON.`);
  }
  expectString(material.owner, `Scoped legal material ${artifactScope} owner`);
  const source = path.join(repositoryRoot, ...declaredPath.split("/"));
  if (!existsSync(source) || !statSync(source).isFile() || statSync(source).size === 0) {
    throw new Error(`Missing externally managed legal material: ${declaredPath}.`);
  }
  const artifactProfile = artifactProfiles.get(artifactScope);
  if (!artifactProfile) {
    throw new Error(`Scoped legal material references unknown Web profile ${artifactScope}.`);
  }
  validateScopedRustReport(readJson(source), artifactProfile, artifactScope);
  return {
    artifactScope,
    relative: projectionPath.slice(legalPathPrefix.length),
    source,
  };
}

function loadWebArtifactProfileRecipes() {
  const descriptor = expectRecord(
    readJson(artifactProfilesPath),
    "Artifact profile descriptor",
  );
  const profiles = new Map();
  for (const raw of expectArray(descriptor.profiles, "Artifact profiles")) {
    const profile = expectRecord(raw, "Artifact profile");
    if (profile.semantic_target !== "web") continue;
    const id = expectSlug(profile.id, "Web artifact profile ID");
    if (profiles.has(id)) {
      throw new Error(`Artifact profile descriptor repeats ${id}.`);
    }
    profiles.set(id, parseWebArtifactProfile(profile, id));
  }
  return profiles;
}

function parseWebArtifactProfile(profile, id) {
  const cargo = expectRecord(profile.cargo, `Artifact profile ${id} cargo`);
  assertExactKeys(
    cargo,
    [
      "package",
      "manifest",
      "profile",
      "default_features",
      "features",
      "target",
      "build_target",
    ],
    `Artifact profile ${id} cargo`,
  );
  if (cargo.default_features !== false) {
    throw new Error(`Artifact profile ${id} must disable default features.`);
  }
  const manifest = expectString(cargo.manifest, `Artifact profile ${id} manifest`);
  if (
    path.posix.isAbsolute(manifest) ||
    path.win32.parse(manifest).root !== "" ||
    manifest.includes("\\") ||
    path.posix.normalize(manifest) !== manifest ||
    manifest.split("/").some((part) => !part || part === "." || part === "..")
  ) {
    throw new Error(`Artifact profile ${id} manifest must be repository-relative.`);
  }
  const features = expectStringArray(cargo.features, `Artifact profile ${id} features`);
  const sortedFeatures = [...features].sort(compareStrings);
  if (
    features.length === 0 ||
    new Set(features).size !== features.length ||
    features.some((feature, index) => feature !== sortedFeatures[index])
  ) {
    throw new Error(`Artifact profile ${id} features must be non-empty, unique, and sorted.`);
  }
  const target = expectRecord(cargo.target, `Artifact profile ${id} target`);
  assertExactKeys(
    target,
    ["name", "kinds", "crate_types", "required_features"],
    `Artifact profile ${id} target`,
  );
  const buildTarget = expectRecord(
    cargo.build_target,
    `Artifact profile ${id} build target`,
  );
  assertExactKeys(
    buildTarget,
    ["kind", "triples"],
    `Artifact profile ${id} build target`,
  );
  const triples = expectStringArray(
    buildTarget.triples,
    `Artifact profile ${id} build target triples`,
  );
  if (
    buildTarget.kind !== "target-set" ||
    triples.length !== 1 ||
    triples[0] !== "wasm32-unknown-unknown"
  ) {
    throw new Error(`Artifact profile ${id} must target wasm32-unknown-unknown.`);
  }
  return {
    id,
    semantic_target: "web",
    cargo: {
      package: expectString(cargo.package, `Artifact profile ${id} package`),
      manifest,
      profile: expectString(cargo.profile, `Artifact profile ${id} Cargo profile`),
      default_features: false,
      features,
      target: {
        name: expectString(target.name, `Artifact profile ${id} target name`),
        kinds: expectStringArray(target.kinds, `Artifact profile ${id} target kinds`),
        crate_types: expectStringArray(
          target.crate_types,
          `Artifact profile ${id} target crate types`,
        ),
        required_features: expectStringArray(
          target.required_features,
          `Artifact profile ${id} target required features`,
        ),
      },
      build_target: { kind: "target-set", triples },
    },
  };
}

export function validateScopedRustReport(
  reportValue,
  artifactProfile,
  artifactScope,
) {
  const report = expectRecord(reportValue, `Rust dependency report ${artifactScope}`);
  assertExactKeys(
    report,
    ["schema_version", "artifact_profile", "generator", "dependency_closure", "licenses"],
    `Rust dependency report ${artifactScope}`,
  );
  if (report.schema_version !== 2) {
    throw new Error(`Rust dependency report ${artifactScope} has an unsupported schema.`);
  }
  if (canonicalJson(report.artifact_profile) !== canonicalJson(artifactProfile)) {
    throw new Error(
      `Rust dependency report ${artifactScope} does not match its artifact recipe.`,
    );
  }
  const generator = expectRecord(
    report.generator,
    `Rust dependency report ${artifactScope} generator`,
  );
  assertExactKeys(
    generator,
    [
      "name",
      "version",
      "command_profile",
      "offline",
      "cargo_lock_sha256",
      "configuration_sha256",
      "artifact_profile_sha256",
    ],
    `Rust dependency report ${artifactScope} generator`,
  );
  if (
    generator.name !== "cargo-about" ||
    generator.version !== "0.9.1" ||
    generator.command_profile !== "artifact-profile-runtime" ||
    generator.offline !== true
  ) {
    throw new Error(
      `Rust dependency report ${artifactScope} must use pinned offline cargo-about 0.9.1.`,
    );
  }
  const expectedGeneratorInputs = {
    cargo_lock_sha256: sha256File(cargoLockPath),
    configuration_sha256: sha256File(cargoAboutConfigurationPath),
    artifact_profile_sha256: sha256Text(canonicalJson(artifactProfile)),
  };
  for (const [field, expected] of Object.entries(expectedGeneratorInputs)) {
    if (generator[field] !== expected) {
      throw new Error(
        `Rust dependency report ${artifactScope} generator inputs have drifted.`,
      );
    }
  }
  const licenses = expectArray(
    report.licenses,
    `Rust dependency report ${artifactScope} licenses`,
  );
  if (licenses.length === 0) {
    throw new Error(`Rust dependency report ${artifactScope} has no licenses.`);
  }
  const packages = new Map();
  const licenseOrder = [];
  for (const [licenseIndex, licenseValue] of licenses.entries()) {
    const licenseLabel = `Rust dependency report ${artifactScope} licenses[${licenseIndex}]`;
    const license = expectRecord(licenseValue, licenseLabel);
    assertExactKeys(
      license,
      ["id", "name", "text_sha256", "text", "packages"],
      licenseLabel,
    );
    const licenseId = expectString(license.id, `${licenseLabel} id`);
    expectString(license.name, `${licenseLabel} name`);
    if (typeof license.text !== "string" || license.text.length === 0) {
      throw new Error(`${licenseLabel} text must be a non-empty string.`);
    }
    const textDigest = expectDigest(license.text_sha256, `${licenseLabel} text digest`);
    if (textDigest !== sha256Text(license.text)) {
      throw new Error(`${licenseLabel} text digest does not match its license text.`);
    }
    licenseOrder.push(`${licenseId}\0${textDigest}`);

    const packageOrder = [];
    for (const [packageIndex, dependency] of expectArray(
      license.packages,
      `${licenseLabel} packages`,
    ).entries()) {
      const record = expectRecord(
        dependency,
        `${licenseLabel} packages[${packageIndex}]`,
      );
      const packageLabel = `${licenseLabel} packages[${packageIndex}]`;
      assertExactKeys(
        record,
        [
          "name",
          "version",
          "source",
          "license_expression",
          "authors",
          "repository",
        ],
        packageLabel,
      );
      const key = [
        expectString(record.name, `${packageLabel} name`),
        expectString(record.version, `${packageLabel} version`),
        expectString(record.source, `${packageLabel} source`),
      ].join("\0");
      const authors = expectStringArray(record.authors, `${packageLabel} authors`);
      const sortedAuthors = [...authors].sort(compareStrings);
      if (authors.some((author, index) => author !== sortedAuthors[index])) {
        throw new Error(`${packageLabel} authors must be sorted.`);
      }
      for (const field of ["license_expression", "repository"]) {
        if (record[field] !== null) {
          expectString(record[field], `${packageLabel} ${field}`);
        }
      }
      const existing = packages.get(key);
      if (existing && canonicalJson(existing) !== canonicalJson(record)) {
        throw new Error(`${packageLabel} conflicts with another package record.`);
      }
      packages.set(key, record);
      packageOrder.push(key);
    }
    const sortedPackageOrder = [...packageOrder].sort(compareStrings);
    if (
      packageOrder.length === 0 ||
      new Set(packageOrder).size !== packageOrder.length ||
      packageOrder.some((key, index) => key !== sortedPackageOrder[index])
    ) {
      throw new Error(`${licenseLabel} packages must be non-empty, unique, and sorted.`);
    }
  }
  const sortedLicenseOrder = [...licenseOrder].sort(compareStrings);
  if (
    new Set(licenseOrder).size !== licenseOrder.length ||
    licenseOrder.some((key, index) => key !== sortedLicenseOrder[index])
  ) {
    throw new Error(`Rust dependency report ${artifactScope} licenses must be unique and sorted.`);
  }
  const closure = expectRecord(
    report.dependency_closure,
    `Rust dependency report ${artifactScope} closure`,
  );
  assertExactKeys(
    closure,
    ["package_count", "packages_sha256"],
    `Rust dependency report ${artifactScope} closure`,
  );
  if (
    !Number.isSafeInteger(closure.package_count) ||
    closure.package_count <= 0 ||
    closure.package_count !== packages.size
  ) {
    throw new Error(`Rust dependency report ${artifactScope} has an incomplete closure.`);
  }
  const closureDigest = expectDigest(
    closure.packages_sha256,
    `Rust dependency report ${artifactScope} closure digest`,
  );
  const orderedPackages = [...packages.keys()]
    .sort(compareStrings)
    .map((key) => packages.get(key));
  if (closureDigest !== sha256Text(canonicalJson(orderedPackages))) {
    throw new Error(`Rust dependency report ${artifactScope} has an incomplete closure.`);
  }
}

function parseComponent(raw) {
  const component = expectRecord(raw, "Third-party component");
  const id = expectSlug(component.id, "Third-party component ID");
  const source = expectRecord(component.source, `Third-party component ${id} source`);
  const licenseFiles = expectArray(
    component.license_files,
    `Third-party component ${id} license files`,
  ).map((file) => parseLicenseFile(file, id));

  return {
    id,
    licenseFiles,
    licenseExpression: expectString(
      component.license_expression,
      `Third-party component ${id} license expression`,
    ),
    name: expectString(component.name, `Third-party component ${id} name`),
    notice: expectString(component.notice, `Third-party component ${id} notice`),
    relationships: expectStringArray(
      component.relationships,
      `Third-party component ${id} relationships`,
    ).sort(compareStrings),
    selectedLicense:
      component.selected_license === undefined
        ? null
        : expectString(
            component.selected_license,
            `Third-party component ${id} selected license`,
          ),
    source: {
      commit: expectString(source.commit, `Third-party component ${id} source commit`),
      path: expectString(source.path, `Third-party component ${id} source path`),
      ref: expectString(source.ref, `Third-party component ${id} source ref`),
      repository: expectString(
        source.repository,
        `Third-party component ${id} source repository`,
      ),
    },
    version: expectString(component.version, `Third-party component ${id} version`),
  };
}

function parseLicenseFile(raw, componentId) {
  const file = expectRecord(
    raw,
    `Third-party component ${componentId} legal file`,
  );
  const declaredPath = expectString(
    file.path,
    `Third-party component ${componentId} legal file path`,
  );
  if (
    !declaredPath.startsWith(legalPathPrefix) ||
    declaredPath.includes("\\") ||
    declaredPath.split("/").includes("..")
  ) {
    throw new Error(
      `Third-party component ${componentId} legal file must remain in ${legalPathPrefix}.`,
    );
  }
  const relative = declaredPath.slice(legalPathPrefix.length);
  if (!relative) {
    throw new Error(`Third-party component ${componentId} legal file path is empty.`);
  }
  const source = path.join(canonicalLegalRoot, ...relative.split("/"));
  if (!existsSync(source) || !statSync(source).isFile() || statSync(source).size === 0) {
    throw new Error(`Missing canonical legal file for ${componentId}: ${declaredPath}.`);
  }
  return {
    relative,
    role: expectString(
      file.role,
      `Third-party component ${componentId} legal file role`,
    ),
    sha256: expectString(
      file.sha256,
      `Third-party component ${componentId} legal file SHA-256`,
    ),
    source,
  };
}

function parseScope(raw) {
  const scope = expectRecord(raw, "Third-party artifact scope");
  const id = expectSlug(scope.id, "Third-party artifact scope ID");
  return {
    components: expectStringArray(
      scope.components,
      `Third-party artifact scope ${id} components`,
    ),
    description: expectString(
      scope.description,
      `Third-party artifact scope ${id} description`,
    ),
    extends: expectStringArray(
      scope.extends,
      `Third-party artifact scope ${id} parents`,
    ),
    id,
  };
}

function resolveScopeComponents(scopeId, scopes, visiting = new Set()) {
  if (visiting.has(scopeId)) {
    throw new Error(`Third-party legal scope inheritance cycles at ${scopeId}.`);
  }
  const scope = scopes.get(scopeId);
  if (!scope) {
    throw new Error(`Third-party legal scope ${scopeId} does not exist.`);
  }
  visiting.add(scopeId);
  const componentIds = new Set(scope.components);
  for (const parentId of scope.extends) {
    for (const componentId of resolveScopeComponents(parentId, scopes, visiting)) {
      componentIds.add(componentId);
    }
  }
  visiting.delete(scopeId);
  return [...componentIds].sort(compareStrings);
}

function legalFilesForComponents(components) {
  const files = new Map();
  for (const component of components) {
    for (const file of component.licenseFiles) {
      const existing = files.get(file.relative);
      if (existing && existing.source !== file.source) {
        throw new Error(
          `Third-party legal projection has conflicting material for ${file.relative}.`,
        );
      }
      files.set(file.relative, file);
    }
  }
  return [...files.values()].sort((left, right) =>
    compareStrings(left.relative, right.relative),
  );
}

function scopedNotice(scope, components, scopedMaterial) {
  const lines = [
    "# Third-Party Notices",
    "",
    "This file is generated from `docs/release/THIRD_PARTY_COMPONENTS.json`.",
    `It records the exact legal closure for browser artifact scope \`${scope.id}\`.`,
    "",
    "## Artifact Scope",
    "",
    scope.description,
    "",
    "## Components",
    "",
  ];

  for (const component of components) {
    lines.push(
      `### ${component.name} (\`${component.id}\`)`,
      "",
      component.notice,
      "",
      `- Version: \`${component.version}\``,
      `- Source: <${component.source.repository}>`,
      `- Source ref: \`${component.source.ref}\``,
      `- Source commit: \`${component.source.commit}\``,
      `- Source path: \`${component.source.path}\``,
      `- Relationship: ${component.relationships.map((item) => `\`${item}\``).join(", ")}`,
      `- License expression: \`${component.licenseExpression}\``,
    );
    if (component.selectedLicense !== null) {
      lines.push(`- Selected license path: \`${component.selectedLicense}\``);
    }
    lines.push("- Legal files:");
    for (const file of component.licenseFiles) {
      lines.push(
        `  - [\`THIRD_PARTY_LICENSES/${file.relative}\`](THIRD_PARTY_LICENSES/${file.relative}) (${file.role}, SHA-256 \`${file.sha256}\`)`,
      );
    }
    lines.push("");
  }
  lines.push(
    "## Exact Rust Dependency Closure",
    "",
    `The artifact-profile-specific cargo-about report is [\`THIRD_PARTY_LICENSES/${scopedMaterial.relative}\`](THIRD_PARTY_LICENSES/${scopedMaterial.relative}).`,
    "",
  );
  return `${lines.join("\n")}\n`;
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(
      `Cannot read third-party component inventory ${file}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

function expectRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
  return value;
}

function expectArray(value, label) {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array.`);
  }
  return value;
}

function expectString(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    /[\u0000-\u001f]/.test(value)
  ) {
    throw new Error(`${label} must be a non-empty string.`);
  }
  return value;
}

function expectSlug(value, label) {
  const string = expectString(value, label);
  if (!/^[a-z0-9][a-z0-9-]*$/.test(string)) {
    throw new Error(`${label} must be a lowercase slug.`);
  }
  return string;
}

function expectStringArray(value, label) {
  return expectArray(value, label).map((item, index) =>
    expectString(item, `${label}[${index}]`),
  );
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

function canonicalJson(value) {
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalJson(item)).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort(compareStrings)
      .map((key) => `${jsonStringAscii(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return jsonStringAscii(value);
}

function jsonStringAscii(value) {
  const encoded = JSON.stringify(value);
  if (encoded === undefined) {
    throw new Error("Cannot canonicalize undefined JSON values.");
  }
  return encoded.replace(/[^\x00-\x7f]/g, (character) =>
    `\\u${character.charCodeAt(0).toString(16).padStart(4, "0")}`,
  );
}

function expectDigest(value, label) {
  const digest = expectString(value, label);
  if (!/^[a-f0-9]{64}$/.test(digest)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest.`);
  }
  return digest;
}

function sha256File(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function sha256Text(value) {
  return createHash("sha256").update(value, "utf8").digest("hex");
}

function compareStrings(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}
