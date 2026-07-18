import assert from "node:assert/strict";
import test from "node:test";

import { SUPPORTED_THEMES } from "@mermanjs/web/catalog";
import {
  buildMermaidConfig,
  sourceWithConfig,
} from "./mermaid-config.ts";

test("Mermaid config accepts every canonical 11.16 theme", () => {
  for (const theme of SUPPORTED_THEMES) {
    const config = buildMermaidConfig("{}", theme);
    assert.equal(config.theme, theme === "default" ? undefined : theme);
  }
});

test("explicit config theme takes precedence over the selected theme", () => {
  assert.equal(
    buildMermaidConfig('{"theme":"dark"}', "redux-color").theme,
    "dark"
  );
});

test("config directives preserve frontmatter and newline style", () => {
  assert.equal(
    sourceWithConfig("flowchart TD\nA-->B", "dark", "{}"),
    '%%{init: {"theme":"dark"}}%%\nflowchart TD\nA-->B'
  );
  assert.equal(
    sourceWithConfig("---\r\ntitle: Sample\r\n---\r\nflowchart TD", "dark", "{}"),
    '---\r\ntitle: Sample\r\n---\r\n%%{init: {"theme":"dark"}}%%\r\nflowchart TD'
  );
});
