import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

const args = process.argv.slice(2);
const project = valueAfter(args, "--project");
const expectedVersion = valueAfter(args, "--version");
const expectedTarget = valueAfter(args, "--target");
if (!project || !expectedVersion || !expectedTarget) {
  throw new Error(
    "usage: node smoke-installed-package.mjs --project <dir> --version <version> --target <target>",
  );
}

const requireFromProject = createRequire(path.join(path.resolve(project), "package.json"));
const entrypoint = requireFromProject.resolve("@mermanjs/node");
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

function valueAfter(values, flag) {
  const index = values.indexOf(flag);
  return index === -1 ? null : values[index + 1] ?? null;
}
