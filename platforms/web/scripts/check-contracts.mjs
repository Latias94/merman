import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  allSurfaceRuntimeExportNames,
  surfaces,
} from "./surface-manifest.mjs";
import { loadTypeScriptContract } from "./typescript-contract.mjs";
import { scanWebArchitecture } from "./web-architecture.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const fullWasmTypes = path.join(root, "pkg", "full", "merman_wasm.d.ts");
const publicEntry = path.join(root, "src", "index.ts");
const publicCatalog = path.join(root, "src", "public-catalog.ts");
const publicTypes = path.join(root, "src", "public-types.ts");
const generatedTokenDescriptor = path.join(
  root,
  "src",
  "generated",
  "token-descriptor.ts",
);
const surfaceRuntime = path.join(root, "src", "surface-runtime.ts");
const surfaceEntries = surfaces.map((surface) => surface.entry);

const contract = loadTypeScriptContract({
  tsconfigPath: path.join(root, "tsconfig.json"),
  extraRootNames: [fullWasmTypes],
});
const diagnostics = contract.diagnostics();
if (diagnostics.length > 0) {
  console.error("check-contracts: TypeScript program is invalid");
  console.error(contract.formatDiagnostics(diagnostics));
  process.exit(1);
}

const wasmGlueExports = new Set(["default", "EditorSession", "initSync", "start"]);
const runtimeWrapperOnlyExports = new Set([
  "createEditorSession",
  "initMerman",
  "getMerman",
  "isMermanInitialized",
  "renderSvgElement",
  "renderSvgToElement",
  "parseObject",
  "layoutObject",
  "detectDiagramFacts",
]);
const stableWrapperOnlyExports = new Set([
  "createBrowserTextMeasurementSession",
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
const stablePublicTypes = new Set([
  "HostTextMeasurementOperation",
  "HostTextMeasurementResultKind",
]);

const rawWasmExports = contract.exportedValueNames(fullWasmTypes);
const publicValueExports = contract.exportedValueNames(publicEntry);
const publicTypeExports = contract.exportedTypeNames(publicEntry);
const catalogValueExports = contract.exportedValueNames(publicCatalog);
const catalogTypeExports = contract.exportedTypeNames(publicCatalog);
const declaredPublicTypes = contract.exportedTypeNames(publicTypes);
const wasmModuleProperties = contract.exportedTypePropertyNames(
  publicEntry,
  "MermanWasmModule",
);
const runtimeBindings = contract.exportedFunctionReturnPropertyNames(
  surfaceRuntime,
  "bindSurfaceRuntime",
);
const generatedSurfaceBindings = new Set(allSurfaceRuntimeExportNames);
const requiredRawWrappers = [...rawWasmExports].filter(
  (name) => !wasmGlueExports.has(name),
);
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
    "BrowserEditorSession",
    [
      "version",
      "uri",
      "update",
      "diagnostics",
      "diagramDetection",
      "codeActions",
      "completions",
      "hover",
      "documentSymbols",
      "workspaceSymbols",
      "definition",
      "references",
      "prepareRename",
      "rename",
      "semanticTokens",
      "dispose",
    ],
  ],
  ["BrowserTextMeasurementSession", ["measure", "dispose"]],
  ["ResourceOptions", ["profile", "limits"]],
  [
    "RuntimeContract",
    [
      "schema_version",
      "abi_version",
      "package_version",
      "options_schema_version",
      "payload_schemas",
      "features",
      "registry",
      "resources",
    ],
  ],
  [
    "RuntimeResourceContract",
    [
      "schema_version",
      "general_binding_default_profile",
      "cli_default_profile",
      "limits",
      "profiles",
    ],
  ],
  [
    "RuntimeResourceLimit",
    ["id", "phase", "description", "overridable", "hard_cap"],
  ],
  [
    "RuntimeResourceProfile",
    [
      "id",
      "purpose",
      "trust_assumption",
      "recommended_binding_default",
      "limits",
    ],
  ],
  [
    "AsciiRenderOptions",
    ["relation_summary_diagnostics", "relationSummaryDiagnostics"],
  ],
  ["CommonBindingOptions", ["analysis", "merman"]],
  ["AnalysisBindingOptions", ["resources"]],
  [
    "AnalysisDiagramSyntaxFacts",
    ["source_mapped_spans", "effective_layout"],
  ],
  [
    "AvailableDiagramDetectionFacts",
    ["status", "diagramType", "syntaxId", "effectiveLayoutId"],
  ],
  [
    "UnavailableDiagramDetectionFacts",
    ["status", "diagramType", "syntaxId", "effectiveLayoutId"],
  ],
  ["AnalysisSemanticItemFacts", ["rename_policy"]],
  [
    "EditorDiagnosticData",
    ["id", "code", "codeName", "category", "diagramType", "help", "fixes"],
  ],
]);
const requiredTypeStringLiterals = new Map([
  [
    "EditorSemanticFactSource",
    [
      "unavailable",
      "parser_complete",
      "parser_recovered",
    ],
  ],
]);
const exactTypeStringLiterals = new Map([
  [
    "AnalysisRenamePolicy",
    contract.exportedStringLiteralMembers(
      generatedTokenDescriptor,
      "EditorRenamePolicy",
    ),
  ],
]);
const requiredTypePropertyTypes = [
  ["AnalysisResult", "version", "1"],
  ["AnalysisFactsResult", "version", "1"],
];

let failed = false;
failed ||= reportMissing(
  "check-contracts: root public API is missing public catalog values",
  difference(catalogValueExports, publicValueExports),
);
failed ||= reportMissing(
  "check-contracts: root public API is missing public catalog types",
  difference(catalogTypeExports, publicTypeExports),
);
failed ||= reportMissing(
  "check-contracts: root public API is missing public option/result types",
  difference(declaredPublicTypes, publicTypeExports),
);
failed ||= reportMissing(
  "check-contracts: wasm-bindgen exports without public TypeScript wrappers",
  requiredRawWrappers.filter((name) => !publicValueExports.has(name)),
);
failed ||= reportMissing(
  "check-contracts: wasm-bindgen exports missing from MermanWasmModule",
  requiredRawWrappers.filter((name) => !wasmModuleProperties.has(name)),
);
failed ||= reportMissing(
  "check-contracts: stable public TypeScript helpers are missing",
  requiredPublicWrappers.filter((name) => !publicValueExports.has(name)),
);
failed ||= reportMissing(
  "check-contracts: generated public ABI types are missing",
  [...stablePublicTypes].filter((name) => !publicTypeExports.has(name)),
);
failed ||= reportPolicyFailure(
  "check-contracts: legacy createBrowserTextMeasurer export must be removed",
  publicValueExports.has("createBrowserTextMeasurer"),
);
failed ||= reportMissing(
  "check-contracts: runtime-dependent wrappers are not returned by bindSurfaceRuntime()",
  requiredRuntimeBindings.filter((name) => !runtimeBindings.has(name)),
);
failed ||= reportMissing(
  "check-contracts: surface manifest will not regenerate runtime-bound wrappers",
  requiredRuntimeBindings.filter((name) => !generatedSurfaceBindings.has(name)),
);
failed ||= reportUnexpected(
  "check-contracts: platform Web imports a Playground-owned Mermaid renderer module",
  scanWebArchitecture(path.join(root, "src")).map(
    ({ file, line, column, rule, detail }) =>
      `${file}:${line}:${column} [${rule}] ${detail}`,
  ),
);

for (const [interfaceName, requiredProperties] of requiredTypeProperties) {
  const properties = contract.exportedTypePropertyNames(
    publicEntry,
    interfaceName,
  );
  failed ||= reportMissing(
    `check-contracts: ${interfaceName} is missing required properties`,
    requiredProperties.filter((name) => !properties.has(name)),
  );
}

for (const [typeName, requiredLiterals] of requiredTypeStringLiterals) {
  const literals = contract.exportedStringLiteralMembers(publicEntry, typeName);
  failed ||= reportMissing(
    `check-contracts: ${typeName} is missing required string members`,
    requiredLiterals.filter((literal) => !literals.has(literal)),
  );
}

for (const [typeName, expectedLiterals] of exactTypeStringLiterals) {
  const literals = contract.exportedStringLiteralMembers(publicEntry, typeName);
  failed ||= reportPolicyFailure(
    `check-contracts: ${typeName} must be a closed generated string-literal union`,
    !contract.exportedTypeIsStringLiteralUnion(publicEntry, typeName),
  );
  failed ||= reportMissing(
    `check-contracts: ${typeName} is missing generated string members`,
    difference(expectedLiterals, literals),
  );
  failed ||= reportUnexpected(
    `check-contracts: ${typeName} has members outside the generated contract`,
    difference(literals, expectedLiterals),
  );
}

for (const [interfaceName, propertyName, expectedType] of requiredTypePropertyTypes) {
  const actualType = contract.exportedTypePropertyText(
    publicEntry,
    interfaceName,
    propertyName,
  );
  failed ||= reportPolicyFailure(
    `check-contracts: ${interfaceName}.${propertyName} must use type ${expectedType}`,
    actualType !== expectedType,
  );
}

for (const surface of surfaces) {
  const entry = path.join(root, "src", "surfaces", `${surface.entry}.ts`);
  const actualValues = contract.declaredValueExportNames(entry);
  const expectedValues = new Set([
    ...surface.runtimeExportNames,
    ...surface.valueExportNames,
  ]);
  const typeStars = contract.typeOnlyStarExportSpecifiers(entry);
  const valueStars = contract.valueStarExportSpecifiers(entry);

  failed ||= reportMissing(
    `check-contracts: ./${surface.entry} surface is missing declared value exports`,
    difference(expectedValues, actualValues),
  );
  failed ||= reportUnexpected(
    `check-contracts: ./${surface.entry} surface exports undeclared values`,
    difference(actualValues, expectedValues),
  );
  failed ||= reportPolicyFailure(
    `check-contracts: ./${surface.entry} surface must type-export the shared root contract`,
    !typeStars.has("../index.js"),
  );
  failed ||= reportUnexpected(
    `check-contracts: ./${surface.entry} surface has forbidden value star exports`,
    [...valueStars],
  );
}

if (failed) {
  console.error(
    [
      "",
      "A Rust WASM export, TypeScript facade, or generated subpath contract drifted.",
      "Run `npm run build --prefix platforms/web` after updating the owning manifest or descriptor.",
    ].join("\n"),
  );
  process.exit(1);
}

console.log(
  `check-contracts: ${requiredRawWrappers.length} wasm exports, ` +
    `${requiredRuntimeBindings.length} runtime bindings, ` +
    `${surfaceEntries.length} surfaces checked through TypeScript.`,
);

function difference(left, right) {
  return [...left].filter((value) => !right.has(value));
}

function reportMissing(title, missing) {
  if (missing.length === 0) return false;
  console.error([title, ...missing.sort().map((name) => `  - ${name}`)].join("\n"));
  return true;
}

function reportUnexpected(title, unexpected) {
  if (unexpected.length === 0) return false;
  console.error(
    [title, ...unexpected.sort().map((name) => `  - ${name}`)].join("\n"),
  );
  return true;
}

function reportPolicyFailure(title, isFailure) {
  if (!isFailure) return false;
  console.error(title);
  return true;
}
