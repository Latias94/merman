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

  const entrypoint = resolveInstalledEntrypoint(project);
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
    console.log(
      JSON.stringify({
        package: "@mermanjs/node",
        version: expectedVersion,
        target: expectedTarget,
        svg_bytes: Buffer.byteLength(svg),
      }),
    );
  } finally {
    await engine.dispose();
  }
}

export function resolveInstalledEntrypoint(project) {
  // Resolve from the installed project with ESM import conditions. createRequire() would select
  // CommonJS conditions and reject this intentionally ESM-only package.
  const entrypointUrl = execFileSync(
    process.execPath,
    [
      "--input-type=module",
      "--eval",
      'process.stdout.write(import.meta.resolve("@mermanjs/node"));',
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
