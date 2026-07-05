import type { editor, languages } from "monaco-editor";
import type {
  MermanWasm,
  ValidationResult,
} from "@/src/lib/wasm-loader";
import type {
  EditorCodeAction,
  EditorCompletionItem,
  EditorDiagnostic,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorLocation,
  EditorRange,
  EditorSemanticToken,
  EditorSymbolKind,
  EditorWorkspaceEdit,
} from "@mermanjs/web";

export const MERMAID_LANGUAGE_ID = "mermaid";

const MARKER_OWNER = "merman";

export interface MermaidSemanticTokenLegend {
  tokenTypes: string[];
  tokenModifiers: string[];
}

const STATIC_SEMANTIC_TOKEN_LEGEND: MermaidSemanticTokenLegend = {
  tokenTypes: [
    "namespace",
    "class",
    "struct",
    "variable",
    "property",
    "event",
    "function",
    "string",
  ],
  tokenModifiers: ["entity", "outline", "payload"],
};

const mermaidLanguageConfig: languages.LanguageConfiguration = {
  comments: {
    lineComment: "%%",
  },
  brackets: [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
  ],
  autoClosingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
};

const mermaidTokensProvider: languages.IMonarchLanguage = {
  keywords: [
    "flowchart",
    "graph",
    "sequenceDiagram",
    "classDiagram",
    "stateDiagram",
    "stateDiagram-v2",
    "erDiagram",
    "gantt",
    "pie",
    "mindmap",
    "timeline",
    "eventmodeling",
    "gitGraph",
    "xychart",
    "architecture-beta",
    "block-beta",
    "packet",
    "kanban",
    "quadrantChart",
    "sankey",
    "radar-beta",
    "treemap-beta",
    "requirementDiagram",
    "subgraph",
    "end",
    "participant",
    "actor",
    "note",
    "loop",
    "alt",
    "else",
    "opt",
    "par",
    "critical",
    "break",
    "rect",
    "class",
    "section",
    "title",
    "dateFormat",
    "axisFormat",
    "excludes",
    "includes",
    "todayMarker",
    "showData",
    "direction",
    "TB",
    "TD",
    "BT",
    "RL",
    "LR",
  ],
  operators: [
    "-->",
    "---",
    "-.->",
    "==>",
    "->>",
    "-->>",
    "-x",
    "--x",
    "-)",
    "--)",
  ],
  tokenizer: {
    root: [
      [/%%.*$/, "comment"],
      [
        /[a-zA-Z][\w-]*/,
        {
          cases: {
            "@keywords": "keyword",
            "@default": "identifier",
          },
        },
      ],
      [/"[^"]*"/, "string"],
      [/'[^']*'/, "string"],
      [/\|[^|]*\|/, "string"],
      [/\[[^\]]*\]/, "type"],
      [/\([^)]*\)/, "type"],
      [/\{[^}]*\}/, "type"],
      [/-->|---|-\.->|==>|->>|-->>|-x|--x|-\)|-\-\)/, "operator"],
      [/[{}()[\]]/, "delimiter"],
      [/[0-9]+(?:\.[0-9]+)?/, "number"],
    ],
  },
};

interface CompletionSpec {
  label: string;
  insertText: string;
  detail?: string;
  documentation?: string;
  snippet?: boolean;
}

const keywordCompletions: CompletionSpec[] = [
  keyword("flowchart"),
  keyword("sequenceDiagram"),
  keyword("classDiagram"),
  keyword("stateDiagram-v2"),
  keyword("erDiagram"),
  keyword("eventmodeling"),
  keyword("xychart"),
  keyword("architecture-beta"),
  keyword("block-beta"),
  keyword("packet"),
  keyword("kanban"),
  keyword("quadrantChart"),
  keyword("sankey"),
  keyword("radar-beta"),
  keyword("treemap-beta"),
  keyword("requirementDiagram"),
  keyword("participant"),
  keyword("subgraph"),
  keyword("autonumber"),
  keyword("alt"),
  keyword("loop"),
  keyword("classDef"),
];

const snippetCompletions: CompletionSpec[] = [
  snippet(
    "flowchart TD",
    "flowchart TD\n  ${1:A}[${2:Start}] --> ${3:B}{${4:Condition?}}\n  ${3:B} -->|Yes| ${5:C}[${6:Done}]"
  ),
  snippet(
    "sequenceDiagram",
    "sequenceDiagram\n  participant ${1:A} as ${2:Client}\n  participant ${3:B} as ${4:Server}\n  ${1:A}->>${3:B}: ${5:Request}\n  ${3:B}-->>${1:A}: ${6:Response}"
  ),
  snippet(
    "classDiagram",
    "classDiagram\n  class ${1:Renderer} {\n    +${2:renderSvg}()\n  }\n  class ${3:Parser}\n  ${3:Parser} --> ${1:Renderer} : ${4:feeds}"
  ),
  snippet(
    "requirementDiagram",
    "requirementDiagram\n  requirement ${1:api} {\n    id: ${2:1}\n    text: ${3:Stable render API}\n    risk: medium\n    verifymethod: test\n  }\n  element ${4:wasm} {\n    type: library\n  }\n  ${4:wasm} - satisfies -> ${1:api}"
  ),
  snippet(
    "eventmodeling",
    "eventmodeling\n  tf ${1:01} ui ${2:CartScreen}\n  tf ${3:02} cmd ${4:AddItem} ->> ${1:01}\n  tf ${5:03} evt ${6:ItemAdded} ->> ${3:02}"
  ),
  snippet(
    "xychart",
    "xychart\n  title \"${1:Render timings}\"\n  x-axis [\"${2:Parse}\", \"${3:Layout}\", \"${4:SVG}\"]\n  y-axis \"${5:ms}\" 0 --> ${6:100}\n  bar [${7:12}, ${8:34}, ${9:58}]"
  ),
  snippet(
    "kanban",
    "kanban\n  ${1:backlog}[${2:Backlog}]\n    ${3:item}[${4:Task}]@{ assigned: \"${5:Team}\", priority: \"${6:High}\" }"
  ),
  snippet(
    "packet",
    "packet\n  +4: \"${1:Version}\"\n  +4: \"${2:IHL}\"\n  +8: \"${3:DSCP}\"\n  +16: \"${4:Total Length}\""
  ),
  snippet(
    "sankey",
    "sankey\n  ${1:Editor},${2:Parser},${3:8}\n  ${2:Parser},${4:Layout},${5:7}\n  ${4:Layout},${6:SVG},${7:6}"
  ),
];

let hoverDocs: Record<string, string> = {};
let editorService: MermanWasm | null = null;

let registered = false;

export function setMermaidEditorService(service: MermanWasm | null): void {
  editorService = service;
}

export function registerMermaidLanguage(
  monaco: typeof import("monaco-editor")
): void {
  if (registered) return;

  if (
    !monaco.languages
      .getLanguages()
      .some((lang) => lang.id === MERMAID_LANGUAGE_ID)
  ) {
    monaco.languages.register({ id: MERMAID_LANGUAGE_ID });
  }

  monaco.languages.setLanguageConfiguration(
    MERMAID_LANGUAGE_ID,
    mermaidLanguageConfig
  );
  monaco.languages.setMonarchTokensProvider(
    MERMAID_LANGUAGE_ID,
    mermaidTokensProvider
  );
  monaco.languages.registerCompletionItemProvider(MERMAID_LANGUAGE_ID, {
    triggerCharacters: [" ", "\n", "-", "@", ":"],
    provideCompletionItems(model, position) {
      const service = editorService;
      if (service) {
        try {
          const completions = service.editor_completions(model.getValue(), {
            line: position.lineNumber - 1,
            character: position.column - 1,
          });
          return {
            suggestions: completions.items.map((item) =>
              toEditorCompletionItem(monaco, item, position)
            ),
          };
        } catch {
          // Static snippets remain useful while the WASM artifact is loading or stale.
        }
      }

      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(
        position.lineNumber,
        word.startColumn,
        position.lineNumber,
        word.endColumn
      );
      return {
        suggestions: [
          ...keywordCompletions.map((item) =>
            toCompletionItem(monaco, item, range, false)
          ),
          ...snippetCompletions.map((item) =>
            toCompletionItem(monaco, item, range, true)
          ),
        ],
      };
    },
  });
  monaco.languages.registerHoverProvider(MERMAID_LANGUAGE_ID, {
    provideHover(model, position) {
      const service = editorService;
      if (service) {
        try {
          const hover = service.editor_hover(model.getValue(), {
            line: position.lineNumber - 1,
            character: position.column - 1,
          });
          if (hover) {
            return {
              range: hover.range
                ? toMonacoRange(monaco, hover.range)
                : undefined,
              contents: [{ value: hover.contents.value }],
            };
          }
        } catch {
          // Fall through to lightweight keyword docs.
        }
      }

      const token = tokenAtPosition(
        model.getLineContent(position.lineNumber),
        position.column
      );
      if (!token) return null;

      const documentation = hoverDocs[token.value];
      if (!documentation) return null;

      return {
        range: new monaco.Range(
          position.lineNumber,
          token.startColumn,
          position.lineNumber,
          token.endColumn
        ),
        contents: [{ value: `**${token.value}**\n\n${documentation}` }],
      };
    },
  });
  monaco.languages.registerDocumentFormattingEditProvider(MERMAID_LANGUAGE_ID, {
    provideDocumentFormattingEdits(model) {
      const formatted = formatMermaidCode(model.getValue());
      if (formatted === model.getValue()) return [];
      return [{ range: model.getFullModelRange(), text: formatted }];
    },
  });
  monaco.languages.registerCodeActionProvider(MERMAID_LANGUAGE_ID, {
    provideCodeActions(model, _range, context) {
      const service = editorService;
      if (!service) {
        return { actions: [], dispose() {} };
      }
      try {
          const actions = service
            .editor_code_actions(model.getValue())
            .filter((action) =>
              action.diagnostics.some((diagnostic) =>
                context.markers.some((marker) =>
                  markerMatchesDiagnostic(monaco, model, marker, diagnostic)
                )
              )
            )
            .map((action) => toMonacoCodeAction(monaco, model, action));
        return { actions, dispose() {} };
      } catch {
        return { actions: [], dispose() {} };
      }
    },
  });
  monaco.languages.registerDocumentSymbolProvider(MERMAID_LANGUAGE_ID, {
    provideDocumentSymbols(model) {
      const service = editorService;
      if (!service) return [];
      try {
        return service
          .editor_document_symbols(model.getValue())
          .map((symbol) => toMonacoDocumentSymbol(monaco, symbol));
      } catch {
        return [];
      }
    },
  });
  monaco.languages.registerDefinitionProvider(MERMAID_LANGUAGE_ID, {
    provideDefinition(model, position) {
      const service = editorService;
      if (!service) return null;
      try {
        const location = service.editor_definition(model.getValue(), {
          line: position.lineNumber - 1,
          character: position.column - 1,
        });
        return location ? toMonacoLocation(monaco, model, location) : null;
      } catch {
        return null;
      }
    },
  });
  monaco.languages.registerReferenceProvider(MERMAID_LANGUAGE_ID, {
    provideReferences(model, position, context) {
      const service = editorService;
      if (!service) return [];
      try {
        return service
          .editor_references(
            model.getValue(),
            {
              line: position.lineNumber - 1,
              character: position.column - 1,
            },
            context.includeDeclaration
          )
          .map((location) => toMonacoLocation(monaco, model, location));
      } catch {
        return [];
      }
    },
  });
  monaco.languages.registerRenameProvider(MERMAID_LANGUAGE_ID, {
    resolveRenameLocation(model, position) {
      const service = editorService;
      if (!service) return null;
      try {
        const prepare = service.editor_prepare_rename(model.getValue(), {
          line: position.lineNumber - 1,
          character: position.column - 1,
        });
        if (!prepare) {
          return null;
        }
        return {
          range: toMonacoRange(monaco, prepare.range),
          text: prepare.placeholder,
        };
      } catch {
        return null;
      }
    },
    provideRenameEdits(model, position, newName) {
      const service = editorService;
      if (!service) {
        return { edits: [], rejectReason: "No Mermaid symbol at cursor." };
      }
      try {
        const edit = service.editor_rename(
          model.getValue(),
          {
            line: position.lineNumber - 1,
            character: position.column - 1,
          },
          newName
        );
        if (!edit) {
          return { edits: [], rejectReason: "No Mermaid symbol at cursor." };
        }
        return toMonacoWorkspaceEdit(monaco, model, edit);
      } catch (error) {
        return {
          edits: [],
          rejectReason:
            error instanceof Error ? error.message : "Rename is not available.",
        };
      }
    },
  });
  monaco.languages.registerDocumentSemanticTokensProvider(MERMAID_LANGUAGE_ID, {
    getLegend() {
      return semanticTokenLegendForService(editorService);
    },
    provideDocumentSemanticTokens(model) {
      const service = editorService;
      if (!service) {
        return { data: new Uint32Array(0), resultId: undefined };
      }
      try {
        const legend = semanticTokenLegendForService(service);
        const tokens = service.editor_semantic_tokens(model.getValue());
        return {
          data: encodeSemanticTokensForLegend(tokens, legend),
          resultId: undefined,
        };
      } catch {
        return { data: new Uint32Array(0), resultId: undefined };
      }
    },
    releaseDocumentSemanticTokens() {},
  });

  registered = true;
}

export function setMermaidHoverDocs(docs: Record<string, string>): void {
  hoverDocs = docs;
}

export function getMermaidHoverDocs(
  t: (key: string) => string
): Record<string, string> {
  return {
    flowchart: t("editor.hover.flowchart"),
    graph: t("editor.hover.graph"),
    sequenceDiagram: t("editor.hover.sequenceDiagram"),
    classDiagram: t("editor.hover.classDiagram"),
    "stateDiagram-v2": t("editor.hover.stateDiagram"),
    erDiagram: t("editor.hover.erDiagram"),
    xychart: t("editor.hover.xychart"),
    "architecture-beta": t("editor.hover.architecture"),
    "block-beta": t("editor.hover.block"),
    packet: t("editor.hover.packet"),
    kanban: t("editor.hover.kanban"),
    quadrantChart: t("editor.hover.quadrant"),
    sankey: t("editor.hover.sankey"),
    "radar-beta": t("editor.hover.radar"),
    "treemap-beta": t("editor.hover.treemap"),
    requirementDiagram: t("editor.hover.requirement"),
    eventmodeling: t("editor.hover.eventmodeling"),
    participant: t("editor.hover.participant"),
    subgraph: t("editor.hover.subgraph"),
    autonumber: t("editor.hover.autonumber"),
  };
}

export function formatMermaidCode(source: string): string {
  if (!source.trim()) return "";

  const lines = source
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => line.trimEnd());

  while (lines.length > 1 && lines[lines.length - 1]?.trim() === "") {
    lines.pop();
  }

  return `${lines.join("\n")}\n`;
}

export function updateMermaidMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  validation: ValidationResult
): void {
  if (validation.valid) {
    clearMermaidMarkers(monaco, model);
    return;
  }

  const message = validation.error || "Mermaid syntax error";
  monaco.editor.setModelMarkers(model, MARKER_OWNER, [
    {
      ...rangeFromError(model, message),
      severity: monaco.MarkerSeverity.Error,
      message,
      source: "Merman",
    },
  ]);
}

export function updateMermaidEditorMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  diagnostics: EditorDiagnosticsResult
): void {
  monaco.editor.setModelMarkers(
    model,
    MARKER_OWNER,
    diagnostics.diagnostics.map((diagnostic) =>
      toMarkerData(monaco, model, diagnostic)
    )
  );
}

export function clearMermaidMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel
): void {
  monaco.editor.setModelMarkers(model, MARKER_OWNER, []);
}

function keyword(label: string): CompletionSpec {
  return {
    label,
    insertText: label,
  };
}

function snippet(label: string, insertText: string): CompletionSpec {
  return {
    label,
    insertText,
    snippet: true,
  };
}

function toCompletionItem(
  monaco: typeof import("monaco-editor"),
  item: CompletionSpec,
  range: languages.CompletionItem["range"],
  snippetItem: boolean
): languages.CompletionItem {
  return {
    label: item.label,
    kind: snippetItem
      ? monaco.languages.CompletionItemKind.Snippet
      : monaco.languages.CompletionItemKind.Keyword,
    insertText: item.insertText,
    insertTextRules:
      item.snippet || snippetItem
        ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
        : undefined,
    detail: item.detail,
    documentation: item.documentation,
    range,
  };
}

function toEditorCompletionItem(
  monaco: typeof import("monaco-editor"),
  item: EditorCompletionItem,
  position: { lineNumber: number; column: number }
): languages.CompletionItem {
  const fallbackRange = (() => {
    const lineNumber = position.lineNumber;
    const column = position.column;
    return new monaco.Range(lineNumber, column, lineNumber, column);
  })();
  return {
    label: item.label,
    kind:
      item.kind === "variable"
        ? monaco.languages.CompletionItemKind.Variable
        : monaco.languages.CompletionItemKind.Keyword,
    insertText: item.text_edit?.new_text ?? item.insert_text ?? item.label,
    insertTextRules:
      item.insert_text_format === "snippet"
        ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
        : undefined,
    detail: item.detail ?? undefined,
    documentation: item.data
      ? {
          value: completionDocumentation(item.data.kind, item.data.label),
        }
      : undefined,
    range: item.text_edit?.range
      ? toMonacoEditRange(monaco, item.text_edit.range)
      : fallbackRange,
  };
}

function completionDocumentation(kind: string, label: string): string {
  switch (kind) {
    case "diagram_header":
      return `Starts a Mermaid \`${label}\` diagram.`;
    case "operator":
      return `Inserts the Mermaid \`${label}\` relationship operator.`;
    case "direction":
      return `Sets flow direction with \`${label}\`.`;
    case "directive":
      return `Inserts \`${label}\` as a Mermaid directive or comment helper.`;
    case "shape":
      return `Inserts Mermaid flowchart shape syntax for \`${label}\`.`;
    case "node_identifier":
      return `Reuses the \`${label}\` identifier in this diagram.`;
    default:
      return label;
  }
}

function toMonacoCodeAction(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  action: EditorCodeAction
): languages.CodeAction {
  return {
    title: action.title,
    kind: action.kind,
    diagnostics: action.diagnostics.map((diagnostic) =>
      toMarkerData(monaco, model, diagnostic)
    ),
    edit: {
      edits: Object.values(action.edit.changes).flatMap((edits) =>
        edits.map((edit) => ({
          resource: model.uri,
          versionId: model.getVersionId(),
          textEdit: {
            range: toMonacoEditRange(monaco, edit.range),
            text: edit.newText,
          },
        }))
      ),
    },
    isPreferred: action.isPreferred,
  };
}

function toMonacoWorkspaceEdit(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  edit: EditorWorkspaceEdit
): languages.WorkspaceEdit {
  return {
    edits: Object.values(edit.changes).flatMap((edits) =>
      edits.map((textEdit) => ({
        resource: model.uri,
        versionId: model.getVersionId(),
        textEdit: {
          range: toMonacoEditRange(monaco, textEdit.range),
          text: textEdit.newText,
        },
      }))
    ),
  };
}

function toMonacoLocation(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  location: EditorLocation
): languages.Location {
  return {
    uri: model.uri,
    range: toMonacoRange(monaco, location.range),
  };
}

function toMonacoDocumentSymbol(
  monaco: typeof import("monaco-editor"),
  symbol: EditorDocumentSymbol
): languages.DocumentSymbol {
  return {
    name: symbol.name,
    detail: symbol.detail ?? "",
    kind: symbolKind(monaco, symbol.kind),
    tags: [],
    range: toMonacoRange(monaco, symbol.range),
    selectionRange: toMonacoRange(monaco, symbol.selectionRange),
    children: symbol.children.map((child) =>
      toMonacoDocumentSymbol(monaco, child)
    ),
  };
}

export function semanticTokenLegendForService(
  service: Pick<MermanWasm, "editor_semantic_token_legend"> | null,
): MermaidSemanticTokenLegend {
  try {
    return service?.editor_semantic_token_legend() ?? STATIC_SEMANTIC_TOKEN_LEGEND;
  } catch {
    return STATIC_SEMANTIC_TOKEN_LEGEND;
  }
}

export function encodeSemanticTokensForLegend(
  tokens: EditorSemanticToken[],
  legend: MermaidSemanticTokenLegend,
): Uint32Array {
  const data: number[] = [];
  let previousLine = 0;
  let previousStart = 0;
  const sorted = [...tokens].sort(
    (left, right) =>
      left.line - right.line ||
      left.start - right.start ||
      left.length - right.length
  );

  for (const token of sorted) {
    const deltaLine = token.line - previousLine;
    const deltaStart = deltaLine === 0 ? token.start - previousStart : token.start;
    const tokenType = Math.max(0, legend.tokenTypes.indexOf(token.tokenType));
    const modifierIndex = legend.tokenModifiers.indexOf(token.tokenModifier);
    const tokenModifiers = modifierIndex >= 0 ? 1 << modifierIndex : 0;

    data.push(deltaLine, deltaStart, token.length, tokenType, tokenModifiers);
    previousLine = token.line;
    previousStart = token.start;
  }

  return new Uint32Array(data);
}

function toMarkerData(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  diagnostic: EditorDiagnostic
): editor.IMarkerData {
  const range = toMonacoDisplayRange(monaco, diagnostic.range);
  const relatedRanges = diagnostic.related.map((related) => ({
    related,
    range: toMonacoDisplayRange(monaco, related.range),
  }));
  return {
    startLineNumber: range.startLineNumber,
    startColumn: range.startColumn,
    endLineNumber: range.endLineNumber,
    endColumn: range.endColumn,
    severity: diagnosticSeverity(monaco, diagnostic.severity),
    message: diagnostic.message,
    source: diagnostic.source || "Merman",
    code:
      typeof diagnostic.code === "number"
        ? String(diagnostic.code)
        : diagnostic.code,
    relatedInformation: relatedRanges.map(({ related, range }) => ({
      resource: model.uri,
      message: related.message,
      startLineNumber: range.startLineNumber,
      startColumn: range.startColumn,
      endLineNumber: range.endLineNumber,
      endColumn: range.endColumn,
    })),
  };
}

function markerMatchesDiagnostic(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  marker: editor.IMarkerData,
  diagnostic: EditorDiagnostic
): boolean {
  const expected = toMarkerData(monaco, model, diagnostic);
  return (
    marker.message === expected.message &&
    marker.source === expected.source &&
    normalizeMarkerCode(marker.code) === normalizeMarkerCode(expected.code) &&
    marker.startLineNumber === expected.startLineNumber &&
    marker.startColumn === expected.startColumn &&
    marker.endLineNumber === expected.endLineNumber &&
    marker.endColumn === expected.endColumn
  );
}

function normalizeMarkerCode(code: editor.IMarkerData["code"]): string {
  if (code === undefined) {
    return "";
  }
  if (typeof code === "object") {
    return String(code.value);
  }
  return String(code);
}

function toMonacoRange(
  monaco: typeof import("monaco-editor"),
  range: EditorRange
): InstanceType<typeof monaco.Range> {
  const startLineNumber = range.start.line + 1;
  const startColumn = range.start.character + 1;
  const endLineNumber = range.end.line + 1;
  const endColumn = Math.max(range.end.character + 1, 1);
  return new monaco.Range(
    startLineNumber,
    startColumn,
    endLineNumber,
    endColumn
  );
}

function toMonacoEditRange(
  monaco: typeof import("monaco-editor"),
  range: EditorRange
): InstanceType<typeof monaco.Range> {
  return toMonacoRange(monaco, range);
}

function toMonacoDisplayRange(
  monaco: typeof import("monaco-editor"),
  range: EditorRange
): InstanceType<typeof monaco.Range> {
  const monacoRange = toMonacoRange(monaco, range);
  if (
    monacoRange.startLineNumber === monacoRange.endLineNumber &&
    monacoRange.startColumn === monacoRange.endColumn
  ) {
    return new monaco.Range(
      monacoRange.startLineNumber,
      monacoRange.startColumn,
      monacoRange.endLineNumber,
      monacoRange.endColumn + 1
    );
  }
  return monacoRange;
}

function diagnosticSeverity(
  monaco: typeof import("monaco-editor"),
  severity: EditorDiagnostic["severity"]
): (typeof monaco.MarkerSeverity)[keyof typeof monaco.MarkerSeverity] {
  switch (severity) {
    case "hint":
      return monaco.MarkerSeverity.Hint;
    case "info":
      return monaco.MarkerSeverity.Info;
    case "warning":
      return monaco.MarkerSeverity.Warning;
    case "error":
    default:
      return monaco.MarkerSeverity.Error;
  }
}

function symbolKind(
  monaco: typeof import("monaco-editor"),
  kind: EditorSymbolKind
): languages.SymbolKind {
  switch (kind) {
    case "class":
      return monaco.languages.SymbolKind.Class;
    case "event":
      return monaco.languages.SymbolKind.Event;
    case "function":
      return monaco.languages.SymbolKind.Function;
    case "module":
      return monaco.languages.SymbolKind.Module;
    case "namespace":
      return monaco.languages.SymbolKind.Namespace;
    case "object":
      return monaco.languages.SymbolKind.Object;
    case "package":
      return monaco.languages.SymbolKind.Package;
    case "property":
      return monaco.languages.SymbolKind.Property;
    case "string":
      return monaco.languages.SymbolKind.String;
    case "struct":
      return monaco.languages.SymbolKind.Struct;
    case "variable":
    default:
      return monaco.languages.SymbolKind.Variable;
  }
}

function tokenAtPosition(
  line: string,
  column: number
): { value: string; startColumn: number; endColumn: number } | null {
  const index = Math.max(0, column - 1);
  const tokenPattern = /[A-Za-z][\w-]*/g;
  let match: RegExpExecArray | null;

  while ((match = tokenPattern.exec(line))) {
    const startIndex = match.index;
    const endIndex = startIndex + match[0].length;
    if (index >= startIndex && index <= endIndex) {
      return {
        value: match[0],
        startColumn: startIndex + 1,
        endColumn: endIndex + 1,
      };
    }
  }

  return null;
}

function rangeFromError(
  model: editor.ITextModel,
  message: string
): Pick<
  editor.IMarkerData,
  "startLineNumber" | "startColumn" | "endLineNumber" | "endColumn"
> {
  const offsetMatch = message.match(/offset:?\s*(\d+)/i);
  if (offsetMatch) {
    const offset = Number(offsetMatch[1]);
    const position = model.getPositionAt(Number.isFinite(offset) ? offset : 0);
    return markerRangeAt(model, position.lineNumber, position.column);
  }

  const lineMatch = message.match(/line\s+(\d+)/i);
  if (lineMatch) {
    const lineNumber = clampLineNumber(model, Number(lineMatch[1]));
    return markerRangeAt(model, lineNumber, 1);
  }

  return markerRangeAt(model, firstNonEmptyLine(model), 1);
}

function markerRangeAt(
  model: editor.ITextModel,
  lineNumber: number,
  column: number
): Pick<
  editor.IMarkerData,
  "startLineNumber" | "startColumn" | "endLineNumber" | "endColumn"
> {
  const startLineNumber = clampLineNumber(model, lineNumber);
  const maxColumn = model.getLineMaxColumn(startLineNumber);
  const startColumn = Math.max(1, Math.min(column, maxColumn));
  const endColumn = Math.max(startColumn + 1, maxColumn);

  return {
    startLineNumber,
    startColumn,
    endLineNumber: startLineNumber,
    endColumn,
  };
}

function firstNonEmptyLine(model: editor.ITextModel): number {
  for (let lineNumber = 1; lineNumber <= model.getLineCount(); lineNumber += 1) {
    if (model.getLineContent(lineNumber).trim()) {
      return lineNumber;
    }
  }
  return 1;
}

function clampLineNumber(model: editor.ITextModel, value: number): number {
  if (!Number.isFinite(value)) return 1;
  return Math.max(1, Math.min(model.getLineCount(), Math.floor(value)));
}
