import { createHash } from "node:crypto";
import { mkdir, readdir, rename, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "vite";

const playgroundRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  ".."
);
const outputRoot = path.join(playgroundRoot, ".runtime");
const legacyOutputs = new Set([
  "opaque-benchmark-mermaid.js",
  "opaque-benchmark-mermaid.json",
  "opaque-compare.js",
  "opaque-compare.json",
]);

const engines = [
  {
    id: "compare-mermaid",
    file: "compare-mermaid-engine",
    entry: "src/runtime/realm/engines/compare-mermaid-artifact-entry.ts",
  },
  {
    id: "benchmark-mermaid",
    file: "benchmark-mermaid-engine",
    entry: "src/benchmark/realm/engines/benchmark-mermaid-artifact-entry.ts",
  },
  {
    id: "benchmark-merman",
    file: "benchmark-merman-engine",
    entry: "src/benchmark/realm/engines/benchmark-merman-artifact-entry.ts",
  },
];

const bootstraps = [
  {
    id: "compare",
    engineId: "compare-mermaid",
    file: "opaque-compare-bootstrap",
    entry: "src/runtime/realm/opaque-compare-entry.ts",
  },
  {
    id: "benchmark",
    engineId: "benchmark-mermaid",
    file: "opaque-benchmark-mermaid-bootstrap",
    entry: "src/benchmark/realm/opaque-mermaid-entry.ts",
  },
];

const generated = [];
const engineManifests = new Map();
for (const artifact of engines) {
  const built = await buildArtifact(artifact, "es");
  generated.push(...built.outputs);
  engineManifests.set(artifact.id, built.manifest);
}
for (const artifact of bootstraps) {
  const engineManifest = engineManifests.get(artifact.engineId);
  if (!engineManifest) {
    throw new Error(`${artifact.id} has no engine artifact identity.`);
  }
  const built = await buildArtifact(artifact, "iife", engineManifest);
  generated.push(...built.outputs);
}
const expectedOutputs = new Set(generated.map(({ file }) => file));
await mkdir(outputRoot, { recursive: true });
await rejectUnknownOutputs(expectedOutputs);
for (const { file, value } of generated) {
  await atomicWrite(path.join(outputRoot, file), value);
}
for (const legacy of legacyOutputs) {
  await unlink(path.join(outputRoot, legacy)).catch((error) => {
    if (error?.code !== "ENOENT") throw error;
  });
}
await assertExactOutputs(expectedOutputs);

async function buildArtifact(artifact, format, engineManifest = null) {
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
        formats: [format],
        name: `Merman${pascalCase(artifact.id)}Artifact`,
      },
      rolldownOptions: {
        output: { codeSplitting: false },
      },
    },
  });
  const chunks = Array.isArray(output)
    ? output.flatMap((item) => item.output)
    : output.output;
  const scripts = chunks.filter((item) => item.type === "chunk");
  const assets = chunks.filter((item) => item.type === "asset");
  if (scripts.length !== 1) {
    throw new Error(`${artifact.id} must emit exactly one script.`);
  }
  if (assets.length !== 0) {
    throw new Error(
      `${artifact.id} emitted external assets: ${assets
        .map((item) => item.fileName)
        .join(", ")}`
    );
  }
  const source = scripts[0].code;
  assertSelfContainedScript(artifact.id, source, format);
  const sha256 = createHash("sha256").update(source).digest("hex");
  const manifest = {
    schemaVersion: 1,
    id: artifact.id,
    bytes: Buffer.byteLength(source),
    sha256,
    ...(format === "iife"
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
      { file: `${artifact.file}.js`, value: source },
      {
        file: `${artifact.file}.json`,
        value: `${JSON.stringify(manifest, null, 2)}\n`,
      },
    ],
  };
}

async function rejectUnknownOutputs(expected) {
  const entries = await readdir(outputRoot, { withFileTypes: true });
  const unknown = entries
    .filter(
      (entry) =>
        !entry.isFile() ||
        (!expected.has(entry.name) && !legacyOutputs.has(entry.name))
    )
    .map((entry) => entry.name)
    .sort();
  if (unknown.length > 0) {
    throw new Error(
      `Opaque realm output directory contains unowned files: ${unknown.join(", ")}`
    );
  }
}

async function assertExactOutputs(expected) {
  const actual = (await readdir(outputRoot)).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(
      `Opaque realm output set is invalid: expected ${wanted.join(", ")}; found ${actual.join(", ")}`
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

function assertSelfContainedScript(id, source, format) {
  if (!source.trim()) throw new Error(`${id} script is empty.`);
  if (/sourceMappingURL=/.test(source)) {
    throw new Error(`${id} retains an external source map.`);
  }
  if (/^\s*import\s/m.test(source)) {
    throw new Error(`${id} retains a static module import.`);
  }
  if (format === "iife" && /<\/script/i.test(source)) {
    throw new Error(`${id} contains an inline-script terminator.`);
  }
}

function pascalCase(value) {
  return value
    .split(/[^a-z0-9]+/i)
    .filter(Boolean)
    .map((part) => `${part[0].toUpperCase()}${part.slice(1)}`)
    .join("");
}
