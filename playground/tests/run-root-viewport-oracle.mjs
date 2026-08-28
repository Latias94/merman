import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { chromium } from "playwright";
import packageJson from "playwright/package.json" with { type: "json" };

import {
  auditMountedSvg,
  classifyRootViewportContainment,
  exactRootViewportResidualEvidenceIsEligible,
  ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX,
  ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX,
  ROOT_VIEWPORT_PAINT_GUARD_CSS_PX,
  ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
} from "./root-viewport-oracle.ts";
import {
  matchingRootViewportResidual,
  parseRootViewportResidualCatalog,
  ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION,
} from "./root-viewport-residuals.ts";

const options = parseArguments(process.argv.slice(2));
const localRoot = path.resolve(options.local);
const upstreamRoot = path.resolve(options.upstream);
const outputPath = path.resolve(options.output);
const residualCatalogPath = path.resolve(
  options.residuals ?? path.join(upstreamRoot, "..", "_verification", "root-viewport-residuals.json"),
);
const residualCatalog = parseRootViewportResidualCatalog(
  await readFile(residualCatalogPath, "utf8"),
);
const usedResidualFixtures = new Set();
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
    const local = await auditMountedSvg(page, {
      svgSource: localSvg,
    });
    const compareWithUpstream =
      local.paintAudit.status !== "collected" || local.violations.length > 0;
    const upstreamSvg = compareWithUpstream
      ? await readFile(upstreamPath, "utf8").catch(() => null)
      : null;
    const upstream =
      upstreamSvg === null
        ? null
        : await auditMountedSvg(page, {
            svgSource: upstreamSvg,
          });
    const baseContainmentClassification = classifyRootViewportContainment(local, upstream);
    const fixture = relativePath.replaceAll(path.sep, "/").replace(/\.svg$/u, "");
    const localSha256 = sha256(localSvg);
    const upstreamSha256 = upstreamSvg === null ? null : sha256(upstreamSvg);
    const residual =
      baseContainmentClassification === "blocking" &&
      exactRootViewportResidualEvidenceIsEligible(local, upstream)
        ? matchingRootViewportResidual(
            residualCatalog,
            fixture,
            localSha256,
            upstreamSha256,
          )
        : null;
    if (residual !== null) usedResidualFixtures.add(residual.fixture);
    const containmentClassification =
      residual === null ? baseContainmentClassification : "exact-residual";
    entries.push({
      fixture,
      localSha256,
      upstreamSha256,
      containmentClassification,
      residualReason: residual?.reason ?? null,
      local: reportAudit(local),
      upstream: upstream === null ? null : reportAudit(upstream),
    });
  }
} finally {
  await browser.close();
}

const unusedResidualFixtures = residualCatalog.entries
  .map((entry) => entry.fixture)
  .filter((fixture) => !usedResidualFixtures.has(fixture));

const report = {
  schemaVersion: 10,
  contractRevision: "browser-root-paint-containment-v10",
  residualComparisonRevision: ROOT_VIEWPORT_RESIDUAL_COMPARISON_REVISION,
  quantizationEpsilonCssPx: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
  paintGuardCssPx: ROOT_VIEWPORT_PAINT_GUARD_CSS_PX,
  maxCaptureDimensionCssPx: ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX,
  maxCaptureAreaCssPx: ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX,
  environment: {
    playwright: packageJson.version,
    browser: `Chromium ${browserVersion}`,
    locale: "en-US",
    timezone: "UTC",
    platform: `${process.platform}-${process.arch}`,
    localPaintAudit: "transparent Chromium screenshot alpha",
    upstreamPaintAudit:
      "collected after any local overflow or indeterminate evidence; otherwise omitted",
  },
  summary: {
    fixtures: entries.length,
    localContainmentFailures: entries.filter(
      (entry) => entry.containmentClassification !== "contained",
    ).length,
    blockingContainmentFailures: entries.filter(
      (entry) => entry.containmentClassification === "blocking",
    ).length,
    browserOwnedPaintDiagnostics: entries.filter(
      (entry) => entry.containmentClassification === "browser-owned-diagnostic",
    ).length,
    exactResidualContainmentFailures: entries.filter(
      (entry) => entry.containmentClassification === "exact-residual",
    ).length,
    upstreamInheritedContainmentFailures: entries.filter(
      (entry) => entry.containmentClassification === "upstream-inherited",
    ).length,
    localIndeterminate: entries.filter(
      (entry) => entry.local.paintAudit.status === "indeterminate",
    ).length,
    upstreamDiagnosticsMissing: entries.filter(
      (entry) =>
        entry.containmentClassification === "blocking" &&
        entry.upstream === null,
    ).length,
    unusedExactResiduals: unusedResidualFixtures.length,
  },
  unusedResidualFixtures,
  entries,
};

await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(
  `root viewport oracle fixtures=${report.summary.fixtures} blocking_containment_failures=${report.summary.blockingContainmentFailures} browser_owned_diagnostics=${report.summary.browserOwnedPaintDiagnostics} upstream_inherited=${report.summary.upstreamInheritedContainmentFailures} exact_residuals=${report.summary.exactResidualContainmentFailures} unused_exact_residuals=${report.summary.unusedExactResiduals} output=${outputPath}`,
);
if (
  report.summary.blockingContainmentFailures > 0 ||
  report.summary.unusedExactResiduals > 0
) {
  process.exitCode = 1;
}

function parseArguments(arguments_) {
  const options = {
    local: "target/compare",
    upstream: "fixtures/upstream-svgs",
    output: "target/root-viewport-diagnostic.json",
    residuals: null,
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--local") options.local = requiredValue(arguments_, ++index, argument);
    else if (argument === "--upstream") {
      options.upstream = requiredValue(arguments_, ++index, argument);
    } else if (argument === "--output") {
      options.output = requiredValue(arguments_, ++index, argument);
    } else if (argument === "--residuals") {
      options.residuals = requiredValue(arguments_, ++index, argument);
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
  const diagrams = await readdir(root, { withFileTypes: true });
  diagrams.sort((left, right) => left.name.localeCompare(right.name));
  for (const diagram of diagrams) {
    if (!diagram.isDirectory()) continue;
    const fixtures = await readdir(path.join(root, diagram.name), {
      withFileTypes: true,
    });
    fixtures.sort((left, right) => left.name.localeCompare(right.name));
    for (const fixture of fixtures) {
      if (fixture.isFile() && fixture.name.endsWith(".svg")) {
        output.push(path.join(diagram.name, fixture.name));
      }
    }
  }
  return output;
}

function reportAudit({ structuralPixelKeys, ...audit }) {
  return {
    ...audit,
    paintedPixelCount: audit.violations.reduce(
      (total, violation) => total + violation.paintedPixelCount,
      0,
    ),
    structuralPaintedPixelCount: structuralPixelKeys.length,
    structuralPixelSha256: sha256(structuralPixelKeys.join("\n")),
  };
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
