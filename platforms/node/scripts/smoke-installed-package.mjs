import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { resolveNodeTarget } from "../src/native-loader.mjs";
import { parseSvg, runThemeAuthoringSmoke } from "./theme-authoring-smoke.mjs";
import { customizeNodeColors } from "../../web/examples/custom-theme.mjs";

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

  const module = await import(pathToFileURL(entrypoint));
  const engine = await module.createNodeEngine();
  try {
    assert.equal(engine.runtimeCatalog.package_version, expectedVersion);
    const svg = await engine.renderSvg("flowchart TD\nA --> B");
    assert.match(svg, /<svg\b/);
    assert.match(svg, /<\/svg>/);
    const themeAuthoring = await runThemeAuthoringSmoke(module, engine);
    const recipeExchange = await verifyCustomizedRecipeExchange(engine, entrypoint);
    console.log(
      JSON.stringify({
        package: packageName,
        version: expectedVersion,
        target: expectedTarget,
        svg_bytes: Buffer.byteLength(svg),
        theme_authoring: themeAuthoring,
        recipe_exchange: recipeExchange,
      }),
    );
  } finally {
    await engine.dispose();
  }
}

async function verifyCustomizedRecipeExchange(engine, entrypoint) {
  const exported = JSON.parse((await engine.executeOperation({
    operationId: "export-theme-preset-json", source: "cyberpunk",
  })).data);
  const original = structuredClone(exported);
  const sources = {
    class: "classDiagram\nclass Account\nclass Owned\nAccount --> Owned : link\n"
      + "style Owned fill:#334455,stroke:#778899,stroke-width:4px\n",
  };
  for (const family of ["flowchart", "sequence", "xychart"]) {
    sources[family] = await readFile(new URL(
      `../../../crates/merman-theme-fixtures/fixtures/public-cyberpunk/${family}.mmd`,
      import.meta.url,
    ), "utf8");
  }
  // Clearing a typed fill returns to Cyberpunk's dark Mermaid compatibility theme.
  // Render that theme independently so the clear case cannot copy its own result.
  const baseline = await engine.executeOperation({
    operationId: "svg", source: sources.class,
    optionsJson: JSON.stringify({ site_config: { theme: "dark", htmlLabels: false },
      svg: { diagram_id: "node-recipe-class" } }),
  });
  const baselineDocument = parseSvg(baseline.data);
  const baselineAccount = Array.from(baselineDocument.getElementsByTagName("g")).find(
    (element) => element.getAttribute("id")?.includes("-classId-Account-"),
  );
  assert.ok(baselineAccount, "missing dark compatibility Class terminal Account");
  const compatibilityFill = Array.from(baselineAccount.getElementsByTagName("path")).find(
    (shape) => (shape.parentNode.getAttribute("class") ?? "").split(/\s+/).includes("outer-path"),
  )?.getAttribute("fill");
  assert.equal(compatibilityFill, "#1f2020", "dark Mermaid Class compatibility fill");
  // Cyberpunk Flowchart effects are qualified on the classic writer. Keep that selection
  // scoped to Flowchart so recipe exchange still exercises other families' default looks.
  const directory = await mkdtemp(path.join(os.tmpdir(), "merman-node-recipes-"));
  const otherFamilies = new Map();
  let comparisons = 0;
  try {
    for (const [name, classFill, expectedFill] of [
      ["brand", "#22354d", "#22354d"],
      ["clear", null, compatibilityFill],
      ["transparent", "transparent", "transparent"],
    ]) {
      const recipe = customizeNodeColors(exported, {
        background: "#142535", nodeBorder: "#fb7185", classFill,
      });
      assert.deepEqual(exported, original, "customization mutated the exported recipe");
      const preserved = structuredClone(recipe);
      preserved.complete_spec.canvas.base = original.complete_spec.canvas.base;
      preserved.complete_spec.styles.splice(-2);
      assert.deepEqual(preserved, original, "customization replaced unrelated recipe fields");
      const file = path.join(directory, `${name}.json`);
      const saved = JSON.stringify(recipe);
      await writeFile(file, saved);
      const imported = JSON.parse(execFileSync(process.execPath, [
        "--input-type=module", "--eval", `
          import { readFileSync } from "node:fs";
          import { pathToFileURL } from "node:url";
          const module = await import(pathToFileURL(process.argv[1]));
          const theme = JSON.parse(readFileSync(process.argv[2], "utf8"));
          const sources = JSON.parse(process.argv[3]);
          const engine = await module.createNodeEngine();
          try {
            const results = {};
            for (const [family, source] of Object.entries(sources)) {
              results[family] = await engine.executeOperation({
                operationId: "svg", source,
                optionsJson: JSON.stringify({ theme, site_config: { htmlLabels: false, flowchart: { look: "classic" } },
                  svg: { diagram_id: "node-recipe-" + family } }),
              });
            }
            process.stdout.write(JSON.stringify(results));
          } finally { await engine.dispose(); }
        `, entrypoint, file, JSON.stringify(sources),
      ], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024, timeout: 60_000 }));
      assert.equal(await readFile(file, "utf8"), saved, "import changed the saved recipe");
      for (const [family, source] of Object.entries(sources)) {
        const result = await engine.executeOperation({
          operationId: "svg", source,
          optionsJson: JSON.stringify({ theme: recipe, site_config: { htmlLabels: false, flowchart: { look: "classic" } },
            svg: { diagram_id: `node-recipe-${family}` } }),
        });
        assert.deepEqual(imported[family], result, `${name}/${family}: fresh process changed output`);
        const evidence = JSON.parse(result.metadata_json).theme_execution_evidence;
        assert.equal(evidence.family_id, family);
        if (family === "class" && classFill === null) {
          assert.equal(evidence.theme_status, "residual");
          assert.equal(evidence.target_status, "rejected");
          assert.ok(evidence.target_reason_ids.includes("theme_evidence_incomplete"));
          assert.deepEqual(evidence.diagnostics, [{
            code: "unsupported-paint", subject: "rule", target: "node",
            source_document: "complete_spec",
            source_paths: [`/styles/${recipe.complete_spec.styles.length - 1}`], generated: false,
          }]);
        } else {
          assert.equal(evidence.theme_status, "verified");
          assert.equal(evidence.target_status, "unverified");
        }
        const document = parseSvg(result.data);
        const canvas = Array.from(document.getElementsByTagName("rect")).find(
          (element) => element.getAttribute("class") === "merman-theme-canvas-base",
        );
        assert.equal(canvas?.getAttribute("fill"), "#142535", `${family}: canvas paint`);
        if (family === "class") {
          for (const [label, fill, stroke] of [
            ["Account", expectedFill, "#fb7185"], ["Owned", "#334455", "#778899"],
          ]) {
            const group = Array.from(document.getElementsByTagName("g")).find(
              (element) => element.getAttribute("id")?.includes(`-classId-${label}-`),
            );
            assert.ok(group, `missing Class terminal ${label}`);
            const paths = Array.from(group.getElementsByTagName("path")).filter(
              (shape) => (shape.parentNode.getAttribute("class") ?? "").split(/\s+/).includes("outer-path"),
            );
            assert.ok(paths.some((shape) => shape.getAttribute("fill") === fill),
              `${name}/${label}: actual shape fill changed`);
            assert.ok(paths.some((shape) => shape.getAttribute("stroke") === stroke
              && (label !== "Owned" || shape.getAttribute("stroke-width") === "4")),
            `${name}/${label}: actual shape paints or source-owned width changed`);
          }
        } else if (otherFamilies.has(family)) {
          assert.equal(result.data, otherFamilies.get(family), `${family}: Class fill leaked`);
        } else {
          otherFamilies.set(family, result.data);
        }
        comparisons += 1;
      }
    }
    return { saved_recipes: 3, fresh_processes: 3, svg_metadata_comparisons: comparisons };
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
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
