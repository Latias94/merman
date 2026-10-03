import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import test from "node:test";
import { load as parseYaml } from "js-yaml";

import { SUPPORTED_THEMES } from "@mermanjs/web";
import { resolveMermaidCanvasTone } from "./mermaid-canvas-tone.ts";
import { locateMermaidFrontmatter } from "./mermaid-frontmatter.ts";
import {
  buildMermaidConfig,
  buildMermaidOperationInput,
  sourceWithConfig,
} from "./mermaid-config.ts";

test("Mermaid config accepts every canonical target theme", () => {
  for (const theme of SUPPORTED_THEMES) {
    const config = buildMermaidConfig("{}", theme);
    assert.equal(config.theme, theme);
  }
});

test("automatic appearance omits theme while explicit default is preserved", () => {
  assert.deepEqual(buildMermaidConfig("{}", "auto"), {});
  assert.deepEqual(buildMermaidConfig("{}", "default"), { theme: "default" });
  assert.deepEqual(buildMermaidConfig('{"theme":"forest"}', "auto"), { theme: "forest" });
  assert.equal(sourceWithConfig("flowchart TD\nA-->B", "auto", "{}"), "flowchart TD\nA-->B");
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

  assert.equal(resolveMermaidCanvasTone("{}", "auto", source), "dark");
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
    assert.equal(resolveMermaidCanvasTone("{}", "auto", source), "dark");
  }
});

test("scans unmatched Mermaid init directives in linear time", () => {
  const source = "%%{initialize:".repeat(16_384);
  const startedAt = performance.now();

  assert.equal(resolveMermaidCanvasTone("{}", "auto", source), "light");
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


test("canvas tone follows scoped Mermaid 12 themes without parsing diagram headers", () => {
  const source = "flowchart TD\nA-->B";
  assert.equal(resolveMermaidCanvasTone('{"flowchart":{"theme":"dark"}}', "auto", source, "flowchart"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"flowchart":{"theme":"dark"}}', "default", source, "flowchart"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"theme":"dark","flowchart":{"theme":"default"}}', "auto", source, "flowchart"), "light");
  assert.equal(resolveMermaidCanvasTone('{"theme":"dark","flowchart":{"theme":"unknown"}}', "auto", source, "flowchart"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"flowchart":{"theme":"unknown"}}', "dark", source, "flowchart"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"sequence":{"theme":"dark"}}', "auto", source, "flowchart"), "light");
  assert.equal(resolveMermaidCanvasTone('{"xyChart":{"theme":"dark"}}', "auto", "xychart-beta", "xyChart"), "dark");
});

test("canvas tone merges scoped source themes with the injected configuration", () => {
  const source = "---\nconfig:\n  flowchart:\n    theme: dark\n---\nflowchart TD\nA-->B";
  assert.equal(resolveMermaidCanvasTone("{}", "default", source, "flowchart"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"flowchart":{"theme":"default"}}', "auto", source, "flowchart"), "light");
  assert.equal(resolveMermaidCanvasTone("{}", "default", `${source}\n%%{init: {'flowchart': {'theme': 'forest'}}}%%`, "flowchart"), "light");
  assert.equal(resolveMermaidCanvasTone("{}", "default", "flowchart TD\nA-->B\n%%{init: {'flowchart': {'theme': 'dark'}}}%%", "flowchart"), "dark");
});


test("canvas validates themes after merging replacements and ignoring JSON null", () => {
  assert.equal(resolveMermaidCanvasTone('{"theme":"unknown"}', "dark", "flowchart TD\nA-->B", "flowchart"), "light");
  const source = "---\nconfig:\n  flowchart:\n    theme: dark\n---\nflowchart TD\nA-->B";
  assert.equal(
    resolveMermaidCanvasTone('{"theme":"default","flowchart":{"theme":"unknown"}}', "auto", source, "flowchart"),
    "light",
  );
  assert.equal(
    resolveMermaidCanvasTone('{"theme":"default","flowchart":{"theme":null}}', "auto", source, "flowchart"),
    "dark",
  );
  assert.equal(
    resolveMermaidCanvasTone("{}", "auto", `${source}\n%%{init: {"theme":"default","flowchart":{"theme":"unknown"}}}%%`, "flowchart"),
    "light",
  );
  assert.equal(
    resolveMermaidCanvasTone("{}", "auto", '---\nconfig: {theme: dark}\n---\nflowchart TD\nA-->B\n%%{init: {"theme":"unknown"}}%%', "flowchart"),
    "light",
  );
});

test("the string null sentinel retains the initialized theme palette", () => {
  const source = 'flowchart TD\nA-->B\n%%{init: {"flowchart":{"theme":"null"}}}%%';
  assert.equal(resolveMermaidCanvasTone('{"theme":"dark"}', "auto", source, "flowchart"), "dark");
  assert.equal(resolveMermaidCanvasTone('{"theme":"default","flowchart":{"theme":"dark"}}', "auto", source, "flowchart"), "light");
});


test("operation initialization admits only theme names and leaves authored settings in source", () => {
  for (const [selection, expectedTheme] of [["auto", undefined], ["default", "default"], ["dark", "dark"]] as const) {
    const operation = buildMermaidOperationInput("flowchart TD\nA-->B", selection, "{}");
    assert.equal(operation.initializationConfig.theme, expectedTheme);
    assert.equal(operation.initializationConfig.securityLevel, undefined);
    assert.equal(operation.initializationConfig.startOnLoad, undefined);
    assert.equal(Object.isFrozen(operation), true);
    assert.equal(Object.isFrozen(operation.initializationConfig), true);
  }
  const operation = buildMermaidOperationInput(
    '---\nconfig: {theme: forest}\n---\nflowchart TD\nA-->B\n%%{init: {"theme":"neutral"}}%%',
    "dark",
    '{"flowchart":{"theme":"null"},"securityLevel":"strict","secure":["theme"]}',
  );
  assert.equal(operation.initializationConfig.theme, "dark");
  assert.equal(operation.initializationConfig.flowchart, undefined);
  assert.equal(operation.initializationConfig.securityLevel, undefined);
  assert.equal(operation.initializationConfig.secure, undefined);
  assert.deepEqual(Object.keys(operation.initializationConfig), ["theme"]);
  assert.ok(operation.configuredSource.startsWith('---\nconfig: {theme: forest}\n---\n%%{init:'));
  assert.ok(operation.configuredSource.endsWith('%%{init: {"theme":"neutral"}}%%'));
});


test("canvas resolves invalid themes without granting authored secure host authority", () => {
  const invalid = 'flowchart TD\nA-->B\n%%{init: {"theme":"unknown"}}%%';
  assert.equal(resolveMermaidCanvasTone("{}", "dark", invalid, "flowchart"), "dark");
  const override = '---\nconfig: {theme: forest}\n---\nflowchart TD\nA-->B\n%%{init: {"theme":"neutral","flowchart":{"theme":"forest"}}}%%';
  assert.equal(resolveMermaidCanvasTone('{"secure":["theme"]}', "dark", override, "flowchart"), "light");
  assert.equal(resolveMermaidCanvasTone('{"theme":"default","flowchart":{"theme":"dark"},"secure":["flowchart"]}', "auto", override, "flowchart"), "light");
});


test("invalid authored themes initialize the default palette without inheriting the selection", () => {
  for (const theme of ["unknown", "null", null, false, "constructor"]) {
    const operation = buildMermaidOperationInput("flowchart TD\nA-->B", "dark", JSON.stringify({ theme }));
    assert.deepEqual(operation.initializationConfig, {});
    assert.equal(resolveMermaidCanvasTone(JSON.stringify({ theme }), "dark", "flowchart TD\nA-->B", "flowchart"), "light");
  }
});

// Mermaid's unchanged published regex is an oracle only for these bounded inputs.
const publishedFrontmatter = /^([^\S\n\r]*)-{3}\s*[\n\r](.*?)[\n\r]\1-{3}\s*[\n\r]+/s;

function assertFrontmatterMatchesPublished(source: string): void {
  const expected = publishedFrontmatter.exec(source);
  const actual = locateMermaidFrontmatter(source);
  if (!expected) {
    assert.equal(actual, undefined, JSON.stringify(source));
    return;
  }
  assert.ok(actual, JSON.stringify(source));
  assert.equal(actual.indent, expected[1], JSON.stringify(source));
  assert.equal(source.slice(actual.bodyStart, actual.bodyEnd), expected[2], JSON.stringify(source));
  assert.equal(actual.end, expected[0].length, JSON.stringify(source));
}

test("frontmatter boundaries retain Mermaid 12.1 greedy whitespace and UTF-16 semantics", () => {
  for (const source of [
    "---\ntitle: Hello\n---\nflowchart TD\nA-->B",
    "---\n\n---\ntitle: Hello\n---\nflowchart TD\nA-->B",
    "---\n\n---\n\nMORE\n---\n",
    "---\n\n\n---\nflowchart TD\n",
    "---   \ntitle: 😀\n---   \n\n \nflowchart TD\n",
    "---\rtitle: Bare CR\r---\rflowchart TD\rA-->B",
    "\t---\r\n\ttitle: Tab\r\n\t---\r\nflowchart TD\n",
    "\u00a0\u2003---\n\u00a0\u2003title: Unicode\n\u00a0\u2003---\nflowchart TD",
    "\ufeff---\ntitle: BOM indent\n\ufeff---\nflowchart TD",
    "---\u2028\ntitle: Separator\n---\u2029\nflowchart TD",
    "---\ntitle: |\n  ---\n  a scalar\n---\nflowchart TD",
    "---\ntitle: no newline after close\n---",
    "---\ntitle: unterminated\nflowchart TD",
    "  ---\ntitle: different indent\n---\nflowchart TD",
    "----\ntitle: four dashes\n----\nflowchart TD",
    "\u0085---\ntitle: non-JavaScript whitespace\n\u0085---\nflowchart TD",
    "---", "---\n", "", "\n\n\n", "flowchart TD\nA-->B",
  ]) {
    assertFrontmatterMatchesPublished(source);
  }
});

test("frontmatter boundaries agree with the published contract on mixed fences and whitespace", () => {
  let seed = 0x2f6e2b1;
  const pieces = ["---", "----", "\n", "\r\n", "\r", " ", "\t", "\u00a0", "\u2003", "\u2028", "a", "😀", "title: x", ""];
  for (let sample = 0; sample < 3_000; sample++) {
    let source = "";
    for (let part = 0; part < 12; part++) {
      seed = (Math.imul(seed, 1103515245) + 12345) & 0x7fffffff;
      source += pieces[seed % pieces.length];
    }
    assertFrontmatterMatchesPublished(source);
  }
});

test("injected configuration stays outside greedy frontmatter without changing source bytes", () => {
  const directive = '%%{init: {"theme":"dark"}}%%';
  for (const source of [
    "---\n\n---\ntitle: Hello\n---\nflowchart TD\nA-->B",
    "---\rtitle: Bare CR\r---\rflowchart TD\rA-->B",
    "  ---\r\n  title: Mixed\n  ---\r\n\n \r\nflowchart TD\nA-->B",
    "\u00a0---\ntitle: Unicode\n\u00a0---\nflowchart TD",
  ]) {
    const original = locateMermaidFrontmatter(source);
    assert.ok(original);
    const newline = /\r\n|[\r\n]/.exec(source)?.[0] ?? "\n";
    const rendered = sourceWithConfig(source, "dark", "{}");
    assert.equal(rendered, `${source.slice(0, original.end)}${directive}${newline}${source.slice(original.end)}`);
    assert.equal(rendered.slice(0, original.end) + rendered.slice(original.end + directive.length + newline.length), source);
    const after = locateMermaidFrontmatter(rendered);
    assert.deepEqual(after, original);
    assert.ok(after);
    assert.deepEqual(
      parseYaml(rendered.slice(after.bodyStart, after.bodyEnd)),
      parseYaml(source.slice(original.bodyStart, original.bodyEnd)),
    );
  }
});

test("configuration injection preserves malformed and unclosed source verbatim", () => {
  for (const source of [
    "---\ntitle: no closing fence\nflowchart TD",
    "---\ntitle: no trailing newline\n---",
    "---\rtitle: no closing fence\rflowchart TD",
    "  ---\ntitle: different indent\n---\nflowchart TD",
    "----\ntitle: four dashes\n----\nflowchart TD",
    "---\n", "---",
  ]) {
    const newline = /\r\n|[\r\n]/.exec(source)?.[0] ?? "\n";
    assert.equal(sourceWithConfig(source, "dark", "{}"), `%%{init: {"theme":"dark"}}%%${newline}${source}`);
    assert.equal(sourceWithConfig(source, "auto", "{}"), source);
  }
});

test("canvas tone shares greedy and Unicode frontmatter boundaries with injection", () => {
  for (const source of [
    "---\n\n---\nconfig: {theme: dark}\n---\nflowchart TD",
    "---\rconfig: {theme: dark}\r---\rflowchart TD",
    "\t---\r\tconfig: {theme: dark}\r\t---\rflowchart TD",
    "\u00a0---\n\u00a0config: {theme: dark}\n\u00a0---\nflowchart TD",
  ]) {
    assert.equal(resolveMermaidCanvasTone("{}", "auto", source), "dark", JSON.stringify(source));
  }
  assert.equal(resolveMermaidCanvasTone("{}", "auto", "---\nconfig: {theme: dark}\n---"), "light");
});

test("frontmatter consumers scan a whitespace-heavy unterminated block without backtracking", () => {
  const source = "---\n" + " \n".repeat(16_000);
  const startedAt = performance.now();
  assert.equal(locateMermaidFrontmatter(source), undefined);
  assert.equal(resolveMermaidCanvasTone("{}", "auto", source), "light");
  assert.equal(sourceWithConfig(source, "dark", "{}"), `%%{init: {"theme":"dark"}}%%\n${source}`);
  assert.ok(performance.now() - startedAt < 500, "frontmatter consumers must not rescan unmatched suffixes");
});
