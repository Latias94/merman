import { Edit, Language, Parser, Query, type Tree } from "web-tree-sitter";
import runtimeWasmUrl from "web-tree-sitter/web-tree-sitter.wasm?url";
import languageWasmUrl from "../../../distribution/tree-sitter-mermaid/tree-sitter-mermaid.wasm?url";
import highlightsQuery from "../../../distribution/tree-sitter-mermaid/queries/portable/highlights.scm?raw";
import {
  projectMermaidSyntaxTokens,
  type MermaidSyntaxCapture,
} from "./syntax-tokens.ts";
import { singleEdit } from "./syntax-edit.ts";

export interface MermaidSyntaxEngine {
  dispose(): void;
  highlight(): Uint32Array;
  open(source: string): void;
  update(source: string): void;
}

export async function createMermaidSyntaxEngine(): Promise<MermaidSyntaxEngine> {
  await Parser.init({ locateFile: () => runtimeWasmUrl });
  const language = await Language.load(languageWasmUrl);
  const parser = new Parser();
  parser.setLanguage(language);
  const query = new Query(language, highlightsQuery);
  let source = "";
  let tree: Tree | null = null;

  const parseFresh = (next: string): Tree => {
    parser.reset();
    const parsed = parser.parse(next);
    if (!parsed) throw new Error("Tree-sitter Mermaid parse was cancelled.");
    return parsed;
  };

  const replaceTree = (next: string, nextTree: Tree): void => {
    tree?.delete();
    tree = nextTree;
    source = next;
  };

  return Object.freeze({
    dispose(): void {
      tree?.delete();
      tree = null;
      query.delete();
      parser.delete();
    },
    highlight(): Uint32Array {
      if (!tree) throw new Error("Tree-sitter Mermaid document is not open.");
      const captures: MermaidSyntaxCapture[] = query
        .captures(tree.rootNode)
        .map(({ name, node, patternIndex }) => ({
          capture: name,
          endIndex: node.endIndex,
          patternIndex,
          startIndex: node.startIndex,
        }));
      return projectMermaidSyntaxTokens(source, captures);
    },
    open(next: string): void {
      if (tree) throw new Error("Tree-sitter Mermaid document is already open.");
      replaceTree(next, parseFresh(next));
    },
    update(next: string): void {
      if (!tree) throw new Error("Tree-sitter Mermaid document is not open.");
      const previousTree = tree;
      previousTree.edit(new Edit(singleEdit(source, next)));
      const parsed = parser.parse(next, previousTree) ?? parseFresh(next);
      replaceTree(next, parsed);
    },
  });
}
