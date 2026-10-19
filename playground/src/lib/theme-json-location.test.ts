import assert from "node:assert/strict";
import test from "node:test";
import type { ASTNode, ObjectASTNode } from "monaco-editor/languages/features/json/register";

import { findThemeJsonValue } from "./theme-json-location.ts";

const first: ASTNode = { type: "string", value: "#123456", offset: 24, length: 9 };
const second: ASTNode = { type: "string", value: "#123456", offset: 58, length: 9 };
function object(entries: [string, ASTNode][]): ObjectASTNode {
  const properties = entries.map(([key, valueNode]) => ({
    type: "property" as const,
    keyNode: { type: "string" as const, value: key, offset: 0, length: key.length + 2 },
    valueNode,
    offset: 0,
    length: 100,
    children: [],
  }));
  return { type: "object", properties, children: properties, offset: 0, length: 100 };
}
const root = object([["design", object([
  ["tokens", object([["a/b~c", object([["value", first]])], ["constructor", object([["value", second]])]])],
  ["rules", { type: "array", items: [object([["style", object([["fill", second]])]])], children: [], offset: 0, length: 100 }],
])]]);

test("JSON reveal uses the recipe envelope and exact escaped authoring keys", () => {
  assert.equal(findThemeJsonValue(root, "/design/tokens/a~1b~0c/value"), first);
  assert.equal(findThemeJsonValue(root, "/design/tokens/constructor/value"), second);
  assert.equal(findThemeJsonValue(root, "/tokens/a~1b~0c/value"), undefined);
});

test("JSON reveal selects the indexed value rather than an equal literal elsewhere", () => {
  assert.equal(findThemeJsonValue(root, "/design/rules/0/style/fill"), second);
  assert.equal(findThemeJsonValue(root, "/design/rules/01/style/fill"), undefined);
  assert.equal(findThemeJsonValue(root, "/design/rules/1/style/fill"), undefined);
});

test("removed or malformed destinations do not fall back to a parent location", () => {
  for (const pointer of ["design", "/design/tokens/a~2b", "/design/missing/value", "/design/tokens/constructor/value/extra"]) {
    assert.equal(findThemeJsonValue(root, pointer), undefined);
  }
  assert.equal(findThemeJsonValue(undefined, "/design"), undefined);
});
