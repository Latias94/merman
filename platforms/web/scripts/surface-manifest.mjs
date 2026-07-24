import { webPackageDescriptors } from "./wasm-build/web-surface-descriptor.mjs";

const lifecycleRuntimeExportNames = [
  "initMerman",
  "getMerman",
  "isMermanInitialized",
];

const analysisRuntimeExportNames = [
  "analyze",
  "analyzeJson",
  "analysisFacts",
  "detectDiagramFacts",
  "analyzeDocument",
  "analyzeDocumentFacts",
  "validate",
];

const metadataRuntimeExportNames = [
  "runtimeCatalog",
  "supportedDiagrams",
  "diagramFamilyCapabilities",
  "supportedThemes",
  "transportApiVersion",
  "packageVersion",
];

const analysisMetadataRuntimeExportNames = ["lintRuleCatalog"];

const renderRuntimeExportNames = [
  "renderSvg",
  "renderSvgWithTextMeasurer",
  "layoutJsonWithTextMeasurer",
  "renderSvgElement",
  "renderSvgToElement",
  "parseJson",
  "parseObject",
  "layoutJson",
  "layoutObject",
  "supportedHostThemePresets",
];

const asciiRuntimeExportNames = [
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
  "editorWorkspaceSymbols",
  "editorDefinition",
  "editorReferences",
  "editorPrepareRename",
  "editorRename",
  "editorSemanticTokenDescriptor",
  "editorSemanticTokens",
];

const editorDescriptorValueExportNames = [
  "SEMANTIC_TOKEN_DESCRIPTOR",
  "SEMANTIC_TOKEN_DESCRIPTOR_DIGEST",
  "SEMANTIC_TOKEN_MODIFIER_LSP_NAMES",
  "SEMANTIC_TOKEN_RECORD_WIDTH",
  "SEMANTIC_TOKEN_TYPE_LSP_NAMES",
  "SEMANTIC_TOKEN_VALID_MODIFIER_MASK",
  "SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX",
];

export const packageStableValueExportNames = [
  "MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION",
  "UNAVAILABLE_DIAGRAM_DETECTION",
  "SUPPORTED_THEMES",
  "SUPPORTED_HOST_THEME_PRESETS",
  "SUPPORTED_DIAGRAMS",
  "SUPPORTED_ASCII_DIAGRAMS",
  "BINDING_STATUS_CODE_NAMES",
  "isThemeName",
  "isHostThemePresetName",
  "isDiagramType",
  "isAsciiDiagramType",
  "isBindingStatusCodeName",
  "isBindingErrorPayload",
  "normalizeThemeName",
  "normalizeHostThemePresetName",
  "encodeOptions",
];

export const packageRenderValueExportNames = [
  "assertSafeSvgForDom",
  "createBrowserTextMeasurementSession",
];

const analysisProfile = {
  runtimeExportNames: [
    ...lifecycleRuntimeExportNames,
    ...analysisRuntimeExportNames,
    ...metadataRuntimeExportNames,
    ...analysisMetadataRuntimeExportNames,
  ],
  valueExportNames: packageStableValueExportNames,
};

const runtimeProfiles = Object.freeze({
  analysis: analysisProfile,
  render: {
    runtimeExportNames: [
      ...analysisProfile.runtimeExportNames,
      ...renderRuntimeExportNames,
    ],
    valueExportNames: [
      ...packageStableValueExportNames,
      ...packageRenderValueExportNames,
    ],
  },
  ascii: {
    runtimeExportNames: [
      ...lifecycleRuntimeExportNames,
      ...metadataRuntimeExportNames,
      ...asciiRuntimeExportNames,
    ],
    valueExportNames: packageStableValueExportNames,
  },
  editor: {
    runtimeExportNames: [
      ...analysisProfile.runtimeExportNames,
      ...editorRuntimeExportNames,
    ],
    valueExportNames: [
      ...packageStableValueExportNames,
      ...editorDescriptorValueExportNames,
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
      ...editorDescriptorValueExportNames,
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
    valueExportNames: profile.valueExportNames,
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

function unique(names) {
  return [...new Set(names)];
}
