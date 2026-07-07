import {
  createMermanRuntimeState,
  currentMermanRuntimeState,
  type MermanRuntimeState,
} from "./runtime-state.js";
import { assertSafeSvgForDom } from "./svg-safety.js";

import {
  isAsciiDiagramType,
  isDiagramType,
  isHostThemePresetName,
  isThemeName,
} from "./public-catalog.js";
import type {
  AsciiCapability,
  AsciiCapabilityEvidence,
  AsciiDiagramType,
  AsciiSupportLevel,
  BindingCapabilities,
  DiagramFamilyCapability,
  DiagramType,
  HostThemePresetName,
  LintRuleCatalogEntry,
  LintRuleCatalogResponse,
  RegistryProfile,
  TextMeasurementCapabilities,
  ThemeName,
} from "./public-catalog.js";
import type {
  AnalysisFactsResult,
  AnalysisResult,
  AsciiBindingOptions,
  CommonBindingOptions,
  EditorCodeAction,
  EditorCompletionList,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorSemanticToken,
  EditorSemanticTokenLegend,
  EditorSymbolInformation,
  EditorWorkspaceEdit,
  HostTextMeasureRequest,
  HostTextMeasureResult,
  HostTextMeasurer,
  MermanInitInput,
  MermanWasmModule,
  SvgBindingOptions,
  ValidationResult,
} from "./public-types.js";

export {
  ASCII_BINDING_CAPABILITIES,
  BINDING_STATUS_CODE_NAMES,
  CORE_BINDING_CAPABILITIES,
  DEFAULT_BINDING_CAPABILITIES,
  FULL_BINDING_CAPABILITIES,
  RENDER_BINDING_CAPABILITIES,
  SUPPORTED_ASCII_DIAGRAMS,
  SUPPORTED_DIAGRAMS,
  SUPPORTED_HOST_THEME_PRESETS,
  SUPPORTED_THEMES,
  isAsciiDiagramType,
  isBindingErrorPayload,
  isBindingStatusCodeName,
  isDiagramType,
  isHostThemePresetName,
  isThemeName,
  normalizeHostThemePresetName,
  normalizeThemeName,
} from "./public-catalog.js";
export type * from "./public-catalog.js";
export type * from "./public-types.js";
export { assertSafeSvgForDom } from "./svg-safety.js";

const defaultRuntimeState = createMermanRuntimeState(defaultLoader);

export function initMerman(init?: MermanInitInput): Promise<MermanWasmModule> {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  if (state.wasmModule) {
    return Promise.resolve(state.wasmModule);
  }
  if (state.initPromise) {
    return state.initPromise;
  }
  state.initPromise = doInit(state, init).catch((error) => {
    state.initPromise = null;
    throw error;
  });
  return state.initPromise;
}

async function doInit(
  state: MermanRuntimeState,
  init?: MermanInitInput
): Promise<MermanWasmModule> {
  const loader = typeof init === "function" ? init : init?.loader;
  const wasm = typeof init === "function" ? undefined : init?.wasm;
  const module = loader ? await loader() : await state.defaultLoader();
  await module.default(wasm);
  state.wasmModule = module;
  return module;
}

async function defaultLoader(): Promise<MermanWasmModule> {
  return (await import("../pkg/merman_wasm.js")) as unknown as MermanWasmModule;
}

export function getMerman(): MermanWasmModule {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  if (!state.wasmModule) {
    throw new Error("Merman WASM is not initialized. Call initMerman() first.");
  }
  return state.wasmModule;
}

export function isMermanInitialized(): boolean {
  return currentMermanRuntimeState(defaultRuntimeState).wasmModule !== null;
}

export function renderSvg(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().renderSvg(source, encodeOptions(options));
}

export function renderSvgWithTextMeasurer(
  source: string,
  measurer: HostTextMeasurer,
  options?: SvgBindingOptions | string
): string {
  const renderWithMeasurer = getMerman().renderSvgWithTextMeasurer;
  if (!renderWithMeasurer) {
    throw new Error(
      "Merman WASM does not expose renderSvgWithTextMeasurer(). Rebuild @mermanjs/web."
    );
  }
  return renderWithMeasurer(source, encodeOptions(options), measurer);
}

export function layoutJsonWithTextMeasurer(
  source: string,
  measurer: HostTextMeasurer,
  options?: SvgBindingOptions | string
): string {
  const layoutWithMeasurer = getMerman().layoutJsonWithTextMeasurer;
  if (!layoutWithMeasurer) {
    throw new Error(
      "Merman WASM does not expose layoutJsonWithTextMeasurer(). Rebuild @mermanjs/web."
    );
  }
  return layoutWithMeasurer(source, encodeOptions(options), measurer);
}

export function createBrowserTextMeasurer(): HostTextMeasurer {
  let probe: HTMLDivElement | null = null;

  return (request) => {
    probe ??= createTextMeasureProbe();
    if (!probe) {
      return undefined;
    }

    if (!request.text) {
      return {
        width: 0,
        height: request.line_height || request.font_size,
        line_count: 1,
      };
    }

    applyTextMeasureStyle(probe, request);
    const maxWidth = normalizeMeasureMaxWidth(request);
    if (request.wrap_mode === "html-like" && maxWidth !== null) {
      const natural = measureProbeText(probe, request.text, {
        display: "inline-block",
        width: "auto",
        maxWidth: "none",
        whiteSpace: "nowrap",
      });
      if (natural.width <= maxWidth) {
        return natural;
      }

      return measureProbeText(probe, request.text, {
        display: "table",
        width: `${maxWidth}px`,
        maxWidth: `${maxWidth}px`,
        whiteSpace: "break-spaces",
      });
    }

    return measureProbeText(probe, request.text, {
      display: "inline-block",
      width: "auto",
      maxWidth: maxWidth === null ? "none" : `${maxWidth}px`,
      whiteSpace: request.white_space,
    });
  };
}

function applyTextMeasureStyle(
  probe: HTMLDivElement,
  request: HostTextMeasureRequest
) {
    const style = probe.style;
    style.fontFamily = request.font_family || "sans-serif";
    style.fontSize = `${Math.max(1, request.font_size)}px`;
    style.fontWeight = request.font_weight || "normal";
    style.fontStyle = request.font_style || "normal";
    style.lineHeight = `${Math.max(1, request.line_height || request.font_size)}px`;
    style.letterSpacing = `${request.letter_spacing || 0}px`;
    style.wordSpacing = `${request.word_spacing || 0}px`;
    style.direction = request.direction === "rtl" ? "rtl" : "ltr";
}

function measureProbeText(
  probe: HTMLDivElement,
  text: string,
  styleOverride: Pick<
    CSSStyleDeclaration,
    "display" | "width" | "maxWidth" | "whiteSpace"
  >
): HostTextMeasureResult {
    probe.style.display = styleOverride.display;
    probe.style.width = styleOverride.width;
    probe.style.maxWidth = styleOverride.maxWidth;
    probe.style.whiteSpace = styleOverride.whiteSpace;
    probe.textContent = text;
    const rect = probe.getBoundingClientRect();
    const lineHeight = Math.max(1, parseFloat(probe.style.lineHeight) || 1);
    const height = Math.max(lineHeight, rect.height);
    return {
      width: Math.max(0, rect.width),
      height,
      line_count: Math.max(1, Math.round(height / lineHeight)),
    };
}

function normalizeMeasureMaxWidth(
  request: HostTextMeasureRequest
): number | null {
  if (
    !request.has_max_width ||
    typeof request.max_width !== "number" ||
    !Number.isFinite(request.max_width) ||
    request.max_width <= 0
  ) {
    return null;
  }
  return request.max_width;
}

function createTextMeasureProbe(): HTMLDivElement | null {
  if (typeof document === "undefined" || !document.body) {
    return null;
  }

  const probe = document.createElement("div");
  probe.setAttribute("aria-hidden", "true");
  Object.assign(probe.style, {
    position: "fixed",
    left: "-10000px",
    top: "-10000px",
    visibility: "hidden",
    contain: "layout style paint",
    boxSizing: "border-box",
    padding: "0",
    margin: "0",
    border: "0",
    display: "block",
  });
  document.body.appendChild(probe);
  return probe;
}

export function renderSvgElement(
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  if (typeof DOMParser === "undefined" || typeof document === "undefined") {
    throw new Error("renderSvgElement() requires a browser DOM.");
  }

  const svgText = renderSvg(source, options);
  assertSafeSvgForDom(svgText);
  const parsed = new DOMParser().parseFromString(svgText, "image/svg+xml");
  const parseError = parsed.querySelector("parsererror");
  if (parseError) {
    throw new Error(parseError.textContent || "Merman rendered invalid SVG.");
  }

  const svg = parsed.documentElement;
  if (svg.localName !== "svg") {
    throw new Error("Merman render output did not contain an SVG root element.");
  }
  return document.importNode(svg, true) as unknown as SVGSVGElement;
}

export function renderSvgToElement(
  target: Element,
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  const svg = renderSvgElement(source, options);
  target.replaceChildren(svg);
  return svg;
}

export function renderAscii(source: string, options?: AsciiBindingOptions | string): string {
  return getMerman().renderAscii(source, encodeOptions(options));
}

export function parseJson(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().parseJson(source, encodeOptions(options));
}

export function parseObject<T = unknown>(source: string, options?: SvgBindingOptions | string): T {
  return JSON.parse(parseJson(source, options)) as T;
}

export function layoutJson(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().layoutJson(source, encodeOptions(options));
}

export function layoutObject<T = unknown>(source: string, options?: SvgBindingOptions | string): T {
  return JSON.parse(layoutJson(source, options)) as T;
}

export function analyze(source: string, options?: SvgBindingOptions | string): AnalysisResult {
  const merman = getMerman();
  const encodedOptions = encodeOptions(options);
  const analysis =
    merman.analyze?.(source, encodedOptions) ?? merman.analyzeJson?.(source, encodedOptions);
  if (!analysis) {
    throw new Error("Merman analyze() is not available in this artifact.");
  }
  return analysis;
}

export function analyzeJson(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisResult {
  return analyze(source, options);
}

export function analysisFacts(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisFactsResult {
  const merman = getMerman();
  const facts = merman.analysisFacts;
  if (!facts) {
    throw new Error("Merman analysisFacts() is not available in this artifact.");
  }
  return facts(source, encodeOptions(options));
}

export function analyzeDocument(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): AnalysisResult {
  const analyzeDocument = getMerman().analyzeDocument;
  if (!analyzeDocument) {
    throw new Error("Merman analyzeDocument() is not available in this artifact.");
  }
  return analyzeDocument(source, encodeOptions(options), uri);
}

export function analyzeDocumentFacts(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): AnalysisFactsResult {
  const analyzeDocumentFacts = getMerman().analyzeDocumentFacts;
  if (!analyzeDocumentFacts) {
    throw new Error("Merman analyzeDocumentFacts() is not available in this artifact.");
  }
  return analyzeDocumentFacts(source, encodeOptions(options), uri);
}

export function validate(source: string, options?: SvgBindingOptions | string): ValidationResult {
  return getMerman().validate(source, encodeOptions(options));
}

export function editorDiagnostics(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorDiagnosticsResult {
  const diagnostics = requireEditorLanguage("editorDiagnostics", getMerman().editorDiagnostics);
  return diagnostics(source, encodeOptions(options), uri);
}

export function editorCodeActions(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorCodeAction[] {
  const codeActions = requireEditorLanguage("editorCodeActions", getMerman().editorCodeActions);
  return codeActions(source, encodeOptions(options), uri);
}

export function editorCompletions(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorCompletionList {
  const completions = requireEditorLanguage("editorCompletions", getMerman().editorCompletions);
  return completions(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorHover(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorHover | null {
  const hover = requireEditorLanguage("editorHover", getMerman().editorHover);
  return hover(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorDocumentSymbols(
  source: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorDocumentSymbol[] {
  const documentSymbols = requireEditorLanguage(
    "editorDocumentSymbols",
    getMerman().editorDocumentSymbols
  );
  return documentSymbols(source, uri, encodeOptions(options));
}

export function editorWorkspaceSymbols(
  source: string,
  query: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorSymbolInformation[] {
  const workspaceSymbols = requireEditorLanguage(
    "editorWorkspaceSymbols",
    getMerman().editorWorkspaceSymbols
  );
  return workspaceSymbols(source, query, uri, encodeOptions(options));
}

export function editorDefinition(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorLocation | null {
  const definition = requireEditorLanguage("editorDefinition", getMerman().editorDefinition);
  return definition(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorReferences(
  source: string,
  position: EditorPosition,
  includeDeclaration = true,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorLocation[] {
  const refs = requireEditorLanguage("editorReferences", getMerman().editorReferences);
  return refs(source, position.line, position.character, includeDeclaration, uri, encodeOptions(options));
}

export function editorPrepareRename(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorPrepareRename | null {
  const prepare = requireEditorLanguage("editorPrepareRename", getMerman().editorPrepareRename);
  return prepare(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorRename(
  source: string,
  position: EditorPosition,
  newName: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorWorkspaceEdit | null {
  const rename = requireEditorLanguage("editorRename", getMerman().editorRename);
  return rename(source, position.line, position.character, newName, uri, encodeOptions(options));
}

export function editorSemanticTokenLegend(): EditorSemanticTokenLegend {
  const legend = requireEditorLanguage(
    "editorSemanticTokenLegend",
    getMerman().editorSemanticTokenLegend
  );
  return legend();
}

export function editorSemanticTokens(
  source: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorSemanticToken[] {
  const tokens = requireEditorLanguage("editorSemanticTokens", getMerman().editorSemanticTokens);
  return tokens(source, uri, encodeOptions(options));
}

export function bindingCapabilities(): BindingCapabilities {
  const merman = getMerman();
  return normalizeBindingCapabilities(merman.bindingCapabilities());
}

export function selectedRegistryProfile(): RegistryProfile {
  const profile = getMerman().selectedRegistryProfile();
  if (profile === "full" || profile === "tiny") {
    return profile;
  }
  throw new Error(`Merman WASM returned an invalid registry profile: ${String(profile)}`);
}

export function supportedDiagrams(): DiagramType[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.supportedDiagramsCache ??= getMerman().supportedDiagrams().map(assertDiagramType);
  return [...state.supportedDiagramsCache];
}

export function diagramFamilyCapabilities(): DiagramFamilyCapability[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.diagramFamilyCapabilitiesCache ??= (
    getMerman().diagramFamilyCapabilities?.() ?? []
  ).map(normalizeDiagramFamilyCapability);
  return state.diagramFamilyCapabilitiesCache.map((capability) => ({ ...capability }));
}

export function lintRuleCatalog(): LintRuleCatalogEntry[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  const response = getMerman().lintRuleCatalog?.();
  if (!response) {
    throw new Error("Merman lintRuleCatalog() is not available in this artifact.");
  }
  state.lintRuleCatalogCache ??= normalizeLintRuleCatalogResponse(response);
  return state.lintRuleCatalogCache.map((rule) => ({
    ...rule,
    evidence: [...rule.evidence],
  }));
}

export function asciiSupportedDiagrams(): AsciiDiagramType[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.asciiSupportedDiagramsCache ??= getMerman()
    .asciiSupportedDiagrams()
    .map(assertAsciiDiagramType);
  return [...state.asciiSupportedDiagramsCache];
}

export function asciiCapabilities(): AsciiCapability[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.asciiCapabilitiesCache ??= getMerman().asciiCapabilities().map(normalizeAsciiCapability);
  return state.asciiCapabilitiesCache.map((capability) => ({
    ...capability,
    supported_semantics: [...capability.supported_semantics],
    limits: [...capability.limits],
    evidence: capability.evidence.map((evidence) => ({ ...evidence })),
  }));
}

export function supportedThemes(): ThemeName[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.supportedThemesCache ??= getMerman().supportedThemes().map(assertThemeName);
  return [...state.supportedThemesCache];
}

export function supportedHostThemePresets(): HostThemePresetName[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.supportedHostThemePresetsCache ??= getMerman()
    .supportedHostThemePresets()
    .map(assertHostThemePresetName);
  return [...state.supportedHostThemePresetsCache];
}

export function abiVersion(): number {
  return getMerman().abiVersion();
}

export function packageVersion(): string {
  return getMerman().packageVersion();
}

export function encodeOptions(
  options?: CommonBindingOptions | string
): string | undefined {
  if (options === undefined) {
    return undefined;
  }
  return typeof options === "string" ? options : JSON.stringify(options);
}

function assertDiagramType(diagram: string): DiagramType {
  if (isDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown diagram type: ${diagram}`);
}

function assertAsciiDiagramType(diagram: string): AsciiDiagramType {
  if (isAsciiDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown ASCII diagram type: ${diagram}`);
}

function normalizeDiagramFamilyCapability(
  capability: DiagramFamilyCapability
): DiagramFamilyCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid diagram family capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid diagram family capability.");
  }
  const metadataId =
    capability.metadata_id === undefined || capability.metadata_id === null
      ? null
      : assertDiagramType(String(capability.metadata_id));
  return {
    diagram_type: capability.diagram_type,
    metadata_id: metadataId,
    has_semantic_parser: Boolean(capability.has_semantic_parser),
    has_render_parser: Boolean(capability.has_render_parser),
  };
}

function normalizeLintRuleCatalogEntry(
  rule: LintRuleCatalogEntry
): LintRuleCatalogEntry {
  if (!rule || typeof rule !== "object") {
    throw new Error("Merman WASM returned an invalid lint rule catalog entry.");
  }
  return {
    id: assertStringField(rule.id, "lint rule id"),
    description: assertStringField(rule.description, "lint rule description"),
    evidence: assertStringArray(rule.evidence, "lint rule evidence"),
    default_severity: assertCatalogValue(rule.default_severity, [
      "error",
      "warning",
      "info",
      "hint",
    ]),
    category: assertCatalogValue(rule.category, [
      "parse",
      "semantic",
      "config",
      "resource",
      "compatibility",
      "layout",
      "render",
      "internal",
    ]),
    default_enabled: Boolean(rule.default_enabled),
    default_profile: assertCatalogValue(rule.default_profile, [
      "core",
      "recommended",
      "strict",
    ]),
    origin: assertCatalogValue(rule.origin, [
      "mermaid_syntax",
      "mermaid_compatibility",
      "merman_authoring",
      "merman_resource_policy",
      "merman_internal",
    ]),
    configurable: Boolean(rule.configurable),
    fixable: Boolean(rule.fixable),
  };
}

function normalizeLintRuleCatalogResponse(
  response: LintRuleCatalogResponse
): LintRuleCatalogEntry[] {
  if (!response || typeof response !== "object") {
    throw new Error("Merman WASM returned an invalid lint rule catalog response.");
  }
  if (response.version !== 1) {
    throw new Error(
      `Merman WASM returned unsupported lint rule catalog version: ${String(response.version)}.`
    );
  }
  if (!Array.isArray(response.rules)) {
    throw new Error("Merman WASM returned a lint rule catalog response without rules.");
  }
  return response.rules.map(normalizeLintRuleCatalogEntry);
}

function assertStringField(value: unknown, label: string): string {
  if (typeof value === "string") {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertStringArray(value: unknown, label: string): string[] {
  if (Array.isArray(value) && value.every((item) => typeof item === "string")) {
    return [...value];
  }
  throw new Error(`Merman WASM returned invalid ${label}.`);
}

function assertCatalogValue<const T extends string>(
  value: unknown,
  allowed: readonly T[]
): T {
  if (typeof value === "string" && (allowed as readonly string[]).includes(value)) {
    return value as T;
  }
  throw new Error(`Merman WASM returned an invalid lint rule catalog value: ${String(value)}`);
}

function normalizeAsciiCapability(capability: AsciiCapability): AsciiCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }

  const supportLevel = normalizeAsciiSupportLevel(capability.support_level);
  const evidence = Array.isArray(capability.evidence)
    ? capability.evidence.map(normalizeAsciiCapabilityEvidence)
    : [];

  return {
    diagram_type: capability.diagram_type,
    display_name:
      typeof capability.display_name === "string"
        ? capability.display_name
        : capability.diagram_type,
    support_level: supportLevel,
    summary_fallback: Boolean(capability.summary_fallback),
    supported_semantics: Array.isArray(capability.supported_semantics)
      ? capability.supported_semantics.map(String)
      : [],
    limits: Array.isArray(capability.limits) ? capability.limits.map(String) : [],
    evidence,
  };
}

function normalizeAsciiCapabilityEvidence(
  evidence: AsciiCapabilityEvidence
): AsciiCapabilityEvidence {
  return {
    kind: typeof evidence.kind === "string" ? evidence.kind : "support_matrix",
    source: typeof evidence.source === "string" ? evidence.source : "",
    note: typeof evidence.note === "string" ? evidence.note : "",
  };
}

function normalizeAsciiSupportLevel(level: unknown): AsciiSupportLevel {
  return level === "full" ||
    level === "partial" ||
    level === "summary" ||
    level === "unsupported"
    ? level
    : "unsupported";
}

function assertThemeName(theme: string): ThemeName {
  if (isThemeName(theme)) {
    return theme;
  }
  throw new Error(`Merman WASM returned unknown theme: ${theme}`);
}

function assertHostThemePresetName(preset: string): HostThemePresetName {
  if (isHostThemePresetName(preset)) {
    return preset;
  }
  throw new Error(`Merman WASM returned unknown host theme preset: ${preset}`);
}

function normalizeBindingCapabilities(capabilities: BindingCapabilities): BindingCapabilities {
  return {
    render: Boolean(capabilities.render),
    ascii: Boolean(capabilities.ascii),
    core_full: Boolean(capabilities.core_full),
    core_host: Boolean(capabilities.core_host),
    elk_layout: Boolean(capabilities.elk_layout),
    ratex_math: Boolean(capabilities.ratex_math),
    editor_language: Boolean(capabilities.editor_language),
    text_measurement: normalizeTextMeasurementCapabilities(
      capabilities.text_measurement,
      Boolean(capabilities.render)
    ),
  };
}

function normalizeTextMeasurementCapabilities(
  capabilities: Partial<TextMeasurementCapabilities> | undefined,
  renderEnabled: boolean
): TextMeasurementCapabilities {
  return {
    vendored:
      capabilities?.vendored === undefined
        ? renderEnabled
        : Boolean(capabilities.vendored),
    deterministic:
      capabilities?.deterministic === undefined
        ? renderEnabled
        : Boolean(capabilities.deterministic),
    host_callback: Boolean(capabilities?.host_callback),
    font_assets: Boolean(capabilities?.font_assets),
  };
}

function requireEditorLanguage<T>(
  apiName: string,
  binding: T | undefined
): T {
  if (!bindingCapabilities().editor_language || binding === undefined) {
    throw new Error(`Merman ${apiName}() is not available in this artifact.`);
  }
  return binding;
}

function hasEditorLanguageBindings(merman: MermanWasmModule): boolean {
  return (
    typeof merman.editorDiagnostics === "function" &&
    typeof merman.editorCodeActions === "function" &&
    typeof merman.editorCompletions === "function" &&
    typeof merman.editorHover === "function" &&
    typeof merman.editorDocumentSymbols === "function" &&
    typeof merman.editorWorkspaceSymbols === "function" &&
    typeof merman.editorDefinition === "function" &&
    typeof merman.editorReferences === "function" &&
    typeof merman.editorPrepareRename === "function" &&
    typeof merman.editorRename === "function" &&
    typeof merman.editorSemanticTokenLegend === "function" &&
    typeof merman.editorSemanticTokens === "function"
  );
}
