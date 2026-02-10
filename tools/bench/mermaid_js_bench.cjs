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
    '    "measureMs": 1000\n' +
    "  }\n"
  );
}

function parseArgs(argv) {
  const out = { inPath: null, outPath: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--in") {
      out.inPath = argv[++i];
    } else if (a === "--out") {
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

function median(values) {
  if (!values.length) return null;
  const v = values.slice().sort((a, b) => a - b);
  const mid = Math.floor(v.length / 2);
  if (v.length % 2 === 1) return v[mid];
  return (v[mid - 1] + v[mid]) / 2;
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

  const input = JSON.parse(fs.readFileSync(args.inPath, "utf8"));
  const fixtures = input.fixtures || {};
  const theme = String(input.theme || "default");
  const seedStr = String(input.seed || "1");
  const width = Math.max(1, Number(input.width || 800));
  const warmupMs = Math.max(1, Number(input.warmupMs || 1000));
  const measureMs = Math.max(1, Number(input.measureMs || 1000));

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

  const configPath = input.configPath
    ? path.resolve(cliRoot, input.configPath)
    : path.resolve(cliRoot, "..", "..", "tools", "mermaid-config.json");
  const cfg = JSON.parse(fs.readFileSync(configPath, "utf8"));

  const launchOpts = {
    headless: "shell",
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  };
  const browser = await puppeteer.launch(launchOpts);
  const page = await browser.newPage();

  try {
    meta.chromium = await browser.version();
  } catch {
    // ignore
  }
  try {
    meta.user_agent = await page.evaluate(() => navigator.userAgent);
  } catch {
    // ignore
  }
  try {
    if (typeof puppeteer.version === "function") {
      meta.puppeteer = puppeteer.version();
    }
  } catch {
    // ignore
  }

  // Seed Math.random + crypto.getRandomValues for stability.
  await page.evaluateOnNewDocument((seedStr2) => {
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
  }, seedStr);

  await page.setViewport({
    width: Math.max(1, width),
    height: 600,
    deviceScaleFactor: 1,
  });
  await page.goto("file://" + mermaidHtmlPath.replace(/\\/g, "/"));
  await page.addScriptTag({ path: mermaidIifePath });

  const results = {};
  for (const [name, code] of Object.entries(fixtures)) {
    let timesNs;
    try {
      timesNs = await page.evaluate(
        async ({ code2, cfg2, theme2, width2, warmupMs2, measureMs2, name2 }) => {
          const mermaid = globalThis.mermaid;
          if (!mermaid) throw new Error("mermaid global not found");

          // Initialize once per fixture.
          mermaid.initialize(Object.assign({ startOnLoad: false, theme: theme2 }, cfg2));

          const container = document.getElementById("container") || document.body;
          container.innerHTML = "";
          container.style.width = `${Math.max(1, Number(width2) || 1)}px`;

          async function one(i) {
            container.innerHTML = "";
            const { svg } = await mermaid.render(`${name2}-${i}`, code2, container);
            // prevent accidental DCE
            return svg.length;
          }

          // Warmup until wall clock threshold.
          const t0 = performance.now();
          let i = 0;
          while (performance.now() - t0 < warmupMs2) {
            // eslint-disable-next-line no-await-in-loop
            await one(i++);
          }

          // Measure.
          const samples = [];
          const m0 = performance.now();
          let j = 0;
          while (performance.now() - m0 < measureMs2) {
            const s0 = performance.now();
            // eslint-disable-next-line no-await-in-loop
            await one(j++);
            const s1 = performance.now();
            samples.push((s1 - s0) * 1e6); // ms -> ns
          }

          return samples;
        },
        {
          code2: String(code),
          cfg2: cfg,
          theme2: theme,
          width2: width,
          warmupMs2: warmupMs,
          measureMs2: measureMs,
          name2: name,
        }
      );
    } catch (err) {
      results[name] = {
        median_ns: null,
        samples: 0,
        error: err && err.message ? String(err.message) : String(err),
      };
      // eslint-disable-next-line no-console
      console.error("[mermaid-js-bench] fixture failed:", name, results[name].error);
      continue;
    }

    if (!Array.isArray(timesNs)) {
      const tag = Object.prototype.toString.call(timesNs);
      results[name] = {
        median_ns: null,
        samples: 0,
        error: `page.evaluate returned a non-array result: ${tag}`,
      };
      // eslint-disable-next-line no-console
      console.error("[mermaid-js-bench] fixture failed:", name, results[name].error);
      continue;
    }

    results[name] = {
      median_ns: median(timesNs),
      samples: timesNs.length,
    };
  }

  await browser.close();
  fs.writeFileSync(args.outPath, JSON.stringify({ meta, results }, null, 2), "utf8");
}

main().catch((err) => {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
});
