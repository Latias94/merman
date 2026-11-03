import assert from "node:assert/strict";
import { once } from "node:events";
import fs from "node:fs/promises";
import { test } from "node:test";
import { Worker } from "node:worker_threads";
import ts from "typescript";

const source = await fs.readFile(new URL("../src/svg-safety-policy.ts", import.meta.url), "utf8");
const javascript = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText + "\n//# sourceURL=merman-step-safety-policy-test.mjs";
const { assertSelfContainedSvgWithMessagePrefix, assertNavigableSvgWithMessagePrefix } =
  await import(`data:text/javascript;base64,${Buffer.from(javascript).toString("base64")}`);
const publicSource = await fs.readFile(new URL("../src/svg-safety.ts", import.meta.url), "utf8");
const publicJavascript = ts.transpileModule(publicSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
}).outputText.replace("./svg-safety-policy.js", `data:text/javascript;base64,${Buffer.from(javascript).toString("base64")}`) + "\n//# sourceURL=merman-step-safety-host-test.mjs";
const publicHost = await import(`data:text/javascript;base64,${Buffer.from(publicJavascript).toString("base64")}`);

function fixture({ root = "diagram", edge = "e.😀", step = 3, duration = 0.25, count = 5, prefix = "" } = {}) {
  const edgeId = `${root}-${edge}`;
  const motion = `step_${Array.from(`${edgeId}-step`, (character) => /[a-z0-9]/i.test(character) ? character : `_${character.codePointAt(0).toString(16)}_`).join("")}`;
  const transform = `<animateTransform attributeName="transform" type="scale" values="1;1" dur="${duration}s" begin="${prefix}${motion}.begin" fill="remove"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" id="${prefix}${root}"><g class="root"><g class="edgePaths"><path id="${prefix}${edgeId}" data-edge="true" d="M0,0C0,10 100,10 100,0"/><path class="edge-step-highlight" aria-hidden="true" d="M0,0C0,10 100,10 100,0" fill="none" stroke="#333333" stroke-width="4" stroke-linecap="round" stroke-opacity="0.6" pointer-events="none" transform="translate(0,0) scale(0)" data-step="${step}">${transform}</path><circle class="edge-step-dot" r="4" aria-hidden="true" transform="translate(0,0) scale(0)" data-step="${step}" fill="#333333" stroke="none"><animateMotion id="${prefix}${motion}" path="M0,0C0,10 100,10 100,0" dur="${duration}s" begin="${(step - 1) * duration}s;${prefix}${motion}.end+${(count - 1) * duration}s" fill="remove"/>${transform}</circle></g><g class="edge-step-badges"><g class="edge-step-badge" data-step="${step}"><circle r="9"/><text>${step}</text></g></g></g><style class="edge-step-style">@media (prefers-reduced-motion: reduce) { #${prefix}${root} .edge-step-dot, #${prefix}${root} .edge-step-highlight { display: none; } }</style></svg>`;
}

test("Step admits only same-diagram dot/highlight timing, including scoped rebasing", () => {
  for (const svg of [fixture(), fixture({ step: 1, count: 1 }), fixture({ prefix: "embed-step_bad-" }), fixture({ duration: 0.1, count: 4 })]) {
    assert.doesNotThrow(() => assertSelfContainedSvgWithMessagePrefix(svg, "test"));
    assert.doesNotThrow(() => assertNavigableSvgWithMessagePrefix(svg, "test"));
  }
});

test("Step admits decimal timing at large step counts without admitting fractional cycles", () => {
  for (const step of [1, 999999999]) {
    const svg = fixture({ step, count: 999999999, duration: 0.1 });
    assert.doesNotThrow(() => assertSelfContainedSvgWithMessagePrefix(svg, "test"));
    assert.doesNotThrow(() => assertNavigableSvgWithMessagePrefix(svg, "test"));
    const fractional = svg.replace(".end+99999999.80000001s", ".end+99999999.825s");
    assert.notEqual(fractional, svg);
    assert.throws(() => assertSelfContainedSvgWithMessagePrefix(fractional, "test"), /Step animation/i);
  }
  const svg = fixture({ step: 999999999, count: 999999999, duration: 0.1 });
  for (const gap of ["99999999.7s", "1e309s"]) {
    const invalid = svg.replace(".end+99999999.80000001s", `.end+${gap}`);
    assert.throws(() => assertSelfContainedSvgWithMessagePrefix(invalid, "test"), /Step animation/i);
  }
  const distant = fixture({ step: 1e16, count: 1e16, duration: 0.1 });
  assert.doesNotThrow(() => assertSelfContainedSvgWithMessagePrefix(distant, "test"));
  const early = distant.replace(".end+1000000000000000s", ".end+999999999999999.8s");
  assert.notEqual(early, distant);
  assert.throws(() => assertSelfContainedSvgWithMessagePrefix(early, "test"), /Step animation/i);
});

test("Step admission bounds work for deeply nested parallel edges", { timeout: 5000 }, async (context) => {
  const edges = Array.from({ length: 10000 }, (_, index) => {
    const svg = fixture({ edge: `edge${index}`, step: 1, count: 1 });
    return /<g class="edgePaths">(.*?)<\/g>/s.exec(svg)[1];
  }).join("");
  const depth = 200000;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" id="diagram">${"<g>".repeat(depth)}<g class="edgePaths">${edges}</g>${"</g>".repeat(depth)}</svg>`;
  const workerCode = `import { parentPort, workerData } from "node:worker_threads";
    const { assertSelfContainedSvgWithMessagePrefix } = await import(workerData.policy);
    assertSelfContainedSvgWithMessagePrefix(workerData.svg, "test");
    parentPort.postMessage(true);`;
  const worker = new Worker(new URL(`data:text/javascript;base64,${Buffer.from(workerCode).toString("base64")}`), {
    workerData: { svg, policy: `data:text/javascript;base64,${Buffer.from(javascript).toString("base64")}` },
  });
  try {
    assert.deepEqual(await once(worker, "message", { signal: context.signal }), [true]);
  } finally {
    await worker.terminate();
  }
});

test("Step rejects active attributes, alternate targets, malformed paths and clocks", () => {
  const svg = fixture();
  const attacks = [
    svg.replace('attributeName="transform"', 'attributeName="href"'),
    svg.replace('type="scale"', 'type="translate"'),
    svg.replace('values="1;1"', 'values="1;100"'),
    svg.replace('<animateMotion ', '<animateMotion href="#victim" '),
    svg.replace('<animateMotion ', '<animateMotion onbegin="alert(1)" '),
    svg.replace('<animateMotion ', '<animateMotion repeatCount="indefinite" '),
    svg.replace('<animateMotion ', '<animateMotion additive="sum" '),
    svg.replace('path="M0,0C0,10 100,10 100,0"', 'path="javascript:alert(1)"'),
    svg.replace('path="M0,0C0,10 100,10 100,0"', 'path="M0,0L1e999,0"'),
    svg.replace('path="M0,0C0,10 100,10 100,0"', 'path="M0,0L1"'),
    svg.replace('path="M0,0C0,10 100,10 100,0"', 'path="M0,0A2,2,0,2,0,4,4"'),
    svg.replace('begin="0.5s;', 'begin="click;'),
    svg.replace('begin="0.5s;', 'begin="NaNs;'),
    svg.replace('begin="0.5s;', 'begin="-1s;'),
    svg.replace('begin="0.5s;', 'begin="0.6s;'),
    svg.replace('.end+1s', '.end+0.1s'),
    svg.replace('.begin"', '.click"'),
    svg.replace('dur="0.25s"', 'dur="0s"'),
    svg.replace('fill="remove"', 'fill="freeze"'),
    svg.replace('transform="translate(0,0) scale(0)"', 'transform="scale(1000)"'),
    svg.replace('id="diagram"', 'id="other"'),
    svg.replace('class="edgePaths"', 'class="unrelated"'),
    svg.replace('<g class="root">', '<svg><g class="root">').replace('<style class="edge-step-style">', '</svg><style class="edge-step-style">'),
    svg.replace('<g class="root">', '<s:svg xmlns:s="http://www.w3.org/2000/svg"><g class="root">').replace('<style class="edge-step-style">', '</s:svg><style class="edge-step-style">'),
    svg.replace('</svg>', '<animateMotion path="M0,0L1,1"/></svg>'),
    svg.replace('</svg>', '<set attributeName="href" to="javascript:alert(1)"/></svg>'),
    svg.replace('fill="remove"/>', 'fill="remove"><rect/></animateTransform>'),
    svg.replace('data-edge="true"', 'data-edge="false"'),
    svg.replace('</svg>', `<path id="${/animateMotion id="([^"]+)"/.exec(svg)[1]}"/></svg>`),
  ];
  for (const [index, attack] of attacks.entries()) {
    assert.throws(() => assertSelfContainedSvgWithMessagePrefix(attack, "test"), /Step animation|active embedded|event handler/i, `attack ${index}`);
  }
});

test("Step rejects matching malformed or nonfinite motion and highlight paths", () => {
  const svg = fixture();
  for (const path of ["javascript:alert(1)", "M0,0L1e999,0", "M0,0L1", "M0,0A2,2,0,2,0,4,4"]) {
    const invalid = svg.replaceAll("M0,0C0,10 100,10 100,0", path);
    assert.notEqual(invalid, svg);
    assert.throws(() => assertSelfContainedSvgWithMessagePrefix(invalid, "test"), /Step animation/i);
    assert.throws(() => assertNavigableSvgWithMessagePrefix(invalid, "test"), /Step animation/i);
  }
});

test("ordinary SVG continues rejecting general SMIL", () => {
  for (const tag of ["animate", "animateColor", "animateMotion", "animateTransform", "set", "mpath", "discard"]) {
    assert.throws(() => assertSelfContainedSvgWithMessagePrefix(`<svg><${tag}/></svg>`, "test"));
  }
});

test("Web admissions revalidate Step at mount and preserve the navigation capability split", () => {
  const svg = fixture();
  const ownerDocument = { URL: "https://example.com/diagram", baseURI: "https://example.com/diagram" };
  const admission = publicHost.assertSelfContainedSvgForDom(svg);
  assert.doesNotThrow(() => publicHost.prepareSelfContainedSvgForDomMount(admission, { outerHTML: svg, ownerDocument }, ownerDocument));
  const mutated = svg.replace("<animateMotion ", "<animateMotion href=\"#outside\" ");
  assert.throws(() => publicHost.prepareSelfContainedSvgForDomMount(admission, { outerHTML: mutated, ownerDocument }, ownerDocument), /Step animation/i);
  const navigable = svg.replace("</svg>", '<a href="https://example.com/docs"><text>docs</text></a></svg>');
  assert.throws(() => publicHost.assertSelfContainedSvgForDom(navigable), /external resource/i);
  const navigation = publicHost.assertNavigableSvgForDom(navigable);
  assert.throws(() => publicHost.prepareSelfContainedSvgForDomMount(navigation, { outerHTML: navigable, ownerDocument }, ownerDocument), /navigable capability/i);
});
