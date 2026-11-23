import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { measureBatch } from "./workload_node.mjs";

const cli = fileURLToPath(new URL("./workload_node.mjs", import.meta.url));
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const request = (source, diagram_id) => ({ source, diagram_id, fixture: "shared-fixture" });

function project(t) {
  const dir = mkdtempSync(path.join(tmpdir(), "workload-node-"));
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

function run(dir, engine, extra = []) {
  const result = spawnSync(process.execPath, [cli, "--plan", "plan.json", "--engine", engine, ...extra], {
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
    return '<svg><text>' + id + ' ' + Buffer.from(source).toString('base64') + ' 夜</text></svg>';
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

test("public entries isolate duplicate scenarios, reuse one engine and preserve source bytes and IDs", (t) => {
  const dir = project(t);
  install(dir, "@mermanjs/node", drawModule);
  install(dir, "pele", drawModule);
  const source = "---\r\nconfig:\r\n  theme: dark\r\n---\r\nflowchart LR\r\nA[夜] --> B\r\n";
  const requests = [request(source, "doc-1"), request("\uFEFFsequenceDiagram\nA->>B: 🚀\n", "doc-2")];
  requests[0].source_sha256 = sha256(source);
  const raw = plan(dir, [{ id: "doc", requests }, { id: "doc", requests: [requests[0]] }]);

  for (const [engine, name, backend] of [["merman", "@mermanjs/node", "napi"], ["pele", "pele", "javascript"]]) {
    const { report, status, stderr } = run(dir, engine, ["--samples", "2"]);
    assert.equal(status, 0, stderr);
    assert.equal(report.package.name, name);
    assert.equal(report.package.version, "1.2.3");
    assert.equal(report.backend, backend);
    assert.equal(report.surface, "node");
    assert.equal(report.output_format, "svg");
    assert.equal(report.host.node, process.version);
    assert.equal(report.plan_sha256, sha256(raw));
    assert.deepEqual(report.scenarios.map((row) => [row.index, row.id]), [[0, "doc"], [1, "doc"]]);
    assert.deepEqual(stderr.match(/call \d+ doc-\d/g), [
      "call 1 doc-1", "call 2 doc-2", "call 3 doc-1", "call 4 doc-2", "call 5 doc-1", "call 6 doc-2",
      "call 1 doc-1", "call 2 doc-1", "call 3 doc-1",
    ]);
    if (engine === "merman") {
      assert.equal(stderr.match(/create/g).length, 2);
      assert.equal(stderr.match(/dispose/g).length, 2);
    }
    for (const row of report.scenarios) {
      assert.ok(row.init_ms >= 0);
      assert.equal(row.first_render_ms, row.cold_batch.results[0].elapsed_ms);
      assert.equal(row.warm_samples.length, 2);
      assert.equal(row.coverage.full_batch_comparable, true);
      for (const batch of [row.cold_batch, ...row.warm_samples]) {
        assert.equal(batch.elapsed_ms, batch.complete_batch_ms);
        assert.ok(batch.elapsed_ms >= batch.successful_render_ms);
        for (const [i, result] of batch.results.entries()) {
          const { fixture, source, diagram_id } = requests[i];
          assert.deepEqual(row.requests[i], { fixture, diagram_id, source_sha256: sha256(source) });
          const svg = `<svg><text>${diagram_id} ${Buffer.from(source).toString("base64")} 夜</text></svg>`;
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
  const { report, status, stderr } = run(dir, "pele", ["--module", pathToFileURL(entry).href, "--samples", "2"]);
  assert.equal(status, 0, stderr);
  const [row] = report.scenarios;
  assert.equal(row.first_render_ms, null);
  assert.deepEqual(row.coverage, {
    planned_requests: 11, label_reader: null, expected_label_request_indices: [], verified_label_request_indices: [],
    successful_request_indices: [1, 7], comparable_request_indices: [1], full_batch_comparable: false,
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

test("expected labels must appear as SVG or HTML label text, including on a changed warm output", (t) => {
  const dir = project(t);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" id="IdOnly"><style>.CssOnly{fill:red}</style>
    <defs><text>DefsOnly</text></defs><title>TitleOnly</title><!-- CommentOnly -->
    <x:text xmlns:x="urn:not-svg">WrongNamespace</x:text>
    <g><text><tspan>St</tspan><tspan>art</tspan></text><text>Other</text>
    <foreignObject><div xmlns="http://www.w3.org/1999/xhtml" data-id="AttributeOnly">
      <style>.InnerCss{color:blue}</style><p>End &amp; <strong>夜</strong></p><p>Second line</p>
    </div></foreignObject></g></svg>`;
  const absent = ["IdOnly", "CssOnly", "DefsOnly", "TitleOnly", "CommentOnly", "AttributeOnly", "InnerCss", "WrongNamespace", "tart", "StartOther"];
  const sources = [
    { ...request("good", "g"), expected_labels: ["Start", "End & 夜", "Second line"] },
    { ...request("missing", "m"), expected_labels: absent },
    { ...request("changes", "c"), expected_labels: ["Start"] },
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
  assert.deepEqual(row.warm_samples[0].results[2].missing_labels, ["Start"]);
  assert.equal(row.warm_samples[1].complete_batch_ms, null);
});

test("the batch clock stops before byte counts, hashing, label checks and error inspection", async (t) => {
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
  const batch = await measureBatch(requests, ({ index }) => {
    events.push(`render-${index}`);
    if (index === 2) throw error;
    return index === 1 ? "<svg><text>diagram</text></svg>" : "<svg><rect/></svg>";
  }, {
    now() { events.push("clock"); return ++ticks; },
    validate(jobs) { events.push(`validate-${jobs.length}`); return jobs.map(() => ({ status: "ok" })); },
  });
  assert.deepEqual(events, ["clock", "clock", "render-1", "clock", "clock", "render-2", "clock", "clock", "render-3", "clock", "clock",
    "bytes", "hash", "message", "bytes", "hash", "validate-2"]);
  assert.equal(batch.elapsed_ms, 7);
  assert.equal(batch.successful_render_ms, 2);
});

test("each position reuses checked hashes and sends changed outputs in a single validation batch", async () => {
  const requests = [{ index: 0 }, { index: 1, expected_labels: ["B"] }];
  const seen = [];
  const options = {
    cache: requests.map(() => new Map()),
    validate(jobs) {
      seen.push(jobs.map(({ svg, expected_labels }) => [svg, expected_labels]));
      return jobs.map(({ expected_labels }) => ({ status: expected_labels ? "missing-labels" : "ok" }));
    },
  };
  const a = "<svg><text>A</text></svg>", c = "<svg><text>C</text></svg>";
  for (const outputs of [[a, a], [a, a], [a, c], [c, a], [a, a]]) {
    const batch = await measureBatch(requests, ({ index }) => outputs[index], options);
    assert.deepEqual(batch.results.map((result) => result.status), ["ok", "missing-labels"]);
  }
  assert.deepEqual(seen, [[[a, undefined], [a, ["B"]]], [[c, ["B"]]], [[c, undefined]]]);
});

test("a blocked scenario times out with its identity, host, plan and completed rows", (t) => {
  const dir = project(t);
  const raw = plan(dir, [
    { id: "same", requests: [request("first", "d1")] },
    { id: "same", requests: [request("hang", "d2")] },
  ]);
  install(dir, "pele", `export function render(source) {
    if (source === 'hang') Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
    return {svg: '<svg><rect/></svg>'};
  }`);
  const { report, status, stderr } = run(dir, "pele", ["--samples", "1", "--timeout-seconds", "2"]);
  assert.equal(status, 1);
  assert.equal(report.status, "failed");
  assert.deepEqual(report.failed_scenario, { index: 1, id: "same", status: "timeout" });
  assert.equal(report.plan_sha256, sha256(raw));
  assert.equal(report.host.node, process.version);
  assert.equal(report.timeout_seconds, 2);
  assert.deepEqual(report.scenarios.map((row) => row.index), [0]);
  assert.equal(report.scenarios[0].coverage.full_batch_comparable, true);
  assert.match(report.error, /timed out/);
  assert.ok(stderr.includes(report.error));
});

test("module identity distinguishes Node-WASM and rejects browser entries and mismatched runtimes", (t) => {
  const dir = project(t);
  plan(dir, [{ id: "one", requests: [request("source", "one")] }]);
  const wasm = install(dir, "@mermanjs/node-wasm", drawModule);
  const ok = run(dir, "merman", ["--module", path.relative(dir, wasm), "--samples", "1"]);
  assert.equal(ok.status, 0, ok.stderr);
  assert.equal(ok.report.backend, "node-wasm");
  assert.equal(ok.report.scenarios[0].transport_api_version, 2);
  const browser = install(dir, "@mermanjs/web", drawModule);
  const rejected = run(dir, "merman", ["--module", browser]);
  assert.equal(rejected.status, 1);
  assert.match(rejected.report.error, /@mermanjs\/web/);
  assert.ok(rejected.stderr.includes(rejected.report.error));
  install(dir, "@mermanjs/node", drawModule.replace("package_version: '1.2.3'", "package_version: '1.0.0'"));
  const mismatch = run(dir, "merman");
  assert.equal(mismatch.status, 1);
  assert.match(mismatch.report.error, /Runtime version 1\.0\.0 differs/);
  assert.match(mismatch.stderr, /dispose/);
});

test("invalid request identity and sample counts fail before package loading", (t) => {
  const dir = project(t);
  const item = request("", "duplicate");
  plan(dir, [{ id: "one", requests: [item, item] }]);
  assert.match(run(dir, "pele").report.error, /distinct diagram_id/);
  plan(dir, [{ id: "one", requests: [{ ...item, source_sha256: sha256("other") }] }]);
  assert.match(run(dir, "pele").report.error, /Source SHA-256 mismatch/);
  plan(dir, [{ id: "one", requests: [{ ...item, expected_labels: "not an array" }] }]);
  assert.match(run(dir, "pele").report.error, /Invalid expected_labels/);
  assert.match(run(dir, "pele", ["--samples", "0"]).report.error, /--samples/);
  assert.match(run(dir, "pele", ["--timeout-seconds", "0"]).report.error, /--timeout-seconds/);
});
