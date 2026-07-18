import {
  publicWebSurfaceDescriptors,
} from "./web-surface-descriptor.mjs";

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
  "bindingCapabilities",
  "selectedRegistryProfile",
  "supportedDiagrams",
  "diagramFamilyCapabilities",
  "supportedThemes",
  "abiVersion",
  "packageVersion",
];

const analysisMetadataRuntimeExportNames = [
  "lintRuleCatalog",
];

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
  "editorDiagnostics",
  "editorCodeActions",
  "editorCompletions",
  "editorHover",
  "editorDocumentSymbols",
  "editorWorkspaceSymbols",
  "editorDefinition",
  "editorReferences",
  "editorPrepareRename",
  "editorRename",
  "editorSemanticTokenLegend",
  "editorSemanticTokens",
];

export const surfaceStableValueExportNames = [
  "SUPPORTED_THEMES",
  "SUPPORTED_HOST_THEME_PRESETS",
  "SUPPORTED_DIAGRAMS",
  "SUPPORTED_ASCII_DIAGRAMS",
  "BINDING_STATUS_CODE_NAMES",
  "DEFAULT_BINDING_CAPABILITIES",
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

export const surfaceRenderValueExportNames = [
  "assertSafeSvgForDom",
  "createBrowserTextMeasurementSession",
];

const coreRuntimeExportNames = [
  ...lifecycleRuntimeExportNames,
  ...analysisRuntimeExportNames,
  ...metadataRuntimeExportNames,
  ...analysisMetadataRuntimeExportNames,
];

const renderSurfaceRuntimeExportNames = [
  ...coreRuntimeExportNames,
  ...renderRuntimeExportNames,
];

const renderOnlySurfaceRuntimeExportNames = [
  ...lifecycleRuntimeExportNames,
  ...metadataRuntimeExportNames,
  ...renderRuntimeExportNames,
];

const asciiSurfaceRuntimeExportNames = [
  ...lifecycleRuntimeExportNames,
  ...metadataRuntimeExportNames,
  ...asciiRuntimeExportNames,
];

const editorSurfaceRuntimeExportNames = [
  ...lifecycleRuntimeExportNames,
  ...analysisRuntimeExportNames,
  ...metadataRuntimeExportNames,
  ...analysisMetadataRuntimeExportNames,
  ...editorRuntimeExportNames,
];

const fullRuntimeExportNames = [
  ...renderSurfaceRuntimeExportNames,
  ...asciiRuntimeExportNames,
  ...editorRuntimeExportNames,
];

const runtimeProfiles = Object.freeze({
  core: {
    runtimeExportNames: coreRuntimeExportNames,
    valueExportNames: surfaceStableValueExportNames,
  },
  render: {
    runtimeExportNames: renderSurfaceRuntimeExportNames,
    valueExportNames: [
      ...surfaceStableValueExportNames,
      ...surfaceRenderValueExportNames,
    ],
  },
  "render-only": {
    runtimeExportNames: renderOnlySurfaceRuntimeExportNames,
    valueExportNames: [
      ...surfaceStableValueExportNames,
      ...surfaceRenderValueExportNames,
    ],
  },
  ascii: {
    runtimeExportNames: asciiSurfaceRuntimeExportNames,
    valueExportNames: surfaceStableValueExportNames,
  },
  editor: {
    runtimeExportNames: editorSurfaceRuntimeExportNames,
    valueExportNames: surfaceStableValueExportNames,
  },
  full: {
    runtimeExportNames: fullRuntimeExportNames,
    valueExportNames: [
      ...surfaceStableValueExportNames,
      ...surfaceRenderValueExportNames,
    ],
  },
});

export const surfaces = publicWebSurfaceDescriptors.map((descriptor) => {
  const profile = runtimeProfiles[descriptor.runtime_profile];
  return Object.freeze({
    entry: descriptor.entry,
    preset: descriptor.preset,
    pkgDirRel: descriptor.pkg_dir_rel,
    runtimeProfile: descriptor.runtime_profile,
    defaultBindingCapabilitiesExportName:
      descriptor.entry.replaceAll("-", "_").toUpperCase() +
      "_BINDING_CAPABILITIES",
    runtimeExportNames: profile.runtimeExportNames,
    valueExportNames: profile.valueExportNames,
  });
});

export const allSurfaceRuntimeExportNames = unique(
  surfaces.flatMap((surface) => surface.runtimeExportNames),
);

export const allSurfaceValueExportNames = unique(
  surfaces.flatMap((surface) => surface.valueExportNames),
);

export const surfaceRuntimeExportNames = allSurfaceRuntimeExportNames;

function unique(names) {
  return [...new Set(names)];
}
