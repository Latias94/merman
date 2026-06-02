import type { editor, languages } from "monaco-editor";
import type { ValidationResult } from "@/src/lib/wasm-loader";

export const MERMAID_LANGUAGE_ID = "mermaid";

const MARKER_OWNER = "merman";

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

let registered = false;

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
    triggerCharacters: [" ", "\n"],
    provideCompletionItems(model, position) {
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
