import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { OPAQUE_REALM_ARTIFACT_PLAN } from "./opaque-realm-artifact-plan.mjs";
import { verifyPreparedOpaqueRealmArtifacts } from "./opaque-realm-artifact-verifier.mjs";
import { renderOpaqueRealmBrowserProjections } from "./opaque-realm-browser-projection.mjs";

test("prepared verifier binds manifests, CSP hashes, public bytes, and projections", async (t) => {
  const root = await createPreparedFixture(t);
  await verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN);

  const engineManifest = path.join(root, ".runtime", "mermaid-engine.json");
  const identity = JSON.parse(await readFile(engineManifest, "utf8"));
  identity.sha256 = "0".repeat(64);
  await writeFile(engineManifest, `${JSON.stringify(identity)}\n`);
  await assert.rejects(
    verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN),
    /Expected values to be strictly equal/u,
  );
});

test("prepared verifier rejects an injected inline-script terminator", async (t) => {
  const root = await createPreparedFixture(t, {
    "compare-mermaid": "</script><script>throw new Error()</script>",
  });
  await assert.rejects(
    verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN),
    /expected to not match/u,
  );
});

test("prepared verifier rejects a wrong bootstrap CSP hash", async (t) => {
  const root = await createPreparedFixture(t);
  const manifestFile = path.join(
    root,
    ".runtime",
    "opaque-compare-bootstrap.json",
  );
  const manifest = JSON.parse(await readFile(manifestFile, "utf8"));
  manifest.cspHash = `sha256-${Buffer.alloc(32).toString("base64")}`;
  await writeFile(manifestFile, `${JSON.stringify(manifest)}\n`);
  await assert.rejects(
    verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN),
    /Expected values to be strictly equal/u,
  );
});

test("prepared verifier rejects stale generated browser projection", async (t) => {
  const root = await createPreparedFixture(t);
  const projection = path.join(
    root,
    OPAQUE_REALM_ARTIFACT_PLAN.browserMetadataModule,
  );
  await writeFile(projection, "// stale\n");
  await assert.rejects(
    verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN),
    /is stale/u,
  );
});

test("prepared verifier rejects an oversized engine artifact", async (t) => {
  const root = await createPreparedFixture(
    t,
    {},
    { "benchmark-merman": "x".repeat(256 * 1024 + 1) },
  );
  await assert.rejects(
    verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN),
    /benchmark-merman engine exceeds its byte budget/u,
  );
});

test("prepared verifier rejects parent-owned WASM embedded in an engine", async (t) => {
  const root = await createPreparedFixture(t, {}, {
    "benchmark-merman":
      'export const benchmarkEngineAdapter = "data:application/wasm;base64,AA==";\n',
  });
  await assert.rejects(
    verifyPreparedOpaqueRealmArtifacts(root, OPAQUE_REALM_ARTIFACT_PLAN),
    /embeds its parent-owned WASM resource/u,
  );
});

async function createPreparedFixture(
  t,
  bootstrapSources = {},
  engineSources = {},
) {
  const root = await mkdtemp(path.join(os.tmpdir(), "merman-opaque-artifacts-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const plan = OPAQUE_REALM_ARTIFACT_PLAN;
  const outputRoot = path.join(root, plan.roots.generated);
  const publicRoot = path.join(root, plan.roots.publicEngines);
  await mkdir(outputRoot, { recursive: true });
  await mkdir(publicRoot, { recursive: true });

  const identities = new Map();
  for (const engine of plan.engines) {
    const source =
      engineSources[engine.id] ?? `export const ${engine.exports[0]} = {};\n`;
    const manifest = identity(engine.id, source);
    identities.set(engine.id, manifest);
    await writeArtifact(outputRoot, engine.outputBase, source, manifest);
    if (engine.publish) {
      await writeFile(path.join(publicRoot, `${engine.outputBase}.js`), source);
    }
  }
  for (const realm of plan.realms) {
    if (!realm.bootstrap) continue;
    const source = bootstrapSources[realm.key] ?? "(()=>{})();\n";
    const manifest = {
      ...identity(realm.kind, source),
      cspHash: `sha256-${createHash("sha256").update(source).digest("base64")}`,
      engineArtifact: identities.get(realm.engine),
    };
    await writeArtifact(outputRoot, realm.bootstrap.outputBase, source, manifest);
  }
  for (const [file, source] of renderOpaqueRealmBrowserProjections(plan)) {
    const destination = path.join(root, file);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, source);
  }
  return root;
}

function identity(id, source) {
  return {
    schemaVersion: 1,
    id,
    bytes: Buffer.byteLength(source),
    sha256: createHash("sha256").update(source).digest("hex"),
  };
}

async function writeArtifact(root, outputBase, source, manifest) {
  await writeFile(path.join(root, `${outputBase}.js`), source);
  await writeFile(
    path.join(root, `${outputBase}.json`),
    `${JSON.stringify(manifest)}\n`,
  );
}
