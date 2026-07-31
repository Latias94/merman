/* eslint-disable no-console */

// Bench upstream Mermaid JS rendering via the same pinned toolchain used for parity SVG baselines:
// - Launch a single headless Chromium instance (puppeteer)
// - Load mermaid-cli's dist HTML + Mermaid IIFE bundle
// - Measure repeated `mermaid.render(...)` calls in-page (warm + measure loops)
//
// This intentionally does NOT include browser startup time in per-iteration timings.

const fs = require("fs");
const path = require("path");
const puppeteer = require(require.resolve("puppeteer", { paths: [process.cwd()] }));

const OUTPUT_SCHEMA_VERSION = 3;
const MAX_TIMER_MS = 2_147_483_647;
const MAX_SAMPLES = 1_000_000;
const PROTOCOL_TIMEOUT_GRACE_MS = 1_000;
const INPUT_KEYS = new Set([
  "fixtures",
  "configPath",
  "theme",
  "seed",
  "width",
  "warmupMs",
  "measureMs",
  "maxSamples",
  "navigationTimeoutMs",
  "fixtureTimeoutMs",
]);

class WatchdogError extends Error {
  constructor(stage, timeoutMs) {
    super(`${stage} timed out after ${timeoutMs} ms`);
    this.name = "WatchdogError";
    this.stage = stage;
    this.timeoutMs = timeoutMs;
  }
}

function errorMessage(error) {
  return error && error.message ? String(error.message) : String(error);
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireExactKeys(value, allowedKeys, label) {
  for (const key of Object.keys(value)) {
    if (!allowedKeys.has(key)) {
      throw new TypeError(`${label} contains unknown field ${JSON.stringify(key)}`);
    }
  }
  for (const key of allowedKeys) {
    if (!Object.prototype.hasOwnProperty.call(value, key)) {
      throw new TypeError(`${label}.${key} is required`);
    }
  }
}

function requireNonEmptyString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return value;
}

function requirePositiveInteger(value, label, maximum = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value <= 0 || value > maximum) {
    throw new TypeError(`${label} must be a positive integer no greater than ${maximum}`);
  }
  return value;
}

function validateInput(value) {
  if (!isObject(value)) {
    throw new TypeError("input must be an object");
  }
  requireExactKeys(value, INPUT_KEYS, "input");

  if (!isObject(value.fixtures)) {
    throw new TypeError("input.fixtures must be an object");
  }
  const fixtureEntries = Object.entries(value.fixtures);
  if (fixtureEntries.length === 0) {
    throw new TypeError("input.fixtures must not be empty");
  }
  const fixtures = Object.create(null);
  for (const [name, code] of fixtureEntries) {
    if (
      name.length > 128 ||
      !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(name) ||
      name === "__proto__" ||
      name === "constructor" ||
      name === "prototype"
    ) {
      throw new TypeError(
        `input.fixtures contains an invalid fixture name: ${JSON.stringify(name)}`
      );
    }
    fixtures[name] = requireNonEmptyString(code, `input.fixtures[${JSON.stringify(name)}]`);
  }

  const seed = requireNonEmptyString(value.seed, "input.seed");
  if (!/^-?[0-9]+$/.test(seed) || seed.length > 128) {
    throw new TypeError("input.seed must be a base-10 integer string of at most 128 characters");
  }

  const warmupMs = requirePositiveInteger(value.warmupMs, "input.warmupMs", MAX_TIMER_MS);
  const measureMs = requirePositiveInteger(value.measureMs, "input.measureMs", MAX_TIMER_MS);
  const fixtureTimeoutMs = requirePositiveInteger(
    value.fixtureTimeoutMs,
    "input.fixtureTimeoutMs",
    MAX_TIMER_MS
  );
  if (fixtureTimeoutMs <= warmupMs + measureMs) {
    throw new TypeError("input.fixtureTimeoutMs must exceed warmupMs + measureMs");
  }

  return {
    fixtures,
    configPath: requireNonEmptyString(value.configPath, "input.configPath"),
    theme: requireNonEmptyString(value.theme, "input.theme"),
    seed,
    width: requirePositiveInteger(value.width, "input.width"),
    warmupMs,
    measureMs,
    maxSamples: requirePositiveInteger(value.maxSamples, "input.maxSamples", MAX_SAMPLES),
    navigationTimeoutMs: requirePositiveInteger(
      value.navigationTimeoutMs,
      "input.navigationTimeoutMs",
      MAX_TIMER_MS
    ),
    fixtureTimeoutMs,
  };
}

async function withWatchdog(operation, timeoutMs, stage) {
  let timer;
  const watchdog = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new WatchdogError(stage, timeoutMs)), timeoutMs);
  });
  try {
    return await Promise.race([Promise.resolve().then(operation), watchdog]);
  } finally {
    clearTimeout(timer);
  }
}

function isTimeoutFailure(error) {
  return (
    error instanceof WatchdogError ||
    (error && error.name === "TimeoutError") ||
    (error &&
      error.name === "ProtocolError" &&
      /timed out.*protocolTimeout/i.test(errorMessage(error)))
  );
}

function validatePreflight(value) {
  if (
    !isObject(value) ||
    !Number.isSafeInteger(value.svg_chars) ||
    value.svg_chars <= 0 ||
    !Number.isSafeInteger(value.svg_bytes) ||
    value.svg_bytes <= 0 ||
    typeof value.svg_sha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(value.svg_sha256) ||
    !Array.isArray(value.view_box) ||
    value.view_box.length !== 4 ||
    value.view_box.some((number) => typeof number !== "number" || !Number.isFinite(number)) ||
    value.view_box[2] <= 0 ||
    value.view_box[3] <= 0
  ) {
    throw new TypeError("page.evaluate returned an invalid SVG preflight receipt");
  }
  return value;
}

function validateMeasurement(value, maxSamples) {
  if (!isObject(value)) {
    throw new TypeError("page.evaluate returned a non-object measurement");
  }
  const timesNs = value.timesNs;
  if (
    !Array.isArray(timesNs) ||
    timesNs.length === 0 ||
    timesNs.length > maxSamples ||
    timesNs.some((sample) => typeof sample !== "number" || !Number.isFinite(sample) || sample <= 0)
  ) {
    throw new TypeError("page.evaluate returned invalid raw timing samples");
  }
  if (value.stopReason !== "measurement_time" && value.stopReason !== "max_samples") {
    throw new TypeError("page.evaluate returned an invalid measurement stop reason");
  }
  if (value.stopReason === "max_samples" && timesNs.length !== maxSamples) {
    throw new TypeError("page.evaluate reported max_samples before reaching the sample cap");
  }
  if (value.stopReason === "measurement_time" && timesNs.length >= maxSamples) {
    throw new TypeError("page.evaluate reached the sample cap without reporting max_samples");
  }
  return {
    timesNs,
    preflight: validatePreflight(value.preflight),
    stopReason: value.stopReason,
  };
}

function forceKillBrowser(browser) {
  try {
    const child = browser.process();
    if (child && child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
    }
  } catch (error) {
    console.error("[mermaid-js-bench] failed to force-stop Chromium:", errorMessage(error));
  }
}

async function closeResources(page, browser, timeoutMs) {
  const errors = [];
  if (page) {
    try {
      if (!page.isClosed()) {
        await withWatchdog(
          () => page.close({ runBeforeUnload: false }),
          timeoutMs,
          "page cleanup"
        );
      }
    } catch (error) {
      errors.push(error);
    }
  }
  if (browser) {
    try {
      await withWatchdog(() => browser.close(), timeoutMs, "browser cleanup");
    } catch (error) {
      forceKillBrowser(browser);
      errors.push(error);
    }
  }
  if (errors.length > 0) {
    throw new Error(`browser cleanup failed: ${errors.map(errorMessage).join("; ")}`);
  }
}

function usage() {
  return (
    "usage: node mermaid_js_bench.cjs --in <json> --out <json>\n" +
    "\n" +
    "Input JSON:\n" +
    "  {\n" +
    '    "fixtures": { "flowchart_tiny": "flowchart LR\\n  A-->B\\n", ... },\n' +
    '    "configPath": "../tools/mermaid-config.json",\n' +
    '    "theme": "default",\n' +
    '    "seed": "1",\n' +
    '    "width": 800,\n' +
    '    "warmupMs": 1000,\n' +
    '    "measureMs": 1000,\n' +
    '    "maxSamples": 10000,\n' +
    '    "navigationTimeoutMs": 30000,\n' +
    '    "fixtureTimeoutMs": 62000\n' +
    "  }\n"
  );
}

function parseArgs(argv) {
  const out = { inPath: null, outPath: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--in") {
      if (out.inPath !== null) {
        return { error: "duplicate --in" };
      }
      if (i + 1 >= argv.length) {
        return { error: "missing value for --in" };
      }
      out.inPath = argv[++i];
    } else if (a === "--out") {
      if (out.outPath !== null) {
        return { error: "duplicate --out" };
      }
      if (i + 1 >= argv.length) {
        return { error: "missing value for --out" };
      }
      out.outPath = argv[++i];
    } else if (a === "--help" || a === "-h") {
      return { help: true };
    } else {
      return { error: "unknown arg: " + a };
    }
  }
  if (!out.inPath || !out.outPath) {
    return { error: "missing --in/--out" };
  }
  return out;
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  if (args.error) {
    console.error(args.error);
    console.error(usage());
    process.exit(2);
  }

  const input = validateInput(JSON.parse(fs.readFileSync(args.inPath, "utf8")));
  const {
    fixtures,
    theme,
    seed: seedStr,
    width,
    warmupMs,
    measureMs,
    maxSamples,
    navigationTimeoutMs,
    fixtureTimeoutMs,
  } = input;

  // Run under `tools/mermaid-cli` so node can resolve puppeteer + mermaid deps.
  const cliRoot = process.cwd();
  const meta = {
    node: process.version,
    platform: process.platform,
    arch: process.arch,
  };
  const mermaidHtmlPath = path.join(
    cliRoot,
    "node_modules",
    "@mermaid-js",
    "mermaid-cli",
    "dist",
    "index.html"
  );
  const mermaidIifePath = path.join(
    cliRoot,
    "node_modules",
    "mermaid",
    "dist",
    "mermaid.js"
  );
  const zenumlIifePath = path.join(
    cliRoot,
    "node_modules",
    "@mermaid-js",
    "mermaid-zenuml",
    "dist",
    "mermaid-zenuml.js"
  );

  try {
    meta.mermaid = require(path.join(cliRoot, "node_modules", "mermaid", "package.json")).version;
  } catch {
    // ignore
  }
  try {
    meta.mermaid_cli = require(
      path.join(cliRoot, "node_modules", "@mermaid-js", "mermaid-cli", "package.json")
    ).version;
  } catch {
    // ignore
  }
  try {
    meta.mermaid_zenuml = require(
      path.join(cliRoot, "node_modules", "@mermaid-js", "mermaid-zenuml", "package.json")
    ).version;
  } catch {
    // ignore
  }

  const configPath = path.resolve(cliRoot, input.configPath);
  const cfg = JSON.parse(fs.readFileSync(configPath, "utf8"));
  if (!isObject(cfg)) {
    throw new TypeError("Mermaid config must be an object");
  }

  const launchOpts = {
    headless: "shell",
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
    timeout: navigationTimeoutMs,
    protocolTimeout: Math.min(
      MAX_TIMER_MS,
      Math.max(navigationTimeoutMs, fixtureTimeoutMs) + PROTOCOL_TIMEOUT_GRACE_MS
    ),
  };
  const method = {
    measurement_stop_conditions: {
      measure_ms: measureMs,
      max_samples: maxSamples,
    },
    watchdogs: {
      navigation_timeout_ms: navigationTimeoutMs,
      fixture_timeout_ms: fixtureTimeoutMs,
    },
  };
  let browser = null;
  let page = null;
  let output = null;
  let primaryError = null;

  try {
    browser = await puppeteer.launch(launchOpts);
    page = await browser.newPage();
    page.setDefaultNavigationTimeout(navigationTimeoutMs);
    page.setDefaultTimeout(navigationTimeoutMs);

    try {
      meta.chromium = await withWatchdog(
        () => browser.version(),
        navigationTimeoutMs,
        "Chromium version query"
      );
    } catch (error) {
      if (isTimeoutFailure(error)) throw error;
    }
    try {
      meta.user_agent = await withWatchdog(
        () => page.evaluate(() => navigator.userAgent),
        navigationTimeoutMs,
        "user-agent query"
      );
    } catch (error) {
      if (isTimeoutFailure(error)) throw error;
    }
    try {
      if (typeof puppeteer.version === "function") {
        meta.puppeteer = puppeteer.version();
      }
    } catch {
      // ignore
    }

    // Seed Math.random + crypto.getRandomValues for stability.
    await withWatchdog(
      () =>
        page.evaluateOnNewDocument((seedStr2) => {
          const mask64 = (1n << 64n) - 1n;
          let state = BigInt(seedStr2) & mask64;
          if (state === 0n) state = 1n;

          function nextU64() {
            let x = state;
            x ^= x >> 12n;
            x ^= (x << 25n) & mask64;
            x ^= x >> 27n;
            state = x;
            return (x * 0x2545f4914f6cdd1dn) & mask64;
          }

          function nextF64() {
            const u = nextU64() >> 11n;
            return Number(u) / 9007199254740992; // 2^53
          }

          Math.random = nextF64;

          if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === "function") {
            const orig = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
            globalThis.crypto.getRandomValues = (arr) => {
              if (!arr || typeof arr.length !== "number") {
                return orig(arr);
              }

              // Support both Number-typed and BigInt-typed arrays.
              // Some libraries use BigInt64Array/BigUint64Array for RNG seeding.
              if (
                typeof BigInt64Array !== "undefined" &&
                (arr instanceof BigInt64Array || arr instanceof BigUint64Array)
              ) {
                for (let i = 0; i < arr.length; i++) {
                  const u = nextU64();
                  if (arr instanceof BigInt64Array) {
                    // signed
                    arr[i] = BigInt.asIntN(64, u);
                  } else {
                    arr[i] = BigInt.asUintN(64, u);
                  }
                }
                return arr;
              }

              const bits = Number(arr.BYTES_PER_ELEMENT || 1) * 8;
              const max = bits >= 53 ? 2 ** 32 : 2 ** bits;
              for (let i = 0; i < arr.length; i++) {
                arr[i] = Math.floor(nextF64() * max);
              }
              return arr;
            };
          }
        }, seedStr),
      navigationTimeoutMs,
      "new-document initialization"
    );
    await withWatchdog(
      () =>
        page.setViewport({
          width,
          height: 600,
          deviceScaleFactor: 1,
        }),
      navigationTimeoutMs,
      "viewport setup"
    );
    await withWatchdog(
      () =>
        page.goto("file://" + mermaidHtmlPath.replace(/\\/g, "/"), {
          timeout: 0,
          waitUntil: "load",
        }),
      navigationTimeoutMs,
      "navigation"
    );
    await withWatchdog(
      () => page.addScriptTag({ path: mermaidIifePath }),
      navigationTimeoutMs,
      "Mermaid script loading"
    );
    if (Object.values(fixtures).some((code) => /^\s*zenuml\b/.test(code))) {
      await withWatchdog(
        () => page.addScriptTag({ path: zenumlIifePath }),
        navigationTimeoutMs,
        "ZenUML script loading"
      );
      await withWatchdog(
        () =>
          page.evaluate(async () => {
            const mermaid = globalThis.mermaid;
            const zenuml = globalThis["mermaid-zenuml"];
            if (!mermaid || !zenuml) {
              throw new Error("Mermaid ZenUML plugin failed to load");
            }
            await mermaid.registerExternalDiagrams([zenuml], { lazyLoad: false });
          }),
        navigationTimeoutMs,
        "ZenUML registration"
      );
    }

    const results = Object.create(null);
    for (const [name, code] of Object.entries(fixtures)) {
      let measurement;
      try {
        measurement = await withWatchdog(
          () =>
            page.evaluate(
              async ({
                code2,
                cfg2,
                theme2,
                width2,
                warmupMs2,
                measureMs2,
                maxSamples2,
                name2,
              }) => {
                const mermaid = globalThis.mermaid;
                if (!mermaid) throw new Error("mermaid global not found");

                // Initialize once per fixture.
                mermaid.initialize(Object.assign({ startOnLoad: false, theme: theme2 }, cfg2));

                const container = document.getElementById("container") || document.body;
                container.innerHTML = "";
                container.style.width = `${Math.max(1, Number(width2) || 1)}px`;

                async function renderOne(i) {
                  container.innerHTML = "";
                  const { svg } = await mermaid.render(`${name2}-${i}`, code2, container);
                  if (typeof svg !== "string" || svg.length === 0) {
                    throw new Error("Mermaid returned an empty SVG");
                  }
                  return svg;
                }

                async function one(i) {
                  const svg = await renderOne(i);
                  return svg.length;
                }

                const preflightSvg = await renderOne("preflight");
                const parsedDocument = new DOMParser().parseFromString(
                  preflightSvg,
                  "image/svg+xml"
                );
                const root = parsedDocument.documentElement;
                if (!root || root.localName !== "svg") {
                  throw new Error("Mermaid output is not an SVG document");
                }
                const viewBox = root.getAttribute("viewBox");
                const viewBoxNumbers = viewBox ? viewBox.trim().split(/[ ,]+/).map(Number) : [];
                if (
                  viewBoxNumbers.length !== 4 ||
                  viewBoxNumbers.some((value) => !Number.isFinite(value)) ||
                  viewBoxNumbers[2] <= 0 ||
                  viewBoxNumbers[3] <= 0
                ) {
                  throw new Error("Mermaid SVG has no finite four-number viewBox");
                }
                const svgBytes = new TextEncoder().encode(preflightSvg);
                const svgDigest = await crypto.subtle.digest("SHA-256", svgBytes);
                const svgSha256 = Array.from(new Uint8Array(svgDigest))
                  .map((byte) => byte.toString(16).padStart(2, "0"))
                  .join("");

                // Warmup until wall clock threshold.
                const t0 = performance.now();
                let i = 0;
                while (performance.now() - t0 < warmupMs2) {
                  // eslint-disable-next-line no-await-in-loop
                  await one(i++);
                }

                // Measure until the first preregistered threshold is reached.
                const samples = [];
                const m0 = performance.now();
                let j = 0;
                do {
                  const s0 = performance.now();
                  // eslint-disable-next-line no-await-in-loop
                  await one(j++);
                  const s1 = performance.now();
                  samples.push((s1 - s0) * 1e6); // ms -> ns
                } while (performance.now() - m0 < measureMs2 && samples.length < maxSamples2);

                const stopReason =
                  samples.length >= maxSamples2 ? "max_samples" : "measurement_time";

                return {
                  timesNs: samples,
                  stopReason,
                  preflight: {
                    svg_chars: preflightSvg.length,
                    svg_bytes: svgBytes.length,
                    svg_sha256: svgSha256,
                    view_box: viewBoxNumbers,
                  },
                };
              },
              {
                code2: code,
                cfg2: cfg,
                theme2: theme,
                width2: width,
                warmupMs2: warmupMs,
                measureMs2: measureMs,
                maxSamples2: maxSamples,
                name2: name,
              }
            ),
          fixtureTimeoutMs,
          `fixture ${JSON.stringify(name)} evaluate/render`
        );
      } catch (err) {
        if (isTimeoutFailure(err)) {
          throw err;
        }
        results[name] = {
          times_ns: [],
          preflight: null,
          error: errorMessage(err),
        };
        // eslint-disable-next-line no-console
        console.error("[mermaid-js-bench] fixture failed:", name, results[name].error);
        continue;
      }

      try {
        measurement = validateMeasurement(measurement, maxSamples);
      } catch (error) {
        results[name] = {
          times_ns: [],
          preflight: null,
          error: errorMessage(error),
        };
        // eslint-disable-next-line no-console
        console.error("[mermaid-js-bench] fixture failed:", name, results[name].error);
        continue;
      }

      results[name] = {
        times_ns: measurement.timesNs,
        preflight: measurement.preflight,
        stop_reason: measurement.stopReason,
        sample_cap: maxSamples,
        samples_truncated: false,
      };
    }

    output = {
      schema_version: OUTPUT_SCHEMA_VERSION,
      meta,
      method,
      results,
    };
  } catch (error) {
    primaryError = error;
    throw error;
  } finally {
    try {
      await closeResources(page, browser, navigationTimeoutMs);
    } catch (cleanupError) {
      if (primaryError !== null) {
        console.error("[mermaid-js-bench]", errorMessage(cleanupError));
      } else {
        throw cleanupError;
      }
    }
  }

  fs.writeFileSync(args.outPath, JSON.stringify(output, null, 2), "utf8");
}

main().catch((err) => {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
});
