import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION,
  loadWebArtifactProfiles,
  parseWebSurfaceDescriptor,
  resolveWebPackages,
  webPackageDescriptors,
  webSurfaceDescriptor,
} from "./web-surface-descriptor.mjs";

test("checked-in Web descriptor owns one closed package graph", () => {
  assert.equal(webSurfaceDescriptor.schema_version, WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION);
  assert.equal(webSurfaceDescriptor.default_package, "full");
  assert.deepEqual(
    webPackageDescriptors.map(({ id, name, visibility }) => [id, name, visibility]),
    [
      ["full", "@mermanjs/web", "public"],
      ["analysis", "@mermanjs/web-analysis", "public"],
      ["render", "@mermanjs/web-render", "candidate"],
      ["editor", "@mermanjs/web-editor", "public"],
      ["ascii", "@mermanjs/web-ascii", "public"],
    ],
  );
  assert.equal(
    webPackageDescriptors.some(
      (item) =>
        item.artifact_profile.cargo.features.includes("math") ||
        item.artifact_profile.expected.capabilities.includes("math") ||
        item.artifact_profile.expected.runtime_ids.includes("math"),
    ),
    false,
  );
});

test("descriptor rejects an unsupported schema", () => {
  const descriptor = cloneDescriptor();
  descriptor.schema_version += 1;
  assert.throws(() => parseWebSurfaceDescriptor(descriptor), /schema must be 1/);
});

test("descriptor rejects duplicate package ownership and dangling defaults", () => {
  const duplicate = cloneDescriptor();
  duplicate.packages.push(structuredClone(duplicate.packages[0]));
  assert.throws(() => parseWebSurfaceDescriptor(duplicate), /Duplicate package ID/);

  const dangling = cloneDescriptor();
  dangling.default_package = "missing";
  assert.throws(() => parseWebSurfaceDescriptor(dangling), /references unknown package/);
});

test("descriptor rejects package mappings that reintroduce a second feature surface", () => {
  const extraKey = cloneDescriptor();
  extraKey.packages[0].features = ["svg"];
  assert.throws(() => parseWebSurfaceDescriptor(extraKey), /keys must be exactly/);

  const mismatchedDirectory = cloneDescriptor();
  mismatchedDirectory.packages[0].package_dir = "packages/other";
  assert.throws(() => parseWebSurfaceDescriptor(mismatchedDirectory), /package_dir must be packages\/full/);

  const nonRenderCandidate = cloneDescriptor();
  nonRenderCandidate.packages[1].visibility = "candidate";
  assert.throws(() => parseWebSurfaceDescriptor(nonRenderCandidate), /Only the render package/);
});

test("descriptor rejects an unowned Web math artifact recipe", () => {
  const artifactProfiles = loadWebArtifactProfiles();
  const full = structuredClone(artifactProfiles.get("web-full"));
  full.id = "web-math";
  full.cargo.features = [...full.cargo.features, "math"].sort();
  full.expected.capabilities = [...full.expected.capabilities, "math"].sort();
  full.expected.runtime_ids = [...full.expected.runtime_ids, "math"].sort();
  artifactProfiles.set(full.id, full);

  assert.throws(
    () => resolveWebPackages({ descriptor: webSurfaceDescriptor, artifactProfiles }),
    /unowned: web-math/,
  );
});

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
