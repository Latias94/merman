import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  candidateBuildInvocation,
  resolveCandidateRecipe,
} from "../scripts/build-candidate.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");

test("candidate builds project the exact artifact profile leaves plus one transport", async () => {
  const descriptor = await readJson(path.join(nodeRoot, "candidate-builds.json"));
  const artifactProfiles = await readJson(
    path.join(repositoryRoot, descriptor.artifact_profile.descriptor),
  );
  const artifactProfile = artifactProfiles.profiles.find(
    (profile) => profile.id === descriptor.artifact_profile.id,
  );
  assert.ok(artifactProfile);

  const cases = [
    {
      candidate: "node-wasm",
      target: null,
      transportFeature: "transport-wasm",
    },
    {
      candidate: "napi",
      target: "darwin-arm64",
      transportFeature: "transport-napi",
    },
  ];
  for (const item of cases) {
    const recipe = resolveCandidateRecipe(item.candidate, item.target);
    const invocation = candidateBuildInvocation(recipe, "/tmp/merman-node-candidate");
    const featureIndex = invocation.args.indexOf("--features");
    assert.notEqual(featureIndex, -1);
    assert.deepEqual(recipe.capabilityFeatures, artifactProfile.cargo.features);
    assert.deepEqual(
      invocation.args[featureIndex + 1].split(","),
      [...artifactProfile.cargo.features, item.transportFeature].sort(),
    );
    assert.equal(invocation.args.includes("--no-default-features"), true);
    assert.equal(invocation.args.join(" ").includes(descriptor.artifact_profile.id), false);
  }
});

test("merman-node forwards every artifact profile leaf without a private aggregate", async () => {
  const descriptor = await readJson(path.join(nodeRoot, "candidate-builds.json"));
  const artifactProfiles = await readJson(
    path.join(repositoryRoot, descriptor.artifact_profile.descriptor),
  );
  const artifactProfile = artifactProfiles.profiles.find(
    (profile) => profile.id === descriptor.artifact_profile.id,
  );
  assert.ok(artifactProfile);

  const manifest = await readFile(
    path.join(repositoryRoot, descriptor.cargo.manifest),
    "utf8",
  );
  const features = declaredCargoFeatures(manifest);
  for (const feature of [
    ...artifactProfile.cargo.features,
    "transport-napi",
    "transport-wasm",
  ]) {
    assert.equal(features.has(feature), true, `missing Cargo feature ${feature}`);
  }
  assert.equal(features.has(descriptor.artifact_profile.id), false);
});

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

function declaredCargoFeatures(manifest) {
  const marker = "[features]\n";
  const start = manifest.indexOf(marker);
  assert.notEqual(start, -1, "Cargo manifest must contain a [features] section");
  const remainder = manifest.slice(start + marker.length);
  const nextSection = remainder.search(/^\[/m);
  const body = nextSection === -1 ? remainder : remainder.slice(0, nextSection);
  return new Set(
    [...body.matchAll(/^([A-Za-z0-9_-]+)\s*=/gm)].map((match) => match[1]),
  );
}
