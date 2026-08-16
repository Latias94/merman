import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import test from "node:test";

import { SUPPORTED_THEMES } from "@mermanjs/web";
import { resolveMermaidCanvasTone } from "./mermaid-canvas-tone.ts";
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

test("maps every supported effective theme to a canvas tone", () => {
  const expected = {
    default: "light",
    base: "light",
    dark: "dark",
    forest: "light",
    neutral: "light",
    neo: "light",
    "neo-dark": "dark",
    redux: "light",
    "redux-dark": "dark",
    "redux-color": "light",
    "redux-dark-color": "dark",
  } as const;

  assert.deepEqual(
    Object.fromEntries(
      SUPPORTED_THEMES.map((theme) => [
        theme,
        resolveMermaidCanvasTone("{}", theme),
      ]),
    ),
    expected,
  );
});

test("uses buildMermaidConfig precedence for the effective canvas tone", () => {
  assert.equal(resolveMermaidCanvasTone('{"theme":"dark"}', "default"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"theme":"neo"}', "dark"), "light");
  assert.equal(resolveMermaidCanvasTone("{", "dark"), "dark");
});

test("follows source theme precedence when the injected config has no theme", () => {
  const source = `---
config:
  theme: dark
---
flowchart TD
  A --> B`;

  assert.equal(resolveMermaidCanvasTone("{}", "default", source), "dark");
  assert.equal(resolveMermaidCanvasTone("{}", "forest", source), "light");
  assert.equal(
    resolveMermaidCanvasTone(
      '{"theme":"neutral"}',
      "default",
      `${source}\n%%{init: { 'theme': 'neo-dark' }}%%`,
    ),
    "dark",
  );
});

test("reads block and flow-style Mermaid frontmatter themes", () => {
  for (const source of [
    "---\nconfig: { theme: dark }\n---\nflowchart TD\n  A --> B",
    "---\nconfig: {\n  theme: neo-dark\n}\n---\nflowchart TD\n  A --> B",
    "---\nconfig:\n  theme: 'redux-dark'\n---\nflowchart TD\n  A --> B",
  ]) {
    assert.equal(resolveMermaidCanvasTone("{}", "default", source), "dark");
  }
});

test("scans unmatched Mermaid init directives in linear time", () => {
  const source = "%%{initialize:".repeat(16_384);
  const startedAt = performance.now();

  assert.equal(resolveMermaidCanvasTone("{}", "default", source), "light");
  assert.ok(
    performance.now() - startedAt < 500,
    "unmatched directives should not rescan the remaining source",
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

test("config directives ignore frontmatter-like block scalar content", () => {
  assert.equal(
    sourceWithConfig(
      "---\r\ntitle: |\r\n  A YAML block scalar\r\n  ---\r\n---\r\nflowchart TD",
      "dark",
      "{}"
    ),
    '---\r\ntitle: |\r\n  A YAML block scalar\r\n  ---\r\n---\r\n%%{init: {"theme":"dark"}}%%\r\nflowchart TD'
  );
});

test("config directives preserve Mermaid frontmatter indentation", () => {
  assert.equal(
    sourceWithConfig(
      "   ---\n   title: Sample\n   ---\n   flowchart TD",
      "dark",
      "{}"
    ),
    '   ---\n   title: Sample\n   ---\n%%{init: {"theme":"dark"}}%%\n   flowchart TD'
  );
});

test("indented frontmatter does not close on a differently indented scalar line", () => {
  assert.equal(
    sourceWithConfig(
      "   ---\n   title: |\n     A scalar\n     ---\n   ---\n   flowchart TD",
      "dark",
      "{}"
    ),
    '   ---\n   title: |\n     A scalar\n     ---\n   ---\n%%{init: {"theme":"dark"}}%%\n   flowchart TD'
  );
});
