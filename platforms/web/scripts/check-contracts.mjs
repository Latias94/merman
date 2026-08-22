import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  allPackageRuntimeExportNames,
  resourceContractValueExportNames,
  surfaceModules,
  webPackages,
} from "./surface-manifest.mjs";
import { loadTypeScriptContract } from "./typescript-contract.mjs";
import { scanWebArchitecture } from "./web-architecture.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const fullWasmTypes = path.join(root, "pkg", "full", "merman_wasm.d.ts");
const publicEntry = path.join(root, "src", "index.ts");
const publicCatalog = path.join(root, "src", "public-catalog.ts");
const publicTypes = path.join(root, "src", "public-types.ts");
const bindingOptionsTypeTests = path.join(
  root,
  "type-tests",
  "binding-options.ts",
);
const generatedRenamePolicy = path.join(
  root,
  "src",
  "generated",
  "editor-rename-policy.ts",
);
const runtimeState = path.join(root, "src", "runtime-state.ts");
const packageEntries = webPackages.map((descriptor) => descriptor.id);

const contract = loadTypeScriptContract({
  tsconfigPath: path.join(root, "tsconfig.json"),
  extraRootNames: [fullWasmTypes, bindingOptionsTypeTests],
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
const canonicalOperationFacades = new Set(["svgPlanJson"]);
const stableWrapperOnlyExports = new Set([
  "createBrowserTextMeasurementSession",
  "encodeOptions",
  "isAsciiDiagramType",
  "isBindingErrorPayload",
  "isBindingStatusCodeName",
  "isBundledThemePresetName",
  "isDiagramType",
  "isThemeName",
  "normalizeBundledThemePresetName",
  "normalizeThemeName",
]);
const stablePublicTypes = new Set([
  "HostTextMeasurementOperation",
  "HostTextMeasurementResultKind",
]);

const rawWasmExports = contract.exportedValueNames(fullWasmTypes);
const rawEditorSessionProperties = contract.exportedTypePropertyNames(
  fullWasmTypes,
  "EditorSession",
);
const publicValueExports = contract.exportedValueNames(publicEntry);
const publicTypeExports = contract.exportedTypeNames(publicEntry);
const catalogValueExports = contract.exportedValueNames(publicCatalog);
const catalogTypeExports = contract.exportedTypeNames(publicCatalog);
const declaredPublicTypes = contract.exportedTypeNames(publicTypes);
const wasmModuleProperties = contract.exportedTypePropertyNames(
  publicEntry,
  "MermanWasmModule",
);
const generatedPackageBindings = new Set(allPackageRuntimeExportNames);
const requiredRawWrappers = [...rawWasmExports].filter(
  (name) => !wasmGlueExports.has(name),
);
const requiredPublicWrappers = [
  ...requiredRawWrappers,
  ...runtimeWrapperOnlyExports,
  ...canonicalOperationFacades,
  ...stableWrapperOnlyExports,
];
const requiredRuntimeBindings = [
  ...requiredRawWrappers,
  ...runtimeWrapperOnlyExports,
  ...canonicalOperationFacades,
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
      "searchDocumentSymbols",
      "definition",
      "references",
      "prepareRename",
      "rename",
      "dispose",
    ],
  ],
  ["BrowserTextMeasurementSession", ["measure", "dispose"]],
  ["ResourceOptions", ["profile", "limits"]],
  ["EditorResourceOptions", ["profile", "limits"]],
  [
    "RuntimeCatalog",
    [
      "schema_version",
      "transport_api_version",
      "package_version",
      "capabilities",
      "registry",
      "resources",
    ],
  ],
  [
    "RuntimeCapabilities",
    [
      "capability_ids",
      "output_ids",
      "operation_ids",
      "system_adapter_ids",
      "text_measurement",
    ],
  ],
  ["TextMeasurementCapabilities", ["protocol_version", "provider_ids"]],
  [
    "RuntimeResourceContract",
    [
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
    [
      "flowchart_node_label_wrap_width",
      "flowchartNodeLabelWrapWidth",
      "relation_summary_diagnostics",
      "relationSummaryDiagnostics",
    ],
  ],
  ["CommonBindingOptions", ["analysis", "merman", "parse"]],
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
      generatedRenamePolicy,
      "EditorRenamePolicy",
    ),
  ],
]);
const requiredTypePropertyTypes = [
  ["AnalysisResult", "version", "1"],
  ["AnalysisFactsResult", "version", "2"],
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
  "check-contracts: wasm-bindgen EditorSession is missing document symbol search",
  ["searchDocumentSymbols"].filter(
    (name) => !rawEditorSessionProperties.has(name),
  ),
);
failed ||= reportMissing(
  "check-contracts: stable public TypeScript helpers are missing",
  requiredPublicWrappers.filter((name) => !publicValueExports.has(name)),
);
failed ||= reportMissing(
  "check-contracts: root public API is missing resource contract values",
  resourceContractValueExportNames.filter((name) => !publicValueExports.has(name)),
);
failed ||= reportMissing(
  "check-contracts: generated public ABI types are missing",
  [...stablePublicTypes].filter((name) => !publicTypeExports.has(name)),
);
failed ||= reportPolicyFailure(
  "check-contracts: legacy createBrowserTextMeasurer export must be removed",
  publicValueExports.has("createBrowserTextMeasurer"),
);
failed ||= reportPolicyFailure(
  "check-contracts: legacy boolean capability exports must be removed",
  publicValueExports.has("bindingCapabilities") ||
    publicValueExports.has("DEFAULT_BINDING_CAPABILITIES"),
);
failed ||= reportPolicyFailure(
  "check-contracts: legacy ABI version export must be removed",
  publicValueExports.has("MERMAN_ABI_VERSION") || publicValueExports.has("abiVersion"),
);
failed ||= reportPolicyFailure(
  "check-contracts: split runtime metadata exports must be removed",
  publicValueExports.has("runtimeCapabilities") ||
    publicValueExports.has("runtimeContract"),
);
failed ||= reportMissing(
  "check-contracts: package manifest will not regenerate runtime-bound wrappers",
  requiredRuntimeBindings.filter((name) => !generatedPackageBindings.has(name)),
);
failed ||= reportUnexpected(
  "check-contracts: platform Web imports a Playground-owned Mermaid renderer module",
  scanWebArchitecture(path.join(root, "src")).map(
    ({ file, line, column, rule, detail }) =>
      `${file}:${line}:${column} [${rule}] ${detail}`,
  ),
);

for (const module of surfaceModules.filter(
  ({ exactValueExports }) => exactValueExports,
)) {
  const implementation = path.resolve(
    path.join(root, "src", "package-entries"),
    module.specifier.replace(/\.js$/, ".ts"),
  );
  const actualValues = contract.exportedValueNames(implementation);
  const expectedValues = new Set([
    ...module.runtimeExportNames,
    ...module.valueExportNames,
    ...module.internalValueExportNames,
  ]);
  failed ||= reportMissing(
    `check-contracts: shared module ${module.specifier} is missing declared values`,
    difference(expectedValues, actualValues),
  );
  failed ||= reportUnexpected(
    `check-contracts: shared module ${module.specifier} exports an unowned implementation`,
    difference(actualValues, expectedValues),
  );
}

const runtimeStateProperties = contract.exportedTypePropertyNames(
  runtimeState,
  "MermanRuntimeState",
);
const expectedRuntimeStateProperties = new Set([
  "defaultLoader",
  "wasmModule",
  "initPromise",
  "supportedDiagramsCache",
  "diagramFamilyCapabilitiesCache",
  "runtimeCatalogCache",
  "presentationCatalogCache",
  "supportedThemesCache",
]);
failed ||= reportMissing(
  "check-contracts: shared runtime state is missing lifecycle or metadata fields",
  difference(expectedRuntimeStateProperties, runtimeStateProperties),
);
failed ||= reportUnexpected(
  "check-contracts: shared runtime state contains capability-owned fields",
  difference(runtimeStateProperties, expectedRuntimeStateProperties),
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

failed ||= reportPolicyFailure(
  "check-contracts: analysis wrappers must not expose the top-level parse option",
  contract
    .exportedTypePropertyNames(publicEntry, "AnalysisBindingOptions")
    .has("parse"),
);
failed ||= reportPolicyFailure(
  "check-contracts: editor options must expose only analysis-owned configuration fields",
  !contract
    .exportedTypePropertyNames(publicEntry, "EditorBindingOptions")
    .has("analysis") ||
    contract
      .exportedTypePropertyNames(publicEntry, "EditorBindingOptions")
      .has("parse"),
);
failed ||= reportPolicyFailure(
  "check-contracts: SVG options must use presentation instead of the removed host_theme group",
  contract
    .exportedTypePropertyNames(publicEntry, "SvgBindingOptions")
    .has("host_theme") ||
    !contract
      .exportedTypePropertyNames(publicEntry, "SvgBindingOptions")
      .has("presentation"),
);
failed ||= reportPolicyFailure(
  "check-contracts: legacy single-document workspace symbol names must be removed",
  publicValueExports.has("editorWorkspaceSymbols") ||
    rawWasmExports.has("editorWorkspaceSymbols") ||
    rawEditorSessionProperties.has("workspaceSymbols") ||
    contract
      .exportedTypePropertyNames(publicEntry, "BrowserEditorSession")
      .has("workspaceSymbols") ||
    contract
      .exportedTypePropertyNames(publicEntry, "WasmEditorSessionBinding")
      .has("workspaceSymbols") ||
    wasmModuleProperties.has("editorWorkspaceSymbols"),
);

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

for (const descriptor of webPackages) {
  const entry = path.join(root, "src", "package-entries", `${descriptor.id}.ts`);
  const actualValues = contract.declaredValueExportNames(entry);
  const expectedValues = new Set([
    "MERMAN_WASM_URL",
    "loadMermanWasmModule",
    ...descriptor.runtimeExportNames,
    ...descriptor.valueExportNames,
  ]);
  const typeStars = contract.typeOnlyStarExportSpecifiers(entry);
  const valueStars = contract.valueStarExportSpecifiers(entry);

  failed ||= reportMissing(
    `check-contracts: ${descriptor.name} package entry is missing declared value exports`,
    difference(expectedValues, actualValues),
  );
  failed ||= reportUnexpected(
    `check-contracts: ${descriptor.name} package entry exports undeclared values`,
    difference(actualValues, expectedValues),
  );
  failed ||= reportPolicyFailure(
    `check-contracts: ${descriptor.name} package entry must type-export the shared root contract`,
    !typeStars.has("../index.js"),
  );
  failed ||= reportUnexpected(
    `check-contracts: ${descriptor.name} package entry has forbidden value star exports`,
    [...valueStars],
  );

  for (const { specifier, exportNames } of [
    ...descriptor.runtimeExportModules,
    ...descriptor.valueExportModules,
  ]) {
    const implementation = path.resolve(
      path.dirname(entry),
      specifier.replace(/\.js$/, ".ts"),
    );
    const implementationExports = contract.exportedValueNames(implementation);
    failed ||= reportMissing(
      `check-contracts: ${descriptor.name} module owner ${specifier} is missing exports`,
      exportNames.filter((name) => !implementationExports.has(name)),
    );
  }
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
    `${packageEntries.length} package entries checked through TypeScript.`,
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
