import type { editor } from "monaco-editor";
import type { MERMAID_SYNTAX_TOKEN_TYPES } from "./syntax-tokens.ts";

type MermaidSyntaxToken = (typeof MERMAID_SYNTAX_TOKEN_TYPES)[number];
type SemanticPalette = Readonly<Record<MermaidSyntaxToken, string>>;
type WorkbenchTheme = "dark" | "light";
export type WorkbenchEditorThemeName = "merman-dark" | "merman-light";

interface MonacoThemeRegistry {
  readonly editor: {
    defineTheme(name: string, theme: editor.IStandaloneThemeData): void;
  };
}

interface WorkbenchEditorThemeDefinition {
  readonly data: editor.IStandaloneThemeData;
  readonly name: WorkbenchEditorThemeName;
}

const DARK_PALETTE: SemanticPalette = {
  comment: "7F9D78",
  decorator: "C099FF",
  enumMember: "E0AF68",
  function: "C3E88D",
  keyword: "7AA2F7",
  macro: "F7768E",
  namespace: "82AAFF",
  number: "E0AF68",
  operator: "BB9AF7",
  property: "7DCFFF",
  string: "E6B980",
  type: "56B6C2",
  variable: "89B4FA",
};

const LIGHT_PALETTE: SemanticPalette = {
  comment: "607369",
  decorator: "A03565",
  enumMember: "7A4E00",
  function: "6F42C1",
  keyword: "B4232D",
  macro: "A03565",
  namespace: "075FAD",
  number: "7A4E00",
  operator: "6F42C1",
  property: "18794E",
  string: "8B4C22",
  type: "7A3E9D",
  variable: "075FAD",
};

export const WORKBENCH_EDITOR_THEMES = Object.freeze({
  dark: {
    name: "merman-dark",
    data: {
      base: "vs-dark",
      inherit: true,
      rules: semanticRules(DARK_PALETTE),
      colors: {
        "editor.background": "#171B1A",
        "editor.foreground": "#D7DEDB",
        "editor.lineHighlightBackground": "#202725",
        "editor.selectionBackground": "#2D5948",
        "editor.inactiveSelectionBackground": "#28443A",
        "editorCursor.foreground": "#7BD9B2",
        "editorLineNumber.foreground": "#68756F",
        "editorLineNumber.activeForeground": "#AEBAB5",
        "editorIndentGuide.background1": "#2A3430",
        "editorIndentGuide.activeBackground1": "#4B665A",
        "editorBracketHighlight.foreground1": "#8FB7FF",
        "editorBracketHighlight.foreground2": "#C9A7E9",
        "editorBracketHighlight.foreground3": "#73C9B8",
        "editorBracketHighlight.foreground4": "#D8B779",
        "editorBracketHighlight.foreground5": "#D98A9A",
        "editorBracketHighlight.foreground6": "#81AFBD",
        "editorBracketHighlight.unexpectedBracket.foreground": "#F7768E",
        "editorBracketMatch.background": "#3B4F4738",
        "editorBracketMatch.border": "#7BD9B2",
      },
    },
  },
  light: {
    name: "merman-light",
    data: {
      base: "vs",
      inherit: true,
      rules: semanticRules(LIGHT_PALETTE),
      colors: {
        "editor.background": "#FBFDFC",
        "editor.foreground": "#26312D",
        "editor.lineHighlightBackground": "#F2F7F4",
        "editor.selectionBackground": "#B7E4D1",
        "editor.inactiveSelectionBackground": "#D8EEE5",
        "editorCursor.foreground": "#167A59",
        "editorLineNumber.foreground": "#80918A",
        "editorLineNumber.activeForeground": "#4A5A54",
        "editorIndentGuide.background1": "#DDE7E2",
        "editorIndentGuide.activeBackground1": "#9DB8AC",
        "editorBracketHighlight.foreground1": "#4E6FAE",
        "editorBracketHighlight.foreground2": "#7E5AA6",
        "editorBracketHighlight.foreground3": "#2F7A6B",
        "editorBracketHighlight.foreground4": "#8A6114",
        "editorBracketHighlight.foreground5": "#A24655",
        "editorBracketHighlight.foreground6": "#3C7180",
        "editorBracketHighlight.unexpectedBracket.foreground": "#B4232D",
        "editorBracketMatch.background": "#B7E4D166",
        "editorBracketMatch.border": "#167A59",
      },
    },
  },
} satisfies Record<WorkbenchTheme, WorkbenchEditorThemeDefinition>);

export function registerWorkbenchEditorThemes(monaco: MonacoThemeRegistry): void {
  for (const { data, name } of Object.values(WORKBENCH_EDITOR_THEMES)) {
    monaco.editor.defineTheme(name, data);
  }
}

function semanticRules(palette: SemanticPalette): editor.ITokenThemeRule[] {
  return Object.entries(palette).map(([token, foreground]) => ({
    token,
    foreground,
  }));
}
