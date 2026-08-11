import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const scriptPath = fileURLToPath(import.meta.url);
const playgroundRoot = path.resolve(path.dirname(scriptPath), "..");
const workspaceRoot = path.resolve(playgroundRoot, "..");
const bundlePath = path.join(
  workspaceRoot,
  "tools",
  "upstreams",
  "MERMAID_REFERENCE_BUNDLE.json"
);
const officialNpmRegistry = "https://registry.npmjs.org/";
const requiredNpmVersion = "11.17.0";
const packageName = "@zenuml/core";

const requireBrowserTestTool = createRequire(
  new URL("../tests/package.json", import.meta.url)
);
const { chromium } = requireBrowserTestTool("playwright");

export function assertOfficialSignatureAuditResult(result, label) {
  if (result.error) {
    throw new Error(
      `official npm signature verification failed for ${label}: ${result.error.message}`
    );
  }
  if (result.status !== 0) {
    const diagnostic = String(result.stderr || result.stdout || "no diagnostic").trim();
    throw new Error(
      `official npm signature verification failed for ${label} with status ${String(result.status)}: ${diagnostic}`
    );
  }
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(
      `official npm signature verification returned invalid JSON for ${label}: ${error.message}`
    );
  }
}

function parseArguments(arguments_) {
  let candidate = null;
  let output = null;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--candidate" && candidate === null) {
      candidate = arguments_[index + 1] ?? null;
      index += 1;
      continue;
    }
    if (argument === "--output" && output === null) {
      output = arguments_[index + 1] ?? null;
      index += 1;
      continue;
    }
    throw new Error(
      "usage: node zenuml-core-candidate-matrix.mjs --candidate <version> [--output <path>]"
    );
  }
  if (!candidate || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(candidate)) {
    throw new Error("--candidate must be an exact npm version");
  }
  if (output !== null && output.length === 0) {
    throw new Error("--output must name a file");
  }
  return { candidate, output };
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function runNpm(arguments_, options = {}) {
  return spawnSync(npmCommand(), arguments_, {
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    ...options,
  });
}

function requireSuccessfulCommand(result, label) {
  if (result.error) {
    throw new Error(`${label} could not start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `${label} failed with status ${String(result.status)}: ${String(
        result.stderr || result.stdout || "no diagnostic"
      ).trim()}`
    );
  }
  return result.stdout.trim();
}

async function materialize(version, id, temporaryRoot) {
  const root = path.join(temporaryRoot, id);
  await mkdir(root);
  const result = runNpm(
    [
      "install",
      "--ignore-scripts",
      `--registry=${officialNpmRegistry}`,
      "--no-audit",
      "--no-fund",
      "--package-lock=true",
      "--save-exact",
      `${packageName}@${version}`,
    ],
    { cwd: root }
  );
  requireSuccessfulCommand(result, `${id} package materialization`);
  const lock = JSON.parse(await readFile(path.join(root, "package-lock.json"), "utf8"));
  const installed = lock.packages[`node_modules/${packageName}`];
  assert(installed, `${id} lock does not contain ${packageName}`);
  assert.equal(installed.version, version);
  assert.match(installed.integrity, /^sha512-/u);
  assert.equal(installed.resolved.startsWith(officialNpmRegistry), true);
  const distRoot = path.join(root, "node_modules", "@zenuml", "core", "dist");
  const runtimeEntry = path.join(distRoot, "zenuml.esm.mjs");
  return {
    id,
    version,
    root,
    distRoot,
    integrity: installed.integrity,
    tarballUrl: installed.resolved,
    runtimeEntryBytes: (await stat(runtimeEntry)).size,
  };
}

function verifyOfficialSignatures(materialized) {
  const result = runNpm(
    [
      "audit",
      "signatures",
      "--json",
      "--include-attestations",
      `--registry=${officialNpmRegistry}`,
    ],
    { cwd: materialized.root }
  );
  assertOfficialSignatureAuditResult(
    result,
    `${packageName}@${materialized.version}`
  );
  return {
    package: packageName,
    version: materialized.version,
    integrity: materialized.integrity,
    rawOutputSha256: sha256(result.stdout),
    rawOutput: result.stdout,
  };
}

async function loadCorpus() {
  const fixtureRoot = path.join(workspaceRoot, "fixtures", "zenuml");
  const names = (await readdir(fixtureRoot))
    .filter((name) => name.endsWith(".mmd"))
    .sort();
  return Promise.all(
    names.map(async (name) => ({
      name,
      source: (await readFile(path.join(fixtureRoot, name), "utf8")).replace(
        /^\s*zenuml\s*(?:\r?\n|$)/u,
        ""
      ),
    }))
  );
}

async function loadInlineSvgValidator() {
  const source = await readFile(
    path.join(workspaceRoot, "platforms/web/src/svg-safety-policy.ts"),
    "utf8"
  );
  const transpiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: "svg-safety-policy.ts",
    reportDiagnostics: true,
  });
  const errors = (transpiled.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error
  );
  assert.deepEqual(errors, [], "strict inline SVG validator failed to transpile");
  const encoded = Buffer.from(transpiled.outputText).toString("base64");
  const module = await import(`data:text/javascript;base64,${encoded}`);
  return module.assertSelfContainedSvgWithMessagePrefix;
}

async function observeBehavior(current, candidate, corpus) {
  const server = await serveModules({
    current: current.distRoot,
    candidate: candidate.distRoot,
  });
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(`${server.origin}/`);
    return await page.evaluate(
      async ({ entries, fixtures }) => {
        const digest = async (value) =>
          Array.from(
            new Uint8Array(
              await crypto.subtle.digest(
                "SHA-256",
                new TextEncoder().encode(value)
              )
            ),
            (byte) => byte.toString(16).padStart(2, "0")
          ).join("");
        const engines = [];
        for (const [id, url] of entries) {
          const module = await import(url);
          const parser = Object.create(module.default.prototype);
          const rows = [];
          for (const fixture of fixtures) {
            const parse = await parser.parse(fixture.source);
            const rendered = module.renderToSvg(fixture.source);
            rows.push({
              name: fixture.name,
              parse,
              svg: rendered.svg,
              svgSha256: await digest(rendered.svg),
            });
          }
          engines.push({ id, version: module.default.version, rows });
        }
        return engines;
      },
      {
        entries: [
          ["current", `${server.origin}/current/zenuml.esm.mjs`],
          ["candidate", `${server.origin}/candidate/zenuml.esm.mjs`],
        ],
        fixtures: corpus,
      }
    );
  } finally {
    await browser.close();
    await server.close();
  }
}

async function verifyBehavior(current, candidate, corpus) {
  const validator = await loadInlineSvgValidator();
  const observations = await observeBehavior(current, candidate, corpus);
  const currentRows = observations.find(({ id }) => id === "current")?.rows;
  const candidateRows = observations.find(({ id }) => id === "candidate")?.rows;
  assert(currentRows && candidateRows, "browser observation lost an engine");
  assert.equal(currentRows.length, corpus.length);
  assert.equal(candidateRows.length, corpus.length);
  let parseAgreementCount = 0;
  let svgAgreementCount = 0;
  for (let index = 0; index < corpus.length; index += 1) {
    const currentRow = currentRows[index];
    const candidateRow = candidateRows[index];
    assert.equal(candidateRow.name, currentRow.name);
    if (JSON.stringify(candidateRow.parse) === JSON.stringify(currentRow.parse)) {
      parseAgreementCount += 1;
    }
    if (candidateRow.svgSha256 === currentRow.svgSha256) {
      svgAgreementCount += 1;
    }
    validator(candidateRow.svg, `ZenUML admission fixture ${candidateRow.name}`);
  }
  assert.equal(
    parseAgreementCount,
    corpus.length,
    "candidate parser behavior differs from the selected graph"
  );
  assert.equal(
    svgAgreementCount,
    corpus.length,
    "candidate SVG behavior differs from the selected graph"
  );
  return {
    result: "pass",
    fixtureCount: corpus.length,
    parseAgreementCount,
    svgAgreementCount,
    strictInlineSvgCount: corpus.length,
  };
}

async function serveModules(roots) {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://127.0.0.1");
      if (url.pathname === "/") {
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.end("<!doctype html><meta charset=utf-8>");
        return;
      }
      const [, id, ...segments] = url.pathname.split("/");
      const root = roots[id];
      assert(root, "unknown module root");
      const target = path.resolve(root, ...segments.map(decodeURIComponent));
      assert(target.startsWith(`${path.resolve(root)}${path.sep}`), "path escaped module root");
      response.setHeader("content-type", contentType(target));
      response.end(await readFile(target));
    } catch (error) {
      response.statusCode = 404;
      response.end(String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  assert(address && typeof address !== "string");
  return {
    origin: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve()))
      ),
  };
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".js":
    case ".mjs":
      return "text/javascript; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".ttf":
      return "font/ttf";
    default:
      return "application/octet-stream";
  }
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const bundle = JSON.parse(await readFile(bundlePath, "utf8"));
  const zenuml = bundle.externalDiagrams.find(({ id }) => id === "zenuml");
  assert(zenuml?.behavior, "selected reference bundle has no ZenUML behavior package");
  assert.equal(zenuml.behavior.package, packageName);
  assert.notEqual(
    options.candidate,
    zenuml.behavior.version,
    "candidate version must differ from the selected version"
  );
  const npmVersion = requireSuccessfulCommand(runNpm(["--version"]), "npm --version");
  assert.equal(
    npmVersion,
    requiredNpmVersion,
    `Mermaid admission requires npm ${requiredNpmVersion}`
  );
  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "merman-zenuml-admission-"));
  try {
    const current = await materialize(zenuml.behavior.version, "current", temporaryRoot);
    const candidate = await materialize(options.candidate, "candidate", temporaryRoot);
    assert.equal(current.integrity, zenuml.behavior.integrity);
    assert.equal(current.tarballUrl, zenuml.behavior.tarballUrl);
    const officialVerification = [
      verifyOfficialSignatures(current),
      verifyOfficialSignatures(candidate),
    ];
    const behavior = await verifyBehavior(
      current,
      candidate,
      await loadCorpus()
    );
    const report = {
      schemaVersion: 1,
      artifactKind: "mermaid-upgrade-admission-report",
      generatedBy: "playground/scripts/zenuml-core-candidate-matrix.mjs",
      toolchain: {
        node: process.version,
        npm: npmVersion,
        signatureCommand:
          "npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/",
      },
      selected: {
        package: packageName,
        version: current.version,
        integrity: current.integrity,
        runtimeEntryBytes: current.runtimeEntryBytes,
      },
      candidate: {
        package: packageName,
        version: candidate.version,
        integrity: candidate.integrity,
        runtimeEntryBytes: candidate.runtimeEntryBytes,
      },
      officialVerification,
      behavior,
    };
    const serialized = `${JSON.stringify(report, null, 2)}\n`;
    if (options.output) {
      await mkdir(path.dirname(path.resolve(options.output)), { recursive: true });
      await writeFile(path.resolve(options.output), serialized, { flag: "wx" });
    } else {
      process.stdout.write(serialized);
    }
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  await main();
}
