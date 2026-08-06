import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import { compile } from "@tailwindcss/node";

import { OPAQUE_REALM_ARTIFACT_PLAN } from "./opaque-realm-artifact-plan.mjs";
import {
  collectSourceClosure,
  createTypeScriptSourceGraph,
} from "./typescript-source-graph.mjs";

const playgroundRoot = path.resolve(import.meta.dirname, "..");
const stylesheet = path.join(playgroundRoot, "src/styles/globals.css");
const stylesheetRoot = path.dirname(stylesheet);
const productionSources = Object.freeze([
  "../../components",
  "../App.tsx",
  "../components",
]);

test("Tailwind scans only production runtime-owned UI sources", async () => {
  const css = await readFile(stylesheet, "utf8");
  const compilation = await compile(css, {
    base: stylesheetRoot,
    from: stylesheet,
    onDependency() {},
  });

  assert.equal(compilation.root, "none");
  assert.deepEqual(
    compilation.sources,
    productionSources.map((pattern) => ({
      base: stylesheetRoot,
      pattern,
      negated: false,
    })),
  );

  const graph = createTypeScriptSourceGraph({
    rootDir: playgroundRoot,
    entries: OPAQUE_REALM_ARTIFACT_PLAN.pages.map((page) => page.entry),
  });
  const runtimeClosure = collectSourceClosure(graph, graph.entries, {
    includeDynamic: true,
  });
  const scannedFiles = await collectSourceFiles(
    productionSources.map((source) => path.resolve(stylesheetRoot, source)),
  );
  const relativeFiles = scannedFiles.map((file) =>
    path.relative(playgroundRoot, file).split(path.sep).join("/"),
  );

  assert.ok(relativeFiles.length > 0);
  assert.deepEqual(
    relativeFiles.filter((file) => !runtimeClosure.has(file)),
    [],
  );
  assert.equal(
    relativeFiles.some((file) => /\.(?:spec|test)\.[cm]?[jt]sx?$/u.test(file)),
    false,
  );
});

async function collectSourceFiles(roots) {
  const files = [];
  for (const root of roots) {
    const entries = await readdir(root, { withFileTypes: true }).catch(
      (error) => {
        if (error?.code === "ENOTDIR") return null;
        throw error;
      },
    );
    if (entries === null) {
      files.push(root);
      continue;
    }
    for (const entry of entries) {
      const target = path.join(root, entry.name);
      if (entry.isDirectory()) {
        files.push(...(await collectSourceFiles([target])));
      } else if (entry.isFile()) {
        files.push(target);
      }
    }
  }
  return files.sort((left, right) => left.localeCompare(right, "en"));
}
