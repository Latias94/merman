import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION,
  loadWebSurfaceDescriptorSchema,
  loadWebArtifactProfiles,
  parseWebSurfaceDescriptor,
  resolveWebPackages,
  webPackageDescriptors,
  webSurfaceDescriptor,
} from "./web-surface-descriptor.mjs";
import { webPackages } from "./surface-manifest.mjs";

test("checked-in Web descriptor owns one capability-complete default package graph", () => {
  assert.equal(webSurfaceDescriptor.schema_version, WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION);
  assert.equal(webSurfaceDescriptor.default_package, "full");
  assert.deepEqual(
    webPackageDescriptors.map(({ id, name, visibility }) => [id, name, visibility]),
    [
      ["full", "@mermanjs/web", "public"],
      ["analysis", "@mermanjs/web-analysis", "public"],
      ["render", "@mermanjs/web-render", "public"],
      ["editor", "@mermanjs/web-editor", "public"],
      ["ascii", "@mermanjs/web-ascii", "public"],
    ],
  );
  assertPackageContract("full", {
    runtimeProfile: "full",
    features: ["analysis", "ascii", "editor", "layout-cytoscape", "layout-elk", "math", "svg"],
    capabilities: ["analysis", "ascii", "editor", "layout-cytoscape", "layout-elk", "math", "svg"],
    runtimeIds: ["analysis", "ascii", "editor", "layout-cytoscape", "layout-elk", "math", "svg"],
    outputs: ["ascii", "svg"],
  });
  assertPackageContract("render", {
    runtimeProfile: "render",
    features: ["layout-cytoscape", "layout-elk", "math", "svg"],
    capabilities: ["layout-cytoscape", "layout-elk", "math", "svg"],
    runtimeIds: ["layout-cytoscape", "layout-elk", "math", "svg"],
    outputs: ["svg"],
  });
});

test("complete SDK preserves workflows while the complete SVG renderer stays isolated", () => {
  const complete = webPackages.find((item) => item.id === "full");
  const renderer = webPackages.find((item) => item.id === "render");
  assert.ok(complete);
  assert.ok(renderer);
  for (const required of ["renderSvg", "analyze", "renderAscii", "createEditorSession"]) {
    assert.equal(
      complete.runtimeExportNames.includes(required),
      true,
      `full must expose ${required}`,
    );
  }
  assert.equal(renderer.runtimeExportNames.includes("renderSvg"), true);
  for (const forbidden of ["analyze", "renderAscii", "createEditorSession"]) {
    assert.equal(
      renderer.runtimeExportNames.includes(forbidden),
      false,
      `render must not expose ${forbidden}`,
    );
  }
});

test("shared loader guidance names every public browser package", () => {
  const source = readFileSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src", "runtime-core.ts"),
    "utf8",
  );
  for (const descriptor of webPackageDescriptors) {
    if (descriptor.visibility === "public") {
      assert.match(source, new RegExp(escapeRegExp(descriptor.name)));
    }
  }
});

test("descriptor rejects an unsupported schema", () => {
  const descriptor = cloneDescriptor();
  descriptor.schema_version += 1;
  assert.throws(() => parseWebSurfaceDescriptor(descriptor), /schema_version must be 1/);
});

test("checked-in schema is the descriptor rule authority", () => {
  const schema = structuredClone(loadWebSurfaceDescriptorSchema());
  const descriptor = cloneDescriptor();
  descriptor.packages[0].runtime_profile = "experimental";

  assert.throws(
    () => parseWebSurfaceDescriptor(descriptor, { schema }),
    /runtime_profile/,
  );

  schema.$defs.package.properties.runtime_profile.enum.push("experimental");
  assert.equal(
    parseWebSurfaceDescriptor(descriptor, { schema }).packages[0].runtime_profile,
    "experimental",
  );

  const candidate = cloneDescriptor();
  candidate.packages[1].visibility = "candidate";
  assert.throws(() => parseWebSurfaceDescriptor(candidate, { schema }), /not admitted/);
  schema["x-merman-invariants"].conditionalPackages[0].allowed.push({
    id: "analysis",
    runtime_profile: "analysis",
  });
  assert.equal(
    parseWebSurfaceDescriptor(candidate, { schema }).packages[1].visibility,
    "candidate",
  );

  const renamedDefault = cloneDescriptor();
  renamedDefault.packages[0].name = "@mermanjs/web-full";
  schema["x-merman-invariants"].defaultPackage.requiredFields.name =
    "@mermanjs/web-full";
  assert.equal(
    parseWebSurfaceDescriptor(renamedDefault, { schema }).packages[0].name,
    "@mermanjs/web-full",
  );
});

test("descriptor schema fails closed on an unknown keyword", () => {
  const schema = structuredClone(loadWebSurfaceDescriptorSchema());
  schema.$defs.package.properties.runtime_profile.maxLength = 32;
  assert.throws(
    () => parseWebSurfaceDescriptor(cloneDescriptor(), { schema }),
    /unsupported schema keywords/,
  );
});

test("descriptor rejects duplicate package ownership and dangling defaults", () => {
  const duplicate = cloneDescriptor();
  duplicate.packages.push(structuredClone(duplicate.packages[0]));
  assert.throws(() => parseWebSurfaceDescriptor(duplicate), /Duplicate package id/);

  const dangling = cloneDescriptor();
  dangling.default_package = "missing";
  assert.throws(() => parseWebSurfaceDescriptor(dangling), /references unknown package/);
});

test("descriptor rejects package mappings that reintroduce a second feature surface", () => {
  const extraKey = cloneDescriptor();
  extraKey.packages[0].features = ["svg"];
  assert.throws(() => parseWebSurfaceDescriptor(extraKey), /keys must be exact/);

  const mismatchedDirectory = cloneDescriptor();
  mismatchedDirectory.packages[0].package_dir = "packages/other";
  assert.throws(() => parseWebSurfaceDescriptor(mismatchedDirectory), /package_dir must be packages\/full/);

  const mismatchedProfile = cloneDescriptor();
  mismatchedProfile.packages[0].artifact_profile = "web-render";
  assert.throws(() => parseWebSurfaceDescriptor(mismatchedProfile), /artifact_profile must be web-full/);

  const unknownRuntime = cloneDescriptor();
  unknownRuntime.packages[0].runtime_profile = "unknown";
  assert.throws(() => parseWebSurfaceDescriptor(unknownRuntime), /runtime_profile/);

  const nonRenderCandidate = cloneDescriptor();
  nonRenderCandidate.packages[1].visibility = "candidate";
  nonRenderCandidate.packages[1].runtime_profile = "render";
  assert.throws(() => parseWebSurfaceDescriptor(nonRenderCandidate), /not admitted by the schema/);
});

test("descriptor rejects an unowned Web math artifact recipe", () => {
  const artifactProfiles = loadWebArtifactProfiles();
  const math = structuredClone(artifactProfiles.get("web-render"));
  math.id = "web-math";
  math.cargo.features = [...math.cargo.features, "math"].sort();
  math.expected.capabilities = [...math.expected.capabilities, "math"].sort();
  math.expected.runtime_ids = [...math.expected.runtime_ids, "math"].sort();
  artifactProfiles.set(math.id, math);

  assert.throws(
    () => resolveWebPackages({ descriptor: webSurfaceDescriptor, artifactProfiles }),
    /unowned: web-math/,
  );
});

function assertPackageContract(
  id,
  { runtimeProfile, features, capabilities, runtimeIds, outputs },
) {
  const descriptor = webPackageDescriptors.find((item) => item.id === id);
  assert.ok(descriptor, `missing Web package ${id}`);
  assert.equal(descriptor.runtime_profile, runtimeProfile);
  assert.deepEqual(descriptor.artifact_profile.cargo.features, features);
  assert.deepEqual(descriptor.artifact_profile.expected.capabilities, capabilities);
  assert.deepEqual(descriptor.artifact_profile.expected.runtime_ids, runtimeIds);
  assert.deepEqual(descriptor.artifact_profile.expected.outputs, outputs);
}

test("artifact loader rejects a Web target outside the web namespace", () => {
  const descriptor = JSON.parse(readFileSync(artifactDescriptorPath(), "utf8"));
  const full = descriptor.profiles.find((profile) => profile.id === "web-full");
  full.id = "browser-math";
  const directory = mkdtempSync(path.join(tmpdir(), "merman-web-artifacts-"));
  const file = path.join(directory, "artifact-profiles.json");
  try {
    writeFileSync(file, JSON.stringify(descriptor));
    assert.throws(
      () => loadWebArtifactProfiles(file),
      /must use the web-\* namespace/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

function cloneDescriptor() {
  const descriptorPath = path.join(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
    "web-surface-descriptor.json",
  );
  return JSON.parse(readFileSync(descriptorPath, "utf8"));
}

function artifactDescriptorPath() {
  return path.join(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
    "..",
    "..",
    "capabilities",
    "artifact-profiles-v1.json",
  );
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
