import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { chromium } from "playwright";
import packageJson from "playwright/package.json" with { type: "json" };

import {
  auditMountedSvg,
  ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
} from "./root-viewport-oracle.ts";

const options = parseArguments(process.argv.slice(2));
const localRoot = path.resolve(options.local);
const upstreamRoot = path.resolve(options.upstream);
const outputPath = path.resolve(options.output);
const localFiles = await collectSvgFiles(localRoot);
if (localFiles.length === 0) {
  throw new Error(`No local SVG files found under ${localRoot}.`);
}

const browser = await chromium.launch({ headless: true });
let browserVersion = "unknown";
const entries = [];
try {
  browserVersion = browser.version();
  const page = await browser.newPage({ locale: "en-US", timezoneId: "UTC" });
  for (const relativePath of localFiles) {
    const localPath = path.join(localRoot, relativePath);
    const upstreamPath = path.join(upstreamRoot, relativePath);
    const localSvg = await readFile(localPath, "utf8");
    const upstreamSvg = await readFile(upstreamPath, "utf8").catch(() => null);
    const local = await page.evaluate(auditMountedSvg, {
      svgSource: localSvg,
      quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
    });
    const upstream =
      upstreamSvg === null
        ? null
        : await page.evaluate(auditMountedSvg, {
            svgSource: upstreamSvg,
            quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
          });
    entries.push({
      fixture: relativePath.replaceAll(path.sep, "/").replace(/\.svg$/u, ""),
      localSha256: sha256(localSvg),
      upstreamSha256: upstreamSvg === null ? null : sha256(upstreamSvg),
      local,
      upstream,
      exactBrowserDelta: exactBrowserDelta(upstream, local),
    });
  }
} finally {
  await browser.close();
}

const report = {
  schemaVersion: 1,
  contractRevision: "browser-root-viewport-v1",
  quantizationEpsilonCssPx: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
  environment: {
    playwright: packageJson.version,
    browser: `Chromium ${browserVersion}`,
    locale: "en-US",
    timezone: "UTC",
    platform: `${process.platform}-${process.arch}`,
  },
  summary: {
    fixtures: entries.length,
    localContainmentFailures: entries.filter(
      (entry) => entry.local.root === null || entry.local.violations.length > 0,
    ).length,
    upstreamDiagnosticsMissing: entries.filter((entry) => entry.upstream === null).length,
  },
  entries,
};

await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(
  `root viewport oracle fixtures=${report.summary.fixtures} local_containment_failures=${report.summary.localContainmentFailures} output=${outputPath}`,
);
if (report.summary.localContainmentFailures > 0) process.exitCode = 1;

function parseArguments(arguments_) {
  const options = {
    local: "target/compare",
    upstream: "fixtures/upstream-svgs",
    output: "target/root-viewport-diagnostic.json",
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--local") options.local = requiredValue(arguments_, ++index, argument);
    else if (argument === "--upstream") {
      options.upstream = requiredValue(arguments_, ++index, argument);
    } else if (argument === "--output") {
      options.output = requiredValue(arguments_, ++index, argument);
    } else {
      throw new Error(`Unknown root viewport oracle argument: ${argument}`);
    }
  }
  return options;
}

function requiredValue(arguments_, index, flag) {
  const value = arguments_[index];
  if (!value) throw new Error(`${flag} requires a value.`);
  return value;
}

async function collectSvgFiles(root) {
  const output = [];
  async function visit(directory, prefix) {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const relative = path.join(prefix, entry.name);
      if (entry.isDirectory()) await visit(path.join(directory, entry.name), relative);
      else if (entry.isFile() && entry.name.endsWith(".svg")) output.push(relative);
    }
  }
  await visit(root, "");
  return output;
}

function exactBrowserDelta(upstream, local) {
  if (!upstream?.root || !local.root) return null;
  return {
    root: deltaRect(upstream.root, local.root),
    paintedUnion:
      upstream.paintedUnion && local.paintedUnion
        ? deltaRect(upstream.paintedUnion, local.paintedUnion)
        : null,
  };
}

function deltaRect(upstream, local) {
  return Object.fromEntries(
    ["left", "top", "right", "bottom", "width", "height"].map((field) => [
      field,
      local[field] - upstream[field],
    ]),
  );
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
