import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import ts from "typescript";

export const BENCHMARK_SOURCES = Object.freeze({
  bootstrap: "src/benchmark/realm/bootstrap.ts",
  mermanArtifact: "src/benchmark/realm/merman-engine-artifact.ts",
  mermanAdapter: "src/benchmark/realm/engines/merman.ts",
  mermaidAdapter: "src/benchmark/realm/engines/mermaid.ts",
});

export const NON_LITERAL_DYNAMIC_IMPORT_OWNERS = Object.freeze({
  "src/runtime/realm/engine-artifact-loader.ts": 1,
});

export const MERMAN_WEB_ROOT_IMPORT = "@mermanjs/web";
export const MERMAN_WASM_SHIM_IMPORT =
  "@mermanjs/web/pkg/merman_wasm.js";

export const BENCHMARK_ADAPTER_FORBIDDEN_SOURCES = new Set([
  "src/main.tsx",
  "src/lib/bench-runner.ts",
  "src/runtime/RenderCoordinatorBridge.tsx",
  "src/runtime/mermaid-realm-controller.ts",
  "src/runtime/mermaid-realm.ts",
  "src/runtime/realm/compare-bootstrap.ts",
  "src/runtime/realm/parent-channel.ts",
  "src/runtime/render-coordinator-browser.ts",
  "src/runtime/render-coordinator.ts",
  "src/runtime/use-render-coordinator.ts",
  "src/benchmark/realm/controller.ts",
  "src/benchmark/controller.ts",
  "src/benchmark/schedule.ts",
  "src/benchmark/statistics.ts",
  "src/benchmark/report.ts",
  BENCHMARK_SOURCES.mermanArtifact,
]);

const SAFE_MERMAN_WEB_IMPORTS = new Set([
  "@mermanjs/web/catalog",
  "@mermanjs/web/svg-safety",
]);
const MERMAN_WASM_URL_IMPORT =
  "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";
const MERMAN_ADAPTER_DYNAMIC_IMPORTS = [
  MERMAN_WASM_SHIM_IMPORT,
  MERMAN_WEB_ROOT_IMPORT,
].sort();
const SCRIPT_EXTENSION = /\.[cm]?[jt]sx?$/i;

export function inspectBenchmarkSourceBoundaries(rootDir) {
  const root = path.resolve(rootDir);
  const staticGraph = collectSourceImportGraph({
    rootDir: root,
    entries: [BENCHMARK_SOURCES.bootstrap],
    includeDynamicImports: false,
  });
  const adapterGraphs = {
    merman: collectSourceImportGraph({
      rootDir: root,
      entries: [BENCHMARK_SOURCES.mermanAdapter],
      includeDynamicImports: true,
    }),
    mermaid: collectSourceImportGraph({
      rootDir: root,
      entries: [BENCHMARK_SOURCES.mermaidAdapter],
      includeDynamicImports: true,
    }),
  };
  const adapterOwnershipGraphs = {
    merman: collectSourceOwnershipGraph({
      rootDir: root,
      entries: [BENCHMARK_SOURCES.mermanAdapter],
    }),
    mermaid: collectSourceOwnershipGraph({
      rootDir: root,
      entries: [BENCHMARK_SOURCES.mermaidAdapter],
    }),
  };
  const violations = [];

  for (const moduleImport of staticGraph.packageImports) {
    if (
      isMermanWebImport(moduleImport.specifier) &&
      !SAFE_MERMAN_WEB_IMPORTS.has(moduleImport.specifier)
    ) {
      violations.push(
        `Benchmark static graph reaches disallowed Merman runtime import ${JSON.stringify(moduleImport.specifier)} from ${moduleImport.from}.`,
      );
    }
  }

  for (const [engine, graph] of Object.entries(adapterOwnershipGraphs)) {
    const otherAdapter =
      engine === "merman"
        ? BENCHMARK_SOURCES.mermaidAdapter
        : BENCHMARK_SOURCES.mermanAdapter;
    if (graph.files.has(otherAdapter)) {
      violations.push(
        `${capitalize(engine)} adapter reaches the ${otherAdapter} adapter.`,
      );
    }

    for (const source of BENCHMARK_ADAPTER_FORBIDDEN_SOURCES) {
      if (graph.files.has(source)) {
        violations.push(
          `${capitalize(engine)} adapter reaches forbidden source ${source}.`,
        );
      }
    }
  }

  verifyAdapterWebImports("merman", adapterGraphs.merman, violations);
  verifyAdapterWebImports("mermaid", adapterGraphs.mermaid, violations);

  const directMermanDynamicImports = adapterGraphs.merman.packageImports
    .filter(
      (moduleImport) =>
        moduleImport.from === BENCHMARK_SOURCES.mermanAdapter &&
        moduleImport.kind === "dynamic" &&
        isMermanWebImport(moduleImport.specifier),
    )
    .map((moduleImport) => moduleImport.specifier)
    .sort();
  if (!equalStringArrays(directMermanDynamicImports, MERMAN_ADAPTER_DYNAMIC_IMPORTS)) {
    violations.push(
      `Merman adapter must directly and dynamically import exactly ${MERMAN_ADAPTER_DYNAMIC_IMPORTS.join(", ")}; found ${directMermanDynamicImports.join(", ") || "none"}.`,
    );
  }

  return {
    adapterGraphs,
    adapterOwnershipGraphs,
    directMermanDynamicImports,
    staticGraph,
    violations,
  };
}

export function collectSourceImportGraph({
  rootDir,
  entries,
  includeDynamicImports,
}) {
  return collectSourceGraph({
    rootDir,
    entries,
    includeImport: (moduleImport) =>
      moduleImport.kind !== "type" &&
      (includeDynamicImports || moduleImport.kind !== "dynamic"),
  });
}

export function collectSourceOwnershipGraph({ rootDir, entries }) {
  return collectSourceGraph({
    rootDir,
    entries,
    includeImport: () => true,
  });
}

function collectSourceGraph({ rootDir, entries, includeImport }) {
  const root = path.resolve(rootDir);
  const files = new Set();
  const visitedFiles = new Set();
  const localImports = [];
  const packageImports = [];
  const pending = entries.map((entry) => resolveEntry(root, entry));

  while (pending.length > 0) {
    const absoluteFile = pending.pop();
    const source = relativeSource(root, absoluteFile);
    if (visitedFiles.has(source)) continue;
    visitedFiles.add(source);
    files.add(source);
    if (!SCRIPT_EXTENSION.test(absoluteFile)) continue;

    const imports = collectModuleImports(
      readFileSync(absoluteFile, "utf8"),
      source,
    );
    for (const moduleImport of imports) {
      if (!includeImport(moduleImport)) continue;
      if (!isLocalSourceImport(moduleImport.specifier)) {
        packageImports.push({ from: source, ...moduleImport });
        continue;
      }

      const target = resolveLocalModule(root, absoluteFile, moduleImport.specifier);
      const targetSource = relativeSource(root, target);
      localImports.push({ from: source, to: targetSource, ...moduleImport });
      pending.push(target);
    }
  }

  return { files, localImports, packageImports };
}

export function collectRuntimeImports(sourceText, fileName = "source.ts") {
  return collectModuleImports(sourceText, fileName).filter(
    (moduleImport) => moduleImport.kind !== "type",
  );
}

function collectModuleImports(sourceText, fileName) {
  const sourceFile = ts.createSourceFile(
    fileName,
    sourceText,
    ts.ScriptTarget.Latest,
    true,
    ts.getScriptKindFromFileName(fileName),
  );
  const imports = [];
  const expectedOwnedDynamicImports =
    NON_LITERAL_DYNAMIC_IMPORT_OWNERS[fileName];
  let ownedDynamicImports = 0;

  function addModuleRequest(node, moduleSpecifier, kind) {
    if (!moduleSpecifier || !ts.isStringLiteralLike(moduleSpecifier)) {
      if (kind === "dynamic" && expectedOwnedDynamicImports !== undefined) {
        ownedDynamicImports += 1;
        return;
      }
      const { line, character } = sourceFile.getLineAndCharacterOfPosition(
        node.getStart(sourceFile),
      );
      throw new Error(
        `${fileName}:${line + 1}:${character + 1} uses a non-literal ${kind} module request.`,
      );
    }
    imports.push({ kind, specifier: moduleSpecifier.text });
  }

  function visit(node) {
    if (ts.isImportDeclaration(node)) {
      addModuleRequest(
        node,
        node.moduleSpecifier,
        isTypeOnlyImport(node) ? "type" : "static",
      );
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier) {
      addModuleRequest(
        node,
        node.moduleSpecifier,
        isTypeOnlyExport(node) ? "type" : "static",
      );
    } else if (
      ts.isImportEqualsDeclaration(node) &&
      ts.isExternalModuleReference(node.moduleReference)
    ) {
      addModuleRequest(
        node,
        node.moduleReference.expression,
        node.isTypeOnly ? "type" : "static",
      );
    } else if (ts.isImportTypeNode(node)) {
      addModuleRequest(
        node,
        ts.isLiteralTypeNode(node.argument) ? node.argument.literal : undefined,
        "type",
      );
    } else if (ts.isCallExpression(node)) {
      if (node.expression.kind === ts.SyntaxKind.ImportKeyword) {
        addModuleRequest(node, node.arguments[0], "dynamic");
      } else if (
        ts.isIdentifier(node.expression) &&
        node.expression.text === "require"
      ) {
        addModuleRequest(node, node.arguments[0], "static");
      }
    }
    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
  if (
    expectedOwnedDynamicImports !== undefined &&
    ownedDynamicImports !== expectedOwnedDynamicImports
  ) {
    throw new Error(
      `${fileName} must own exactly ${expectedOwnedDynamicImports} non-literal dynamic module request; found ${ownedDynamicImports}.`,
    );
  }
  return imports;
}

function verifyAdapterWebImports(engine, graph, violations) {
  for (const moduleImport of graph.packageImports) {
    if (!isMermanWebImport(moduleImport.specifier)) continue;
    const isSafeImport = SAFE_MERMAN_WEB_IMPORTS.has(moduleImport.specifier);
    const isMermanWasmUrl =
      engine === "merman" &&
      moduleImport.from === BENCHMARK_SOURCES.mermanAdapter &&
      moduleImport.kind === "static" &&
      moduleImport.specifier === MERMAN_WASM_URL_IMPORT;
    const isMermanDynamicEngineImport =
      engine === "merman" &&
      moduleImport.from === BENCHMARK_SOURCES.mermanAdapter &&
      moduleImport.kind === "dynamic" &&
      MERMAN_ADAPTER_DYNAMIC_IMPORTS.includes(moduleImport.specifier);
    if (!isSafeImport && !isMermanWasmUrl && !isMermanDynamicEngineImport) {
      violations.push(
        `${capitalize(engine)} adapter reaches disallowed Merman runtime import ${JSON.stringify(moduleImport.specifier)} from ${moduleImport.from} via a ${moduleImport.kind} edge.`,
      );
    }
  }
}

function isTypeOnlyImport(node) {
  const clause = node.importClause;
  if (!clause) return false;
  if (clause.isTypeOnly) return true;
  if (clause.name || !clause.namedBindings) return false;
  return (
    ts.isNamedImports(clause.namedBindings) &&
    clause.namedBindings.elements.length > 0 &&
    clause.namedBindings.elements.every((element) => element.isTypeOnly)
  );
}

function isTypeOnlyExport(node) {
  if (node.isTypeOnly) return true;
  return (
    node.exportClause &&
    ts.isNamedExports(node.exportClause) &&
    node.exportClause.elements.length > 0 &&
    node.exportClause.elements.every((element) => element.isTypeOnly)
  );
}

function resolveEntry(root, entry) {
  const file = path.resolve(root, entry);
  if (!isFile(file)) {
    throw new Error(`Benchmark source graph entry does not exist: ${entry}`);
  }
  relativeSource(root, file);
  return file;
}

function resolveLocalModule(root, importer, specifier) {
  const sourceSpecifier = specifier.split(/[?#]/, 1)[0];
  const base = sourceSpecifier.startsWith("@/")
    ? path.resolve(root, sourceSpecifier.slice(2))
    : sourceSpecifier.startsWith("/")
      ? path.resolve(root, sourceSpecifier.slice(1))
      : path.resolve(path.dirname(importer), sourceSpecifier);
  const extension = path.extname(base);
  const candidates = [base];

  if (extension === ".js" || extension === ".jsx" || extension === ".mjs") {
    candidates.push(
      base.slice(0, -extension.length) + ".ts",
      base.slice(0, -extension.length) + ".tsx",
      base.slice(0, -extension.length) + ".mts",
    );
  } else if (!extension) {
    candidates.push(
      `${base}.ts`,
      `${base}.tsx`,
      `${base}.mts`,
      `${base}.cts`,
      `${base}.js`,
      `${base}.jsx`,
      path.join(base, "index.ts"),
      path.join(base, "index.tsx"),
      path.join(base, "index.js"),
    );
  }

  const target = candidates.find(isFile);
  if (!target) {
    throw new Error(
      `Cannot resolve local runtime import ${JSON.stringify(specifier)} from ${relativeSource(root, importer)}.`,
    );
  }
  relativeSource(root, target);
  return target;
}

function relativeSource(root, file) {
  const relative = path.relative(root, file);
  if (relative === "" || relative === ".." || relative.startsWith(`..${path.sep}`)) {
    throw new Error(`Source graph escaped the Playground root: ${file}`);
  }
  return relative.replaceAll(path.sep, "/");
}

function isFile(file) {
  try {
    return existsSync(file) && statSync(file).isFile();
  } catch {
    return false;
  }
}

function isMermanWebImport(specifier) {
  return (
    specifier === MERMAN_WEB_ROOT_IMPORT ||
    specifier.startsWith(`${MERMAN_WEB_ROOT_IMPORT}/`)
  );
}

function isLocalSourceImport(specifier) {
  return (
    specifier.startsWith(".") ||
    specifier.startsWith("/") ||
    specifier.startsWith("@/")
  );
}

function equalStringArrays(left, right) {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}

function capitalize(value) {
  return value[0].toUpperCase() + value.slice(1);
}
