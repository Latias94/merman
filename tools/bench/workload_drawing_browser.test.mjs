import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { inspectDrawings } from "./workload_drawing_browser.mjs";

test("Flowchart evidence follows the painted route, arrowhead and cluster borders", async () => {
  const arrow = { edge: "route", from: "A", to: "B" };
  const facts = { arrows: [arrow], groups: { CI: ["A"], Deploy: ["B"] } };
  const marker = '<path class="marker" d="M132 25L124 21L124 29Z" fill="black"/>';
  const route = '<path d="M50 25L125 25" fill="none" stroke="black"/>';
  const inline = `<g class="edge" data-id="route">${route}${marker}</g>`;
  const native = '<path data-edge="true" data-id="route" stroke="black" d="M50 25L132 25" marker-end="url(#point)"/>';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 260 90" width="520" height="180">
    <defs><marker id="point" orient="auto" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" markerUnits="userSpaceOnUse"><path d="M0 0L10 5L0 10Z"/></marker></defs>
    <g transform="translate(5 8) scale(1.2)">
      <g class="cluster" data-id="CI"><rect width="90" height="50" fill="white"/></g>
      <g class="cluster" data-id="Deploy"><rect x="110" width="90" height="50" fill="white"/></g>
      ${inline}
      <g class="node" data-id="A"><rect x="10" y="15" width="40" height="20"/><text y="25">A</text></g>
      <g class="node" data-id="B"><rect x="132" y="15" width="40" height="20"/><text y="25">B</text></g>
    </g></svg>`;
  const withNative = svg.replace(inline, native);
  const pass = ["passed", "passed", "passed"];
  const wrongArrow = ["failed", "passed", "passed"];
  const unsupported = ["unavailable", "passed", "passed"];
  const cases = [
    ["inline", svg, pass],
    ["cyclic triangle vertices", svg.replace("132 25L124 21L124 29", "124 29L132 25L124 21"), pass],
    ["SVG marker", withNative, pass],
    ["reversed path with the arrow at the start", withNative.replace("M50 25L132 25", "M132 25L50 25")
      .replace("marker-end", "marker-start").replace('orient="auto"', 'orient="auto-start-reverse"'), pass],
    ["SVG marker on the wrong end", withNative.replace("marker-end", "marker-start"), wrongArrow],
    ["same edge identity, reversed route", withNative.replace("M50 25L132 25", "M132 25L50 25"), wrongArrow],
    ["missing native definition", withNative.replace("url(#point)", "url(#absent)"), wrongArrow],
    ["flipped native arrowhead", withNative.replace("M0 0L10 5L0 10Z", "M10 0L0 5L10 10Z"), wrongArrow],
    ["flipped inline arrowhead", svg.replace("132 25L124 21L124 29", "123 25L131 21L131 29"), wrongArrow],
    ["detached inline head", svg.replace(marker, marker.replace("class=", 'transform="translate(-40 0)" class=')), wrongArrow],
    ["hidden inline head", svg.replace(marker, marker.replace("fill=", 'opacity="0" fill=')), wrongArrow],
    ["no route to the destination", withNative.replace("L132 25", "L95 25"), wrongArrow],
    ["unrecognized glyph", svg.replace("M132 25L124 21L124 29Z", "M132 25Q124 10 124 29Z"), unsupported],
    ["node crosses its declared border", svg.replace('width="90" height="50"', 'width="35" height="50"'), ["passed", "failed", "passed"]],
    ["overlapping sibling", svg.replace('x="110" width="90"', 'x="40" width="160"'), ["passed", "failed", "passed"]],
    ["SVG scripts cannot repair a missing marker", withNative.replace("marker-end=", "missing-marker=")
      .replace("</svg>", `<script>document.querySelector('[data-edge]').setAttribute('marker-end','url(#point)')</script></svg>`), wrongArrow],
  ];
  const { results, browser, observer_sha256 } = await inspectDrawings(cases.map(([, svg]) => ({ svg, facts })),
    process.env.PUPPETEER_EXECUTABLE_PATH);
  assert.match(browser, /Chrome|Chromium/i);
  assert.match(observer_sha256, /^[0-9a-f]{64}$/);
  for (const [i, [name, , expected]] of cases.entries()) {
    assert.deepEqual(results[i].map((r) => r.status), expected, `${name}: ${JSON.stringify(results[i])}`);
    assert.deepEqual(results[i].map((r) => r.kind), ["arrow", "membership", "membership"]);
  }
});

test("real text ranges detect horizontal overflow in SVG and HTML nodes", async () => {
  const label = "feature/example-branch", facts = { label_widths: { C: label } };
  const text = `<text x="40" y="30" fill="black" font-size="13">${label}</text>`;
  const foreign = `<foreignObject x="40" y="12" width="175" height="28" style="overflow:hidden"><div xmlns="http://www.w3.org/1999/xhtml" style="font:13px sans-serif;white-space:nowrap">${label}</div></foreignObject>`;
  const node = (content) => `<svg xmlns="http://www.w3.org/2000/svg" width="350" height="70"><g class="node" data-id="C" transform="translate(5,3)"><rect x="20" y="5" width="240" height="55" fill="white"/>${content}</g></svg>`;
  const cases = [
    [node(text), "passed"], [node(foreign), "passed"],
    [node(text.replace('x="40"', 'x="245"')), "failed", "node bounds"],
    [node(foreign.replace('width="175"', 'width="25"')), "failed", "foreignObject bounds"],
    [node(text.replace(label, "different")), "failed", "missing from its node"],
  ];
  const { results, environment } = await inspectDrawings(cases.map(([svg]) => ({ svg, facts })), process.env.PUPPETEER_EXECUTABLE_PATH);
  assert.equal(environment.platform, process.platform);
  for (const [i, [, status, reason]] of cases.entries()) {
    const [fact] = results[i];
    assert.equal(fact.status, status, JSON.stringify(fact));
    if (reason) assert.ok(fact.reason.includes(reason), fact.reason);
    if (status === "passed") assert.ok(fact.css_font_families.length && fact.clearance_px.node.left >= 0);
  }
});

test("Sequence counts complete painted message labels with the renderer's message identity", async () => {
  const label = "message-000000";
  const facts = { sequence_messages: { [label]: 3 } };
  const svg = (body, attributes = 'aria-roledescription="sequence"') =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="120" ${attributes}>${body}</svg>`;
  const text = (y, content = label, attributes = 'class="messageText"') =>
    `<text x="10" y="${y}" ${attributes}>${content}</text>`;
  const first = text(20), middle = text(40, '<tspan>message-</tspan><tspan>000000</tspan>'), last = text(60);
  const decoys = `<defs>${middle}</defs><g opacity="0">${middle}</g>${text(40, label, 'class="actor actor-box"')}
    ${text(40, label, '')}${text(40, label, 'class="messageText" style="display:none"')}
    ${text(40, `<tspan visibility="hidden">${label}</tspan>`)}${text(40, label, 'class="messageText" opacity="0"')}
    ${text(40, label, 'class="messageText" fill="none"')}${text(40, label, 'class="messageText" font-size="0"')}
    ${text(40, `<title>${label}</title>`)}${text(40, `<tspan>${label}</tspan><tspan> extra</tspan>`)}`;
  const ids = [1, 2, 3];
  const v13 = (i) => `<g class="message" data-id="m:A&gt;B#${i}">${text(20 * i, label, `class="messageText" data-id="m:A&gt;B#${i}"`)}</g>`;
  const v13Svg = (messages) => svg(`<g class="messages">${messages}</g>`, 'aria-roledescription="sequence" class="mermaid mermaid-sequence"');
  const peleEdges = ids.map((i) => `<g class="pele-edge pele-message" data-id="i${i}"><path d="M0 0h30"/></g>`).join("");
  const peleLabel = (i) => `<g class="pele-edge-label" data-id="i${i}">${text(20 * i, label, '')}</g>`;
  const pele = (labels) => svg(`<g><g class="pele-edges">${peleEdges}</g><g class="pele-edge-labels">${labels}</g></g>`,
    'aria-roledescription="sequence" class="pele pele-sequence"');
  const cases = [
    ["flat message labels and split text runs", svg(first + middle + last), 3],
    ["identical label still present after middle deletion", svg(first + last), 2],
    ["missing middle with hidden, unrelated and partial labels", svg(first + decoys + last), 2],
    ["extra painted occurrence", svg(first + middle + last + text(80)), 4],
    ["all message text gone from a marked message route", svg('<line class="messageLine0" data-et="message" data-id="i1"/>'), 0],
    ["clipped text has no proven paint", svg(first + text(40, label, 'class="messageText" style="clip-path:inset(100%)"') + last), null],
    ["v13 message groups", v13Svg(ids.map(v13).join("")), 3],
    ["hidden v13 message group", v13Svg(v13(1) + v13(2).replace('<g ', '<g opacity="0" ') + v13(3)), 2],
    ["Pele labels associated with messages", pele(ids.map(peleLabel).join("")), 3],
    ["Pele label without a corresponding message", pele([1, 4, 3].map(peleLabel).join("")), 2],
    ["duplicate Pele label identity", pele([1, 1, 3].map(peleLabel).join("")), null],
    ["unsupported Sequence text structure", svg(text(20, label, '')), null],
    ["message class outside a Sequence diagram", svg(first + middle + last, 'aria-roledescription="flowchart-v2"'), null],
  ];
  const upstream = await readFile(new URL("../../fixtures/upstream-svgs/sequence/upstream_docs_sequence_loop_every_minute.svg", import.meta.url), "utf8");
  const jobs = cases.map(([, svg]) => ({ svg, facts }));
  jobs.push({ svg: upstream, facts: { sequence_messages: { "Hello John, how are you?": 1, "Great!": 1 } } });
  const { results } = await inspectDrawings(jobs, process.env.PUPPETEER_EXECUTABLE_PATH);
  for (const [i, [name, , count]] of cases.entries()) {
    assert.equal(results[i].length, 1, name);
    const [result] = results[i];
    assert.equal(result.kind, "sequence-message-count", name);
    assert.equal(result.label, label, name);
    assert.equal(result.expected, 3, name);
    assert.equal(result.observed, count ?? undefined, name);
    assert.equal(result.status, count === null ? "unavailable" : count === 3 ? "passed" : "failed", name);
  }
  assert.deepEqual(results.at(-1).map(({ label, observed, status }) => [label, observed, status]),
    [["Hello John, how are you?", 1, "passed"], ["Great!", 1, "passed"]]);
});
