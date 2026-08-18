import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  collectSourceClosure,
  createTypeScriptSourceGraph,
} from "./typescript-source-graph.mjs";

test("TypeScript owns paths, extensionless, package exports, and type-only resolution", async (t) => {
  const root = await createProject(t, {
    "src/main.ts": [
      'import { value } from "@/dep";',
      'import { nested } from "./folder";',
      'import type { Allowed } from "fixture/allowed";',
      "export const result: Allowed = value + nested;",
    ].join("\n"),
    "src/dep.ts": "export const value = 1;\n",
    "src/folder/index.ts": "export const nested = 2;\n",
    "node_modules/fixture/package.json": JSON.stringify({
      name: "fixture",
      type: "module",
      exports: {
        "./allowed": {
          types: "./allowed.d.ts",
          default: "./allowed.js",
        },
      },
    }),
    "node_modules/fixture/allowed.d.ts": "export type Allowed = number;\n",
    "node_modules/fixture/allowed.js": "export {};\n",
  });
  const graph = createTypeScriptSourceGraph({
    rootDir: root,
    entries: ["src/main.ts"],
  });

  assert.ok(
    graph.edges.some(
      (edge) =>
        edge.specifier === "@/dep" &&
        edge.to === "src/dep.ts" &&
        edge.kind === "static",
    ),
  );
  assert.ok(
    graph.edges.some(
      (edge) =>
        edge.specifier === "./folder" && edge.to === "src/folder/index.ts",
    ),
  );
  assert.ok(
    graph.edges.some(
      (edge) =>
        edge.specifier === "fixture/allowed" &&
        edge.kind === "type" &&
        edge.external,
    ),
  );
});

test("runtime and ownership closures differ only on explicit dynamic/type edges", async (t) => {
  const root = await createProject(t, {
    "src/main.ts": [
      'import "./runtime.ts";',
      'import type { Boundary } from "./types.ts";',
      'void import("./lazy.ts");',
    ].join("\n"),
    "src/runtime.ts": "export {};\n",
    "src/types.ts": "export interface Boundary { value: string }\n",
    "src/lazy.ts": "export {};\n",
  });
  const graph = createTypeScriptSourceGraph({
    rootDir: root,
    entries: ["src/main.ts"],
  });

  assert.deepEqual(
    [...collectSourceClosure(graph, ["src/main.ts"])].sort(),
    ["src/main.ts", "src/runtime.ts"],
  );
  assert.deepEqual(
    [
      ...collectSourceClosure(graph, ["src/main.ts"], {
        includeDynamic: true,
        includeTypeOnly: true,
      }),
    ].sort(),
    ["src/lazy.ts", "src/main.ts", "src/runtime.ts", "src/types.ts"],
  );
});

test("Vite resource queries and relative assets stay in the compiler-owned graph", async (t) => {
  const root = await createProject(t, {
    "src/main.ts": [
      'import rawSource from "./generated.js?raw";',
      'import "./theme.css";',
      "export const value = rawSource;",
    ].join("\n"),
    "src/generated.js": "export default 'generated';\n",
    "src/theme.css": "body { color: black; }\n",
  });
  const graph = createTypeScriptSourceGraph({
    rootDir: root,
    entries: ["src/main.ts"],
  });

  assert.deepEqual(
    [...collectSourceClosure(graph, ["src/main.ts"])].sort(),
    ["src/generated.js", "src/main.ts", "src/theme.css"],
  );
});

test("Playground startup closure includes toolbar ownership but excludes lazy workbenches", () => {
  const root = path.resolve(import.meta.dirname, "..");
  const graph = createTypeScriptSourceGraph({
    rootDir: root,
    entries: ["src/main.tsx"],
  });
  const startup = collectSourceClosure(graph, ["src/main.tsx"], {
    includeTypeOnly: true,
  });

  assert.equal(startup.has("src/components/ToolbarControls.tsx"), true);
  assert.equal(startup.has("src/components/ToolbarFeatureLaunchers.tsx"), true);
  assert.equal(startup.has("src/components/Editor.tsx"), false);
  assert.equal(startup.has("src/editor/monaco.ts"), false);
  assert.deepEqual(
    [...startup]
      .filter(
        (source) =>
          source.startsWith("src/benchmark/") ||
          /^src\/components\/(?:Bench|ConfigEditor|ExampleGallery)/u.test(
            source,
          ) ||
          source === "src/lib/examples.ts",
      )
      .sort(),
    [],
  );
});

test("unexported package subpaths fail closed through the compiler resolver", async (t) => {
  const root = await createProject(t, {
    "src/main.ts": 'import "fixture/private";\n',
    "node_modules/fixture/package.json": JSON.stringify({
      name: "fixture",
      type: "module",
      exports: { "./allowed": "./allowed.js" },
    }),
    "node_modules/fixture/allowed.js": "export {};\n",
    "node_modules/fixture/private.js": "export {};\n",
  });
  assert.throws(
    () =>
      createTypeScriptSourceGraph({
        rootDir: root,
        entries: ["src/main.ts"],
      }),
    /cannot resolve "fixture\/private"/u,
  );
});

async function createProject(t, files) {
  const root = await mkdtemp(path.join(os.tmpdir(), "merman-ts-graph-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const config = {
    compilerOptions: {
      allowImportingTsExtensions: true,
      baseUrl: ".",
      module: "ESNext",
      moduleResolution: "Bundler",
      noEmit: true,
      paths: { "@/*": ["src/*"] },
      strict: true,
      target: "ES2022",
    },
  };
  await writeFile(path.join(root, "tsconfig.json"), JSON.stringify(config));
  for (const [file, source] of Object.entries(files)) {
    const destination = path.join(root, file);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, source);
  }
  return root;
}
