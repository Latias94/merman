import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import test from "node:test";
import { load as parseYaml } from "js-yaml";

import { SUPPORTED_THEMES } from "@mermanjs/web";
import { locateMermaidFrontmatter } from "./mermaid-frontmatter.ts";
import {
  buildMermaidConfig,
  buildMermaidOperationInput,
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
  assert.equal(buildMermaidOperationInput("flowchart TD\nA-->B", "auto", "{}").configuredSource, "flowchart TD\nA-->B");
});

test("explicit config theme takes precedence over the selected theme", () => {
  assert.equal(
    buildMermaidConfig('{"theme":"dark"}', "redux-color").theme,
    "dark"
  );
  for (const selection of ["dark", "auto"]) {
    const operation = buildMermaidOperationInput('pie\n"Tea": 4', selection, '{"theme":"forest"}');
    assert.deepEqual(operation.initializationConfig, { theme: "forest" });
    assert.equal(operation.configuredSource, '%%{init: {"theme":"forest"}}%%\npie\n"Tea": 4');
  }
});

test("inherited fonts add no host or source override and preserve authored fonts", () => {
  const source = "pie\n\"Tea\": 4";
  assert.deepEqual(buildMermaidConfig("{}", "auto", { diagramFont: "inherit" }), {});
  assert.deepEqual(buildMermaidOperationInput(source, "auto", "{}", { diagramFont: "inherit" }), {
    initializationConfig: {}, configuredSource: source, injectedConfigSpan: null,
  });
  const config = { fontFamily: "Authored Font", themeVariables: { fontFamily: "Authored Variable Font" } };
  const operation = buildMermaidOperationInput(source, "auto", JSON.stringify(config), { diagramFont: "inherit" });
  assert.deepEqual(operation.initializationConfig, {});
  assert.equal(operation.configuredSource, `%%{init: ${JSON.stringify(config)}}%%\n${source}`);
});

test("config directives preserve frontmatter and newline style", () => {
  assert.equal(
    buildMermaidOperationInput("flowchart TD\nA-->B", "auto", '{"theme":"dark"}').configuredSource,
    '%%{init: {"theme":"dark"}}%%\nflowchart TD\nA-->B'
  );
  assert.equal(
    buildMermaidOperationInput("---\r\ntitle: Sample\r\n---\r\nflowchart TD", "auto", '{"theme":"dark"}').configuredSource,
    '---\r\ntitle: Sample\r\n---\r\n%%{init: {"theme":"dark"}}%%\r\nflowchart TD'
  );
});

test("config directives ignore frontmatter-like block scalar content", () => {
  assert.equal(
    buildMermaidOperationInput(
      "---\r\ntitle: |\r\n  A YAML block scalar\r\n  ---\r\n---\r\nflowchart TD",
      "auto",
      '{"theme":"dark"}'
    ).configuredSource,
    '---\r\ntitle: |\r\n  A YAML block scalar\r\n  ---\r\n---\r\n%%{init: {"theme":"dark"}}%%\r\nflowchart TD'
  );
});

test("config directives preserve Mermaid frontmatter indentation", () => {
  assert.equal(
    buildMermaidOperationInput(
      "   ---\n   title: Sample\n   ---\n   flowchart TD",
      "auto",
      '{"theme":"dark"}'
    ).configuredSource,
    '   ---\n   title: Sample\n   ---\n%%{init: {"theme":"dark"}}%%\n   flowchart TD'
  );
});

test("indented frontmatter does not close on a differently indented scalar line", () => {
  assert.equal(
    buildMermaidOperationInput(
      "   ---\n   title: |\n     A scalar\n     ---\n   ---\n   flowchart TD",
      "auto",
      '{"theme":"dark"}'
    ).configuredSource,
    '   ---\n   title: |\n     A scalar\n     ---\n   ---\n%%{init: {"theme":"dark"}}%%\n   flowchart TD'
  );
});


test("operation initialization admits only theme names and leaves authored settings in source", () => {
  for (const [selection, expectedTheme] of [["auto", undefined], ["default", "default"], ["dark", "dark"]] as const) {
    const operation = buildMermaidOperationInput("flowchart TD\nA-->B", selection, "{}");
    assert.equal(operation.initializationConfig.theme, expectedTheme);
    assert.equal(operation.configuredSource, "flowchart TD\nA-->B");
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

test("operation keeps UI theme and font defaults out of authored source", () => {
  const source = "stateDiagram-v2\nA\nnote right of A : Terminal note";
  const fontFamily = "Arial, Helvetica, sans-serif";
  for (const configJson of ["{}", '{"htmlLabels":false}']) {
    const operation = buildMermaidOperationInput(source, "default", configJson, {
      diagramFont: "arial",
    });
    assert.deepEqual(operation.initializationConfig, {
      theme: "default",
      fontFamily,
      themeVariables: { fontFamily },
    });
    assert.equal(
      operation.configuredSource,
      configJson === "{}" ? source : `%%{init: ${configJson}}%%\n${source}`,
    );
  }
});

test("operation preserves explicit user theme and fonts as authored source", () => {
  const source = "flowchart TD\nA-->B";
  const authoredConfig = {
    theme: "forest",
    fontFamily: "Authored Root Font",
    themeVariables: { fontFamily: "Authored Variable Font", primaryColor: "#123456" },
  };
  const configJson = JSON.stringify(authoredConfig);
  const operation = buildMermaidOperationInput(source, "dark", configJson, {
    diagramFont: "arial",
  });
  assert.deepEqual(operation.initializationConfig, {
    theme: "forest",
    fontFamily: "Arial, Helvetica, sans-serif",
    themeVariables: { fontFamily: "Arial, Helvetica, sans-serif" },
  });
  assert.equal(operation.configuredSource, `%%{init: ${configJson}}%%\n${source}`);
});

test("operation retains native source appearance after injected user settings", () => {
  const frontmatter = "---\nconfig: {theme: forest}\n---\n";
  const body = 'flowchart TD\nA-->B\n%%{init: {"theme":"dark","fontFamily":"Native Font"}}%%';
  const source = frontmatter + body;
  const configJson = '{"theme":"neutral","fontFamily":"User Font"}';
  const operation = buildMermaidOperationInput(source, "default", configJson, {
    diagramFont: "arial",
  });
  assert.equal(operation.initializationConfig.theme, "neutral");
  assert.equal(operation.configuredSource, `${frontmatter}%%{init: ${configJson}}%%\n${body}`);
  assert.equal(
    buildMermaidOperationInput(source, "default", "{}", { diagramFont: "arial" }).configuredSource,
    source,
  );
});


test("null theme sentinel retains the host selection in both config and source", () => {
  const source = 'pie\n"Tea": 4';
  const configuredSource = '%%{init: {"theme":"null"}}%%\n' + source;
  for (const selection of ["dark", "auto"]) {
    const configForm = buildMermaidOperationInput(source, selection, '{"theme":"null"}');
    const sourceForm = buildMermaidOperationInput(configuredSource, selection, "{}");
    assert.deepEqual(configForm.initializationConfig, selection === "auto" ? {} : { theme: "dark" });
    assert.deepEqual(configForm.initializationConfig, sourceForm.initializationConfig);
    assert.equal(configForm.configuredSource, configuredSource);
    assert.equal(sourceForm.configuredSource, configuredSource);
  }
});

test("invalid authored themes preserve the host selection and remain in source", () => {
  const source = 'pie\n"Tea": 4';
  for (const theme of ["unknown", null, false, 0, "constructor", "auto"]) {
    const configJson = JSON.stringify({ theme });
    for (const selection of ["dark", "auto"]) {
      const operation = buildMermaidOperationInput(source, selection, configJson);
      assert.deepEqual(operation.initializationConfig, selection === "auto" ? {} : { theme: "dark" });
      assert.equal(operation.configuredSource, `%%{init: ${configJson}}%%\n${source}`);
    }
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
    const rendered = buildMermaidOperationInput(source, "auto", '{"theme":"dark"}').configuredSource;
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

test("config injection preserves a leading BOM before frontmatter and UTF-8 spans", () => {
  const configJson = '{"themeVariables":{"textColor":"#123456"},"fontFamily":"字體 Ω"}';
  const directive = `%%{init: ${configJson}}%%`;
  const encoder = new TextEncoder();
  for (const newline of ["\n", "\r\n"]) {
    const frontmatter = `\uFEFF---${newline}title: 雪😀${newline}config: {theme: forest}${newline}---${newline}`;
    const source = `${frontmatter}pie${newline}"Tea": 4`;
    const operation = buildMermaidOperationInput(source, "auto", configJson);
    assert.equal(operation.configuredSource, `${frontmatter}${directive}${newline}pie${newline}"Tea": 4`);
    const start = encoder.encode(frontmatter).byteLength;
    const span = operation.injectedConfigSpan;
    assert.deepEqual(span, { start, end: start + encoder.encode(directive).byteLength });
    assert.ok(span);
    assert.deepEqual(encoder.encode(operation.configuredSource).slice(span.start, span.end), encoder.encode(directive));
    assert.deepEqual(buildMermaidOperationInput(source, "auto", "{}"), {
      initializationConfig: {}, configuredSource: source, injectedConfigSpan: null,
    });
  }
});

test("config injection stays after a leading BOM without frontmatter and records UTF-8 spans", () => {
  const configJson = '{"themeVariables":{"textColor":"#123456"},"fontFamily":"字體 Ω"}';
  const directive = `%%{init: ${configJson}}%%`;
  const encoder = new TextEncoder();
  for (const newline of ["\n", "\r\n"]) {
    const source = `\uFEFFpie${newline}"雪😀": 4`;
    const operation = buildMermaidOperationInput(source, "auto", configJson);
    assert.equal(operation.configuredSource, `\uFEFF${directive}${newline}pie${newline}"雪😀": 4`);
    const span = operation.injectedConfigSpan;
    assert.deepEqual(span, { start: 3, end: 3 + encoder.encode(directive).byteLength });
    assert.ok(span);
    assert.deepEqual(encoder.encode(operation.configuredSource).slice(span.start, span.end), encoder.encode(directive));
    assert.deepEqual(buildMermaidOperationInput(source, "auto", "{}"), {
      initializationConfig: {}, configuredSource: source, injectedConfigSpan: null,
    });
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
    assert.equal(buildMermaidOperationInput(source, "auto", '{"theme":"dark"}').configuredSource, `%%{init: {"theme":"dark"}}%%${newline}${source}`);
    assert.equal(buildMermaidOperationInput(source, "auto", "{}").configuredSource, source);
  }
});

test("configuration injection records exact UTF-8 directive spans after frontmatter", () => {
  for (const source of [
    "---\n\n---\nconfig: {theme: dark}\n---\nflowchart TD",
    "---\rconfig: {theme: dark}\r---\rflowchart TD",
    "\t---\r\tconfig: {theme: dark}\r\t---\rflowchart TD",
    "\u00a0---\n\u00a0config: {theme: dark}\n\u00a0---\nflowchart TD",
  ]) {
    const operation = buildMermaidOperationInput(source, "auto", '{"theme":"dark"}');
    const span = operation.injectedConfigSpan;
    assert.ok(span);
    assert.equal(new TextDecoder().decode(new TextEncoder().encode(operation.configuredSource).slice(span.start, span.end)), '%%{init: {"theme":"dark"}}%%');
  }
});

test("frontmatter consumers scan a whitespace-heavy unterminated block without backtracking", () => {
  const source = "---\n" + " \n".repeat(16_000);
  const startedAt = performance.now();
  assert.equal(locateMermaidFrontmatter(source), undefined);
  assert.equal(buildMermaidOperationInput(source, "auto", '{"theme":"dark"}').configuredSource, `%%{init: {"theme":"dark"}}%%\n${source}`);
  assert.ok(performance.now() - startedAt < 500, "frontmatter consumers must not rescan unmatched suffixes");
});
