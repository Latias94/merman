import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { assertSuccessfulNpmSpawn, npmPackRecord, spawnNpmSync } from "../../../scripts/npm-command.mjs";

const webRoot = fileURLToPath(new URL("../", import.meta.url));
const packageIds = ["full", "render", "analysis", "editor", "ascii"];
const csp = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; "
  + "connect-src 'self'; style-src 'unsafe-inline'; img-src 'self' data:; "
  + "base-uri 'none'; object-src 'none'; frame-ancestors 'none'";

export function assertRequestLog({ baseUrl, requests, wasmUrls }) {
  const base = new URL(baseUrl);
  for (const request of requests) {
    const url = new URL(request);
    assert.ok(url.origin === base.origin && url.pathname.startsWith(base.pathname),
      `Resource outside the consumer base: ${request}`);
  }
  const wasmRequests = requests.filter(url => new URL(url).pathname.endsWith(".wasm"));
  assert.deepEqual([...wasmRequests].sort(), [...wasmUrls].sort(),
    "WASM requests must contain exactly the main and Worker overrides, without a default or repeated load");
}

async function main() {
  const { values } = parseArgs({ options: {
    help: { type: "boolean" },
    package: { type: "string" },
    "packages-root": { type: "string", default: path.join(webRoot, "packages") },
    "vite-module": { type: "string" },
    "playwright-module": { type: "string" },
    "chromium-executable": { type: "string" },
  } });
  if (values.help) {
    console.log(`Usage: node platforms/web/scripts/installed-browser-consumer.mjs [options]

Packs and installs each assembled Web package alone, offline and without lifecycle scripts.
Builds a Vite production consumer and executes its public facade in Chromium and a module Worker.
Requires Vite from playground and Playwright plus Chromium from playground/tests; no Cargo is run.

  --package <${packageIds.join("|")}>  Run one package (default: all five)
  --packages-root <directory>  Assembled package directories (default: platforms/web/packages)
  --vite-module <path>         Vite module entry, resolved from playground by default
  --playwright-module <path>   Playwright module, resolved from playground/tests by default
  --chromium-executable <path> Actual Chromium executable (default: Playwright's installed browser)

Focused tests: node --test platforms/web/scripts/installed-browser-consumer.test.mjs
Runtime: node platforms/web/scripts/installed-browser-consumer.mjs
Each passing package prints a JSON receipt with its tarball digest and browser request evidence.`);
    return;
  }
  if (values.package && !packageIds.includes(values.package)) {
    throw new Error(`Unknown package: ${values.package}`);
  }
  const viteRequire = createRequire(new URL("../../../playground/package.json", import.meta.url));
  const playwrightRequire = createRequire(new URL("../../../playground/tests/package.json", import.meta.url));
  const { build } = await import(pathToFileURL(viteRequire.resolve(values["vite-module"] ?? "vite")).href);
  const { chromium } = playwrightRequire(values["playwright-module"] ?? "playwright");
  const browser = await chromium.launch({
    headless: true,
    ...(values["chromium-executable"] ? { executablePath: values["chromium-executable"] } : {}),
  });
  try {
    for (const id of values.package ? [values.package] : packageIds) {
      const receipt = await consumePackage({
        id, packageRoot: path.resolve(values["packages-root"], id), build, browser,
      });
      console.log(JSON.stringify(receipt));
    }
  } finally {
    await browser.close();
  }
}

async function consumePackage({ id, packageRoot, build, browser }) {
  const scratch = await mkdtemp(path.join(os.tmpdir(), "merman-installed-browser-"));
  let server;
  let context;
  try {
    const manifest = JSON.parse(await readFile(path.join(packageRoot, "package.json"), "utf8"));
    const project = path.join(scratch, "consumer");
    await mkdir(project);
    const npm = args => {
      const result = spawnNpmSync(args, {
        cwd: project, encoding: "utf8",
        env: { ...process.env, npm_config_cache: path.join(scratch, "npm-cache") },
        stdio: ["ignore", "pipe", "pipe"],
      });
      assertSuccessfulNpmSpawn(result, "Installed browser consumer npm command");
      return result.stdout;
    };
    const packed = npmPackRecord(npm([
      "pack", packageRoot, "--json", "--offline", "--ignore-scripts", "--pack-destination", scratch,
    ]), manifest.name, { allowNpm11: true });
    const tarball = path.join(scratch, packed.filename);
    const tarballSha256 = createHash("sha256").update(await readFile(tarball)).digest("hex");
    await writeFile(path.join(project, "package.json"), JSON.stringify({
      name: `merman-installed-consumer-${id}`, private: true, type: "module",
    }));
    npm(["install", tarball, "--offline", "--ignore-scripts", "--no-audit", "--no-fund",
      "--package-lock=false", "--workspaces=false"]);
    const installed = JSON.parse(await readFile(
      path.join(project, "node_modules", manifest.name, "package.json"), "utf8",
    ));
    assert.equal(installed.name, manifest.name);
    assert.equal(installed.version, manifest.version);
    assert.deepEqual(await readdir(path.join(project, "node_modules", "@mermanjs")),
      [path.basename(manifest.name)], "Install one task package independently");
    await cp(path.join(webRoot, "tests", "installed-consumer"), project, { recursive: true });
    const exerciseFile = path.join(project, "exercise.mjs");
    await writeFile(exerciseFile,
      (await readFile(exerciseFile, "utf8")).replaceAll("__MERMAN_PACKAGE__", manifest.name));
    const base = `/installed-consumer/${id}/`;
    await build({
      root: project, configFile: false, envDir: false, base, publicDir: false,
      cacheDir: path.join(scratch, "vite-cache"), logLevel: "warn",
      define: { __MERMAN_PACKAGE_ID__: JSON.stringify(id) },
      build: { assetsInlineLimit: 0, target: "es2022" },
      worker: { format: "es" },
    });

    const responses = [];
    server = createServer(async (request, response) => {
      const requestUrl = new URL(request.url, "http://localhost");
      let status = 404;
      try {
        assert.ok(requestUrl.pathname.startsWith(base));
        const relative = decodeURIComponent(requestUrl.pathname.slice(base.length)) || "index.html";
        const dist = path.join(project, "dist");
        const file = path.resolve(dist, relative);
        assert.ok(file.startsWith(dist + path.sep));
        const body = await readFile(file);
        const contentType = file.endsWith(".wasm") ? "application/wasm"
          : file.endsWith(".html") ? "text/html; charset=utf-8" : "text/javascript; charset=utf-8";
        status = 200;
        response.writeHead(status, {
          "Content-Type": contentType, "Content-Security-Policy": csp, "Cache-Control": "no-store",
        });
        response.end(body);
        responses.push({ url: request.url, status, contentType });
      } catch {
        response.writeHead(status).end();
        responses.push({ url: request.url, status });
      }
    });
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", resolve);
    });
    const baseUrl = `http://127.0.0.1:${server.address().port}${base}`;
    context = await browser.newContext({ serviceWorkers: "block" });
    const requests = [];
    const errors = [];
    context.on("request", request => requests.push(request.url()));
    context.on("requestfailed", request => errors.push(`${request.url()}: ${request.failure()?.errorText}`));
    const page = await context.newPage();
    page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.goto(baseUrl);
    await page.waitForFunction(() => "__installedConsumer" in globalThis, undefined, { timeout: 60_000 });
    const result = await page.evaluate(() => globalThis.__installedConsumer);
    await page.waitForLoadState("networkidle");
    assert.deepEqual(errors, [], "Browser execution must not report errors");
    assert.deepEqual(result.cspViolations, [], "The main thread and Worker must honor WASM CSP");
    for (const [realm, proof] of [["main", result.main], ["worker", result.worker]]) {
      assert.equal(proof.realm, realm);
      assert.equal(proof.version, manifest.version, "Installed facade and WASM versions agree");
      assert.ok(proof.operations.length > 0, `${realm} executed a public operation`);
    }
    assertRequestLog({ baseUrl, requests, wasmUrls: [result.main.wasmUrl, result.worker.wasmUrl] });
    assert.ok(responses.every(response => response.status === 200), "Every deployed asset must load");
    assert.equal(responses.filter(response => response.contentType === "application/wasm").length, 2);
    return {
      package: manifest.name, version: manifest.version, tarballSha256,
      node: process.version, platform: process.platform, arch: process.arch,
      browser: browser.version(), baseUrl, csp, ...result, requests, responses,
    };
  } finally {
    await context?.close();
    if (server) await new Promise(resolve => server.close(resolve));
    await rm(scratch, { recursive: true, force: true });
  }
}

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main().catch(error => {
    console.error(error.stack ?? error);
    process.exitCode = 1;
  });
}
