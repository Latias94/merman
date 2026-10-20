import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, link, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { run } from "./theme-tool.mjs";

async function workspace(t) {
  const dir = await mkdtemp(path.join(os.tmpdir(), "merman-theme-tool-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  return (name) => path.join(dir, name);
}

function runtime(executeOperation) {
  const state = { calls: [], disposed: false };
  return {
    state,
    createEngine: async () => ({
      metadataJson: (id) => { state.calls.push(id); return '{"presets":[]}'; },
      executeOperation: async (request) => { state.calls.push(request); return executeOperation(request); },
      dispose: async () => { state.disposed = true; },
    }),
    stdout: () => {},
  };
}

test("help and invalid command arguments never initialize an engine", async () => {
  let output = "";
  const options = { createEngine: () => { throw Error("Unexpected engine load"); }, stdout: (text) => { output += text; } };
  await run(["--help"], options);
  assert.match(output, /materialize <design.json>/);
  for (const args of [[], ["future"], ["catalog", "extra"], ["render", "a", "b"], ["catalog", "--options", "a"], ["export", "light", "--unknown"]]) {
    await assert.rejects(run(args, options));
  }
});

test("materialization forwards raw design text and writes a separate inspection result", async (t) => {
  const file = await workspace(t);
  const raw = '{"tokens":{"ink":{"type":"color","value":"red","value":"blue"}}}';
  await writeFile(file("design.json"), raw);
  const engine = runtime(() => ({ media_type: "application/json", data: '{"references":[]}' }));
  await run(["materialize", file("design.json"), "--out", file("inspection.json")], engine);
  assert.deepEqual(engine.state.calls, [{ operationId: "materialize-theme-json", source: raw }]);
  assert.deepEqual(JSON.parse(await readFile(file("inspection.json"), "utf8")), { references: [] });
  assert.equal(engine.state.disposed, true);
  assert.equal(await readFile(file("design.json"), "utf8"), raw);
});

test("render retains exact theme text, source and options and writes same-execution metadata", async (t) => {
  const file = await workspace(t);
  const theme = '{"schema_version":1,"kind":"design","design":{"canvas":{"base":"red","base":"blue"}}}';
  const source = "flowchart LR\nA --> B";
  await writeFile(file("theme.json"), theme);
  await writeFile(file("diagram.mmd"), source);
  await writeFile(file("options.json"), '{"site_config":{"flowchart":{"look":"neo"}}}');
  const metadata = { theme_execution_evidence: { theme_status: "residual", target_status: "rejected" } };
  const engine = runtime(() => ({ media_type: "image/svg+xml", data: "<svg/>", metadata_json: JSON.stringify(metadata) }));
  await run(["render", file("theme.json"), file("diagram.mmd"), "--options", file("options.json"), "--out", file("preview.svg")], engine);
  assert.equal(engine.state.calls[0].operationId, "svg");
  assert.equal(engine.state.calls[0].source, source);
  assert.ok(engine.state.calls[0].optionsJson.endsWith(`"theme":${theme}}`));
  assert.deepEqual(JSON.parse(engine.state.calls[0].optionsJson).site_config, { flowchart: { look: "neo" } });
  assert.deepEqual(JSON.parse(await readFile(file("preview.metadata.json"), "utf8")), metadata);
  assert.equal(await readFile(file("preview.svg"), "utf8"), "<svg/>");
  assert.equal(await readFile(file("theme.json"), "utf8"), theme);
  assert.equal(engine.state.disposed, true);
});

test("existing sidecars and input aliases prevent publication even when other outputs are absent", async (t) => {
  const file = await workspace(t);
  await writeFile(file("theme.json"), "{}");
  await writeFile(file("source.mmd"), "flowchart LR\nA --> B");
  await writeFile(file("preview.metadata.json"), "existing");
  const engine = runtime(() => { throw Error("Must not render"); });
  await assert.rejects(run(["render", file("theme.json"), file("source.mmd"), "--out", file("preview.svg")], engine), /already exists/);
  await assert.rejects(readFile(file("preview.svg")), { code: "ENOENT" });
  await assert.rejects(run(["materialize", file("theme.json"), "--out", file("theme.json"), "--force"], engine), /collides/);
  await link(file("theme.json"), file("alias.json"));
  await assert.rejects(run(["materialize", file("theme.json"), "--out", file("alias.json"), "--force"], engine), /linked to an input/);
  assert.equal(engine.state.calls.length, 0);
});

test("render rejects a second theme owner and releases the engine after structured errors", async (t) => {
  const file = await workspace(t);
  await writeFile(file("theme.json"), "{}");
  await writeFile(file("source.mmd"), "flowchart LR\nA --> B");
  await writeFile(file("options.json"), '{"theme":null}');
  const failure = Object.assign(Error("Invalid reference"), { codeName: "MERMAN_INVALID_ARGUMENT", details: { theme_authoring: { diagnostics: [{ path: "/tokens/ink" }] } } });
  const engine = runtime(() => { throw failure; });
  await assert.rejects(run(["render", file("theme.json"), file("source.mmd"), "--out", file("preview.svg"), "--options", file("options.json")], engine), /without theme/);
  assert.equal(engine.state.calls.length, 0);
  await assert.rejects(run(["materialize", file("theme.json")], engine), (error) => error === failure);
  assert.equal(engine.state.disposed, true);
});

test("malformed standalone theme text cannot escape into the render options envelope", async (t) => {
  const file = await workspace(t);
  await writeFile(file("source.mmd"), "flowchart LR\nA --> B");
  const engine = runtime(() => { throw Error("Must not render"); });
  for (const suffix of [',"svg":{"pipeline":"resvg-safe"}', ',"site_config":{"flowchart":{"look":"classic"}}']) {
    await writeFile(file("theme.json"), '{"schema_version":1,"kind":"design","design":{}}' + suffix);
    await assert.rejects(run(["render", file("theme.json"), file("source.mmd"), "--out", file("preview.svg")], engine), SyntaxError);
  }
  assert.equal(engine.state.calls.length, 0);
  await assert.rejects(readFile(file("preview.svg")), { code: "ENOENT" });
});

test("force rejects hard-linked SVG and metadata outputs before either is changed", async (t) => {
  const file = await workspace(t);
  await writeFile(file("theme.json"), "{}");
  await writeFile(file("source.mmd"), "flowchart LR\nA --> B");
  await writeFile(file("preview.svg"), "previous output");
  await link(file("preview.svg"), file("preview.metadata.json"));
  const engine = runtime(() => { throw Error("Must not render"); });
  await assert.rejects(run(["render", file("theme.json"), file("source.mmd"), "--out", file("preview.svg"), "--force"], engine), /Outputs are linked/);
  assert.equal(await readFile(file("preview.svg"), "utf8"), "previous output");
  assert.equal(await readFile(file("preview.metadata.json"), "utf8"), "previous output");
  assert.equal(engine.state.calls.length, 0);
});
