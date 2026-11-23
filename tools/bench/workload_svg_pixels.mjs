// Compare two standalone SVG documents in a fixed Chrome viewport, outside benchmark runs.
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const tools = createRequire(new URL("../mermaid-cli/package.json", import.meta.url));
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const viewport = { width: 800, height: 600 };
const documentUrl = "https://svg-pixels.invalid/document.svg";
const csp = "default-src 'none'; style-src 'unsafe-inline' data:; img-src data:; font-src data:; base-uri 'none'; form-action 'none'; sandbox allow-same-origin";

// Decode Chrome's PNGs in the trusted control document and compare the raw RGBA pixels in place.
export async function inspectPixels(pngs, { width, height }, scale, roots) {
  const pixels = {}, hashes = {};
  for (const [name, png] of Object.entries(pngs)) {
    const bytes = Uint8Array.from(atob(png), (ch) => ch.charCodeAt(0));
    const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
    if (bitmap.width !== width || bitmap.height !== height) throw new Error(`Unexpected screenshot size for ${name}`);
    const ctx = new OffscreenCanvas(width, height).getContext("2d");
    ctx.drawImage(bitmap, 0, 0);
    bitmap.close();
    const rgba = ctx.getImageData(0, 0, width, height).data;
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", rgba));
    hashes[name] = Array.from(digest, (b) => b.toString(16).padStart(2, "0")).join("");
    pixels[name] = new Uint32Array(rgba.buffer);
  }
  function compare(a, b) {
    const left = pixels[a], right = pixels[b];
    let count = 0, xmin = width, ymin = height, xmax = -1, ymax = -1;
    for (let i = 0; i < left.length; i++) {
      if (left[i] === right[i]) continue;
      const x = i % width, y = Math.floor(i / width);
      count++;
      xmin = Math.min(xmin, x); xmax = Math.max(xmax, x);
      ymin = Math.min(ymin, y); ymax = Math.max(ymax, y);
    }
    return { changed_pixels: count, bbox_px: count ? { x: xmin, y: ymin, width: xmax - xmin + 1, height: ymax - ymin + 1 } : null };
  }
  const positive = compare("blank", "positive");
  const box = positive.bbox_px;
  const detected = positive.changed_pixels === (16 * scale) ** 2 && box?.x === 0 && box.y === 0 &&
    box.width === 16 * scale && box.height === 16 * scale;
  const a_a = compare("left", "left_repeat"), a_b = compare("left", "right");
  const blank_inputs = ["left", "right"].filter((name) => hashes[name] === hashes.blank);
  let status = a_b.changed_pixels ? "different" : "identical";
  if (!detected || blank_inputs.length || a_a.changed_pixels || Object.values(roots).some((r) => !r.fully_in_viewport)) {
    status = "inconclusive";
  }
  return { rgba_sha256: hashes, a_a, a_b, positive_control: { ...positive, detected }, blank_inputs, status };
}

async function run({ left, right, outDir, chrome }) {
  const observer_sha256 = sha256(await readFile(new URL(import.meta.url)));
  const inputs = {}, sources = {};
  for (const [name, file] of Object.entries({ left, right })) {
    const bytes = await readFile(file);
    inputs[name] = { path: path.resolve(file), bytes: bytes.length, sha256: sha256(bytes) };
    sources[name] = bytes;
  }
  const control = (body) => Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600">${body}</svg>`);
  const documents = { left: sources.left, left_repeat: sources.left, right: sources.right,
    blank: control(""), positive: control('<rect width="16" height="16" fill="black"/>') };
  await mkdir(path.dirname(outDir), { recursive: true });
  await mkdir(outDir); // Reserve a fresh directory, including against concurrent writers.
  const { default: puppeteer } = await import(pathToFileURL(tools.resolve("puppeteer")).href);
  const browser = await puppeteer.launch({ executablePath: chrome, headless: true });
  try {
    const report = {
      observer_sha256, chrome: await browser.version(), platform: process.platform, arch: process.arch,
      scope: { offline: true, input_scripts_enabled: false }, viewport_css: viewport, inputs, resolutions: [],
    };
    const page = await browser.newPage();
    await page.setJavaScriptEnabled(false);
    await page.setOfflineMode(true);
    await page.setCacheEnabled(false);
    await page.setRequestInterception(true);
    let documentBytes;
    page.on("request", (r) => {
      if (r.url() === documentUrl && r.isNavigationRequest() && r.frame() === page.mainFrame() && documentBytes) {
        const body = documentBytes;
        documentBytes = undefined;
        return r.respond({ contentType: "image/svg+xml", headers: { "Content-Security-Policy": csp, "Cache-Control": "no-store" }, body });
      }
      if (r.url().startsWith("data:") && ["image", "font", "stylesheet"].includes(r.resourceType())) return r.continue();
      return r.abort();
    });

    for (const scale of [1, 2]) {
      await page.setViewport({ ...viewport, deviceScaleFactor: scale });
      const roots = {}, pngs = {};
      for (const [name, bytes] of Object.entries(documents)) {
        documentBytes = bytes;
        await page.goto(documentUrl, { waitUntil: "load" });
        const bounds = await page.evaluate(async () => {
          const root = document.documentElement;
          if (!(root instanceof SVGSVGElement) || document.querySelector("parsererror")) throw new Error("Expected a standalone SVG document");
          root.getBoundingClientRect(); // Start layout-dependent font loads before waiting.
          await document.fonts.ready;
          const { x, y, width, height, right, bottom } = root.getBoundingClientRect();
          return { x, y, width, height, fully_in_viewport: width > 0 && height > 0 && x >= 0 && y >= 0 && right <= innerWidth && bottom <= innerHeight };
        });
        if (name !== "blank" && name !== "positive") roots[name] = bounds;
        const png = Buffer.from(await page.screenshot({ type: "png", captureBeyondViewport: false }));
        await writeFile(path.join(outDir, `${name}-${scale}x.png`), png, { flag: "wx" });
        pngs[name] = png.toString("base64");
      }
      const size = { width: viewport.width * scale, height: viewport.height * scale };
      const observed = await page.evaluate(inspectPixels, pngs, size, scale, roots);
      report.resolutions.push({ scale, size_px: size, roots_css: roots, ...observed });
    }
    await writeFile(path.join(outDir, "report.json"), JSON.stringify(report, null, 2) + "\n", { flag: "wx" });
  } finally {
    await browser.close();
  }
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    const { values } = parseArgs({ options: { left: { type: "string" }, right: { type: "string" }, "out-dir": { type: "string" }, chrome: { type: "string" } } });
    const { left, right, "out-dir": outDir, chrome } = values;
    if (!left || !right || !outDir || !chrome) throw new Error("Usage: workload_svg_pixels.mjs --left A.svg --right B.svg --out-dir NEW --chrome PATH");
    await run({ left, right, outDir: path.resolve(outDir), chrome });
    console.log(path.resolve(outDir, "report.json"));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
