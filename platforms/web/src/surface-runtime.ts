import * as root from "./index.js";
import {
  createMermanRuntimeState,
  withMermanRuntimeState,
} from "./runtime-state.js";

type WorkerGlobalScopeConstructor = new (...args: never[]) => object;
type ServerProcess = {
  release?: { name?: unknown };
  versions?: { node?: unknown };
};

/// Reject server runtimes at the public browser-package boundary.
///
/// A main-window package may use `window` and `document`; an editor package may instead run in a
/// real browser Worker. Node and SSR runtimes match neither shape. This check intentionally runs
/// before a caller-supplied loader so a custom WASM source cannot turn a browser package into an
/// undocumented server transport.
export function assertBrowserRuntime(): void {
  const processLike = (
    globalThis as typeof globalThis & { process?: ServerProcess }
  ).process;
  const isNodeRuntime =
    processLike?.release?.name === "node" &&
    typeof processLike.versions?.node === "string";
  const isDenoRuntime = "Deno" in globalThis;
  const isBunRuntime = "Bun" in globalThis;
  if (isNodeRuntime || isDenoRuntime || isBunRuntime) {
    throw new Error(
      "Merman browser packages require a browser main-thread or Web Worker realm. Use a native or Node transport for SSR and server runtimes.",
    );
  }
  const isBrowserWindow = typeof window !== "undefined" && typeof document !== "undefined";
  const workerGlobalScope = (
    globalThis as typeof globalThis & { WorkerGlobalScope?: WorkerGlobalScopeConstructor }
  ).WorkerGlobalScope;
  const isBrowserWorker =
    typeof workerGlobalScope === "function" && globalThis instanceof workerGlobalScope;
  if (!isBrowserWindow && !isBrowserWorker) {
    throw new Error(
      "Merman browser packages require a browser main-thread or Web Worker realm. Use a native or Node transport for SSR and Node.js.",
    );
  }
}

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
    detectDiagramFacts: (source: string, options?: root.SvgBindingOptions | string) =>
      withState(() => root.detectDiagramFacts(source, options)),
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
    createEditorSession: (
      source: string,
      version: number,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.createEditorSession(source, version, uri, options)),
    editorDiagnostics: (
      source: string,
      options?: root.SvgBindingOptions | string,
      uri?: string
    ) => withState(() => root.editorDiagnostics(source, options, uri)),
    editorDiagramDetection: (
      source: string,
      options?: root.SvgBindingOptions | string,
      uri?: string
    ) => withState(() => root.editorDiagramDetection(source, options, uri)),
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
    editorSemanticTokenDescriptor: () => withState(root.editorSemanticTokenDescriptor),
    editorSemanticTokens: (
      source: string,
      uri?: string,
      options?: root.SvgBindingOptions | string
    ) => withState(() => root.editorSemanticTokens(source, uri, options)),
    runtimeCatalog: () => withState(root.runtimeCatalog),
    supportedDiagrams: () => withState(root.supportedDiagrams),
    diagramFamilyCapabilities: () => withState(root.diagramFamilyCapabilities),
    lintRuleCatalog: () => withState(root.lintRuleCatalog),
    asciiSupportedDiagrams: () => withState(root.asciiSupportedDiagrams),
    asciiCapabilities: () => withState(root.asciiCapabilities),
    supportedThemes: () => withState(root.supportedThemes),
    supportedHostThemePresets: () => withState(root.supportedHostThemePresets),
    transportApiVersion: () => withState(root.transportApiVersion),
    packageVersion: () => withState(root.packageVersion),
  };
}
