import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const fullWasmTypes = path.join(root, "pkg", "full", "merman_wasm.d.ts");
const publicApiSource = path.join(root, "src", "index.ts");
const surfaceRuntimeSource = path.join(root, "src", "surface-runtime.ts");
const surfaceGeneratorSource = path.join(root, "scripts", "build-surface-packages.mjs");
const surfaceEntries = ["core", "render", "ascii", "full"];

const wasmGlueExports = new Set(["initSync", "start"]);
const runtimeWrapperOnlyExports = new Set([
  "initMerman",
  "getMerman",
  "isMermanInitialized",
  "renderSvgElement",
  "renderSvgToElement",
  "parseObject",
  "layoutObject",
]);
const stableWrapperOnlyExports = new Set([
  "createBrowserTextMeasurer",
  "encodeOptions",
  "isAsciiDiagramType",
  "isBindingErrorPayload",
  "isBindingStatusCodeName",
  "isDiagramType",
  "isHostThemePresetName",
  "isThemeName",
  "normalizeHostThemePresetName",
  "normalizeThemeName",
]);

const rawWasmExports = [...extractExportedFunctionNames(read(fullWasmTypes))];
const publicApi = read(publicApiSource);
const publicWrappers = extractExportedFunctionNames(publicApi);
const wasmModuleProperties = extractInterfaceProperties(publicApi, "MermanWasmModule");
const runtimeBindings = extractSurfaceRuntimeBindings(read(surfaceRuntimeSource));
const generatedSurfaceBindings = extractStringArray(
  read(surfaceGeneratorSource),
  "surfaceRuntimeExportNames",
);
const requiredRawWrappers = rawWasmExports.filter((name) => !wasmGlueExports.has(name));
const requiredPublicWrappers = [
  ...requiredRawWrappers,
  ...runtimeWrapperOnlyExports,
  ...stableWrapperOnlyExports,
];
const requiredRuntimeBindings = [
  ...requiredRawWrappers,
  ...runtimeWrapperOnlyExports,
];
const requiredTypeProperties = new Map([
  [
    "ResourceOptions",
    ["max_class_nodes", "max_class_edges", "max_class_namespaces"],
  ],
  [
    "AsciiRenderOptions",
    ["relation_summary_diagnostics", "relationSummaryDiagnostics"],
  ],
  [
    "CommonBindingOptions",
    ["analysis", "merman"],
  ],
  [
    "AnalysisBindingOptions",
    ["resources"],
  ],
]);

let failed = false;
failed ||= reportMissing(
  "check-contracts: wasm-bindgen exports without public TypeScript wrappers",
  requiredRawWrappers.filter((name) => !publicWrappers.has(name)),
);
failed ||= reportMissing(
  "check-contracts: wasm-bindgen exports missing from MermanWasmModule",
  requiredRawWrappers.filter((name) => !wasmModuleProperties.has(name)),
);
failed ||= reportMissing(
  "check-contracts: stable public TypeScript helpers are missing",
  requiredPublicWrappers.filter((name) => !publicWrappers.has(name)),
);
failed ||= reportMissing(
  "check-contracts: runtime-dependent wrappers are not rebound by bindSurfaceRuntime()",
  requiredRuntimeBindings.filter((name) => !runtimeBindings.has(name)),
);
failed ||= reportMissing(
  "check-contracts: build-surface-packages.mjs will not regenerate runtime-bound wrappers",
  requiredRuntimeBindings.filter((name) => !generatedSurfaceBindings.has(name)),
);

for (const [interfaceName, requiredProperties] of requiredTypeProperties) {
  const properties = extractInterfaceProperties(publicApi, interfaceName);
  failed ||= reportMissing(
    `check-contracts: ${interfaceName} is missing required option properties`,
    requiredProperties.filter((name) => !properties.has(name)),
  );
}

for (const entry of surfaceEntries) {
  const surfaceSource = path.join(root, "src", "surfaces", `${entry}.ts`);
  const surfaceBindings = extractRuntimeDestructure(read(surfaceSource), surfaceSource);
  failed ||= reportMissing(
    `check-contracts: ./${entry} surface entry does not re-export runtime-bound wrappers`,
    requiredRuntimeBindings.filter((name) => !surfaceBindings.has(name)),
  );
}

if (failed) {
  console.error(
    [
      "",
      "A Rust wasm export, TypeScript wrapper, or subpath runtime binding drifted.",
      "Run `npm run build --prefix platforms/web` after updating the wrapper surface.",
    ].join("\n"),
  );
  process.exit(1);
}

console.log(
  `check-contracts: ${requiredRawWrappers.length} wasm exports, ` +
    `${requiredRuntimeBindings.length} runtime bindings, ` +
    `${surfaceEntries.length} surfaces checked.`,
);

function read(file) {
  return readFileSync(file, "utf8");
}

function extractExportedFunctionNames(source) {
  return new Set(matches(source, /^export function\s+([A-Za-z_$][\w$]*)\s*(?:<[^(\n]+>)?\s*\(/gm));
}

function extractInterfaceProperties(source, interfaceName) {
  const escapedName = interfaceName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(
    new RegExp(`export interface ${escapedName}(?:\\s+extends\\s+[^\\{]+)?\\s*\\{([\\s\\S]*?)\\n\\}`),
  );
  if (!match) {
    throw new Error(`check-contracts: missing ${interfaceName} interface`);
  }
  return new Set(matches(match[1], /^\s+([A-Za-z_$][\w$]*)\??:\s*/gm));
}

function extractSurfaceRuntimeBindings(source) {
  const names = new Set();
  for (const match of source.matchAll(/^\s{4}([A-Za-z_$][\w$]*)\s*(?:\(|:)/gm)) {
    names.add(match[1]);
  }
  return names;
}

function extractRuntimeDestructure(source, file) {
  const match = source.match(/export const\s+\{([\s\S]*?)\}\s*=\s*runtime;/m);
  if (!match) {
    throw new Error(`check-contracts: missing runtime export destructure in ${path.relative(root, file)}`);
  }

  return new Set(
    match[1]
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean),
  );
}

function extractStringArray(source, constantName) {
  const escapedName = constantName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`const ${escapedName} = \\[([\\s\\S]*?)\\];`));
  if (!match) {
    throw new Error(`check-contracts: missing ${constantName} array`);
  }
  return new Set(matches(match[1], /^\s+"([A-Za-z_$][\w$]*)",?$/gm));
}

function matches(source, pattern) {
  return [...source.matchAll(pattern)].map((match) => match[1]);
}

function reportMissing(title, missing) {
  if (missing.length === 0) {
    return false;
  }

  console.error([title, ...missing.sort().map((name) => `  - ${name}`)].join("\n"));
  return true;
}
