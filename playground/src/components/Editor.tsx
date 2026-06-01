import Editor from "@monaco-editor/react";
import { useCallback, useRef } from "react";
import type { editor, languages } from "monaco-editor";
import { useAppStore } from "@/src/store";

// Mermaid 语法高亮配置
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

// 简化的 Mermaid 语法词法配置
const mermaidTokensProvider = {
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
  operators: ["-->", "---", "-.->", "==>", "->>", "-->>", "-x", "--x", "-)", "--)"],
  tokenizer: {
    root: [
      [/%%.*$/, "comment"],
      [
        /[a-zA-Z_]\w*/,
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
      [/[{}()\[\]]/, "delimiter"],
      [/[0-9]+/, "number"],
    ],
  },
};

interface CodeEditorProps {
  className?: string;
}

export function CodeEditor({ className }: CodeEditorProps) {
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const { code, setCode, uiTheme } = useAppStore();

  const handleEditorDidMount = useCallback(
    (editor: editor.IStandaloneCodeEditor, monaco: typeof import("monaco-editor")) => {
      editorRef.current = editor;

      // 注册 Mermaid 语言
      if (!monaco.languages.getLanguages().some((lang) => lang.id === "mermaid")) {
        monaco.languages.register({ id: "mermaid" });
        monaco.languages.setLanguageConfiguration("mermaid", mermaidLanguageConfig);
        monaco.languages.setMonarchTokensProvider("mermaid", mermaidTokensProvider as never);
      }

      // 设置编辑器选项
      editor.updateOptions({
        minimap: { enabled: false },
        lineNumbers: "on",
        fontSize: 14,
        fontFamily: '"JetBrains Mono", "Fira Code", monospace',
        fontLigatures: true,
        wordWrap: "on",
        scrollBeyondLastLine: false,
        padding: { top: 16, bottom: 16 },
        renderLineHighlight: "line",
        cursorBlinking: "smooth",
        smoothScrolling: true,
        tabSize: 2,
      });

      // 聚焦编辑器
      editor.focus();
    },
    []
  );

  const handleEditorChange = useCallback(
    (value: string | undefined) => {
      setCode(value || "");
    },
    [setCode]
  );

  // 根据 UI 主题决定编辑器主题
  const editorTheme =
    uiTheme === "dark" || (uiTheme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches)
      ? "vs-dark"
      : "light";

  return (
    <div className={className}>
      <Editor
        height="100%"
        language="mermaid"
        theme={editorTheme}
        value={code}
        onChange={handleEditorChange}
        onMount={handleEditorDidMount}
        loading={
          <div className="flex h-full items-center justify-center text-muted-foreground">
            加载编辑器中...
          </div>
        }
        options={{
          automaticLayout: true,
        }}
      />
    </div>
  );
}
