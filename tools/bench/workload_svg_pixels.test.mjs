import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { inspectPixels } from "./workload_svg_pixels.mjs";

const tools = createRequire(new URL("../mermaid-cli/package.json", import.meta.url));
const { default: puppeteer } = await import(pathToFileURL(tools.resolve("puppeteer")).href);
const script = fileURLToPath(new URL("./workload_svg_pixels.mjs", import.meta.url));
const exec = promisify(execFile);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

test("standalone SVG pixels, controls and exclusive output in Chrome", async (t) => {
  const dir = await mkdtemp(path.join(tmpdir(), "svg-pixels-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const chrome = process.env.PUPPETEER_EXECUTABLE_PATH || await puppeteer.executablePath();
  const a = path.join(dir, "A.svg"), b = path.join(dir, "B.svg");
  const compare = async (name) => {
    const out = path.join(dir, name);
    await exec(process.execPath, [script, "--left", a, "--right", b, "--out-dir", out, "--chrome", chrome]);
    return JSON.parse(await readFile(path.join(out, "report.json")));
  };
  const red = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="40">
    <style>:root > foreignObject > .patch { width:12px; height:9px; background:red }</style>
    <foreignObject x="4" y="6" width="30" height="20"><div xmlns="http://www.w3.org/1999/xhtml" class="patch"/></foreignObject></svg>`;
  const utf16 = Buffer.from("\uFEFF" + red, "utf16le");
  await writeFile(a, red);
  await writeFile(b, utf16);
  const same = await compare("encoded");
  assert.equal(same.observer_sha256, sha256(await readFile(script)));
  assert.equal(same.platform, process.platform);
  assert.equal(same.arch, process.arch);
  assert.deepEqual(same.scope, { offline: true, input_scripts_enabled: false });
  assert.match(same.chrome, /Chrome|Chromium/i);
  assert.deepEqual(same.viewport_css, { width: 800, height: 600 });
  assert.deepEqual(same.inputs.left, { path: a, bytes: Buffer.byteLength(red), sha256: sha256(red) });
  assert.deepEqual(same.inputs.right, { path: b, bytes: utf16.length, sha256: sha256(utf16) });
  assert.deepEqual(same.resolutions.map((r) => r.scale), [1, 2]);
  for (const row of same.resolutions) {
    assert.equal(row.status, "identical");
    assert.deepEqual(row.a_a, { changed_pixels: 0, bbox_px: null });
    assert.deepEqual(row.a_b, row.a_a);
    assert.equal(row.rgba_sha256.left, row.rgba_sha256.left_repeat);
    assert.equal(row.rgba_sha256.left, row.rgba_sha256.right);
    assert.equal(row.rgba_sha256.blank, sha256(Buffer.alloc(800 * 600 * 4 * row.scale ** 2, 255)));
    assert.equal(row.positive_control.detected, true);
    assert.equal(row.positive_control.changed_pixels, 256 * row.scale ** 2);
    assert.deepEqual(row.roots_css.left, { x: 0, y: 0, width: 64, height: 40, fully_in_viewport: true });
  }
  const css = path.join(dir, "other.css");
  await writeFile(css, ".patch { background:red !important }");
  const blue = red.replace("background:red", "background:blue").replace("<style>", `<style>@import url('${pathToFileURL(css)}');`)
    .replace("<svg ", `<svg onload="document.querySelector('.patch').style.background = 'red'" `);
  await writeFile(b, blue);
  for (const row of (await compare("recolored")).resolutions) {
    const s = row.scale;
    assert.equal(row.status, "different");
    assert.deepEqual(row.a_b, { changed_pixels: 12 * 9 * s ** 2, bbox_px: { x: 4 * s, y: 6 * s, width: 12 * s, height: 9 * s } });
  }
  await assert.rejects(compare("encoded"), /EEXIST/);
  assert.deepEqual(JSON.parse(await readFile(path.join(dir, "encoded", "report.json"))), same);

  await writeFile(b, red.replace('width="64"', 'width="810"'));
  for (const row of (await compare("clipped")).resolutions) {
    assert.equal(row.roots_css.right.fully_in_viewport, false);
    assert.equal(row.status, "inconclusive");
  }
  await writeFile(b, red.replace("background:red", "background:transparent"));
  for (const row of (await compare("blank")).resolutions) {
    assert.deepEqual(row.blank_inputs, ["right"]);
    assert.equal(row.status, "inconclusive");
  }

  // Replay genuine captures with the blank frame in place of the positive control.
  const browser = await puppeteer.launch({ executablePath: chrome, headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(pathToFileURL(a).href);
    const pngs = {}, row = same.resolutions[0];
    for (const name of Object.keys(row.rgba_sha256)) pngs[name] = await readFile(path.join(dir, "encoded", `${name}-1x.png`), "base64");
    pngs.positive = pngs.blank;
    const replay = await page.evaluate(inspectPixels, pngs, row.size_px, 1, row.roots_css);
    assert.equal(replay.a_b.changed_pixels, 0);
    assert.deepEqual(replay.positive_control, { detected: false, changed_pixels: 0, bbox_px: null });
    assert.equal(replay.status, "inconclusive");
  } finally {
    await browser.close();
  }
});
