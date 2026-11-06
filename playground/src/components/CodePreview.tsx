import { ReadOnlyEditor } from "@/src/components/ReadOnlyEditor";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import { useAppStore } from "@/src/store";

export function CodePreview({ value, filename, label, testId }: {
  readonly value: string;
  readonly filename: string;
  readonly label: string;
  readonly testId?: string;
}) {
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const language = codeLanguage(filename);
  return <div className="min-w-0 overflow-hidden" data-testid={testId} data-code-language={language}>
    <ReadOnlyEditor feature={label} value={value} language={language} theme={WORKBENCH_EDITOR_THEMES[resolvedTheme].name}
      height={Math.min(320, Math.max(100, value.split("\n").length * 18 + 24))}
      options={{ ariaLabel: label, minimap: { enabled: false }, fontSize: 12, lineHeight: 18,
        fontFamily: "Menlo, Consolas, monospace", lineNumbers: "off", folding: false,
        scrollBeyondLastLine: false, renderLineHighlight: "none", selectionHighlight: false,
        automaticLayout: true, tabFocusMode: true, padding: { top: 12, bottom: 12 },
        scrollbar: { alwaysConsumeMouseWheel: false }, overviewRulerLanes: 0, wordWrap: "off" }} />
  </div>;
}

function codeLanguage(filename: string): string {
  if (filename.endsWith(".json")) return "json";
  if (filename.endsWith(".rs")) return "rust";
  if (filename.endsWith(".js")) return "javascript";
  if (filename.endsWith(".py")) return "python";
  if (filename.endsWith(".toml")) return "ini";
  if (filename.endsWith(".html")) return "html";
  if (filename.endsWith(".mmd")) return "mermaid";
  if (filename === "render-command.txt") return "shell";
  return "plaintext";
}
