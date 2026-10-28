import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { prepareReferenceRuntime } from "./reference-runtime.mjs";
import { prepareObservationAdapter, runOracle, validateRequest } from "./parser-oracle.mjs";

const workspace = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(path.join(workspace, "package.json"));
const puppeteer = require("puppeteer");
let runtime;
async function request(cases) {
  runtime ??= (async () => ({
    ...await prepareReferenceRuntime(workspace),
    decoder: await prepareObservationAdapter(path.resolve(workspace, "../../target/parser-differential-tests")),
    browser_executable: await puppeteer.executablePath({ headless: "shell" }),
  }))();
  return { schema_version: 1, runtime: await runtime, cases };
}
function row(id, family, source, lane = "grammar", site_config = { securityLevel: "strict" }) {
  return { id, family, lane, site_config, source };
}
async function observe(cases) {
  const input = await request(cases);
  return runOracle(input, createHash("sha256").update(JSON.stringify(input)).digest("hex"));
}

test("protocol rejects unknown versions, duplicate IDs, unsupported lanes, and oversized input before launching", () => {
  const good = {
    schema_version: 1,
    runtime: {
      artifact_path: "unused-runtime.js",
      artifact_sha256: "0".repeat(64),
      browser_executable: "unused-browser",
      decoder: {
        artifact_path: "unused-decoder.js",
        artifact_sha256: "0".repeat(64),
        source_sha256: "0".repeat(64),
      },
    },
    cases: [row("a", "flowchart", "flowchart TD\n A-->B")],
  };
  assert.equal(validateRequest(good), good);
  assert.throws(() => validateRequest({ ...good, schema_version: 2 }), /schema/);
  assert.throws(() => validateRequest({ ...good, unexpected: true }), /field/);
  assert.throws(() => validateRequest({ ...good, cases: [good.cases[0], good.cases[0]] }), /duplicate/);
  assert.throws(() => validateRequest({ ...good, cases: [row("a", "flowchart", "x", "other")] }), /lane/);
  assert.throws(() => validateRequest({ ...good, cases: [row("a", "flowchart", "x".repeat(4097))] }), /4096/);
  assert.throws(() => validateRequest({ ...good, cases: [row("a", "flowchart", "%%{init: {}}%%\nflowchart TD\n A-->B")] }), /public/);
  assert.throws(() => validateRequest({ ...good, cases: [] }), /Invalid batch size/);
  const scheduled = Array.from({ length: 216 + 2 * 5000 }, (_, index) => ({
    ...good.cases[0], id: `case-${index}`,
  }));
  assert.throws(() => validateRequest({ ...good, cases: scheduled }), /Invalid batch size/);
  for (let offset = 0; offset < scheduled.length; offset += 256) {
    const batch = { ...good, cases: scheduled.slice(offset, offset + 256) };
    assert.equal(validateRequest(batch), batch);
  }
});

test("real Flowchart and Sequence DBs retain semantic records and reset conflicting consecutive diagrams", async () => {
  const response = await observe([
    row("flow", "flowchart", "flowchart TD\n A[开始] e@-->|Yes| B{Choice}\n A --> B\n classDef red fill:red\n class A red"),
    row("other", "flowchart", "flowchart LR\r\n X-->Y"),
    row("seq", "sequence", "sequenceDiagram\n participant A\n participant B\n loop repeat\n A->>+B: Hello\n Note over A,B: note\n B-->>-A: World\n end"),
    row("seq-other", "sequence", "sequenceDiagram\n X->>Y: different"),
  ]);
  assert.equal(response.schema_version, 1);
  assert.equal(response.outcomes.length, 4);
  assert.ok(response.outcomes.every((entry) => entry.status === "accepted"));
  const [flow, other, sequence, last] = response.outcomes.map((entry) => entry.observation);
  assert.deepEqual(flow.nodes.map((node) => node.id), ["A", "B"]);
  assert.equal(flow.edges.length, 2);
  assert.equal(flow.edges[0].id, "e");
  assert.equal(flow.edges[0].label, "Yes");
  assert.deepEqual(other.nodes.map((node) => node.id), ["X", "Y"]);
  assert.deepEqual(sequence.actorOrder, ["A", "B"]);
  assert.ok(sequence.messages.some((message) => message.message === "note" && message.placement === 2));
  assert.ok(sequence.messages.some((message) => message.activate));
  assert.deepEqual(last.actorOrder, ["X", "Y"]);
});

test("public parse lane observes actual frontmatter and init configuration separately from grammar DB", async () => {
  const response = await observe([
    row("frontmatter", "flowchart", "---\ntitle: Example\nconfig:\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\n A-->B", "public"),
    row("init", "sequence", '%%{init: {"sequence":{"wrap":true}}}%%\nsequenceDiagram\n A->>B: hello', "public"),
    row("invalid", "flowchart", "flowchart TD\n A[", "public"),
  ]);
  assert.equal(response.outcomes[0].status, "accepted");
  assert.equal(response.outcomes[0].observation.config.flowchart.htmlLabels, false);
  assert.equal(response.outcomes[1].observation.config.sequence.wrap, true);
  assert.equal(response.outcomes[2].status, "rejected");
});

test("Flowchart preprocessing removes whole-line comments while grammar retains percent punctuation", async () => {
  const cases = [
    ["comment", "A-->B\n%% comment\nC-->D", ["A", "B", "C", "D"]],
    ["indented", " \t%% comment\nA-->B", ["A", "B"]],
    ["eof", "A-->B\n%% comment", ["A", "B"]],
    ["blank-body", "%% \nA-->B", ["A", "B"]],
    ["bare-eof", "%%", ["%%"]],
    ["bare-line", "%%\nA-->B", ["%%", "A", "B"]],
    ["bare-indent", "  %%\n", ["%%"]],
    ["percent-start", "%-->B", ["%", "B"]],
    ["percent-end", "A%-->B", ["A%", "B"]],
    ["percent-middle", "A%%B-->C", ["A%%B", "C"]],
    ["inline-target", "A-->B%%tail", ["A", "B%%tail"]],
    ["direction-prefix", "direction%-->B", ["direction%", "B"]],
    ["click-prefix", "click%%tail-->B", ["click%%tail", "B"]],
    ["reserved-end", "end%-->B", null],
    ["reserved-style", "style%-->B", null],
    ["reserved-interpolation", "interpolate%-->B", null],
    ["reserved-self-target", "_self%-->B", null],
    ["reserved-blank-target", "_blank%-->B", null],
    ["reserved-parent-target", "_parent%-->B", null],
    ["reserved-top-target", "_top%-->B", null],
    ["inline-statement", "A-->B; %%tail", ["A", "B", "%%tail"]],
    ["inline-bare-statement", "A-->B; %%", ["A", "B", "%%"]],
    ["inline-spaced", "A-->B %% comment", null],
    ["inline-continued", "A-->B %% comment\nC-->D", null],
    ["inline-bare", "A-->B %%", null],
    ["inline-comment-statement", "A-->B; %% comment", null],
    ["crlf", "%% 雪😀\r\n雪%-->B%%tail\r\n", ["雪%", "B%%tail"]],
  ];
  const requests = cases.flatMap(([id, body]) => ["grammar", "public"].map(lane =>
    row(`${id}-${lane}`, "flowchart", `flowchart TD\n${body}`, lane)));
  requests.push(row("inline-header", "flowchart", "flowchart TD %% comment\nA-->B"));
  requests.push(row("percent-header-direction", "flowchart", "flowchart LR%\nA-->B"));
  const response = await observe(requests);
  for (const [index, [id, , ids]] of cases.entries()) {
    for (const lane of [0, 1]) {
      const outcome = response.outcomes[index * 2 + lane];
      assert.equal(outcome.status, ids === null ? "rejected" : "accepted", `${id} lane ${lane}`);
      if (ids !== null && lane === 0) {
        assert.deepEqual(outcome.observation.nodes.map(node => node.id), ids, id);
      }
    }
  }
  assert.equal(response.outcomes.at(-1).status, "rejected");
  assert.equal(response.outcomes.at(-2).status, "rejected");
});

test("Flowchart percent labels survive except whole-line comment text inside multiline labels", async () => {
  const response = await observe([
    row("text", "flowchart", 'flowchart TD\nA[100%% done]'),
    row("quoted", "flowchart", 'flowchart TD\nA["100%% done"]'),
    row("markdown", "flowchart", 'flowchart TD\nA["`100%% done`"]'),
    row("quoted-line", "flowchart", 'flowchart TD\nA["first\n%% comment\nlast"]'),
    row("markdown-line", "flowchart", 'flowchart TD\nA["`first\n%% comment\nlast`"]'),
    row("quoted-edge", "flowchart", 'flowchart TD\nA-->|"100%% done"|B'),
    row("markdown-edge", "flowchart", 'flowchart TD\nA-->|"`100%% done`"|B'),
  ]);
  assert.ok(response.outcomes.every(outcome => outcome.status === "accepted"));
  for (const [index, type] of ["text", "string", "markdown"].entries()) {
    const node = response.outcomes[index].observation.nodes[0];
    assert.equal(node.label, "100%% done");
    assert.equal(node.labelType, type);
  }
  for (const index of [3, 4]) {
    assert.equal(response.outcomes[index].observation.nodes[0].label, "first\nlast");
  }
  for (const [index, type] of [[5, "string"], [6, "markdown"]]) {
    const edge = response.outcomes[index].observation.edges[0];
    assert.equal(edge.label, "100%% done");
    assert.equal(edge.labelType, type);
  }
});

test("Flowchart headers require the selected grammar's first statement separator", async () => {
  const cases = [
    ["inline-node", "flowchart LR A-->B", null],
    ["space-semicolon", "flowchart LR ;A-->B", null],
    ["absent-direction-semicolon", "flowchart ;A-->B", null],
    ["inline-direction-statement", "flowchart direction LR\nA-->B", null],
    ["inline-style-statement", "flowchart style A fill:red", null],
    ["unicode-separator", "flowchart TD開始 --> B", null],
    ["direct-semicolon", "flowchart LR;A-->B", "LR"],
    ["newline", "flowchart LR\nA-->B", "LR"],
    ["space-newline", "flowchart LR \t\nA-->B", "LR"],
    ["no-direction", "flowchart\nA-->B", "TB"],
    ["br-direction", "flowchart BR\nA-->B", "BR"],
  ];
  const response = await observe(cases.flatMap(([id, source]) => ["grammar", "public"].map(lane =>
    row(`${id}-${lane}`, "flowchart", source, lane))));
  for (const [index, [id, , direction]] of cases.entries()) {
    for (const lane of [0, 1]) {
      const outcome = response.outcomes[index * 2 + lane];
      assert.equal(outcome.status, direction === null ? "rejected" : "accepted", id);
      if (direction !== null && lane === 0) {
        assert.equal(outcome.observation.direction, direction, id);
        assert.deepEqual(outcome.observation.nodes.map(node => node.id), ["A", "B"], id);
      }
    }
  }
});

test("wrong family and changed runtime digest are tooling failures, never parse rejection", async () => {
  await assert.rejects(observe([row("wrong", "sequence", "flowchart TD\n A-->B")]), /family/);
  const input = await request([row("a", "flowchart", "flowchart TD\n A-->B")]);
  await assert.rejects(runOracle({ ...input, runtime: { ...input.runtime, artifact_sha256: "0".repeat(64) } }, "a".repeat(64)), /digest/);
  await assert.rejects(runOracle({ ...input, runtime: { ...input.runtime, decoder: { ...input.runtime.decoder, source_sha256: "0".repeat(64) } } }, "a".repeat(64)), /decoder/);
  await assert.rejects(runOracle({ ...input, runtime: { ...input.runtime, browser_executable: path.join(workspace, "missing browser.exe") } }, "a".repeat(64)), /Browser|browser|ENOENT/);
});

test("selected decoder observes Flowchart entity labels at the public semantic stage without stripping literal HTML", async () => {
  const response = await observe([
    row("entities", "flowchart", 'flowchart TD\n A["a &amp; b #quot; 雪"] -->|#quot; &amp;| B["<b>Bold</b>"]\nsubgraph G["#quot; &amp;"]\n C\nend'),
  ]);
  const out = response.outcomes[0].observation;
  assert.equal(out.nodes[0].label, 'a & b " 雪');
  assert.equal(out.nodes[1].label, "<b>Bold</b>");
  assert.equal(out.edges[0].label, '" &');
  assert.equal(out.subgraphs[0].title, '" &');
  assert.match(response.decoder_sha256, /^[a-f0-9]{64}$/);
  assert.match(response.decoder_source_sha256, /^[a-f0-9]{64}$/);
});

test("CLI accepts Windows request paths containing spaces and rejects invalid protocol as a process failure", async () => {
  const { execFile } = await import("node:child_process");
  const { promisify } = await import("node:util");
  const { mkdir, writeFile } = await import("node:fs/promises");
  const execute = promisify(execFile);
  const directory = path.resolve(workspace, "../../target/parser-differential-tests/path with spaces");
  await mkdir(directory, { recursive: true });
  const filename = path.join(directory, "request input.json");
  await writeFile(filename, JSON.stringify(await request([row("windows", "flowchart", "flowchart LR\r\n X-->Y")])));
  const { stdout } = await execute(process.execPath, [path.join(workspace, "parser-oracle.mjs"), filename], { timeout: 20_000, maxBuffer: 1024 * 1024 });
  assert.equal(JSON.parse(stdout).outcomes[0].status, "accepted");
  await writeFile(filename, JSON.stringify({ schema_version: 99 }));
  await assert.rejects(execute(process.execPath, [path.join(workspace, "parser-oracle.mjs"), filename], { timeout: 20_000, maxBuffer: 1024 * 1024 }), /request schema/);
});
