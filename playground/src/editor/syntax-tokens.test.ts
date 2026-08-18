import assert from "node:assert/strict";
import test from "node:test";
import {
  MERMAID_SYNTAX_TOKEN_TYPES,
  projectMermaidSyntaxTokens,
} from "./syntax-tokens.ts";

test("syntax captures project to sorted non-overlapping UTF-16 Monaco tokens", () => {
  const source = 'flowchart TD\r\n😀A --> B\r\n"x\r\ny"';
  const identifierStart = source.indexOf("😀A");
  const memberStart = source.indexOf("A -->");
  const operatorStart = source.indexOf("-->");
  const stringStart = source.indexOf('"x');
  const tokens = decode(
    projectMermaidSyntaxTokens(source, [
      { capture: "keyword", startIndex: 0, endIndex: 9, patternIndex: 0 },
      {
        capture: "variable",
        startIndex: identifierStart,
        endIndex: identifierStart + 3,
        patternIndex: 1,
      },
      {
        capture: "variable.member",
        startIndex: memberStart,
        endIndex: memberStart + 1,
        patternIndex: 2,
      },
      {
        capture: "operator",
        startIndex: operatorStart,
        endIndex: operatorStart + 3,
        patternIndex: 3,
      },
      {
        capture: "string",
        startIndex: stringStart,
        endIndex: source.length,
        patternIndex: 4,
      },
    ]),
  );

  assert.deepEqual(tokens, [
    [0, 0, 9, "keyword"],
    [1, 0, 2, "variable"],
    [1, 2, 1, "property"],
    [1, 4, 3, "operator"],
    [2, 0, 2, "string"],
    [3, 0, 2, "string"],
  ]);
});

test("higher-priority captures split a broader span without overlap", () => {
  const tokens = decode(
    projectMermaidSyntaxTokens("alpha", [
      { capture: "string", startIndex: 0, endIndex: 5, patternIndex: 0 },
      { capture: "variable.member", startIndex: 1, endIndex: 4, patternIndex: 1 },
    ]),
  );
  assert.deepEqual(tokens, [
    [0, 0, 1, "string"],
    [0, 1, 3, "property"],
    [0, 4, 1, "string"],
  ]);
});

function decode(data: Uint32Array): [number, number, number, string][] {
  const tokens: [number, number, number, string][] = [];
  let line = 0;
  let start = 0;
  for (let index = 0; index < data.length; index += 5) {
    const deltaLine = data[index] ?? 0;
    line += deltaLine;
    start = deltaLine === 0 ? start + (data[index + 1] ?? 0) : (data[index + 1] ?? 0);
    tokens.push([
      line,
      start,
      data[index + 2] ?? 0,
      MERMAID_SYNTAX_TOKEN_TYPES[data[index + 3] ?? 0] ?? "unknown",
    ]);
  }
  return tokens;
}
