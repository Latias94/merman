import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  BENCHMARK_SOURCES,
  NON_LITERAL_DYNAMIC_IMPORT_OWNERS,
  collectRuntimeImports,
  inspectBenchmarkSourceBoundaries,
} from "./benchmark-build-graph.mjs";

const PLAYGROUND_ROOT = path.resolve(import.meta.dirname, "..");

test("the current benchmark source graph preserves the engine boundary", () => {
  const result = inspectBenchmarkSourceBoundaries(PLAYGROUND_ROOT);
  assert.deepEqual(result.violations, []);
});

test("static graph allows the public Web package and ignores type-only imports", () => {
  withFixture((root) => {
    writeSource(
      root,
      BENCHMARK_SOURCES.bootstrap,
      [
        'import { assertSafeSvgForDom, SUPPORTED_THEMES } from "@mermanjs/web";',
        'import type { SvgBindingOptions } from "@mermanjs/web";',
        'import { type MermanWasmModule } from "@mermanjs/web";',
        "void assertSafeSvgForDom;",
        "void SUPPORTED_THEMES;",
        'export const load = () => import("./engines/merman.ts");',
      ].join("\n"),
    );

    assert.deepEqual(inspectBenchmarkSourceBoundaries(root).violations, []);
  });
});

test("static graph rejects retired Web runtime subpaths", async (t) => {
  for (const specifier of [
    "@mermanjs/web/pkg/merman_wasm.js",
    "@mermanjs/web/catalog",
    "@mermanjs/web/svg-safety",
  ]) {
    await t.test(specifier, () => {
      withFixture((root) => {
        writeSource(
          root,
          BENCHMARK_SOURCES.bootstrap,
          `import ${JSON.stringify(specifier)};`,
        );
        const result = inspectBenchmarkSourceBoundaries(root);
        assert.match(result.violations.join("\n"), /static graph reaches disallowed/);
      });
    });
  }
});

test("adapter graph rejects main, Compare, scheduler, and statistics ownership", async (t) => {
  const cases = [
    ["main app", "../../../main.tsx", "src/main.tsx"],
    [
      "Compare controller",
      "../../../runtime/mermaid-realm-controller.ts",
      "src/runtime/mermaid-realm-controller.ts",
    ],
    ["scheduler", "../../schedule.ts", "src/benchmark/schedule.ts"],
    ["statistics", "../../statistics.ts", "src/benchmark/statistics.ts"],
  ];

  for (const [label, specifier, target] of cases) {
    await t.test(label, () => {
      withFixture((root) => {
        writeSource(root, target, "export const forbidden = true;");
        appendMermanAdapterImport(root, specifier);
        const result = inspectBenchmarkSourceBoundaries(root);
        assert.match(
          result.violations.join("\n"),
          new RegExp(`forbidden source ${escapeRegExp(target)}`),
        );
      });
    });
  }
});

test("adapter ownership rejects type-only scheduler, statistics, and report edges", async (t) => {
  const cases = [
    ["../../schedule.ts", "src/benchmark/schedule.ts"],
    ["../../statistics.ts", "src/benchmark/statistics.ts"],
    ["../../report.ts", "src/benchmark/report.ts"],
  ];

  for (const [specifier, target] of cases) {
    await t.test(target, () => {
      withFixture((root) => {
        writeSource(root, target, "export interface ForbiddenType {}");
        appendMermanAdapterTypeImport(root, specifier);
        const result = inspectBenchmarkSourceBoundaries(root);
        assert.match(
          result.violations.join("\n"),
          new RegExp(`forbidden source ${escapeRegExp(target)}`),
        );
      });
    });
  }
});

test("adapter ownership follows transitive type-only re-exports", () => {
  withFixture((root) => {
    writeSource(
      root,
      "src/benchmark/realm/adapter-types.ts",
      'export type { ForbiddenType } from "../report.ts";',
    );
    writeSource(
      root,
      "src/benchmark/report.ts",
      "export interface ForbiddenType {}",
    );
    appendMermanAdapterTypeImport(root, "../adapter-types.ts");

    const result = inspectBenchmarkSourceBoundaries(root);
    assert.equal(
      result.adapterGraphs.merman.files.has(
        "src/benchmark/realm/adapter-types.ts",
      ),
      false,
    );
    assert.equal(
      result.adapterOwnershipGraphs.merman.files.has(
        "src/benchmark/report.ts",
      ),
      true,
    );
    assert.match(
      result.violations.join("\n"),
      /forbidden source src\/benchmark\/report\.ts/,
    );
  });
});

test("adapter ownership follows TypeScript import type nodes", () => {
  withFixture((root) => {
    writeSource(
      root,
      "src/benchmark/report.ts",
      "export interface ForbiddenType {}",
    );
    appendMermanAdapterImportType(root, "../../report.ts");

    const result = inspectBenchmarkSourceBoundaries(root);
    assert.equal(
      result.adapterGraphs.merman.files.has("src/benchmark/report.ts"),
      false,
    );
    assert.equal(
      result.adapterOwnershipGraphs.merman.files.has(
        "src/benchmark/report.ts",
      ),
      true,
    );
    assert.match(
      result.violations.join("\n"),
      /forbidden source src\/benchmark\/report\.ts/,
    );
  });
});

test("Vite root aliases cannot bypass adapter ownership checks", async (t) => {
  for (const specifier of ["@/src/main.tsx", "/src/main.tsx"]) {
    await t.test(specifier, () => {
      withFixture((root) => {
        writeSource(root, "src/main.tsx", "export const main = true;");
        appendMermanAdapterImport(root, specifier);
        const result = inspectBenchmarkSourceBoundaries(root);
        assert.match(
          result.violations.join("\n"),
          /forbidden source src\/main\.tsx/,
        );
      });
    });
  }
});

test("Merman engine imports must remain direct dynamic adapter edges", () => {
  withFixture((root) => {
    writeSource(
      root,
      BENCHMARK_SOURCES.mermanAdapter,
      [
        'import wasmUrl from "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";',
        'import { renderSvg } from "@mermanjs/web";',
        'export const load = () => import("@mermanjs/web/pkg/merman_wasm.js");',
        "void wasmUrl;",
        "void renderSvg;",
      ].join("\n"),
    );
    const result = inspectBenchmarkSourceBoundaries(root);
    const violations = result.violations.join("\n");
    assert.match(violations, /disallowed Merman runtime import/);
    assert.match(violations, /must directly and dynamically import exactly/);
  });
});

test("runtime import parser fails closed for computed module requests", () => {
  assert.throws(
    () => collectRuntimeImports("const name = './engine.ts'; import(name);"),
    /non-literal dynamic module request/,
  );
});

test("the engine artifact loader exclusively owns one computed module request", () => {
  const [[owner, expectedCount]] = Object.entries(
    NON_LITERAL_DYNAMIC_IMPORT_OWNERS
  );
  assert.equal(expectedCount, 1);
  assert.deepEqual(
    collectRuntimeImports(
      "const verifiedArtifactUrl = './engine.ts'; import(verifiedArtifactUrl);",
      owner,
    ),
    [],
  );
  assert.throws(
    () =>
      collectRuntimeImports(
        "const first = './a.ts'; const second = './b.ts'; import(first); import(second);",
        owner,
      ),
    /must own exactly 1 non-literal dynamic module request; found 2/,
  );
});

function withFixture(run) {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-build-graph-"));
  try {
    writeSource(
      root,
      BENCHMARK_SOURCES.bootstrap,
      [
        'import { assertSafeSvgForDom } from "@mermanjs/web";',
        'import type { SvgBindingOptions } from "@mermanjs/web";',
        "void assertSafeSvgForDom;",
        "void 0 as unknown as SvgBindingOptions;",
      ].join("\n"),
    );
    writeSource(
      root,
      BENCHMARK_SOURCES.mermanAdapter,
      [
        'import type { MermanWasmModule } from "@mermanjs/web";',
        'export const loadWeb = () => import("@mermanjs/web");',
        "void 0 as unknown as MermanWasmModule;",
      ].join("\n"),
    );
    writeSource(
      root,
      BENCHMARK_SOURCES.mermaidAdapter,
      'export const loadMermaid = () => import("mermaid");',
    );
    run(root);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
}

function appendMermanAdapterImport(root, specifier) {
  const file = path.join(root, BENCHMARK_SOURCES.mermanAdapter);
  const current = readFixture(file);
  writeFileSync(file, `import ${JSON.stringify(specifier)};\n${current}`);
}

function appendMermanAdapterTypeImport(root, specifier) {
  const file = path.join(root, BENCHMARK_SOURCES.mermanAdapter);
  const current = readFixture(file);
  writeFileSync(
    file,
    `import type { ForbiddenType } from ${JSON.stringify(specifier)};\n${current}`,
  );
}

function appendMermanAdapterImportType(root, specifier) {
  const file = path.join(root, BENCHMARK_SOURCES.mermanAdapter);
  const current = readFixture(file);
  writeFileSync(
    file,
    `type ForbiddenAlias = import(${JSON.stringify(specifier)}).ForbiddenType;\n${current}`,
  );
}

function readFixture(file) {
  return readFileSync(file, "utf8");
}

function writeSource(root, source, contents) {
  const file = path.join(root, source);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, contents);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
