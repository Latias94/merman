import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { measureScenario } from "./node_runner.mjs";

const cli = fileURLToPath(new URL("./node_runner.mjs", import.meta.url));
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const request = (source, diagram_id) => ({ source, diagram_id, fixture: "shared-fixture" });
const noWarmup = { max_calls: 0, max_ms: 0 };

test("import-to-first uses the first returned drawing and excludes later calls and output checks", async () => {
  const requests = [{}, {}];
  for (const outputClass of [undefined, "draw", "empty"]) {
    let time = 100;
    const options = {
      samples: 1,
      warmup: noWarmup,
      now: () => ++time,
      importStart: 95,
      validate(jobs) {
        time += 1000;
        return jobs.map(() => ({ status: "ok", output_class: outputClass }));
      },
    };
    const { cold_batch, warm_samples } = await measureScenario(requests, () => "<svg/>", options);
    assert.equal(cold_batch.init_through_first_svg_ms, outputClass === "empty" ? null : 8);
    assert.equal(Object.hasOwn(warm_samples[0], "init_through_first_svg_ms"), false);
  }
});

function project(t) {
  const scratch = fileURLToPath(new URL("../../../target/", import.meta.url));
  mkdirSync(scratch, { recursive: true });
  const dir = mkdtempSync(path.join(scratch, "workload-node-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

function install(dir, name, code, version = "1.2.3") {
  const pkg = path.join(dir, "node_modules", name);
  const entry = path.join(pkg, "dist", "public.mjs");
  mkdirSync(path.dirname(entry), { recursive: true });
  writeFileSync(path.join(pkg, "package.json"), JSON.stringify({
    name, version, exports: { ".": { import: "./dist/public.mjs" } },
  }));
  writeFileSync(entry, code);
  return entry;
}

function plan(dir, scenarios) {
  const raw = "\uFEFF" + JSON.stringify({
    schema_version: 1, suite: "test", scenarios, topologies: [{ source: "unused" }],
  }, null, 2).replaceAll("\n", "\r\n");
  writeFileSync(path.join(dir, "plan.json"), raw);
  return raw;
}

function run(dir, engine, extra = [], runner = cli) {
  const result = spawnSync(process.execPath, [runner, "--plan", "plan.json", "--engine", engine, ...extra], {
    cwd: dir, encoding: "utf8", timeout: 20_000,
  });
  if (result.error) throw result.error;
  assert.equal(result.stdout.trim().split("\n").length, 1, result.stderr);
  return { report: JSON.parse(result.stdout), status: result.status, stderr: result.stderr };
}

const drawModule = `
  let calls = 0;
  function draw(source, id) {
    console.log("call", ++calls, id);
    return '<svg id="' + id + '"><text>' + id + ' ' + Buffer.from(source).toString('base64') + ' 夜</text></svg>';
  }
  export function createNodeEngine() {
    console.log('create');
    return {
      runtimeCatalog: { package_version: '1.2.3', transport_api_version: 2 },
      renderSvgSync(source, { optionsJson }) { return draw(source, JSON.parse(optionsJson).svg.diagram_id); },
      dispose() { console.log('dispose'); },
    };
  }
  export function render(source, { idPrefix, now }) {
    if (now !== 0) throw new Error('unexpected Gantt clock');
    return { svg: draw(source, idPrefix) };
  }
`;

test("Merman forwards the frozen ID on every render stage and marks no-options calls ineligible", (t) => {
  const dir = project(t);
  const optionsJson = ' { "site_config": {"svg": {"string": "{\\\"}"}}, "\\u0073vg": { "pipeline": "readable" } } ';
  const explicit = (id) => optionsJson.replace('{ "pipeline"', `{"diagram_id":${JSON.stringify(id)}, "pipeline"`);
  const svg = (id) => `<svg class="test" id="${id}"><text>Drawing</text></svg>`;
  install(dir, "@mermanjs/node", `
    export function createNodeEngine({ bindingOptions }) {
      const layout = bindingOptions.site_config.layout;
      const sources = { first: 'fixed', second: 'rotating' };
      let draws = 0;
      let probes = 0;
      let mode;
      let wrongRequest = false;
      const expected = ${JSON.stringify({ first: explicit("fixed"), second: explicit("rotating") })};
      const matchesOptions = (source, opts) => {
        if (mode === 'none') return opts === undefined;
        if (!opts || Object.keys(opts).length !== 1) return false;
        return mode === 'direct' ? opts.diagramId === sources[source] : opts.optionsJson === expected[source];
      };
      return {
        runtimeCatalog: { package_version: "1.2.3" },
        renderSvgSync(source, opts) {
          draws++;
          mode ??= arguments.length === 1 ? 'none' : (opts?.diagramId === undefined ? 'explicit' : 'direct');
          wrongRequest ||= draws % 2 !== (source === 'first' ? 1 : 0) ||
              arguments.length !== (mode === 'none' ? 1 : 2) || !matchesOptions(source, opts);
          return ${JSON.stringify(svg("ROOT"))}.replace('ROOT', mode === 'none' ? 'merman' : sources[source]);
        },
        svgPlanJsonSync(source, opts) {
          const matchesPlanOptions = mode === 'direct' ? arguments.length === 1 : matchesOptions(source, opts);
          if (draws !== 8 || !sources[source] || !matchesPlanOptions) {
            throw new Error('wrong layout probe');
          }
          probes++;
          return JSON.stringify(source === 'second'
            ? { ready: true, diagram_type: 'sequence', required_capability_ids: ['svg'] }
            : { ready: true, diagram_type: 'flowchart-v2', required_capability_ids: ['layout-' + layout] });
        },
        dispose() {
          if (draws !== 8 || probes !== 2 || wrongRequest) throw new Error('wrong render or layout probe');
        },
      };
    }
  `);
  const requests = [request("first", "fixed"), request("second", "rotating")];
  plan(dir, [{ id: "doc", requests }]);
  for (const [label, text] of [["none", null], ["explicit", optionsJson], ["direct", null]]) {
    const direct = label === "direct";
    const omitted = label === "none";
    const args = ["--variant-label", label, "--site-layout", "shoal", "--samples", "2",
      "--warmup-calls", "2", "--warmup-ms", "10000"];
    if (text !== null) args.push(`--options-json=${text}`);
    if (direct) args.push("--direct-id");
    const { report, status, stderr } = run(dir, "merman", args);
    assert.equal(status, 0, stderr);
    assert.equal(report.engine, "merman");
    assert.deepEqual(report.variant, { label, options_json: text, site_layout: "shoal",
      ...(direct ? { direct_id: true } : {}) });
    assert.equal(report.diagram_id_mode, omitted ? "omitted" : "plan");
    const [row] = report.scenarios;
    assert.deepEqual(row.requests.map((r) => r.diagram_id), ["fixed", "rotating"]);
    assert.equal(row.warmup.calls, 2);
    assert.deepEqual(row.layout_plans, [
      { diagram_type: "flowchart-v2", required_capability_ids: ["layout-shoal"] },
      { diagram_type: "sequence", required_capability_ids: ["svg"] },
    ]);
    assert.equal(row.coverage.full_batch_comparable, !omitted);
    for (const batch of [row.cold_batch, ...row.warm_samples]) {
      if (omitted) assert.equal(batch.complete_batch_ms, null);
      for (const [i, result] of batch.results.entries()) {
        const id = omitted ? "merman" : requests[i].diagram_id;
        assert.equal(result.status, omitted ? "diagram-id-not-applied" : "ok");
        assert.equal(result.svg_root_id, id);
        assert.equal(result.output_sha256, sha256(svg(id)));
      }
    }
  }
  const conflict = run(dir, "merman", ["--variant-label", "conflict", '--options-json={"svg":{"diagram_id":"fixed"}}']);
  assert.equal(conflict.status, 1);
  assert.match(conflict.report.scenarios[0].error, /svg\.diagram_id conflicts with frozen diagram_id "rotating"/u);
  for (const [engine, args] of [["merman", []], ["pele", ["--variant-label", "peer"]],
    ["merman", ["--variant-label", "conflict", "--options-json={}"]]]) {
    const result = run(dir, engine, [...args, "--direct-id"]);
    assert.equal(result.status, 1);
    assert.match(result.report.error, /--direct-id requires a Merman variant without --options-json/u);
  }
});

test("Merman fills nullable SVG options and passes normalized-id inputs verbatim to the SDK", (t) => {
  const dir = project(t);
  const id = 'scope 雪: "well-formed"';
  install(dir, "@mermanjs/node", `
    export function createNodeEngine() { return {
      runtimeCatalog: { package_version: '1.2.3' },
      renderSvgSync(source, { optionsJson }) {
        if (JSON.parse(optionsJson).svg.diagram_id !== ${JSON.stringify(id)}) throw new Error('lost original ID');
        return '<svg id="scope"><text>Drawing</text></svg>';
      },
      dispose() {},
    }; }
  `);
  plan(dir, [{ id: "doc", requests: [request("source", id)] }]);
  for (const options of ['{"svg":null}', '{"svg":{"diagram_id":null}}']) {
    const { report, status, stderr } = run(dir, "merman", ["--variant-label", "matched", `--options-json=${options}`,
      "--samples", "1", "--warmup-calls", "0"]);
    assert.equal(status, 0, stderr);
    assert.equal(report.scenarios[0].coverage.full_batch_comparable, true);
  }
});

test("Merman requires an explicit layout capability and verifies the actual root ID after rendering", (t) => {
  const dir = project(t);
  const options = ' { "svg": { "diagram_id": "fixed" } } ';
  const trapSvg = `<svg data-ignored=' id="fixed"' id="wrong"><text>Drawing</text></svg>`;
  install(dir, "@mermanjs/node", `
    export function createNodeEngine() {
      return {
        runtimeCatalog: { package_version: '1.2.3', capabilities: { capability_ids: ['layout-dagre'] } },
        renderSvgSync(source, { optionsJson }) {
          const { svg: { diagram_id: id } } = JSON.parse(optionsJson);
          if (optionsJson !== ${JSON.stringify(options)}) throw new Error('rewritten explicit ID');
          if (source === 'wrong-root') return ${JSON.stringify(trapSvg)};
          if (source === 'missing-root') return '<svg><text id="' + id + '">Drawing</text></svg>';
          return '<svg xmlns="http://www.w3.org/2000/svg" id="' + id + '"><text>Drawing</text></svg>';
        },
        svgPlanJsonSync(source) {
          const required_capability_ids = source === 'silent' ? [] :
            (source === 'fixed-layout' ? ['layout-cytoscape'] : ['layout-dagre', 'math']);
          return JSON.stringify({ready: true, diagram_type: 'flowchart-v2', required_capability_ids});
        },
        dispose() {},
      };
    }
  `);
  const names = ["silent", "fixed-layout", "wrong-root", "missing-root", "literal"];
  plan(dir, names.map((name) => ({ id: name, requests: [request(name, "fixed")] })));
  const { report, status } = run(dir, "merman", ["--variant-label", "dagre", "--site-layout", "dagre",
    `--options-json=${options}`, "--samples", "1", "--warmup-calls", "0"]);
  assert.equal(status, 1);
  const [empty, fixed, wrong, missing, good] = report.scenarios;
  assert.match(empty.error, /requested dagre\. SVG plan reports no required capabilities/u);
  assert.match(fixed.error, /requested dagre\. SVG plan reports layout-cytoscape/u);
  for (const [row, id] of [[wrong, "wrong"], [missing, null]]) {
    assert.equal(row.warm_samples[0].complete_batch_ms, null);
    assert.equal(row.warm_samples[0].results[0].status, "diagram-id-mismatch");
    assert.equal(row.warm_samples[0].results[0].svg_root_id, id);
  }
  assert.equal(good.coverage.full_batch_comparable, true);
});

test("Merman site layouts admit fixed family capabilities without admitting selectable fallbacks", (t) => {
  const dir = project(t);
  install(dir, "@mermanjs/node", `
    export function createNodeEngine() {
      return {
        runtimeCatalog: { package_version: '1.2.3' },
        renderSvgSync(source, { diagramId }) {
          return '<svg id="' + diagramId + '"><text>Drawing</text></svg>';
        },
        svgPlanJsonSync(source) {
          const [diagram_type, ...required_capability_ids] = source.split(' ');
          return JSON.stringify({ ready: true, diagram_type, required_capability_ids });
        },
        dispose() {},
      };
    }
  `);
  for (const layout of ["dagre", "elk", "shoal"]) {
    const selected = `layout-${layout}`;
    const shoal = layout === "shoal";
    const accepted = [
      ["info", "math"], ["gitGraph"], ["railroadAbnf"],
      ["architecture", shoal ? selected : "layout-cytoscape"],
      ["block", ...(shoal ? [selected] : [])], ["c4", ...(shoal ? [selected] : [])],
      ["swimlane", selected], ["classDiagram", selected],
    ];
    const rejected = layout === "elk" ? ["swimlane", "layout-dagre"]
      : ["architecture", shoal ? "layout-cytoscape" : selected];
    const probe = ([type, ...caps]) => request([type, ...caps].join(" "), type);
    plan(dir, [
      { id: "accepted", requests: accepted.map(probe) },
      { id: "rejected", requests: [probe(rejected)] },
    ]);
    const { report, status } = run(dir, "merman", ["--variant-label", layout, "--site-layout", layout,
      "--direct-id", "--samples", "1", "--warmup-calls", "0"]);
    assert.equal(status, 1);
    const [good, failure] = report.scenarios;
    assert.equal(good.coverage?.full_batch_comparable, true, good.error);
    assert.deepEqual(good.layout_plans, accepted.map(([diagram_type, ...required_capability_ids]) =>
      ({ diagram_type, required_capability_ids })));
    const [type, capability] = rejected;
    assert.equal(failure.status, "failed");
    assert.equal(failure.error, `Scenario 1 (rejected): ${type}: requested ${layout}. SVG plan reports ${capability}.`);
  }
});

test("public entries isolate duplicate scenarios, reuse one engine and preserve source bytes and IDs", (t) => {
  const dir = project(t);
  install(dir, "@mermanjs/node", drawModule);
  install(dir, "pele", drawModule);
  const source = "---\r\nconfig:\r\n  theme: dark\r\n---\r\nflowchart LR\r\nA[夜] --> B\r\n";
  const requests = [request(source, "doc-1"), request("\uFEFFsequenceDiagram\nA->>B: 🚀\n", "doc-2")];
  requests[0].source_sha256 = sha256(source);
  const raw = plan(dir, [{ id: "doc", requests }, { id: "doc", requests: [requests[0]] }]);

  for (const [engine, name, backend] of [["merman", "@mermanjs/node", "napi"], ["pele", "pele", "javascript"]]) {
    const { report, status, stderr } = run(dir, engine, ["--samples", "2", "--warmup-calls", "3", "--warmup-ms", "10000"]);
    assert.equal(status, 0, stderr);
    assert.equal(report.package.name, name);
    assert.equal(report.package.version, "1.2.3");
    assert.equal(report.backend, backend);
    const identity = report.artifact_identity;
    const entry = readFileSync(report.package.entry);
    assert.equal(identity.scope, engine === "merman" ? "node-entry-and-transport" : "node-entry");
    assert.equal(identity.status, engine === "merman" ? "unknown" : "recorded");
    assert.deepEqual(identity.files, [{ role: "entry", path: report.package.entry, bytes: entry.length, sha256: sha256(entry) }]);
    assert.equal(report.surface, "node");
    assert.equal(report.output_format, "svg");
    assert.equal(report.host.node, process.version);
    assert.equal(report.plan_sha256, sha256(raw));
    assert.deepEqual(report.warmup, { max_calls: 3, max_ms: 10000 });
    assert.deepEqual(report.scenarios.map((row) => [row.index, row.id]), [[0, "doc"], [1, "doc"]]);
    assert.deepEqual(stderr.match(/call \d+ doc-\d/g), [
      "call 1 doc-1", "call 2 doc-2",
      "call 3 doc-1", "call 4 doc-2", "call 5 doc-1",
      "call 6 doc-1", "call 7 doc-2", "call 8 doc-1", "call 9 doc-2",
      "call 1 doc-1", "call 2 doc-1", "call 3 doc-1", "call 4 doc-1", "call 5 doc-1", "call 6 doc-1",
    ]);
    if (engine === "merman") {
      assert.equal(stderr.match(/create/g).length, 2);
      assert.equal(stderr.match(/dispose/g).length, 2);
    }
    for (const row of report.scenarios) {
      assert.ok(row.init_ms >= 0);
      assert.equal(row.first_render_ms, row.cold_batch.results[0].elapsed_ms);
      assert.equal(row.warmup.calls, 3);
      assert.ok(row.warmup.elapsed_ms >= 0);
      assert.equal(row.warm_samples.length, 2);
      assert.equal(row.coverage.full_batch_comparable, true);
      for (const batch of [row.cold_batch, ...row.warm_samples]) {
        assert.equal(batch.elapsed_ms, batch.complete_batch_ms);
        assert.ok(batch.elapsed_ms >= batch.successful_render_ms);
        for (const [i, result] of batch.results.entries()) {
          const { fixture, source, diagram_id } = requests[i];
          assert.deepEqual(row.requests[i], { fixture, diagram_id, source_sha256: sha256(source), expected_outcome: "draw" });
          const svg = `<svg id="${diagram_id}"><text>${diagram_id} ${Buffer.from(source).toString("base64")} 夜</text></svg>`;
          assert.equal(result.status, "ok");
          assert.equal(result.output_bytes, Buffer.byteLength(svg));
          assert.equal(result.output_sha256, sha256(svg));
        }
      }
    }
  }
});

test("Mermaid v13 awaits the public Node render in request order and retains a rejected request", (t) => {
  const dir = project(t);
  const entry = install(dir, "mermaid", `
    let active = false;
    export default { async render(id, source) {
      if (active) throw new Error('concurrent renders');
      active = true;
      await new Promise((done) => setImmediate(done));
      active = false;
      if (source === 'broken') throw Object.assign(new Error('invalid diagram'), {code: 'parse'});
      return { svg: '<svg><text>' + id + '</text><text>' + source + '</text></svg>' };
    } };
  `, "13.0.0-alpha.3");
  const sources = ["first", "broken", "last"];
  plan(dir, [{ id: "edit", requests: sources.map((s, i) => ({ ...request(s, `edit-${i}`), expected_labels: [s] })) }]);
  const { report, status, stderr } = run(dir, "mermaid-v13", ["--samples", "2"]);
  assert.equal(status, 0, stderr);
  assert.equal(report.render_api, "mermaid.render");
  assert.equal(report.package.version, "13.0.0-alpha.3");
  assert.ok(report.node_project.cwd);
  const [row] = report.scenarios;
  for (const batch of [row.cold_batch, ...row.warm_samples]) {
    assert.deepEqual(batch.results.map((r) => r.status), ["ok", "render-error", "ok"]);
    assert.equal(batch.results[1].code, "parse");
    assert.equal(batch.results[2].output_sha256, sha256("<svg><text>edit-2</text><text>last</text></svg>"));
  }
  const browser = path.join(path.dirname(entry), "browser.mjs");
  writeFileSync(browser, "throw new Error('unexpected import');");
  assert.match(run(dir, "mermaid-v13", ["--module", browser]).report.error, /public Node export/);
  install(dir, "mermaid", "throw new Error('unexpected import');", "12.2.0");
  assert.match(run(dir, "mermaid-v13").report.error, /Expected mermaid 13\.x/);
});

test("drawing files retain the last timed UTF-8 output at declared positions in isolated workers", (t) => {
  const dir = project(t);
  const relative = path.join("bundle", "drawings", "mermaid-v13");
  const drawings = path.join(dir, relative);
  const svg = "\uFEFF<svg>\r\n  <text>夜🚀 e\u0301 BATCH</text>\r\n</svg>\r\n";
  const selected = { ...request(svg, "../shared"), drawing_facts: {} };
  const lostOutput = { ...request("loses output", "lost"), drawing_facts: {} };
  plan(dir, [
    { id: "doc/repeated", requests: [request(svg, "unchecked"), { ...selected, expected_labels: ["absent"] }, lostOutput] },
    { id: "doc/repeated", requests: [{ ...selected, drawing_facts: null }] },
  ]);
  install(dir, "mermaid", `
    import { existsSync } from 'node:fs';
    process.chdir('node_modules');
    const calls = new Map();
    export default { async render(id, source) {
      const call = (calls.get(id) ?? 0) + 1;
      calls.set(id, call);
      console.log('render', id, call);
      if (source === 'loses output') {
        if (existsSync(${JSON.stringify(path.join(drawings, "0-1.svg"))})) throw new Error('drawing already visible during render');
        return { svg: call === 3 ? null : '<svg><rect/></svg>' };
      }
      return { svg: source.replace('BATCH', String(call)) };
    } };
  `, "13.0.0-alpha.3");

  const { report, status, stderr } = run(dir, "mermaid-v13", [
    "--drawing-dir", relative, "--samples", "2", "--warmup-calls", "2", "--warmup-ms", "10000",
  ]);
  assert.equal(status, 0, stderr);
  assert.equal(stderr.match(/^render /gm)?.length, 16);
  assert.ok(path.isAbsolute(report.drawing_dir));
  assert.equal(realpathSync(report.drawing_dir), realpathSync(drawings));
  assert.deepEqual(readdirSync(drawings).sort(), ["0-1.svg", "1-0.svg"]);
  const [first, second] = report.scenarios;
  for (const [row, index, basename, lastCall] of [[first, 1, "0-1.svg", "4"], [second, 0, "1-0.svg", "5"]]) {
    assert.equal(row.requests[index].drawing_artifact, basename);
    const bytes = readFileSync(path.join(drawings, basename));
    assert.deepEqual(bytes, Buffer.from(svg.replace("BATCH", lastCall), "utf8"));
    const last = row.warm_samples.at(-1).results[index];
    assert.equal(last.output_sha256, sha256(bytes));
    assert.equal(last.output_bytes, bytes.length);
    assert.notEqual(row.warm_samples[0].results[index].output_sha256, last.output_sha256);
  }
  assert.equal(first.warm_samples.at(-1).results[1].status, "missing-labels");
  assert.equal(first.cold_batch.results[2].status, "ok");
  const lost = first.warm_samples.at(-1).results[2];
  assert.equal(lost.status, "invalid-svg", lost.message);
  assert.equal(lost.output_sha256, null);
  assert.equal(Object.hasOwn(first.requests[0], "drawing_artifact"), false);
  assert.equal(Object.hasOwn(first.requests[2], "drawing_artifact"), false);
});

test("drawing capture is opt-in and a failed exclusive write fails its scenario", (t) => {
  const dir = project(t);
  plan(dir, [
    { id: "old", requests: [request("old", "a")] },
    { id: "selected", requests: [{ ...request("new", "b"), drawing_facts: {} }] },
  ]);
  install(dir, "pele", drawModule);
  const off = run(dir, "pele", ["--samples", "1"]);
  assert.equal(off.status, 0, off.stderr);
  assert.equal(Object.hasOwn(off.report, "drawing_dir"), false);
  assert.deepEqual(off.report.scenarios[1].requests[0], {
    fixture: "shared-fixture", diagram_id: "b", source_sha256: sha256("new"), expected_outcome: "draw", drawing_required: true,
  });

  const all = run(dir, "pele", ["--samples", "1", "--drawing-dir", "all", "--all-drawings"]);
  assert.equal(all.status, 0, all.stderr);
  for (const [index, row] of all.report.scenarios.entries()) {
    const [request] = row.requests;
    assert.equal(request.drawing_artifact, `${index}-0.svg`);
    assert.equal(Object.hasOwn(request, "drawing_required"), index === 1);
    const svg = readFileSync(path.join(dir, "all", request.drawing_artifact));
    assert.equal(sha256(svg), row.warm_samples.at(-1).results[0].output_sha256);
  }
  assert.match(run(dir, "pele", ["--all-drawings"]).report.error, /requires --drawing-dir/);

  const drawings = path.join(dir, "drawings");
  mkdirSync(drawings);
  const existing = path.join(drawings, "1-0.svg");
  writeFileSync(existing, "previous report");
  for (const destination of [drawings, existing]) {
    const { report, status, stderr } = run(dir, "pele", ["--samples", "1", "--drawing-dir", destination]);
    assert.equal(status, 1, stderr);
    assert.equal(report.status, "failed");
    assert.deepEqual(report.scenarios.map((row) => row.index), [0, 1]);
    const failed = report.scenarios[1];
    assert.equal(failed.status, "failed");
    assert.equal(failed.requests[0].drawing_required, true);
    assert.equal(Object.hasOwn(failed.requests[0], "drawing_artifact"), false);
    assert.equal(Object.hasOwn(report.scenarios[0].requests[0], "drawing_artifact"), false);
    assert.ok(failed.error.includes(existing), failed.error);
    assert.equal(readFileSync(existing, "utf8"), "previous report");
  }
});

test("partial batches keep attempted time, error SVG identity and stable successful coverage by index", (t) => {
  const dir = project(t);
  const cases = ["role", "ok", "blank", "shell", "defs", "throw", "marker", "unstable", "flaky", "html", "malformed"];
  plan(dir, [{ id: "mixed", requests: cases.map((source, i) => request(source, `id-${i}`)) }]);
  const entry = install(dir, "pele", `
    const calls = new Map();
    export function render(source) {
      const n = (calls.get(source) ?? 0) + 1;
      calls.set(source, n);
      if (source === 'throw' || (source === 'flaky' && n === 2)) {
        throw Object.assign(new Error('bad input'), { code: 'syntax' });
      }
      const outputs = {
        role: '<svg aria-roledescription="error"><path d="M0 0"/></svg>',
        ok: '<svg><style>.error-text{fill:red}</style><text data-class="error-text">Syntax error in text</text></svg>',
        blank: '', shell: '<svg/>', defs: '<svg><defs><path d="M0 0"/></defs><!-- <text/> --><g/></svg>',
        marker: '<svg><text class="error-text">Syntax error in text</text></svg>',
        unstable: '<svg><text>' + n + '</text></svg>', flaky: '<svg><rect/></svg>', html: '<p>error</p>',
        malformed: '<svg><g><rect/></svg>',
      };
      return { svg: outputs[source] };
    }
  `);
  const { report, status, stderr } = run(dir, "pele", [
    "--module", pathToFileURL(entry).href, "--samples", "2", "--warmup-calls", "0",
  ]);
  assert.equal(status, 0, stderr);
  const [row] = report.scenarios;
  assert.equal(row.first_render_ms, null);
  assert.deepEqual(row.coverage, {
    planned_requests: 11, label_reader: null, expected_label_request_indices: [], verified_label_request_indices: [],
    successful_request_indices: [1, 7], expected_rejection_request_indices: [], comparable_request_indices: [1], full_batch_comparable: false,
  });
  const first = row.cold_batch.results;
  assert.deepEqual(first.map((item) => item.status), [
    "error-svg", "ok", "empty-svg", "empty-svg", "empty-svg", "render-error", "error-svg", "ok", "ok", "invalid-svg", "invalid-svg",
  ]);
  assert.equal(first[0].output_sha256, sha256('<svg aria-roledescription="error"><path d="M0 0"/></svg>'));
  assert.equal(first[2].output_bytes, 0);
  assert.equal(first[5].output_sha256, null);
  assert.equal(first[5].code, "syntax");
  assert.equal(first[5].message, "bad input");
  assert.match(first[10].message, /mismatched tag/);
  assert.equal(row.warm_samples[0].results[8].status, "render-error");
  for (const batch of [row.cold_batch, ...row.warm_samples]) {
    const ok = batch.results.filter((item) => item.status === "ok");
    assert.equal(batch.complete_batch_ms, null);
    assert.equal(batch.successful_requests, ok.length);
    assert.equal(batch.successful_output_bytes, ok.reduce((sum, item) => sum + item.output_bytes, 0));
    assert.equal(batch.successful_render_ms, ok.reduce((sum, item) => sum + item.elapsed_ms, 0));
  }
});

test("a word wrap on a shared SVG text baseline anchor preserves the label", (t) => {
  const dir = project(t);
  const samples = [
    '<text><tspan x="0" y="-3.43">Inventory</tspan><tspan x="0" y="12.67">allocation</tspan></text>',
    '<text><tspan x="0" y="0">Inventory</tspan><tspan x="0" y="0">allocation</tspan></text>',
    '<text x="0" y="0">Inventory</text><text x="0" y="12.67">allocation</text>',
  ];
  plan(dir, [{ id: "wrap", requests: samples.map((s, i) => ({...request(s, `wrap-${i}`), expected_labels: ["Inventory allocation"]})) }]);
  install(dir, "pele", "export const render = (source) => ({svg: '<svg>' + source + '</svg>'});");
  const { report } = run(dir, "pele", ["--samples", "1"]);
  assert.deepEqual(report.scenarios[0].cold_batch.results.map((r) => r.status), ["ok", "missing-labels", "missing-labels"]);
});

test("the public reader classifies expected rejections, allowed empty SVGs and unreviewed output", (t) => {
  const dir = project(t);
  const diagnostic = "Diagram parse error (flowchart-v2): unexpected end of input; expected node identifier, edge label";
  const sources = [
    ["syntax", "reject"], ["resource", "reject"], ["valid", "reject"], ["error-svg", "reject"],
    ["changing", "reject"], ["empty", "allow-empty"], ["valid", "allow-empty"], ["empty", "draw"],
    ["empty", "unreviewed"], ["valid", "draw"],
  ];
  plan(dir, [{ id: "outcomes", requests: sources.map(([source, expected_outcome], i) => ({
    ...request(source, `r${i}`), expected_outcome,
  })) }]);
  install(dir, "pele", `
    let calls = 0;
    export function render(source) {
      if (source === 'syntax' || (source === 'changing' && ++calls === 1)) {
        throw Object.assign(new Error(${JSON.stringify(diagnostic)}), { codeName: 'MERMAN_PARSE_ERROR', code: 99 });
      }
      if (source === 'resource' || source === 'changing') throw Object.assign(new Error('limit'), { code: 'RESOURCE' });
      if (source === 'error-svg') return { svg: '<svg aria-roledescription="error"><path/></svg>' };
      if (source === 'empty') return { svg: '<svg viewBox="0 0 12 12"/>' };
      return { svg: '<svg><rect/></svg>' };
    }
  `);
  const { report, status, stderr } = run(dir, "pele", ["--samples", "2"]);
  assert.equal(status, 0, stderr);
  const [row] = report.scenarios;
  assert.deepEqual(row.requests.map((r) => r.expected_outcome), sources.map(([, outcome]) => outcome));
  assert.equal(row.first_render_ms, null);
  assert.deepEqual(row.coverage.expected_rejection_request_indices, [0]);
  assert.deepEqual(row.coverage.successful_request_indices, [5, 6, 8, 9]);
  assert.deepEqual(row.coverage.comparable_request_indices, [5, 6, 9]);
  assert.equal(row.coverage.full_batch_comparable, false);
  for (const batch of [row.cold_batch, ...row.warm_samples]) {
    const statuses = ["expected-rejection", "render-error", "unexpected-success", "error-svg",
      batch === row.cold_batch ? "expected-rejection" : "render-error", "ok", "ok", "empty-svg", "ok", "ok"];
    assert.deepEqual(batch.results.map((r) => r.status), statuses);
    assert.equal(batch.results[0].code, "MERMAN_PARSE_ERROR");
    assert.equal(batch.results[0].message, diagnostic);
    assert.equal(batch.results[0].output_sha256, null);
    assert.equal(batch.results[1].code, "RESOURCE");
    assert.deepEqual(batch.results.slice(5, 7).map((r) => r.output_class), ["empty", "draw"]);
    assert.equal(batch.expected_rejections, batch === row.cold_batch ? 2 : 1);
    assert.equal(batch.successful_requests, 4);
    assert.equal(batch.complete_batch_ms, null);
  }
});

test("expected labels must appear as SVG or HTML label text, including on a changed warm output", (t) => {
  const dir = project(t);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" id="IdOnly"><style>.CssOnly{fill:red}</style>
    <defs><text>DefsOnly</text></defs><title>TitleOnly</title><!-- CommentOnly -->
    <x:text xmlns:x="urn:not-svg">WrongNamespace</x:text>
    <g><text><tspan>St</tspan><tspan>art</tspan></text><text>Other</text>
    <foreignObject><div xmlns="http://www.w3.org/1999/xhtml" data-id="AttributeOnly">
      <style>.InnerCss{color:blue}</style><p>End &amp; <strong>夜</strong></p><p>Second line</p>
    </div></foreignObject></g></svg>`;
  const start = ["Begin", " Start "];
  const absent = ["IdOnly", "CssOnly", "DefsOnly", "TitleOnly", "CommentOnly", "AttributeOnly", "InnerCss", "WrongNamespace", ["tart", "StartOther"]];
  const sources = [
    { ...request("good", "g"), expected_labels: [start, ["End & 夜", "Finish"], "Second line"] },
    { ...request("missing", "m"), expected_labels: absent },
    { ...request("changes", "c"), expected_labels: [start] },
    { ...request("broken", "b"), expected_labels: ["Start"] },
    request("unchecked", "u"),
  ];
  plan(dir, [{ id: "labels", requests: sources }]);
  install(dir, "pele", `
    const svg = ${JSON.stringify(svg)};
    let changes = 0;
    export function render(source) {
      if (source === 'broken') return {svg: '<svg><text>Start</svg>'};
      if (source === 'changes' && changes++) return {svg: svg.replace('art</tspan>', 'op</tspan>')};
      return {svg};
    }
  `);
  const { report, status, stderr } = run(dir, "pele", ["--samples", "2"]);
  assert.equal(status, 0, stderr);
  const [row] = report.scenarios;
  assert.deepEqual(row.coverage.expected_label_request_indices, [0, 1, 2, 3]);
  assert.deepEqual(row.coverage.verified_label_request_indices, [0]);
  assert.deepEqual(row.coverage.comparable_request_indices, [0, 4]);
  assert.deepEqual(row.requests[0].expected_labels, sources[0].expected_labels);
  assert.deepEqual(row.cold_batch.results.map((result) => result.status), ["ok", "missing-labels", "ok", "invalid-svg", "ok"]);
  assert.deepEqual(row.cold_batch.results[1].missing_labels, absent);
  assert.deepEqual(row.warm_samples[0].results[2].missing_labels, [start]);
  assert.equal(row.warm_samples[1].complete_batch_ms, null);
});

test("all batch clocks stop before byte counts, hashing and error inspection", async (t) => {
  const events = [];
  const hashPrototype = Object.getPrototypeOf(createHash("sha256"));
  const originalUpdate = hashPrototype.update;
  t.mock.method(hashPrototype, "update", function (...args) {
    events.push("hash");
    return originalUpdate.apply(this, args);
  });
  const originalByteLength = Buffer.byteLength;
  t.mock.method(Buffer, "byteLength", (...args) => { events.push("bytes"); return originalByteLength(...args); });
  const error = { get message() { events.push("message"); return "broken"; } };
  let ticks = 0;
  const requests = [{ index: 1, expected_labels: ["diagram"] }, { index: 2 }, { index: 3 }];
  const { cold_batch, warmup, warm_samples } = await measureScenario(requests, ({ index }) => {
    events.push(`render-${index}`);
    if (index === 2) throw error;
    return index === 1 ? "<svg><text>diagram</text></svg>" : "<svg><rect/></svg>";
  }, {
    samples: 1,
    warmup: noWarmup,
    now() { events.push("clock"); return ++ticks; },
    validate(jobs) { events.push(`validate-${jobs.length}`); return jobs.map(() => ({ status: "ok" })); },
    afterLastBatch() { events.push("drawing"); },
  });
  const measured = ["clock", "clock", "render-1", "clock", "clock", "render-2", "clock", "clock", "render-3", "clock", "clock"];
  assert.deepEqual(events.slice(0, measured.length * 2), [...measured, ...measured]);
  const validation = events.slice(measured.length * 2, -2);
  assert.deepEqual([...new Set(validation)].sort(), ["bytes", "hash", "message"]);
  assert.deepEqual(events.slice(-2), ["validate-2", "drawing"]);
  assert.deepEqual(warmup, { calls: 0, elapsed_ms: 0 });
  for (const batch of [cold_batch, ...warm_samples]) {
    assert.equal(batch.elapsed_ms, 7);
    assert.equal(batch.successful_render_ms, 2);
  }
});

test("default warmup awaits calls in request order and stops at the first budget", async () => {
  const requests = ["a", "b", "c"];
  for (const [callMs, warmupCalls] of [[1, 20], [80, 2]]) {
    let time = 0;
    let active = false;
    const order = [];
    const svg = (id) => `<svg><text>${id}</text></svg>`;
    const measured = [...requests, ...requests, ...requests];
    const expectedOrder = [...requests, ...Array.from({ length: warmupCalls }, (_, i) => requests[i % requests.length]),
      ...requests, ...requests];
    const report = await measureScenario(requests, async (id) => {
      assert.equal(active, false);
      active = true;
      await Promise.resolve();
      active = false;
      time += callMs;
      const call = order.push(id);
      if (call === requests.length + 1) throw new Error("warmup failure");
      return { svg: call > requests.length && call <= requests.length + warmupCalls ? svg(`warmup-${id}`) : svg(id) };
    }, {
      samples: 2,
      asyncSvg: true,
      now: () => time,
      validate(jobs) {
        assert.deepEqual(order, expectedOrder);
        assert.deepEqual(jobs.map((job) => job.svg), requests.map(svg));
        order.push("validate");
        return jobs.map(() => ({ status: "ok" }));
      },
    });
    assert.deepEqual(order, [...expectedOrder, "validate"]);
    assert.deepEqual(report.warmup, { calls: warmupCalls, elapsed_ms: warmupCalls * callMs });
    const batches = [report.cold_batch, ...report.warm_samples];
    assert.deepEqual(batches.flatMap((batch) => batch.results.map((result) => result.output_sha256)),
      measured.map((id) => sha256(svg(id))));
    assert.deepEqual(batches.map((batch) => batch.complete_batch_ms), [3, 3, 3].map((calls) => calls * callMs));
  }
});

test("one deferred batch checks distinct outputs, label and outcome policies, and diagnostics", async () => {
  const requests = [
    { index: 0 }, { index: 1, expected_labels: ["B"] }, { index: 2 },
    { index: 3, expected_outcome: "allow-empty" }, { index: 4, expected_outcome: "reject" },
  ];
  const seen = [];
  const a = "<svg><text>A</text></svg>", c = "<svg><text>C</text></svg>";
  const outputs = [[a, a, a, a], [a, c, c, a], [c, a, a, a]];
  let calls = 0;
  const { cold_batch, warm_samples } = await measureScenario(requests, ({ index }) => {
    const sample = Math.floor(calls++ / requests.length);
    if (index === 4) throw Object.assign(new Error("invalid diagram"), { code: "syntax" });
    return outputs[sample][index];
  }, {
    samples: 2,
    warmup: noWarmup,
    now: () => 0,
    validate(jobs) {
      assert.equal(calls, outputs.length * requests.length);
      seen.push(jobs);
      return jobs.map(({ error, expected_labels }) => ({ status: error ? "expected-rejection" :
        expected_labels ? "missing-labels" : "ok" }));
    },
  });
  for (const batch of [cold_batch, ...warm_samples]) {
    assert.deepEqual(batch.results.map((result) => result.status), ["ok", "missing-labels", "ok", "ok", "expected-rejection"]);
    assert.equal(batch.expected_rejections, 1);
  }
  assert.deepEqual(seen, [[
    { svg: a, expected_labels: undefined, expected_outcome: "draw" },
    { svg: a, expected_labels: ["B"], expected_outcome: "draw" },
    { svg: a, expected_labels: undefined, expected_outcome: "allow-empty" },
    { error: { code: "syntax", message: "invalid diagram" }, expected_outcome: "reject" },
    { svg: c, expected_labels: ["B"], expected_outcome: "draw" },
    { svg: c, expected_labels: undefined, expected_outcome: "draw" },
  ]]);
});

test("spilled SVGs are checked after sampling and reused across validation batches", async (t) => {
  const validationDirectory = project(t);
  const a = "<svg><rect/></svg>";
  const large = `<svg><text>${"large".repeat(20)}</text></svg>`;
  const c = "<svg><circle/></svg>";
  const outputs = [a, large, c, a];
  const seen = [];
  let calls = 0;
  const report = await measureScenario([{}], () => {
    assert.deepEqual(readdirSync(validationDirectory), []);
    return outputs[calls++];
  }, {
    samples: 3,
    warmup: noWarmup,
    now: () => 0,
    validationDirectory,
    validationBatchBytes: 32,
    validate(jobs) {
      assert.equal(calls, outputs.length);
      seen.push(jobs.map((job) => job.svg));
      return jobs.map(({ svg }) => svg === large ? { status: "missing-labels" } : { status: "ok" });
    },
    afterLastBatch(attempts, results) {
      assert.equal(attempts[0].svg, a);
      assert.equal(results[0].status, "ok");
    },
  });
  assert.deepEqual(seen, [[a], [large], [c]]);
  assert.deepEqual(readdirSync(validationDirectory).sort(), ["0.json", "1.json"]);
  const batches = [report.cold_batch, ...report.warm_samples];
  assert.deepEqual(batches.map((batch) => batch.results[0].output_sha256), outputs.map(sha256));
  assert.deepEqual(batches.map((batch) => batch.results[0].status), ["ok", "missing-labels", "ok", "ok"]);
});

test("SVGs with the same UTF-8 digest still get separate checks when their JSON strings differ", async () => {
  const outputs = ["<svg><text>\ufffd</text></svg>", "<svg><text>\ud800</text></svg>"];
  const report = await measureScenario(outputs, (svg) => svg, {
    samples: 1,
    warmup: noWarmup,
    now: () => 0,
    validate(jobs) {
      assert.deepEqual(jobs.map((job) => job.svg), outputs);
      return [{ status: "ok" }, { status: "invalid-svg" }];
    },
  });
  assert.equal(sha256(outputs[0]), sha256(outputs[1]));
  assert.deepEqual(report.cold_batch.results.map((result) => result.status), ["ok", "invalid-svg"]);
});

test("collector fingerprints bind the configured warmup policy", (t) => {
  const dir = project(t);
  install(dir, "pele", "throw new Error('unexpected import');");
  plan(dir, []);
  const baseline = run(dir, "pele");
  const explicit = run(dir, "pele", ["--warmup-calls", "20", "--warmup-ms", "150"]);
  const disabled = run(dir, "pele", ["--warmup-ms", "0"]);
  for (const { status, stderr } of [baseline, explicit, disabled]) assert.equal(status, 0, stderr);
  assert.deepEqual(baseline.report.warmup, { max_calls: 20, max_ms: 150 });
  assert.deepEqual(disabled.report.warmup, { max_calls: 20, max_ms: 0 });
  assert.equal(explicit.report.collector_sha256, baseline.report.collector_sha256);
  assert.notEqual(disabled.report.collector_sha256, baseline.report.collector_sha256);
});

test("mid-plan request exceptions, process exits and timeouts preserve every independent worker", (t) => {
  const dir = project(t);
  const modes = ["before", "throw", "exit", "hang", "after"];
  const raw = plan(dir, modes.map((mode) => ({ id: mode, requests: [
    request("ok", "first"), { ...request(mode, "middle"), expected_labels: ["pass"] }, request("ok", "last"),
  ] })));
  install(dir, "pele", `export function render(source) {
    if (source === 'throw') throw new Error('render failed');
    if (source === 'exit') process.exit(73);
    if (source === 'hang') Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
    return {svg: '<svg><text>pass</text></svg>'};
  }`);
  const { report, status, stderr } = run(dir, "pele", ["--samples", "1", "--timeout-seconds", "2"]);
  assert.equal(status, 1, stderr);
  assert.equal(report.status, "failed");
  assert.equal(report.plan_sha256, sha256(raw));
  assert.equal(report.host.node, process.version);
  assert.equal(report.timeout_seconds, 2);
  assert.equal(report.planned_scenarios, modes.length);
  assert.deepEqual(report.scenarios.map((row) => row.id), modes);
  const [before, thrown, exited, timeout, after] = report.scenarios;
  assert.ok(before.coverage.full_batch_comparable && after.coverage.full_batch_comparable);
  for (const batch of [thrown.cold_batch, ...thrown.warm_samples]) {
    assert.deepEqual(batch.results.map((r) => r.status), ["ok", "render-error", "ok"]);
    assert.equal(batch.complete_batch_ms, null);
  }
  for (const [row, mode, workerStatus, message] of [[exited, "exit", "failed", /exit 73/], [timeout, "hang", "timeout", /timed out/]]) {
    assert.equal(row.index, modes.indexOf(mode));
    assert.equal(row.status, workerStatus);
    assert.match(row.error, message);
    assert.ok(stderr.includes(row.error));
    assert.deepEqual(row.requests.map((r) => r.diagram_id), ["first", "middle", "last"]);
    assert.deepEqual(row.requests[1], { fixture: "shared-fixture", diagram_id: "middle", source_sha256: sha256(mode),
      expected_labels: ["pass"], expected_outcome: "draw" });
    assert.equal(row.init_ms, null);
    assert.equal(row.unmeasured_requests, 3);
    assert.equal(row.first_render_ms, null);
    assert.equal(row.cold_batch, null);
    assert.deepEqual(row.warm_samples, []);
  }
});

function validatorScenario(t, mode) {
  const dir = project(t);
  const runner = path.join(dir, "collectors", "node_runner.mjs");
  mkdirSync(path.dirname(runner));
  mkdirSync(path.join(dir, "merman_bench"));
  copyFileSync(cli, runner);
  copyFileSync(new URL("../workloads.json", import.meta.url), path.join(dir, "workloads.json"));
  writeFileSync(path.join(dir, "validator-case"), mode);
  writeFileSync(path.join(dir, "merman_bench", "svg.py"), `
import json, os, sys, time
from pathlib import Path

mode = Path("validator-case").read_text()
pid = Path("validator-pid")

def mark_started():
    Path("starting").write_text(str(os.getpid()))
    Path("starting").rename(pid)

def hold():
    deadline = time.monotonic() + 8
    while not Path("release-validator").exists() and time.monotonic() < deadline:
        time.sleep(0.01)

# Close stdin without consuming the 4 MiB job so the collector encounters EPIPE.
if mode.startswith("epipe-") and not pid.exists():
    mark_started()
    os.close(0)
    if mode == "epipe-exit":
        time.sleep(1)
        sys.exit(7)
    hold()
    sys.exit(0)

jobs = json.load(sys.stdin)
if any(f"<text>{mode}</text>" in job["svg"] for job in jobs):
    mark_started()
    if mode != "normal":
        hold()
print(json.dumps([{"status": "ok", "output_class": "draw"} for _ in jobs]))
`);
  install(dir, "@mermanjs/node", `
    import { existsSync, readFileSync, writeFileSync } from 'node:fs';
    import { Worker } from 'node:worker_threads';
    function validatorState() {
      const pid = readFileSync('validator-pid', 'utf8');
      try {
        return /^State:\\s+(\\w)/m.exec(readFileSync('/proc/' + pid + '/status', 'utf8'))[1];
      } catch (error) {
        if (error.code !== 'ENOENT' && error.code !== 'ESRCH') throw error;
        return 'gone';
      }
    }
    export function createNodeEngine() {
      let source;
      let killer;
      return {
        runtimeCatalog: { package_version: '1.2.3' },
        renderSvgSync(input, { optionsJson }) {
          source = input;
          if (source === 'worker-exit' && !killer) {
            killer = new Worker(
              "const { existsSync } = require('node:fs'); setInterval(() => {" +
              "if (existsSync('validator-pid')) process.kill(process.pid, 'SIGKILL'); }, 10);",
              { eval: true },
            ).unref();
          }
          if (source === 'after' && !existsSync('validator-state')) {
            try { writeFileSync('validator-state', validatorState()); }
            finally { writeFileSync('release-validator', ''); }
          }
          const id = JSON.parse(optionsJson).svg.diagram_id;
          const text = source.startsWith('epipe-') ? 'x'.repeat(4 * 1024 * 1024) : source;
          return '<svg id="' + id + '"><text>' + text + '</text></svg>';
        },
        dispose() {
          if (!source.startsWith('epipe-')) return;
          const state = validatorState();
          writeFileSync('validator-at-dispose', state);
          // Turn premature disposal into a failure the collector must isolate.
          if (!['gone', 'Z', 'X'].includes(state)) process.exit(73);
        },
      };
    }
  `);
  plan(dir, [mode, "after"].map((id) => ({ id, requests: [request(id, id)] })));
  return { dir, ...run(dir, "merman", ["--samples", "1", "--warmup-calls", "0", "--timeout-seconds", "3"], runner) };
}

for (const [mode, expectedStatus, expectedError] of [
  ["normal", undefined, null],
  ["timeout", "timeout", /timed out after 3s/u],
  ["epipe-timeout", "timeout", /timed out after 3s/u],
  ["epipe-exit", "failed", /SVG validator failed \(exit 7\)/u],
  ["worker-exit", "failed", /SIGKILL/u],
]) {
  test(`Linux retires the SVG validator before the next scenario (${mode})`, { skip: process.platform !== "linux" }, (t) => {
    const { dir, report, status, stderr } = validatorScenario(t, mode);
    const [first, after] = report.scenarios;
    assert.equal(after.coverage.full_batch_comparable, true, stderr);
    const state = readFileSync(path.join(dir, "validator-state"), "utf8");
    assert.match(state, /^(?:gone|Z|X)$/u, `validator still alive at the next render (Linux state ${state})`);
    assert.equal(status, expectedStatus ? 1 : 0, stderr);
    assert.equal(first.status, expectedStatus, stderr);
    if (expectedError) assert.match(first.error, expectedError);
    else assert.equal(first.coverage.full_batch_comparable, true);
    if (mode === "epipe-exit") {
      const disposeState = readFileSync(path.join(dir, "validator-at-dispose"), "utf8");
      assert.match(disposeState, /^(?:gone|Z|X)$/u);
    }
  });
}

test("module identity distinguishes Node-WASM and rejects browser entries and mismatched runtimes", (t) => {
  const dir = project(t);
  plan(dir, [{ id: "one", requests: [request("source", "one")] }]);
  const wasm = install(dir, "@mermanjs/node-wasm", drawModule);
  const ok = run(dir, "merman", ["--module", path.relative(dir, wasm), "--samples", "1"]);
  assert.equal(ok.status, 0, ok.stderr);
  assert.equal(ok.report.backend, "node-wasm");
  assert.equal(ok.report.artifact_identity.status, "unknown");
  assert.equal(ok.report.scenarios[0].transport_api_version, 2);
  const browser = install(dir, "@mermanjs/web", drawModule);
  const rejected = run(dir, "merman", ["--module", browser]);
  assert.equal(rejected.status, 1);
  assert.match(rejected.report.error, /@mermanjs\/web/);
  assert.ok(rejected.stderr.includes(rejected.report.error));
  install(dir, "@mermanjs/node", drawModule.replace("package_version: '1.2.3'", "package_version: '1.0.0'"));
  const mismatch = run(dir, "merman");
  assert.equal(mismatch.status, 1);
  assert.match(mismatch.report.scenarios[0].error, /Runtime version 1\.0\.0 differs/);
  assert.match(mismatch.stderr, /dispose/);
});

test("native identity follows the candidate's loaded optional addon across same-version rebuilds", (t) => {
  const dir = project(t);
  const candidate = path.join(dir, "candidate");
  const entry = install(candidate, "@mermanjs/node", `
    import { createRequire } from 'node:module';
    const require = createRequire(import.meta.url);
    // Exercise the CJS resolution/cache contract without compiling a test addon.
    require.extensions['.node'] = require.extensions['.js'];
    export function createNodeEngine() { return {
      runtimeCatalog: {package_version: '1.2.3'}, dispose() {},
      renderSvgSync(source) {
        const name = '@mermanjs/node-' + (source === 'replace' ? 'selected' : source);
        const render = require(name);
        if (source === 'replace') require('node:fs').appendFileSync(require.resolve(name), '\\n// replaced');
        return render();
      },
    }; }
  `);
  const pkg = path.join(candidate, "node_modules", "@mermanjs");
  const manifest = path.join(pkg, "node", "package.json");
  writeFileSync(manifest, JSON.stringify({ ...JSON.parse(readFileSync(manifest)),
    optionalDependencies: { "@mermanjs/node-selected": "1.2.3", "@mermanjs/node-other": "1.2.3" } }));
  const addon = (root, id, build) => {
    const target = path.join(root, "node_modules", "@mermanjs", `node-${id}`);
    mkdirSync(target, { recursive: true });
    writeFileSync(path.join(target, "package.json"), JSON.stringify({ name: `@mermanjs/node-${id}`, version: "1.2.3", main: "merman.node" }));
    const file = path.join(target, "merman.node");
    writeFileSync(file, `module.exports = () => '<svg id="one"><rect/></svg>'; // ${build}`);
    return file;
  };
  addon(dir, "selected", "unrelated consumer installation");
  addon(candidate, "other", "other optional package");
  const scenario = (id) => ({ id, requests: [request(id, "one")] });
  plan(dir, [scenario("selected")]);
  const digests = [];
  for (const build of ["A", "B"]) {
    const file = addon(candidate, "selected", build);
    const { report, status, stderr } = run(dir, "merman", ["--module", entry, "--samples", "1"]);
    assert.equal(status, 0, stderr);
    assert.equal(report.package.version, "1.2.3");
    assert.equal(report.artifact_identity.status, "recorded");
    const [facade, native] = report.artifact_identity.files;
    assert.equal(facade.sha256, sha256(readFileSync(entry)));
    const bytes = readFileSync(file);
    assert.deepEqual(native, { role: "native-addon", path: realpathSync(file), bytes: bytes.length, sha256: sha256(bytes) });
    digests.push(native.sha256);
  }
  assert.notEqual(...digests);

  plan(dir, [scenario("selected"), scenario("other")]);
  const { report } = run(dir, "merman", ["--module", entry, "--samples", "1"]);
  assert.equal(report.artifact_identity.status, "unknown");
  assert.match(report.artifact_identity.reason, /different product files/);
  assert.notEqual(...report.scenarios.map((row) => row.artifact_identity.files[1].sha256));

  plan(dir, [scenario("replace")]);
  const replaced = run(dir, "merman", ["--module", entry, "--samples", "1"]);
  assert.equal(replaced.status, 0, replaced.stderr);
  assert.equal(replaced.report.artifact_identity.status, "unknown");
  assert.match(replaced.report.artifact_identity.reason, /changed or was unavailable/);
  assert.deepEqual(replaced.report.artifact_identity.files.map((file) => file.role), ["entry"]);
});

test("WASM identity checks the loaded glue and the compiled module against the packaged bytes", (t) => {
  const dir = project(t);
  plan(dir, [{ id: "one", requests: [request("source", "one")] }]);
  const entry = install(dir, "@mermanjs/node-wasm", `
    import { createRequire } from 'node:module';
    const wasm = createRequire(import.meta.url)('../artifact/merman_node.cjs');
    export function createNodeEngine() { return { runtimeCatalog: { package_version: '1.2.3' },
      renderSvgSync() { return '<svg id="one"><text>' + wasm.v() + '</text></svg>'; }, dispose() {} }; }
  `);
  const artifacts = path.join(path.dirname(entry), "..", "artifact");
  mkdirSync(artifacts);
  const glue = path.join(artifacts, "merman_node.cjs");
  const wasm = path.join(artifacts, "merman_node_bg.wasm");
  const load = (name) => `const bytes = require('node:fs').readFileSync(require('node:path').join(__dirname, '${name}'));
    module.exports = new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;`;
  writeFileSync(glue, load(path.basename(wasm)));
  const moduleBytes = (n) => Buffer.from(`0061736d010000000105016000017f03020100070501017600000a0601040041${n}0b`, "hex");
  for (const version of ["01", "02"]) {
    const bytes = moduleBytes(version);
    writeFileSync(wasm, bytes);
    const { report, status, stderr } = run(dir, "merman", ["--module", entry, "--samples", "1"]);
    assert.equal(status, 0, stderr);
    const identity = report.artifact_identity;
    assert.equal(identity.status, "recorded", identity.reason);
    assert.deepEqual(identity.files.map((file) => file.role), ["entry", "wasm-glue", "wasm-module"]);
    assert.equal(identity.files[1].sha256, sha256(readFileSync(glue)));
    assert.deepEqual(identity.files[2], { role: "wasm-module", path: realpathSync(wasm), bytes: bytes.length, sha256: sha256(bytes) });
  }
  writeFileSync(glue, load("other.wasm"));
  writeFileSync(path.join(artifacts, "other.wasm"), moduleBytes("03"));
  const { report, status, stderr } = run(dir, "merman", ["--module", entry, "--samples", "1"]);
  assert.equal(status, 0, stderr);
  assert.equal(report.artifact_identity.status, "unknown");
  assert.deepEqual(report.artifact_identity.files.map((file) => file.role), ["entry", "wasm-glue"]);
  assert.match(report.artifact_identity.reason, /could not be matched/);
});

test("invalid request identity and sample counts fail before package loading", (t) => {
  const dir = project(t);
  const item = request("", "preview");
  plan(dir, [{ id: "one", requests: [{ ...item, diagram_id: "" }] }]);
  assert.match(run(dir, "pele").report.error, /nonempty diagram_id/);
  plan(dir, [{ id: "one", requests: [{ ...item, source_sha256: sha256("other") }] }]);
  assert.match(run(dir, "pele").report.error, /Source SHA-256 mismatch/);
  for (const expected_labels of ["not an array", [""], [[]], [["valid", " \t"]], [[["nested"]]], [["valid", 1]]]) {
    plan(dir, [{ id: "one", requests: [{ ...item, expected_labels }] }]);
    assert.match(run(dir, "pele").report.error, /Invalid expected_labels/);
  }
  for (const expected_outcome of ["other", null, true]) {
    plan(dir, [{ id: "one", requests: [{ ...item, expected_outcome }] }]);
    assert.match(run(dir, "pele").report.error, /Invalid expected_outcome/);
  }
  assert.match(run(dir, "pele", ["--samples", "0"]).report.error, /--samples/);
  assert.match(run(dir, "pele", ["--warmup-calls=-1"]).report.error, /--warmup-calls/);
  assert.match(run(dir, "pele", ["--warmup-ms", "Infinity"]).report.error, /--warmup-ms/);
  assert.match(run(dir, "pele", ["--timeout-seconds", "0"]).report.error, /--timeout-seconds/);
});
