const lifecycleRuntimeExportNames = [
  "initMerman",
  "getMerman",
  "isMermanInitialized",
];

const analysisRuntimeExportNames = [
  "analyze",
  "analyzeJson",
  "analysisFacts",
  "analyzeDocument",
  "analyzeDocumentFacts",
  "validate",
];

const metadataRuntimeExportNames = [
  "bindingCapabilities",
  "selectedRegistryProfile",
  "supportedDiagrams",
  "diagramFamilyCapabilities",
  "lintRuleCatalog",
  "supportedThemes",
  "abiVersion",
  "packageVersion",
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
  "createBrowserTextMeasurer",
];

const coreRuntimeExportNames = [
  ...lifecycleRuntimeExportNames,
  ...analysisRuntimeExportNames,
  ...metadataRuntimeExportNames,
];

const renderSurfaceRuntimeExportNames = [
  ...coreRuntimeExportNames,
  ...renderRuntimeExportNames,
];

const asciiSurfaceRuntimeExportNames = [
  ...coreRuntimeExportNames,
  ...asciiRuntimeExportNames,
];

const fullRuntimeExportNames = [
  ...renderSurfaceRuntimeExportNames,
  ...asciiRuntimeExportNames,
  ...editorRuntimeExportNames,
];

export const surfaces = [
  {
    entry: "core",
    preset: "browser-core",
    pkgDirRel: "pkg/core",
    defaultBindingCapabilitiesExportName: "CORE_BINDING_CAPABILITIES",
    runtimeExportNames: coreRuntimeExportNames,
    valueExportNames: surfaceStableValueExportNames,
  },
  {
    entry: "render",
    preset: "browser-render",
    pkgDirRel: "pkg/render",
    defaultBindingCapabilitiesExportName: "RENDER_BINDING_CAPABILITIES",
    runtimeExportNames: renderSurfaceRuntimeExportNames,
    valueExportNames: [
      ...surfaceStableValueExportNames,
      ...surfaceRenderValueExportNames,
    ],
  },
  {
    entry: "ascii",
    preset: "browser-ascii",
    pkgDirRel: "pkg/ascii",
    defaultBindingCapabilitiesExportName: "ASCII_BINDING_CAPABILITIES",
    runtimeExportNames: asciiSurfaceRuntimeExportNames,
    valueExportNames: surfaceStableValueExportNames,
  },
  {
    entry: "full",
    preset: "browser-full",
    pkgDirRel: "pkg/full",
    defaultBindingCapabilitiesExportName: "FULL_BINDING_CAPABILITIES",
    runtimeExportNames: fullRuntimeExportNames,
    valueExportNames: [
      ...surfaceStableValueExportNames,
      ...surfaceRenderValueExportNames,
    ],
  },
];

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
