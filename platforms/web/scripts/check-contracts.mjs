import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  allSurfaceRuntimeExportNames,
  surfaces,
} from "./surface-manifest.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const fullWasmTypes = path.join(root, "pkg", "full", "merman_wasm.d.ts");
const publicApiSource = path.join(root, "src", "index.ts");
const publicApiTypeSources = [
  publicApiSource,
  path.join(root, "src", "public-catalog.ts"),
  path.join(root, "src", "public-types.ts"),
];
const surfaceRuntimeSource = path.join(root, "src", "surface-runtime.ts");
const surfaceEntries = surfaces.map((surface) => surface.entry);

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
const publicEntryApi = read(publicApiSource);
const publicApi = publicApiTypeSources.map(read).join("\n");
const publicCatalogValueExports = extractNamedReExport(
  publicEntryApi,
  "./public-catalog.js",
  publicApiSource,
);
const publicWrappers = new Set([
  ...extractExportedFunctionNames(publicEntryApi),
  ...publicCatalogValueExports,
]);
const wasmModuleProperties = extractInterfaceProperties(publicApi, "MermanWasmModule");
const runtimeBindings = extractSurfaceRuntimeBindings(read(surfaceRuntimeSource));
const generatedSurfaceBindings = new Set(allSurfaceRuntimeExportNames);
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
  [
    "AnalysisDiagramSyntaxFacts",
    ["source_mapped_spans"],
  ],
  [
    "EditorDiagnosticData",
    ["id", "code", "codeName", "category", "diagramType", "help", "fixes"],
  ],
]);
const requiredTypeStringLiterals = new Map([
  [
    "EditorSemanticFactSource",
    [
      "text_scan",
      "parser_complete",
      "parser_complete_degraded_spans",
      "parser_recovered",
      "parser_recovered_degraded_spans",
    ],
  ],
]);

let failed = false;
failed ||= reportPolicyFailure(
  "check-contracts: root public API must type-re-export the public catalog",
  !hasTypeStarReExportFrom(publicEntryApi, "./public-catalog.js"),
);
failed ||= reportPolicyFailure(
  "check-contracts: root public API must type-re-export the public types",
  !hasTypeStarReExportFrom(publicEntryApi, "./public-types.js"),
);
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

for (const [typeName, requiredLiterals] of requiredTypeStringLiterals) {
  const literals = extractTypeStringLiterals(publicApi, typeName);
  failed ||= reportMissing(
    `check-contracts: ${typeName} is missing required string members`,
    requiredLiterals.filter((literal) => !literals.has(literal)),
  );
}

for (const surface of surfaces) {
  const surfaceSource = path.join(root, "src", "surfaces", `${surface.entry}.ts`);
  const surfaceApi = read(surfaceSource);
  const surfaceBindings = extractRuntimeDestructure(surfaceApi, surfaceSource);
  const allowedRuntimeBindings = new Set(surface.runtimeExportNames);
  const surfaceValueExports = extractNamedReExport(surfaceApi, "../index.js", surfaceSource);

  failed ||= reportMissing(
    `check-contracts: ./${surface.entry} surface entry is missing allowed runtime-bound wrappers`,
    surface.runtimeExportNames.filter((name) => !surfaceBindings.has(name)),
  );
  failed ||= reportUnexpected(
    `check-contracts: ./${surface.entry} surface entry exports unsupported runtime-bound wrappers`,
    [...surfaceBindings].filter((name) => !allowedRuntimeBindings.has(name)),
  );
  failed ||= reportMissing(
    `check-contracts: ./${surface.entry} surface entry is missing stable value exports`,
    surface.valueExportNames.filter((name) => !surfaceValueExports.has(name)),
  );
  failed ||= reportUnexpected(
    `check-contracts: ./${surface.entry} surface entry exports undeclared stable values`,
    [...surfaceValueExports].filter((name) => !surface.valueExportNames.includes(name)),
  );
  failed ||= reportPolicyFailure(
    `check-contracts: ./${surface.entry} surface entry must not use value star re-exports`,
    hasValueStarExport(surfaceApi),
  );
  failed ||= reportPolicyFailure(
    `check-contracts: ./${surface.entry} surface entry must type-re-export the shared public types`,
    !hasTypeStarExport(surfaceApi),
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

function extractTypeStringLiterals(source, typeName) {
  const escapedName = typeName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`export type ${escapedName}\\s*=([\\s\\S]*?);`));
  if (!match) {
    throw new Error(`check-contracts: missing ${typeName} type`);
  }
  return new Set(matches(match[1], /"([^"]+)"/g));
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

function extractNamedReExport(source, fromPath, file) {
  const escapedPath = fromPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(
    new RegExp(`export\\s+\\{([\\s\\S]*?)\\}\\s+from\\s+"${escapedPath}";`, "m"),
  );
  if (!match) {
    throw new Error(`check-contracts: missing named re-export in ${path.relative(root, file)}`);
  }

  return new Set(
    match[1]
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean),
  );
}

function hasValueStarExport(source) {
  return /^\s*export\s+\*\s+from\s+["'][^"']+["'];/m.test(source);
}

function hasTypeStarExport(source) {
  return /^\s*export\s+type\s+\*\s+from\s+["']\.\.\/index\.js["'];/m.test(source);
}

function hasTypeStarReExportFrom(source, fromPath) {
  const escapedPath = fromPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^\\s*export\\s+type\\s+\\*\\s+from\\s+["']${escapedPath}["'];`, "m").test(
    source,
  );
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

function reportUnexpected(title, unexpected) {
  if (unexpected.length === 0) {
    return false;
  }

  console.error([title, ...unexpected.sort().map((name) => `  - ${name}`)].join("\n"));
  return true;
}

function reportPolicyFailure(title, failed) {
  if (!failed) {
    return false;
  }

  console.error(title);
  return true;
}
