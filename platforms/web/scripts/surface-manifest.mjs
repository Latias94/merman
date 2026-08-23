import { webPackageDescriptors } from "./wasm-build/web-surface-descriptor.mjs";

const lifecycleRuntimeExportNames = [
  "initMerman",
  "getMerman",
  "isMermanInitialized",
];

const lifecycleWasmExportNames = ["default"];

const analysisRuntimeExportNames = [
  "analyze",
  "analyzeJson",
  "analysisFacts",
  "detectDiagramFacts",
  "analyzeDocument",
  "analyzeDocumentFacts",
  "validate",
];

const analysisWasmExportNames = [
  "analyze",
  "analyzeJson",
  "analysisFacts",
  "analyzeDocument",
  "analyzeDocumentFacts",
  "validate",
];

const metadataRuntimeExportNames = [
  "runtimeCatalog",
  "presentationCatalog",
  "supportedDiagrams",
  "diagramFamilyCapabilities",
  "supportedThemes",
  "transportApiVersion",
  "packageVersion",
];

const metadataWasmExportNames = [
  "runtimeCatalog",
  "presentationCatalog",
  "supportedDiagrams",
  "diagramFamilyCapabilities",
  "supportedThemes",
  "transportApiVersion",
  "packageVersion",
];

const analysisMetadataRuntimeExportNames = ["lintRuleCatalog"];
const analysisMetadataWasmExportNames = ["lintRuleCatalog"];

const renderRuntimeExportNames = [
  "renderSvg",
  "svgPlanJson",
  "renderSvgWithTextMeasurer",
  "layoutJsonWithTextMeasurer",
  "renderSvgElement",
  "renderSvgToElement",
  "parseJson",
  "parseObject",
  "layoutJson",
  "layoutObject",
];

const renderWasmExportNames = [
  "renderSvg",
  "svgPlanJson",
  "renderSvgWithTextMeasurer",
  "layoutJsonWithTextMeasurer",
  "parseJson",
  "layoutJson",
];

const asciiRuntimeExportNames = [
  "renderAscii",
  "asciiSupportedDiagrams",
  "asciiCapabilities",
  "asciiDiagrammaticDiagrams",
];

const asciiWasmExportNames = [
  "renderAscii",
  "asciiSupportedDiagrams",
  "asciiCapabilities",
];

const editorRuntimeExportNames = [
  "createEditorSession",
  "editorDiagnostics",
  "editorDiagramDetection",
  "editorCodeActions",
  "editorCompletions",
  "editorHover",
  "editorDocumentSymbols",
  "editorSearchDocumentSymbols",
  "editorDefinition",
  "editorReferences",
  "editorPrepareRename",
  "editorRename",
  "editorCompletionTriggerCharacters",
];

const editorWasmExportNames = [
  "EditorSession",
  "editorDiagnostics",
  "editorDiagramDetection",
  "editorCodeActions",
  "editorCompletions",
  "editorHover",
  "editorDocumentSymbols",
  "editorSearchDocumentSymbols",
  "editorDefinition",
  "editorReferences",
  "editorPrepareRename",
  "editorRename",
  "editorCompletionTriggerCharacters",
];

export const resourceContractValueExportNames = [
  "BINDING_OPTIONS_SCHEMA_VERSION",
  "RESOURCE_LIMIT_IDS",
  "RESOURCE_LIMIT_METADATA",
  "RESOURCE_OVERRIDE_IDS",
  "RESOURCE_PROFILES",
  "isKnownResourceLimitId",
  "rawResourceOptionsJson",
  "resourceLimitMetadata",
  "resourceOptions",
  "resourceOptionsJson",
];

export const packageStableValueExportNames = [
  "MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION",
  "UNAVAILABLE_DIAGRAM_DETECTION",
  "BUNDLED_THEME_PRESETS",
  "SUPPORTED_THEMES",
  "SUPPORTED_DIAGRAMS",
  "SUPPORTED_ASCII_DIAGRAMS",
  "DIAGRAMMATIC_ASCII_DIAGRAMS",
  "BINDING_STATUS_CODE_NAMES",
  "isThemeName",
  "isDiagramType",
  "isAsciiDiagramType",
  "isBindingStatusCodeName",
  "isBundledThemePresetName",
  "isBindingErrorPayload",
  "normalizeThemeName",
  "normalizeBundledThemePresetName",
  "encodeOptions",
  ...resourceContractValueExportNames,
];

export const packageRenderValueExportNames = [
  "assertNavigableSvgForDom",
  "assertSelfContainedSvgForDom",
  "prepareNavigableSvgForDomMount",
  "prepareSelfContainedSvgForDomMount",
  "createBrowserTextMeasurementSession",
];

export const surfaceModules = defineSurfaceModules([
  {
    id: "core",
    specifier: "../runtime-core.js",
    owner: "shared",
    runtimeExportNames: [
      ...lifecycleRuntimeExportNames,
      ...metadataRuntimeExportNames,
    ],
    valueExportNames: ["UNAVAILABLE_DIAGRAM_DETECTION", "encodeOptions"],
    internalValueExportNames: ["currentRuntimeState", "withResourceOptions"],
    exactValueExports: true,
  },
  {
    id: "analysis",
    specifier: "../runtime-analysis.js",
    owner: "analysis",
    runtimeExportNames: [
      ...analysisRuntimeExportNames,
      ...analysisMetadataRuntimeExportNames,
    ],
  },
  {
    id: "ascii",
    specifier: "../runtime-ascii.js",
    owner: "ascii",
    runtimeExportNames: asciiRuntimeExportNames,
  },
  {
    id: "render",
    specifier: "../runtime-render.js",
    owner: "render",
    runtimeExportNames: renderRuntimeExportNames,
    valueExportNames: ["createBrowserTextMeasurementSession"],
  },
  {
    id: "editor",
    specifier: "../runtime-editor.js",
    owner: "editor",
    runtimeExportNames: editorRuntimeExportNames,
  },
  {
    specifier: "../public-catalog.js",
    owner: "shared",
    valueExportNames: packageStableValueExportNames.filter(
      (name) =>
        name !== "MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION" &&
        name !== "UNAVAILABLE_DIAGRAM_DETECTION" &&
        name !== "encodeOptions" &&
        !resourceContractValueExportNames.includes(name),
    ),
  },
  {
    specifier: "../generated/text-measurement-abi.js",
    owner: "shared",
    valueExportNames: ["MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION"],
  },
  {
    specifier: "../svg-safety.js",
    owner: "render",
    valueExportNames: [
      "assertNavigableSvgForDom",
      "assertSelfContainedSvgForDom",
      "prepareNavigableSvgForDomMount",
      "prepareSelfContainedSvgForDomMount",
    ],
  },
  {
    specifier: "../runtime-state.js",
    owner: "shared",
    internalValueExportNames: [
      "createMermanRuntimeState",
      "currentMermanRuntimeState",
      "withMermanRuntimeState",
    ],
    exactValueExports: true,
  },
  {
    specifier: "../surface-runtime.js",
    owner: "shared",
    internalValueExportNames: ["assertBrowserRuntime", "bindSurfaceRuntime"],
    exactValueExports: true,
  },
  { specifier: "../generated/binding-contract.js", owner: "shared" },
  { specifier: "../generated/capability-surface.js", owner: "shared" },
  { specifier: "../generated/diagram-catalog.js", owner: "shared" },
  {
    specifier: "../generated/resource-contract.js",
    owner: "shared",
    valueExportNames: resourceContractValueExportNames,
  },
  { specifier: "../svg-safety-policy.js", owner: "render" },
]);

export const surfaceModuleOwners = Object.freeze(
  Object.fromEntries(
    surfaceModules.map(({ specifier, owner }) => [specifier, owner]),
  ),
);

const runtimeExportModuleByName = exportOwnershipMap(
  surfaceModules,
  "runtimeExportNames",
);
const valueExportModuleByName = exportOwnershipMap(
  surfaceModules,
  "valueExportNames",
);

const analysisProfile = {
  runtimeExportNames: [
    ...lifecycleRuntimeExportNames,
    ...analysisRuntimeExportNames,
    ...metadataRuntimeExportNames,
    ...analysisMetadataRuntimeExportNames,
  ],
  valueExportNames: packageStableValueExportNames,
  wasmExportNames: [
    ...lifecycleWasmExportNames,
    ...analysisWasmExportNames,
    ...metadataWasmExportNames,
    ...analysisMetadataWasmExportNames,
  ],
};

const renderProfile = {
  runtimeExportNames: [
    ...lifecycleRuntimeExportNames,
    ...metadataRuntimeExportNames,
    ...renderRuntimeExportNames,
  ],
  valueExportNames: [
    ...packageStableValueExportNames,
    ...packageRenderValueExportNames,
  ],
  wasmExportNames: [
    ...lifecycleWasmExportNames,
    ...metadataWasmExportNames,
    ...renderWasmExportNames,
  ],
};

const runtimeProfiles = Object.freeze({
  analysis: analysisProfile,
  render: renderProfile,
  ascii: {
    runtimeExportNames: [
      ...lifecycleRuntimeExportNames,
      ...metadataRuntimeExportNames,
      ...asciiRuntimeExportNames,
    ],
    valueExportNames: packageStableValueExportNames,
    wasmExportNames: [
      ...lifecycleWasmExportNames,
      ...metadataWasmExportNames,
      ...asciiWasmExportNames,
    ],
  },
  editor: {
    runtimeExportNames: [
      ...analysisProfile.runtimeExportNames,
      ...editorRuntimeExportNames,
    ],
    valueExportNames: packageStableValueExportNames,
    wasmExportNames: [
      ...analysisProfile.wasmExportNames,
      ...editorWasmExportNames,
    ],
  },
  full: {
    runtimeExportNames: [
      ...analysisProfile.runtimeExportNames,
      ...renderRuntimeExportNames,
      ...asciiRuntimeExportNames,
      ...editorRuntimeExportNames,
    ],
    valueExportNames: [
      ...packageStableValueExportNames,
      ...packageRenderValueExportNames,
    ],
    wasmExportNames: [
      ...analysisProfile.wasmExportNames,
      ...renderWasmExportNames,
      ...asciiWasmExportNames,
      ...editorWasmExportNames,
    ],
  },
});

export const webPackages = webPackageDescriptors.map((descriptor) => {
  const profile = runtimeProfiles[descriptor.runtime_profile];
  if (!profile) {
    throw new Error(`Missing wrapper profile ${descriptor.runtime_profile}.`);
  }
  return Object.freeze({
    ...descriptor,
    runtimeExportNames: profile.runtimeExportNames,
    runtimeExportModules: exportGroups(
      profile.runtimeExportNames,
      runtimeExportModuleByName,
      `${descriptor.id} runtime export`,
    ),
    valueExportNames: profile.valueExportNames,
    valueExportModules: exportGroups(
      profile.valueExportNames,
      valueExportModuleByName,
      `${descriptor.id} value export`,
    ),
    wasmExportNames: unique(profile.wasmExportNames),
  });
});

export const publicWebPackages = webPackages.filter(
  (item) => item.visibility === "public",
);

export const allPackageRuntimeExportNames = unique(
  webPackages.flatMap((item) => item.runtimeExportNames),
);

export const allPackageValueExportNames = unique(
  webPackages.flatMap((item) => item.valueExportNames),
);

export const allPackageWasmExportNames = unique(
  webPackages.flatMap((item) => item.wasmExportNames),
);

function unique(names) {
  return [...new Set(names)];
}

function defineSurfaceModules(modules) {
  const specifiers = new Set();
  const ids = new Set();
  return Object.freeze(
    modules.map(
      ({
        id,
        specifier,
        owner,
        runtimeExportNames = [],
        valueExportNames = [],
        internalValueExportNames = [],
        exactValueExports = false,
      }) => {
        if (specifiers.has(specifier)) {
          throw new Error(`Duplicate Web surface module ${specifier}.`);
        }
        specifiers.add(specifier);
        if (id !== undefined) {
          if (ids.has(id)) {
            throw new Error(`Duplicate Web surface module id ${id}.`);
          }
          ids.add(id);
        }
        return Object.freeze({
          ...(id === undefined ? {} : { id }),
          specifier,
          owner,
          runtimeExportNames: Object.freeze([...runtimeExportNames]),
          valueExportNames: Object.freeze([...valueExportNames]),
          internalValueExportNames: Object.freeze([...internalValueExportNames]),
          exactValueExports,
        });
      },
    ),
  );
}

function exportOwnershipMap(modules, exportField) {
  const result = new Map();
  for (const module of modules) {
    for (const name of module[exportField]) {
      if (result.has(name)) {
        throw new Error(`Duplicate Web surface owner for ${name}.`);
      }
      result.set(name, module.specifier);
    }
  }
  return result;
}

function exportGroups(names, ownership, label) {
  const groups = new Map();
  for (const name of names) {
    const specifier = ownership.get(name);
    if (!specifier) {
      throw new Error(`Missing module owner for ${label} ${name}.`);
    }
    const group = groups.get(specifier) ?? [];
    group.push(name);
    groups.set(specifier, group);
  }
  return Object.freeze(
    [...groups].map(([specifier, exportNames]) =>
      Object.freeze({
        specifier,
        exportNames: Object.freeze(exportNames),
      }),
    ),
  );
}
