import assert from "node:assert/strict";
import { DOMParser } from "@xmldom/xmldom";

export function assertMathSvg(svg) {
  const document = new DOMParser({
    onError(level, message) {
      throw new Error(`invalid math SVG (${level}): ${message}`);
    },
  }).parseFromString(svg, "image/svg+xml");
  assert.equal(document.documentElement.localName, "svg");
  assert.equal(document.documentElement.namespaceURI, "http://www.w3.org/2000/svg");
  assert.doesNotMatch(svg, /\$\$/, "formula delimiters must be replaced by rendered geometry");
  const formulas = Array.from(document.getElementsByTagName("svg")).filter(
    (element) => element !== document.documentElement,
  );
  assert.ok(formulas.length > 0, "expected an inline formula SVG viewport");
  for (const formula of formulas) {
    const viewBox = (formula.getAttribute("viewBox") ?? "").trim().split(/[\s,]+/).map(Number);
    assert.ok(viewBox.length === 4 && viewBox.every(Number.isFinite)
      && viewBox[2] > 0 && viewBox[3] > 0, "expected positive formula viewport geometry");
    for (const dimension of ["width", "height"]) {
      assert.ok(Number.parseFloat(formula.getAttribute(dimension)) > 0,
        `expected positive formula ${dimension}`);
    }
    assert.ok(Array.from(formula.getElementsByTagName("path")).some(
      (element) => (element.getAttribute("d") ?? "").trim().length > 0,
    ), "expected outlined formula glyph paths");
  }
  return formulas.length;
}

export async function runThemeAuthoringSmoke(module, engine) {
  const catalog = engine.themeCatalog();
  assert.equal(catalog.schema_version, 2);
  assert.equal(catalog.presets.length, 13);
  assert.ok(catalog.mermaid_themes.includes("dark"));
  assert.ok(catalog.families.some(({ id, capabilities }) => id === "flowchart" && capabilities > 0));
  const studio = catalog.presets.find(({ id }) => id === "studio");
  assert.ok(studio, "missing the Studio preset");
  assert.deepEqual(studio.appearances, ["light", "dark"]);
  assert.equal(studio.swatches.length, 6);

  const exported = await engine.exportThemePreset(studio.id);
  assert.equal(exported.schema_version, 2);
  assert.equal(exported.id, studio.id);
  assert.deepEqual(Object.keys(exported.appearances).sort(), ["dark", "light"]);
  const syncExport = engine.executeOperationSync({ operationId: "export-theme-preset", source: studio.id });
  assert.deepEqual(JSON.parse(syncExport.data), exported);

  const background = "#121820";
  const document = {
    ...exported,
    roles: { ...exported.roles, canvas: background, "text.primary": background },
  };
  const edited = JSON.stringify(document, null, 2);
  const resolution = await engine.resolveTheme(edited, { appearance: "dark", auto_fix_contrast: true });
  assert.equal(resolution.ok, true, JSON.stringify(resolution));
  assert.equal(resolution.resolved.appearance, "dark");
  assert.ok(resolution.notices.some(({ kind, foreground }) =>
    kind === "contrast_adjusted" && foreground === "text.primary"));

  for (const [text, message] of [
    ['{\n  "schema_version": 2,\n  "extends": }', /expected value/i],
    ['{\n  "schema_version": 2,\n  "schema_version": 2\n}', /duplicate key `schema_version`/],
  ]) {
    const diagnostic = await engine.resolveTheme(text);
    assert.equal(diagnostic.ok, false);
    assert.equal(diagnostic.line, 3);
    assert.ok(diagnostic.column > 0);
    assert.equal(typeof diagnostic.path, "string");
    assert.match(diagnostic.message, message);
  }

  const themed = await module.createNodeEngine({ bindingOptions: { theme: studio.id } });
  const source = "flowchart LR\nA[Review] --> B[Publish]";
  const renderOptions = { site_config: { htmlLabels: false }, svg: { diagram_id: "node-theme-smoke" } };
  const render = (theme, diagram = source) => themed.executeOperation({
    operationId: "svg",
    source: diagram,
    optionsJson: JSON.stringify({ ...renderOptions, theme }),
  });
  const outcome = (result) => JSON.parse(result.metadata_json).theme_outcome;
  try {
    const inherited = await render();
    assert.deepEqual(outcome(inherited), {
      mode: "merman_native", preset_id: studio.id, appearance: studio.default_appearance, notices: [],
    });
    assert.deepEqual(outcome(await render("none")), { mode: "mermaid_parity", notices: [] });

    const restored = JSON.parse(edited);
    const direct = await render(restored);
    assert.equal(outcome(direct).appearance, studio.default_appearance);
    const selection = { document: restored, appearance: "dark" };
    const selected = await render(selection);
    assert.equal(outcome(selected).mode, "merman_native");
    assert.equal(outcome(selected).appearance, "dark");
    assert.ok(outcome(selected).notices.some(({ kind }) => kind === "low_contrast"));
    assert.equal(await themed.renderSvg(source, {
      optionsJson: JSON.stringify({ ...renderOptions, theme: selection }),
    }), selected.data);

    const svg = new DOMParser().parseFromString(selected.data, "image/svg+xml");
    const canvas = Array.from(svg.getElementsByTagName("g")).find(
      (element) => element.getAttribute("class") === "merman-theme-canvas",
    );
    const fill = canvas?.getElementsByTagName("rect")[0]?.getAttribute("fill");
    assert.equal(fill?.toLowerCase(), background);

    const authored = `%%{init: {theme: 'dark'}}%%\n${source}`;
    const sourceOutcome = outcome(await render({ preset: studio.id, precedence: "source" }, authored));
    assert.equal(sourceOutcome.mode, "mermaid_parity");
    assert.ok(sourceOutcome.notices.some((notice) => typeof notice === "string"));
    assert.deepEqual(outcome(await render()), outcome(inherited));
  } finally {
    await themed.dispose();
  }
  return { schema_version: 2, presets: catalog.presets.length };
}
