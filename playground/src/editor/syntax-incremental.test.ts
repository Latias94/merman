import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { Edit, Language, Parser, Query, type Tree } from "web-tree-sitter";
import { singleEdit } from "./syntax-edit.ts";
import { projectMermaidSyntaxTokens } from "./syntax-tokens.ts";

const grammarUrl = new URL(
  "../../../distribution/tree-sitter-mermaid/tree-sitter-mermaid.wasm",
  import.meta.url,
);
const queryUrl = new URL(
  "../../../distribution/tree-sitter-mermaid/queries/portable/highlights.scm",
  import.meta.url,
);

test("astral-prefix incremental highlighting matches a fresh browser parse", async () => {
  await Parser.init();
  const language = await Language.load(fileURLToPath(grammarUrl));
  const parser = new Parser();
  parser.setLanguage(language);
  const query = new Query(language, await readFile(queryUrl, "utf8"));
  const previous = "%% 😀\r\nflowchart TD\r\nA --> B";
  const next = "%% 😀\r\nflowchart TD\r\nA --> bright";
  const oldTree = requireTree(parser.parse(previous));
  oldTree.edit(new Edit(singleEdit(previous, next)));
  const incremental = requireTree(parser.parse(next, oldTree));
  const fresh = requireTree(parser.parse(next));

  const incrementalTokens = tokens(next, query, incremental);
  const freshTokens = tokens(next, query, fresh);
  assert.deepEqual(incrementalTokens, freshTokens);
  assert(incrementalTokens.length > 0);
  assert(
    captures(query, incremental).some(
      ({ capture, startIndex }) =>
        capture === "variable" &&
        startIndex === next.indexOf("bright"),
    ),
  );

  oldTree.delete();
  incremental.delete();
  fresh.delete();
  query.delete();
  parser.delete();
});

function tokens(source: string, query: Query, tree: Tree): number[] {
  return [...projectMermaidSyntaxTokens(source, captures(query, tree))];
}

function captures(query: Query, tree: Tree) {
  return query.captures(tree.rootNode).map(({ name, node, patternIndex }) => ({
    capture: name,
    endIndex: node.endIndex,
    patternIndex,
    startIndex: node.startIndex,
  }));
}

function requireTree(tree: Tree | null): Tree {
  if (!tree) throw new Error("Tree-sitter parse was cancelled.");
  return tree;
}
