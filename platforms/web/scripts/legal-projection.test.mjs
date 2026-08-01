import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  legalProjectionForArtifactProfile,
  validateScopedRustReport,
} from "./legal-projection.mjs";
import { webPackages } from "./surface-manifest.mjs";

test("every browser artifact profile has an exact scoped legal projection", () => {
  for (const descriptor of webPackages) {
    const profileId = descriptor.artifact_profile.id;
    const projection = legalProjectionForArtifactProfile(profileId);

    assert.equal(projection.scopeId, profileId);
    assert.ok(projection.componentIds.length > 0);
    assert.ok(projection.files.length > 0);
    const rustReports = projection.files.filter(
      (file) => file.relative === "rust-cargo-dependencies.json",
    );
    assert.equal(rustReports.length, 1);
    assert.match(
      rustReports[0].source,
      new RegExp(`platforms/web/legal/rust-cargo-dependencies/${profileId}\\.json$`),
    );
    assert.deepEqual(
      projection.excludedExternalMaterials.map((material) => material.relative),
      [],
    );
    assert.doesNotMatch(projection.notice, /Explicit Scope Exclusions/);
    assert.match(projection.notice, /Exact Rust Dependency Closure/);
    assert.match(projection.notice, new RegExp(`scope \`${profileId}\``));
  }
});

test("browser scopes select distinct reports bound to their artifact recipes", () => {
  const sources = new Set();
  for (const descriptor of webPackages) {
    const profileId = descriptor.artifact_profile.id;
    const projection = legalProjectionForArtifactProfile(profileId);
    const reportFile = projection.files.find(
      (file) => file.relative === "rust-cargo-dependencies.json",
    );
    assert.ok(reportFile);
    sources.add(reportFile.source);
    const report = JSON.parse(readFileSync(reportFile.source, "utf8"));
    assert.equal(report.artifact_profile.id, profileId);
    assert.equal(report.artifact_profile.cargo.package, "merman-wasm");
    assert.equal(
      report.artifact_profile.cargo.manifest,
      "crates/merman-wasm/Cargo.toml",
    );
    assert.equal(report.artifact_profile.cargo.profile, "wasm-size");
    assert.equal(report.artifact_profile.cargo.default_features, false);
    assert.deepEqual(
      report.artifact_profile.cargo.features,
      descriptor.artifact_profile.cargo.features,
    );
    assert.deepEqual(
      report.artifact_profile.cargo.build_target.triples,
      ["wasm32-unknown-unknown"],
    );
  }
  assert.equal(sources.size, webPackages.length);
});

test("slim browser legal scopes exclude full-renderer-only components", () => {
  const full = legalProjectionForArtifactProfile("web-full");
  const analysis = legalProjectionForArtifactProfile("web-analysis");
  const editor = legalProjectionForArtifactProfile("web-editor");
  const ascii = legalProjectionForArtifactProfile("web-ascii");
  const render = legalProjectionForArtifactProfile("web-render");

  assert.equal(full.componentIds.includes("eclipse-elk"), true);
  assert.equal(full.componentIds.includes("ratex"), true);
  assert.equal(full.componentIds.includes("katex-fonts"), true);

  for (const projection of [analysis, editor, ascii]) {
    assert.equal(
      projection.componentIds.includes("eclipse-elk"),
      false,
      `${projection.scopeId} must not carry ELK legal material.`,
    );
    assert.equal(
      projection.componentIds.includes("ratex"),
      false,
      `${projection.scopeId} must not carry RaTeX legal material.`,
    );
  }
  assert.equal(ascii.componentIds.includes("mermaid-ascii"), true);
  assert.equal(render.componentIds.includes("cose-base-v1"), true);
  assert.equal(render.componentIds.includes("eclipse-elk"), true);
  assert.equal(render.componentIds.includes("ratex"), true);
});

test("scoped Rust report validation fails closed on input and closure drift", () => {
  const projection = legalProjectionForArtifactProfile("web-analysis");
  const source = projection.files.find(
    (file) => file.relative === "rust-cargo-dependencies.json",
  ).source;
  const report = JSON.parse(readFileSync(source, "utf8"));
  const expectedProfile = report.artifact_profile;

  const staleInput = structuredClone(report);
  staleInput.generator.cargo_lock_sha256 = "0".repeat(64);
  assert.throws(
    () => validateScopedRustReport(staleInput, expectedProfile, "web-analysis"),
    /generator inputs have drifted/,
  );

  const incomplete = structuredClone(report);
  const removedPackage = incomplete.licenses[0].packages[0];
  incomplete.licenses = incomplete.licenses
    .map((license) => ({
      ...license,
      packages: license.packages.filter(
        (dependency) =>
          dependency.name !== removedPackage.name ||
          dependency.version !== removedPackage.version ||
          dependency.source !== removedPackage.source,
      ),
    }))
    .filter((license) => license.packages.length > 0);
  assert.throws(
    () => validateScopedRustReport(incomplete, expectedProfile, "web-analysis"),
    /incomplete closure/,
  );

  const unknownField = structuredClone(report);
  unknownField.generator.workspace = true;
  assert.throws(
    () => validateScopedRustReport(unknownField, expectedProfile, "web-analysis"),
    /keys must be exactly/,
  );
});

test("artifact recipe comparison is independent of JSON object key order", () => {
  const projection = legalProjectionForArtifactProfile("web-analysis");
  const source = projection.files.find(
    (file) => file.relative === "rust-cargo-dependencies.json",
  ).source;
  const report = JSON.parse(readFileSync(source, "utf8"));
  const expectedProfile = report.artifact_profile;
  const reordered = structuredClone(report);
  reordered.artifact_profile = Object.fromEntries(
    Object.entries(reordered.artifact_profile).reverse(),
  );

  assert.doesNotThrow(() =>
    validateScopedRustReport(reordered, expectedProfile, "web-analysis"),
  );
});
