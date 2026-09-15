import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { DOMParser } from "@xmldom/xmldom";

const FIXTURES = new URL(
  "../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/",
  import.meta.url,
);
const SOURCES = {
  flowchart: "flowchart LR\nA[Alpha] --> B[Beta]\n",
  state: "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n",
  sequence: "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World\n",
};

// This oracle deliberately covers only the frozen fixtures' ASCII keys and safe integers.
// It is not a general JSON canonicalization implementation.
export function fixtureCanonicalJson(value) {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    assert.ok(Number.isSafeInteger(value), "fixture numbers must be safe integers");
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(fixtureCanonicalJson).join(",")}]`;
  }
  assert.ok(value && Object.getPrototypeOf(value) === Object.prototype, "expected JSON object");
  const entries = Object.keys(value).sort().map((key) => {
    assert.match(key, /^[\x00-\x7f]*$/, "fixture keys must be ASCII");
    return `${JSON.stringify(key)}:${fixtureCanonicalJson(value[key])}`;
  });
  return `{${entries.join(",")}}`;
}

export function parseSvg(svg) {
  const document = new DOMParser({
    onError(level, message) {
      throw new Error(`invalid SVG (${level}): ${message}`);
    },
  }).parseFromString(svg, "image/svg+xml");
  assert.equal(document.documentElement.localName, "svg", "expected SVG root");
  assert.equal(document.documentElement.namespaceURI, "http://www.w3.org/2000/svg");
  assert.equal(document.getElementsByTagName("foreignObject").length, 0, "expected native text");
  assert.ok(document.getElementsByTagName("text").length > 0, "expected terminal native text");
  return document;
}

export function stateFill(svg) {
  const document = parseSvg(svg);
  const hasClass = (element, name) => (element.getAttribute("class") ?? "").split(/\s+/).includes(name);
  const states = Array.from(document.getElementsByTagName("g")).filter(
    (element) => hasClass(element, "statediagram-state") && element.textContent.trim() === "Active",
  );
  assert.equal(states.length, 1, "expected one Active state");
  const shapes = Array.from(states[0].getElementsByTagName("rect")).filter(
    (element) => hasClass(element, "label-container"),
  );
  assert.equal(shapes.length, 1, "expected one Active state shape");
  const shape = shapes[0];
  for (const name of ["width", "height"]) {
    const value = Number(shape.getAttribute(name));
    assert.ok(Number.isFinite(value) && value > 0, `Active shape needs positive ${name}`);
  }
  // Inspect this bounded writer declaration, without recreating CSS cascade.
  const declarations = new Map((shape.getAttribute("style") ?? "").split(";").flatMap((part) => {
    const colon = part.indexOf(":");
    return colon < 0 ? [] : [[part.slice(0, colon).trim(), part.slice(colon + 1).trim()]];
  }));
  const fill = declarations.get("fill") ?? shape.getAttribute("fill");
  assert.equal(typeof fill, "string", "Active shape must expose a fill declaration");
  return fill.replace(/\s*!important\s*$/, "").trim();
}

export function assertAuthoringDiagnostic(error, vector) {
  assert.equal(error.codeName, vector.code_name);
  assert.deepEqual(error.details?.resource ?? null, vector.resource ?? null);
  const envelope = structuredClone(error.details?.theme_authoring);
  assert.ok(Array.isArray(envelope?.diagnostics));
  for (const diagnostic of envelope.diagnostics) {
    assert.equal(typeof diagnostic.message, "string");
    assert.ok(diagnostic.message.trim().length > 0);
    delete diagnostic.message;
  }
  assert.deepEqual(envelope, vector.theme_authoring, vector.id);
  return true;
}

function assertResourceLimit(error) {
  assert.equal(error.codeName, "MERMAN_RESOURCE_LIMIT_EXCEEDED");
  assert.deepEqual(error.details?.resource, {
    cause: "ceiling",
    limit_id: "max_theme_encoded_bytes",
    phase: "theme_input",
    actual: 2,
    max: 1,
    profile: "interactive",
  });
  assert.equal(error.details?.theme_authoring?.schema_version, 1);
  assert.equal(error.details?.theme_authoring?.diagnostics?.[0]?.code,
    "theme-authoring.resource-limit-exceeded");
  return true;
}

// The caller supplies the actual installed module. No transport or engine is substituted here.
export async function runThemeAuthoringSmoke(module, engine) {
  const counts = {
    schema_version: 1,
    shared_vectors: 0,
    catalog_checks: 0,
    family_isolation_checks: 0,
    rule_override_checks: 0,
    cold_spec_checks: 0,
    preset_export_checks: 0,
    support_queries: 0,
    authoring_diagnostics: 0,
    resource_limit_checks: 0,
    json_operations: 0,
    svg_renders: 0,
  };
  async function executeJson(client, operationId, source) {
    const result = await client.executeOperation({ operationId, source });
    assert.equal(result.operation_id, operationId);
    assert.equal(result.media_type, "application/json");
    counts.json_operations += 1;
    return JSON.parse(result.data);
  }
  async function materialize(client, definition) {
    const result = await executeJson(client, "materialize-theme-json", definition);
    assert.deepEqual([
      result.schema_version, result.authoring_schema_version,
      result.expansion_version, result.spec_schema_version,
    ], [1, 1, 1, 1]);
    return result.spec;
  }
  async function renderTheme(client, family, theme, diagramId = `node-authoring-${family}`) {
    const svg = await client.renderSvg(SOURCES[family], {
      optionsJson: JSON.stringify({
        version: 3, theme, site_config: { htmlLabels: false }, svg: { diagram_id: diagramId },
      }),
    });
    parseSvg(svg);
    counts.svg_renders += 1;
    return svg;
  }
  async function renderDefinition(client, definition, family) {
    return renderTheme(client, family, { spec: await materialize(client, definition) });
  }

  const fresh = await module.createNodeEngine();
  try {
    const expectedPresets = JSON.parse(await readFile(new URL("preset-catalog.json", FIXTURES), "utf8"));
    for (const client of [engine, fresh]) {
      const catalog = JSON.parse(client.metadataJson("theme-catalog"));
      assert.equal(catalog.schema_version, 1);
      assert.equal(catalog.structured_spec_available, true);
      assert.deepEqual(catalog.presets, expectedPresets);
      counts.catalog_checks += 1;
    }
    const definitions = {};
    for (const name of ["light", "dark"]) {
      const readable = await readFile(new URL(`${name}.definition.json`, FIXTURES), "utf8");
      const definition = JSON.parse(readable);
      const expectedDefinition = (await readFile(
        new URL(`${name}.definition.canonical.json`, FIXTURES), "utf8",
      )).trim();
      const expectedSpec = (await readFile(
        new URL(`${name}.spec.canonical.json`, FIXTURES), "utf8",
      )).trim();
      assert.equal(fixtureCanonicalJson(definition), expectedDefinition, `${name} definition oracle`);
      const exported = JSON.stringify(definition, null, 2);
      assert.equal(fixtureCanonicalJson(JSON.parse(exported)), expectedDefinition);
      for (const client of [engine, fresh]) {
        assert.equal(fixtureCanonicalJson(await materialize(client, exported)), expectedSpec,
          `${name} installed materialization differs from the shared spec oracle`);
      }
      definitions[name] = exported;
      counts.shared_vectors += 1;
    }

    for (const family of Object.keys(SOURCES)) {
      const light = await renderDefinition(engine, definitions.light, family);
      const dark = await renderDefinition(engine, definitions.dark, family);
      assert.notEqual(light, dark, `${family} ignored light/dark tokens`);
      assert.equal(await renderDefinition(engine, definitions.light, family), light,
        `${family} leaked dark state into the next light render`);
      assert.equal(await renderDefinition(fresh, definitions.light, family), light,
        `${family} disagrees across independent engines`);
      if (family === "state") {
        assert.equal(stateFill(light), "#fef3c7");
        assert.equal(stateFill(dark), "#172554");
      }
      counts.family_isolation_checks += 1;
    }

    const edited = JSON.parse(definitions.light);
    edited.styles = [{
      kind: "rule", target: "state", family: "state", style: { fill: "#123abc" },
    }];
    assert.equal(stateFill(await renderDefinition(engine, JSON.stringify(edited), "state")), "#123abc");
    assert.equal(await renderDefinition(engine, JSON.stringify(edited), "sequence"),
      await renderDefinition(engine, definitions.light, "sequence"), "State rule leaked into Sequence");
    assert.equal(stateFill(await renderDefinition(engine, definitions.light, "state")), "#fef3c7");
    counts.rule_override_checks += 1;

    const cold = await module.createNodeEngine();
    try {
      const spec = { styles: [{
        kind: "rule", target: "state", family: "state", style: { fill: "#456def" },
      }] };
      assert.equal(stateFill(await renderTheme(cold, "state", { spec })), "#456def");
      counts.cold_spec_checks += 1;
    } finally {
      await cold.dispose();
    }

    for (const preset of ["brutalist", "spotless", "cyberpunk"]) {
      const exported = await executeJson(engine, "export-theme-preset-json", preset);
      assert.equal(exported.kind, "complete_spec");
      const spec = exported.complete_spec;
      assert.ok(!Object.hasOwn(spec, "assets") && !Object.hasOwn(spec, "mermaid"),
        "native preset must not embed fonts or Mermaid compatibility");
      const diagramId = `node-preset-${preset}`;
      assert.equal(await renderTheme(engine, "state", { preset }, diagramId),
        await renderTheme(fresh, "state", { spec }, diagramId), `${preset} export changed terminal SVG`);
      counts.preset_export_checks += 1;
    }

    const supportVectors = JSON.parse(await readFile(new URL("support.json", FIXTURES), "utf8"));
    for (const vector of supportVectors) {
      const request = {
        operationId: "describe-theme-support-json", source: JSON.stringify(vector.query),
      };
      for (const response of [
        await engine.executeOperation(request), engine.executeOperationSync(request),
      ]) {
        assert.equal(response.operation_id, request.operationId);
        assert.equal(response.media_type, "application/json");
        assert.deepEqual(JSON.parse(response.data), vector.expected, vector.id);
        counts.support_queries += 1;
      }
    }

    const errorVectors = JSON.parse(await readFile(new URL("errors.json", FIXTURES), "utf8"));
    for (const vector of errorVectors) {
      const invalid = {
        operationId: "materialize-theme-json", source: vector.source,
        optionsJson: vector.options_json,
      };
      const checkDiagnostic = (error) => {
        assert.ok(error instanceof module.MermanOperationError);
        return assertAuthoringDiagnostic(error, vector);
      };
      await assert.rejects(engine.executeOperation(invalid), checkDiagnostic);
      counts.authoring_diagnostics += 1;
      assert.throws(() => engine.executeOperationSync(invalid), checkDiagnostic);
      counts.authoring_diagnostics += 1;
    }

    const limitedRequest = {
      operationId: "materialize-theme-json",
      source: "{}",
      optionsJson: JSON.stringify({ resources: { limits: { max_theme_encoded_bytes: 1 } } }),
    };
    const checkResource = (error) => {
      assert.ok(error instanceof module.MermanOperationError);
      return assertResourceLimit(error);
    };
    await assert.rejects(engine.executeOperation(limitedRequest), checkResource);
    counts.resource_limit_checks += 1;
    assert.throws(() => engine.executeOperationSync(limitedRequest), checkResource);
    counts.resource_limit_checks += 1;
    return counts;
  } finally {
    await fresh.dispose();
  }
}
