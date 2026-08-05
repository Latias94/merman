import { createHash } from "node:crypto";
import { mkdir, readdir, rename, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "vite";

import {
  OPAQUE_REALM_ARTIFACT_PLAN,
  artifactOutputFiles,
  publicEngineFiles,
} from "./opaque-realm-artifact-plan.mjs";
import { renderOpaqueRealmBrowserProjections } from "./opaque-realm-browser-projection.mjs";
import { assertNoRuntimeModuleRequests } from "./runtime-module-request-policy.mjs";

const playgroundRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const plan = OPAQUE_REALM_ARTIFACT_PLAN;
const outputRoot = path.join(playgroundRoot, plan.roots.generated);
const publicEngineRoot = path.join(playgroundRoot, plan.roots.publicEngines);
const browserProjections = renderOpaqueRealmBrowserProjections(plan);
const metadataDestination = path.join(playgroundRoot, plan.browserMetadataModule);

await mkdir(path.dirname(metadataDestination), { recursive: true });
await atomicWrite(
  metadataDestination,
  browserProjections.get(plan.browserMetadataModule),
);

const generated = [];
const engineManifests = new Map();
for (const engine of plan.engines) {
  const built = await buildArtifact({
    id: engine.id,
    entry: engine.entry,
    outputBase: engine.outputBase,
    format: "es",
    expectedExports: engine.exports,
  });
  generated.push(...built.outputs);
  engineManifests.set(engine.id, built.manifest);
}
for (const realm of plan.realms) {
  if (!realm.bootstrap) continue;
  const engineManifest = engineManifests.get(realm.engine);
  if (!engineManifest) {
    throw new Error(`${realm.key} has no engine artifact identity.`);
  }
  const built = await buildArtifact(
    {
      id: realm.kind,
      entry: realm.bootstrap.entry,
      outputBase: realm.bootstrap.outputBase,
      format: "iife",
      expectedExports: [],
    },
    engineManifest,
  );
  if (built.manifest.bytes > realm.bootstrap.maxBytes) {
    throw new Error(`${realm.key} bootstrap exceeds its byte budget.`);
  }
  generated.push(...built.outputs);
}

await mkdir(outputRoot, { recursive: true });
await mkdir(publicEngineRoot, { recursive: true });
await assertOwnedDirectory(outputRoot, artifactOutputFiles(plan));
await assertOwnedDirectory(publicEngineRoot, publicEngineFiles(plan));
for (const { file, value } of generated) {
  await atomicWrite(path.join(outputRoot, file), value);
}
for (const engine of plan.engines.filter((candidate) => candidate.publish)) {
  const output = generated.find(
    ({ file }) => file === `${engine.outputBase}.js`,
  );
  if (!output) throw new Error(`Missing generated engine ${engine.id}.`);
  await atomicWrite(
    path.join(publicEngineRoot, `${engine.outputBase}.js`),
    output.value,
  );
}
for (const [file, source] of browserProjections) {
  const destination = path.join(playgroundRoot, file);
  await mkdir(path.dirname(destination), { recursive: true });
  await atomicWrite(destination, source);
}
await assertExactDirectory(outputRoot, artifactOutputFiles(plan));
await assertExactDirectory(publicEngineRoot, publicEngineFiles(plan));

async function buildArtifact(artifact, engineManifest = null) {
  const output = await build({
    configFile: false,
    root: playgroundRoot,
    logLevel: "warn",
    define: {
      "process.env.NODE_ENV": JSON.stringify("production"),
      ...(engineManifest
        ? {
            __MERMAN_ENGINE_ARTIFACT_IDENTITY__:
              JSON.stringify(engineManifest),
          }
        : {}),
    },
    build: {
      target: "es2022",
      write: false,
      cssCodeSplit: false,
      minify: "oxc",
      lib: {
        entry: path.join(playgroundRoot, artifact.entry),
        formats: [artifact.format],
        name: `Merman${pascalCase(artifact.id)}Artifact`,
      },
      rolldownOptions: {
        output: { codeSplitting: false },
      },
    },
  });
  const outputs = Array.isArray(output)
    ? output.flatMap((item) => item.output)
    : output.output;
  const scripts = outputs.filter((item) => item.type === "chunk");
  const assets = outputs.filter((item) => item.type === "asset");
  if (scripts.length !== 1) {
    throw new Error(`${artifact.id} must emit exactly one script.`);
  }
  if (assets.length !== 0) {
    throw new Error(
      `${artifact.id} emitted external assets: ${assets
        .map((item) => item.fileName)
        .join(", ")}`,
    );
  }
  const [script] = scripts;
  assertChunkContract(artifact, script);
  const source = script.code;
  if (!source.trim()) throw new Error(`${artifact.id} script is empty.`);
  if (artifact.format === "es") {
    assertNoRuntimeModuleRequests(source, `${artifact.outputBase}.js`);
  }
  if (/sourceMappingURL=/u.test(source)) {
    throw new Error(`${artifact.id} retains an external source map.`);
  }
  if (artifact.format === "iife" && /<\/script/iu.test(source)) {
    throw new Error(`${artifact.id} contains an inline-script terminator.`);
  }
  const sha256 = createHash("sha256").update(source).digest("hex");
  const manifest = {
    schemaVersion: 1,
    id: artifact.id,
    bytes: Buffer.byteLength(source),
    sha256,
    ...(artifact.format === "iife"
      ? {
          cspHash: `sha256-${createHash("sha256")
            .update(source)
            .digest("base64")}`,
          engineArtifact: engineManifest,
        }
      : {}),
  };
  return {
    manifest,
    outputs: [
      { file: `${artifact.outputBase}.js`, value: source },
      {
        file: `${artifact.outputBase}.json`,
        value: `${JSON.stringify(manifest, null, 2)}\n`,
      },
    ],
  };
}

function assertChunkContract(artifact, chunk) {
  const externalImports = [
    ...(chunk.imports ?? []),
    ...(chunk.dynamicImports ?? []),
  ].filter((file) => file !== chunk.fileName);
  if (externalImports.length !== 0) {
    throw new Error(
      `${artifact.id} emitted external module requests: ${externalImports.join(", ")}`,
    );
  }
  const actualExports = [...(chunk.exports ?? [])].sort();
  const expectedExports = [...artifact.expectedExports].sort();
  if (JSON.stringify(actualExports) !== JSON.stringify(expectedExports)) {
    throw new Error(
      `${artifact.id} exports ${actualExports.join(", ") || "nothing"}; expected ${expectedExports.join(", ") || "nothing"}.`,
    );
  }
}

async function assertOwnedDirectory(directory, expectedFiles) {
  const expected = new Set(expectedFiles);
  const entries = await readdir(directory, { withFileTypes: true });
  const unknown = entries
    .filter((entry) => !entry.isFile() || !expected.has(entry.name))
    .map((entry) => entry.name)
    .sort();
  if (unknown.length > 0) {
    throw new Error(
      `${path.relative(playgroundRoot, directory)} contains unowned outputs: ${unknown.join(", ")}`,
    );
  }
}

async function assertExactDirectory(directory, expectedFiles) {
  const actual = (await readdir(directory)).sort();
  const expected = [...expectedFiles].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `${path.relative(playgroundRoot, directory)} output set is invalid: expected ${expected.join(", ")}; found ${actual.join(", ")}`,
    );
  }
}

async function atomicWrite(destination, value) {
  const temporary = `${destination}.tmp-${process.pid}`;
  try {
    await writeFile(temporary, value, { flag: "wx" });
    await rename(temporary, destination);
  } catch (error) {
    await unlink(temporary).catch(() => {});
    throw error;
  }
}

function pascalCase(value) {
  return value
    .split(/[^a-z0-9]+/iu)
    .filter(Boolean)
    .map((part) => `${part[0].toUpperCase()}${part.slice(1)}`)
    .join("");
}
