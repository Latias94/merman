import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

if (isMainModule()) {
  await main();
}

async function main() {
  const args = process.argv.slice(2);
  const project = valueAfter(args, "--project");
  const expectedVersion = valueAfter(args, "--version");
  const expectedTarget = valueAfter(args, "--target");
  if (!project || !expectedVersion || !expectedTarget) {
    throw new Error(
      "usage: node smoke-installed-package.mjs --project <dir> --version <version> --target <target>",
    );
  }

  const packageName = expectedTarget === "node-wasm" ? "@mermanjs/node-wasm" : "@mermanjs/node";
  const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
  const drawingListFixtureSource = await readFile(
    path.join(repositoryRoot, "fixtures", "bindings", "drawing-list-v1-smoke.mmd"),
    "utf8",
  );
  const entrypoint = resolveInstalledEntrypoint(project, packageName);
  const packageManifest = JSON.parse(
    await readFile(path.resolve(path.dirname(entrypoint), "..", "package.json"), "utf8"),
  );
  assert.equal(packageManifest.version, expectedVersion);

  const module = await import(pathToFileURL(entrypoint));
  const engine = await module.createNodeEngine();
  try {
    assert.equal(engine.runtimeCatalog.package_version, expectedVersion);
    const svg = await engine.renderSvg("flowchart TD\nA --> B");
    assert.match(svg, /<svg\b/);
    assert.match(svg, /<\/svg>/);
    const drawingListResult = await engine.executeOperation({
      operationId: "drawing-list-json",
      source: drawingListFixtureSource,
    });
    assert.equal(drawingListResult.operation_id, "drawing-list-json");
    assert.equal(
      drawingListResult.media_type,
      "application/vnd.merman.drawing-list+json;version=1",
    );
    assertDrawingListDocument(JSON.parse(drawingListResult.data));
    console.log(
      JSON.stringify({
        package: packageName,
        version: expectedVersion,
        target: expectedTarget,
        svg_bytes: Buffer.byteLength(svg),
      }),
    );
  } finally {
    await engine.dispose();
  }
}

function assertDrawingListDocument(value) {
  assert.equal(value.version, 1);
  assert.equal(value.coordinate_system, "logical_pixels_y_down");
  assert.ok(Array.isArray(value.commands) && value.commands.length > 0);
}

export function resolveInstalledEntrypoint(project, packageName = "@mermanjs/node") {
  // Resolve from the installed project with ESM import conditions. createRequire() would select
  // CommonJS conditions and reject this intentionally ESM-only package.
  const entrypointUrl = execFileSync(
    process.execPath,
    [
      "--input-type=module",
      "--eval",
      `process.stdout.write(import.meta.resolve(${JSON.stringify(packageName)}));`,
    ],
    {
      cwd: path.resolve(project),
      encoding: "utf8",
    },
  ).trim();
  return fileURLToPath(entrypointUrl);
}

function valueAfter(values, flag) {
  const index = values.indexOf(flag);
  return index === -1 ? null : values[index + 1] ?? null;
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
