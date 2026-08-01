import {
  encodeOptions,
  getMerman,
  currentRuntimeState,
  runtimeCatalog,
  UNAVAILABLE_DIAGRAM_DETECTION,
} from "./runtime-core.js";
import {
  validatePackedSemanticTokens,
  validateSemanticTokenDescriptor,
} from "./editor-semantic-tokens.js";
import { isDiagramType } from "./public-catalog.js";
import {
  type MermanRuntimeState,
  withMermanRuntimeState,
} from "./runtime-state.js";
import type {
  BrowserEditorSession,
  DiagramDetectionFacts,
  EditorCodeAction,
  EditorCompletionList,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorSemanticTokenDescriptor,
  EditorSymbolInformation,
  EditorWorkspaceEdit,
  SvgBindingOptions,
  WasmEditorSessionBinding,
} from "./public-types.js";

const editorSemanticTokenDescriptorCaches = new WeakMap<
  MermanRuntimeState,
  EditorSemanticTokenDescriptor
>();

export function createEditorSession(
  source: string,
  version: number,
  uri?: string,
  options?: SvgBindingOptions | string
): BrowserEditorSession {
  const runtimeState = currentRuntimeState();
  const EditorSession = requireEditorLanguage(
    "createEditorSession",
    getMerman().EditorSession
  );
  const native = new EditorSession(source, version, uri, encodeOptions(options));
  return new BrowserEditorSessionImpl(native, runtimeState);
}

class BrowserEditorSessionImpl implements BrowserEditorSession {
  private native: WasmEditorSessionBinding | null;

  constructor(
    native: WasmEditorSessionBinding,
    private readonly runtimeState: MermanRuntimeState
  ) {
    this.native = native;
  }

  get version(): number {
    return this.withNative((native) => native.version);
  }

  get uri(): string {
    return this.withNative((native) => native.uri);
  }

  update(source: string, version: number): void {
    this.withNative((native) => native.update(source, version));
  }

  diagnostics(): EditorDiagnosticsResult {
    return this.withNative((native) => native.diagnostics());
  }

  diagramDetection(): DiagramDetectionFacts {
    return this.withNative((native) =>
      validateEditorDiagramDetection(native.diagramDetection())
    );
  }

  codeActions(): EditorCodeAction[] {
    return this.withNative((native) => native.codeActions());
  }

  completions(position: EditorPosition): EditorCompletionList {
    return this.withNative((native) =>
      native.completions(position.line, position.character)
    );
  }

  hover(position: EditorPosition): EditorHover | null {
    return this.withNative((native) => native.hover(position.line, position.character));
  }

  documentSymbols(): EditorDocumentSymbol[] {
    return this.withNative((native) => native.documentSymbols());
  }

  searchDocumentSymbols(query: string): EditorSymbolInformation[] {
    return this.withNative((native) => native.searchDocumentSymbols(query));
  }

  definition(position: EditorPosition): EditorLocation | null {
    return this.withNative((native) =>
      native.definition(position.line, position.character)
    );
  }

  references(
    position: EditorPosition,
    includeDeclaration = true
  ): EditorLocation[] {
    return this.withNative((native) =>
      native.references(position.line, position.character, includeDeclaration)
    );
  }

  prepareRename(position: EditorPosition): EditorPrepareRename | null {
    return this.withNative((native) =>
      native.prepareRename(position.line, position.character)
    );
  }

  rename(
    position: EditorPosition,
    newName: string
  ): EditorWorkspaceEdit | null {
    return this.withNative((native) =>
      native.rename(position.line, position.character, newName)
    );
  }

  semanticTokens(): Uint32Array {
    return this.withNative((native) => {
      cachedEditorSemanticTokenDescriptor();
      return validatePackedSemanticTokens(native.semanticTokens());
    });
  }

  dispose(): void {
    const native = this.native;
    if (!native) return;
    this.native = null;
    withMermanRuntimeState(this.runtimeState, () => native.free());
  }

  private withNative<T>(run: (native: WasmEditorSessionBinding) => T): T {
    const native = this.native;
    if (!native) {
      throw new Error("Merman editor session is disposed.");
    }
    return withMermanRuntimeState(this.runtimeState, () => run(native));
  }
}

export function editorDiagnostics(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorDiagnosticsResult {
  const diagnostics = requireEditorLanguage("editorDiagnostics", getMerman().editorDiagnostics);
  return diagnostics(source, encodeOptions(options), uri);
}

export function editorDiagramDetection(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): DiagramDetectionFacts {
  const detection = requireEditorLanguage(
    "editorDiagramDetection",
    getMerman().editorDiagramDetection
  );
  return validateEditorDiagramDetection(detection(source, encodeOptions(options), uri));
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

export function editorSearchDocumentSymbols(
  source: string,
  query: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorSymbolInformation[] {
  const searchDocumentSymbols = requireEditorLanguage(
    "editorSearchDocumentSymbols",
    getMerman().editorSearchDocumentSymbols
  );
  return searchDocumentSymbols(source, query, uri, encodeOptions(options));
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

export function editorSemanticTokenDescriptor(): EditorSemanticTokenDescriptor {
  return cloneSemanticTokenDescriptor(cachedEditorSemanticTokenDescriptor());
}

function cachedEditorSemanticTokenDescriptor(): EditorSemanticTokenDescriptor {
  const state = currentRuntimeState();
  const cached = editorSemanticTokenDescriptorCaches.get(state);
  if (cached) {
    return cached;
  }
  const descriptor = requireEditorLanguage(
    "editorSemanticTokenDescriptor",
    getMerman().editorSemanticTokenDescriptor
  );
  const validated = validateSemanticTokenDescriptor(descriptor());
  editorSemanticTokenDescriptorCaches.set(state, validated);
  return validated;
}

function cloneSemanticTokenDescriptor(
  descriptor: EditorSemanticTokenDescriptor
): EditorSemanticTokenDescriptor {
  return {
    ...descriptor,
    renamePolicies: [...descriptor.renamePolicies],
    tokenTypes: descriptor.tokenTypes.map((tokenType) => ({ ...tokenType })),
    modifiers: descriptor.modifiers.map((modifier) => ({ ...modifier })),
    packed: {
      ...descriptor.packed,
      fieldOrder: [...descriptor.packed.fieldOrder],
    },
    overlayPrecedence: descriptor.overlayPrecedence.map((entry) => ({ ...entry })),
    tokenTypeLspNames: [...descriptor.tokenTypeLspNames],
    modifierLspNames: [...descriptor.modifierLspNames],
  } as unknown as EditorSemanticTokenDescriptor;
}

export function editorSemanticTokens(
  source: string,
  uri?: string,
  options?: SvgBindingOptions | string
): Uint32Array {
  cachedEditorSemanticTokenDescriptor();
  const tokens = requireEditorLanguage("editorSemanticTokens", getMerman().editorSemanticTokens);
  return validatePackedSemanticTokens(tokens(source, uri, encodeOptions(options)));
}

function requireEditorLanguage<T>(
  apiName: string,
  binding: T | undefined
): T {
  if (
    !runtimeCatalog().capabilities.capability_ids.includes("editor") ||
    binding === undefined
  ) {
    throw new Error(`Merman ${apiName}() is not available in this artifact.`);
  }
  return binding;
}

function validateEditorDiagramDetection(value: unknown): DiagramDetectionFacts {
  if (!isRecord(value)) {
    throw new Error("Merman returned an invalid editor diagram detection result.");
  }
  if (
    value.status === "unavailable" &&
    value.validity === "unknown" &&
    value.diagramType === null &&
    value.syntaxId === null &&
    value.effectiveLayoutId === null
  ) {
    return UNAVAILABLE_DIAGRAM_DETECTION;
  }
  if (
    value.status !== "available" ||
    (value.validity !== "valid" && value.validity !== "recoverable-invalid") ||
    typeof value.diagramType !== "string" ||
    !isDiagramType(value.diagramType) ||
    typeof value.syntaxId !== "string" ||
    value.syntaxId.trim().length === 0 ||
    typeof value.effectiveLayoutId !== "string" ||
    value.effectiveLayoutId.trim().length === 0
  ) {
    throw new Error("Merman returned an invalid editor diagram detection result.");
  }
  return Object.freeze({
    status: value.status,
    validity: value.validity,
    diagramType: value.diagramType,
    syntaxId: value.syntaxId,
    effectiveLayoutId: value.effectiveLayoutId,
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
