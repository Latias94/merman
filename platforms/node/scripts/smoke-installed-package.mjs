import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { resolveNodeTarget } from "../src/native-loader.mjs";
import { localLayoutCapability, nodePackageLocalLayout } from "../src/transport-contract.mjs";
import { readLocalLayoutFlag } from "./local-layout.mjs";
import { assertMathSvg, runThemeAuthoringSmoke } from "./theme-authoring-smoke.mjs";
import { runIconRegistrySmoke } from "./icon-registry-smoke.mjs";

if (isMainModule()) {
  await main();
}

async function main() {
  const args = process.argv.slice(2);
  const project = valueAfter(args, "--project");
  const expectedVersion = valueAfter(args, "--version");
  const expectedTarget = valueAfter(args, "--target");
  const localLayout = readLocalLayoutFlag(args);
  if (!project || !expectedVersion || !expectedTarget) {
    throw new Error(
      "usage: node smoke-installed-package.mjs --project <dir> --version <version> --target <target> [--local-grid | --local-layered | --local-shoal]",
    );
  }

  if (expectedTarget !== "node-wasm") {
    assert.equal(
      expectedTarget,
      resolveNodeTarget(),
      "Installed native package smoke target must match the current host.",
    );
  }

  const packageName = expectedTarget === "node-wasm" ? "@mermanjs/node-wasm" : "@mermanjs/node";
  const entrypoint = resolveInstalledEntrypoint(project, packageName);
  const packageManifest = JSON.parse(
    await readFile(path.resolve(path.dirname(entrypoint), "..", "package.json"), "utf8"),
  );
  assert.equal(packageManifest.version, expectedVersion);
  if (localLayout !== null) assert.equal(nodePackageLocalLayout(packageManifest), localLayout);

  const module = await import(pathToFileURL(entrypoint));
  const engine = await module.createNodeEngine();
  try {
    assert.equal(engine.runtimeCatalog.package_version, expectedVersion);
    const source = "flowchart TD\nA --> B";
    const svg = await engine.renderSvg(source);
    assert.match(svg, /<svg\b/);
    assert.match(svg, /<\/svg>/);
    const diagramId = "installed-diagram-id";
    const withId = engine.renderSvgSync(source, { diagramId });
    assert.match(withId, /<svg\b[^>]*\bid="installed-diagram-id"/);
    assert.equal(await engine.renderSvg(source, { diagramId }), withId);
    if (expectedTarget !== "node-wasm") {
      assert.throws(() => engine.renderSvgSync(source, { timeoutMs: 0 }), (error) =>
        error instanceof module.MermanOperationError &&
        error.codeName === "MERMAN_CANCELLED" &&
        error.cancellationDetails?.reason === "deadline_exceeded" &&
        error.cancellationDetails?.phase === "admission",
      );
    }
    const theme = await runThemeAuthoringSmoke(module, engine);
    const iconRegistry = await runIconRegistrySmoke(module);
    await runMathSmoke(module, engine);
    if (localLayout === "grid" || localLayout === "layered") await runLocalLayoutSmoke(engine, localLayout);
    console.log(
      JSON.stringify({
        package: packageName,
        version: expectedVersion,
        target: expectedTarget,
        node_version: process.version,
        platform: process.platform,
        arch: process.arch,
        glibc_runtime: process.report?.getReport?.()?.header?.glibcVersionRuntime ?? null,
        svg_bytes: Buffer.byteLength(svg),
        theme,
        icon_registry: iconRegistry,
      }),
    );
  } finally {
    await engine.dispose();
  }
}

async function runLocalLayoutSmoke(engine, localLayout) {
  const capability = localLayoutCapability(localLayout);
  assert.ok(engine.runtimeCatalog.capabilities.capability_ids.includes(capability));
  const source = `flowchart LR\nA[Start] -->|Approved| B[Done]\n`;
  const optionsJson = JSON.stringify({ site_config: { layout: localLayout } });
  const plan = JSON.parse(await engine.svgPlanJson(source, { optionsJson }));
  assert.equal(plan.ready, true);
  assert.deepEqual(plan.required_capability_ids, [capability]);
  const layout = await engine.executeOperation({ operationId: "layout-json", source, optionsJson });
  const geometry = JSON.parse(layout.data);
  assert.equal(geometry.meta.effective_config.layout, localLayout);
  assert.ok(geometry.layout.FlowchartV2);
  const svg = await engine.renderSvg(source, { optionsJson });
  assert.match(svg, /^<svg\b/);
  assert.equal(engine.renderSvgSync(source, { optionsJson }), svg);
}

async function runMathSmoke(module, engine) {
  assert.ok(engine.runtimeCatalog.capabilities.capability_ids.includes("math"));
  const flowchart = 'flowchart LR\nA["$$x^2$$"]';
  const sequence = "sequenceDiagram\nAlice->>Bob: $$x^2$$";
  const options = { optionsJson: '{"site_config":{"htmlLabels":true}}' };
  for (const source of [flowchart, sequence]) {
    assertMathSvg(await engine.renderSvg(source, options));
    assertMathSvg(engine.renderSvgSync(source, options));
  }
  const disabled = { optionsJson: '{"environment":{"math_renderer":"none"}}' };
  const missingMath = (error) => error instanceof module.MermanOperationError &&
    error.codeName === "MERMAN_UNSUPPORTED_OPERATION" && error.kind === "missing-capability" &&
    error.capabilityId === "math";
  await assert.rejects(engine.renderSvg(flowchart, disabled), missingMath);
  assert.throws(() => engine.renderSvgSync(flowchart, disabled), missingMath);
  assertMathSvg(await engine.renderSvg(flowchart));
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
  return process.argv[1] &&
    realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
}
