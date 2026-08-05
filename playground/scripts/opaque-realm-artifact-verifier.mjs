import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

import {
  artifactOutputFiles,
  publicEngineFiles,
} from "./opaque-realm-artifact-plan.mjs";
import { renderOpaqueRealmBrowserProjections } from "./opaque-realm-browser-projection.mjs";

export async function verifyPreparedOpaqueRealmArtifacts(playgroundRoot, plan) {
  const outputRoot = path.join(playgroundRoot, plan.roots.generated);
  const publicEngineRoot = path.join(playgroundRoot, plan.roots.publicEngines);
  assert.deepEqual(
    (await readdir(outputRoot)).sort(),
    [...artifactOutputFiles(plan)].sort(),
    "generated opaque-realm output set drifted from the artifact plan",
  );

  const engineArtifacts = new Map();
  for (const engine of plan.engines) {
    const artifact = await readArtifact(outputRoot, engine.outputBase);
    assert.deepEqual(Object.keys(artifact.manifest).sort(), [
      "bytes",
      "id",
      "schemaVersion",
      "sha256",
    ]);
    verifyIdentity(artifact.manifest, artifact.source, engine.id);
    engineArtifacts.set(engine.id, artifact);
  }

  for (const realm of plan.realms) {
    if (!realm.bootstrap) continue;
    const artifact = await readArtifact(
      outputRoot,
      realm.bootstrap.outputBase,
    );
    assert.deepEqual(Object.keys(artifact.manifest).sort(), [
      "bytes",
      "cspHash",
      "engineArtifact",
      "id",
      "schemaVersion",
      "sha256",
    ]);
    verifyIdentity(artifact.manifest, artifact.source, realm.kind);
    assert.deepEqual(
      artifact.manifest.engineArtifact,
      engineArtifacts.get(realm.engine)?.manifest,
      `${realm.key} bootstrap is bound to the wrong engine identity`,
    );
    assert.equal(
      artifact.manifest.cspHash,
      `sha256-${createHash("sha256")
        .update(artifact.source)
        .digest("base64")}`,
    );
    assert.ok(
      artifact.manifest.bytes <= realm.bootstrap.maxBytes,
      `${realm.key} bootstrap exceeds its byte budget`,
    );
    assert.doesNotMatch(artifact.source, /<\/script/iu);
  }

  assert.deepEqual(
    (await readdir(publicEngineRoot)).sort(),
    [...publicEngineFiles(plan)].sort(),
    "public opaque-realm engine set drifted from the artifact plan",
  );
  for (const engine of plan.engines.filter((candidate) => candidate.publish)) {
    const source = engineArtifacts.get(engine.id)?.source;
    assert.equal(typeof source, "string");
    assert.equal(
      await readFile(
        path.join(publicEngineRoot, `${engine.outputBase}.js`),
        "utf8",
      ),
      source,
      `${engine.id} public source drifted from its verified identity`,
    );
  }

  for (const [file, expected] of renderOpaqueRealmBrowserProjections(plan)) {
    assert.equal(
      await readFile(path.join(playgroundRoot, file), "utf8"),
      expected,
      `${file} is stale; run npm run build:opaque-realm`,
    );
  }
}

async function readArtifact(outputRoot, outputBase) {
  const source = await readFile(path.join(outputRoot, `${outputBase}.js`), "utf8");
  const manifest = JSON.parse(
    await readFile(path.join(outputRoot, `${outputBase}.json`), "utf8"),
  );
  return { manifest, source };
}

function verifyIdentity(manifest, source, id) {
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.id, id);
  assert.equal(manifest.bytes, Buffer.byteLength(source));
  assert.ok(manifest.bytes > 0);
  assert.equal(
    manifest.sha256,
    createHash("sha256").update(source).digest("hex"),
  );
}
