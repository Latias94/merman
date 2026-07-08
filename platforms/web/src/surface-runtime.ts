import * as root from "./index.js";
import {
  createMermanRuntimeState,
  withMermanRuntimeState,
} from "./runtime-state.js";

export function bindSurfaceRuntime(surfaceLoader: root.MermanWasmLoader) {
  const state = createMermanRuntimeState(surfaceLoader);
  const withState = <T>(run: () => T): T => withMermanRuntimeState(state, run);

  return {
    initMerman(init?: root.MermanInitInput) {
      if (typeof init === "function") {
        return withState(() => root.initMerman(init));
      }
      const options: root.MermanInitOptions = init ?? {};
      return withState(() =>
        root.initMerman({
          loader: surfaceLoader,
          ...options,
        })
      );
    },
    getMerman: () => withState(root.getMerman),
    isMermanInitialized: () => withState(root.isMermanInitialized),
    renderSvg: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.renderSvg(source, options)),
    renderSvgWithTextMeasurer: (
      source: string,
      measurer: root.HostTextMeasurer,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.renderSvgWithTextMeasurer(source, measurer, options)),
    layoutJsonWithTextMeasurer: (
      source: string,
      measurer: root.HostTextMeasurer,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.layoutJsonWithTextMeasurer(source, measurer, options)),
    renderSvgElement: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.renderSvgElement(source, options)),
    renderSvgToElement: (
      target: Element,
      source: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.renderSvgToElement(target, source, options)),
    renderAscii: (source: string, options?: root.AsciiBindingOptions | string) =>
      withState(() => root.renderAscii(source, options)),
    parseJson: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.parseJson(source, options)),
    parseObject: <T = unknown>(
      source: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.parseObject<T>(source, options)),
    layoutJson: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.layoutJson(source, options)),
    layoutObject: <T = unknown>(
      source: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.layoutObject<T>(source, options)),
    analyze: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.analyze(source, options)),
    analyzeJson: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.analyzeJson(source, options)),
    analysisFacts: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.analysisFacts(source, options)),
    analyzeDocument: (
      source: string,
      options?: root.SvgBindingOptions | string,
      uri?: string
    ) => withState(() => root.analyzeDocument(source, options, uri)),
    analyzeDocumentFacts: (
      source: string,
      options?: root.SvgBindingOptions | string,
      uri?: string
    ) => withState(() => root.analyzeDocumentFacts(source, options, uri)),
    validate: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.validate(source, options)),
    editorDiagnostics: (
      source: string,
      options?: root.SvgBindingOptions | string,
      uri?: string
    ) => withState(() => root.editorDiagnostics(source, options, uri)),
    editorCodeActions: (
      source: string,
      options?: root.SvgBindingOptions | string,
      uri?: string
    ) => withState(() => root.editorCodeActions(source, options, uri)),
    editorCompletions: (
      source: string,
      position: root.EditorPosition,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorCompletions(source, position, uri, options)),
    editorHover: (
      source: string,
      position: root.EditorPosition,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorHover(source, position, uri, options)),
    editorDocumentSymbols: (
      source: string,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorDocumentSymbols(source, uri, options)),
    editorWorkspaceSymbols: (
      source: string,
      query: string,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorWorkspaceSymbols(source, query, uri, options)),
    editorDefinition: (
      source: string,
      position: root.EditorPosition,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorDefinition(source, position, uri, options)),
    editorReferences: (
      source: string,
      position: root.EditorPosition,
      includeDeclaration = true,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) =>
      withState(() =>
        root.editorReferences(source, position, includeDeclaration, uri, options)
      ),
    editorPrepareRename: (
      source: string,
      position: root.EditorPosition,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorPrepareRename(source, position, uri, options)),
    editorRename: (
      source: string,
      position: root.EditorPosition,
      newName: string,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorRename(source, position, newName, uri, options)),
    editorSemanticTokenLegend: () => withState(root.editorSemanticTokenLegend),
    editorSemanticTokens: (
      source: string,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorSemanticTokens(source, uri, options)),
    bindingCapabilities: () => withState(root.bindingCapabilities),
    selectedRegistryProfile: () => withState(root.selectedRegistryProfile),
    supportedDiagrams: () => withState(root.supportedDiagrams),
    diagramFamilyCapabilities: () => withState(root.diagramFamilyCapabilities),
    lintRuleCatalog: () => withState(root.lintRuleCatalog),
    asciiSupportedDiagrams: () => withState(root.asciiSupportedDiagrams),
    asciiCapabilities: () => withState(root.asciiCapabilities),
    supportedThemes: () => withState(root.supportedThemes),
    supportedHostThemePresets: () => withState(root.supportedHostThemePresets),
    abiVersion: () => withState(root.abiVersion),
    packageVersion: () => withState(root.packageVersion),
  };
}
