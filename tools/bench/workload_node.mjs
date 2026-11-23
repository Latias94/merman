// Run an expanded workload plan against one installed Node SVG renderer.
import { execFileSync, fork } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { arch, cpus, hostname, platform, release } from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const products = JSON.parse(readFileSync(new URL("./workload_node_products.json", import.meta.url), "utf8"));
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const errorMessage = (error) => String(error?.message ?? error);
const validator = fileURLToPath(new URL("./workload_svg.py", import.meta.url));

function resolveImport(specifier, cwd) {
  // Let Node select the same conditional export as an import in the consumer project.
  const url = execFileSync(process.execPath, [
    "--input-type=module", "--eval", "process.stdout.write(import.meta.resolve(process.argv[1]))",
    "--", specifier,
  ], { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
  return fileURLToPath(url);
}

function installedPackage(specifier, engine) {
  const product = products[engine];
  let entry;
  if (specifier.startsWith("file:")) {
    entry = fileURLToPath(specifier);
  } else if (path.isAbsolute(specifier) || existsSync(specifier)) {
    entry = path.resolve(specifier);
  } else {
    entry = resolveImport(specifier, process.cwd());
  }
  entry = realpathSync(entry);
  let directory = path.dirname(entry);
  while (true) {
    const manifest = path.join(directory, "package.json");
    if (existsSync(manifest)) {
      const { name, version } = JSON.parse(readFileSync(manifest, "utf8"));
      if (name) {
        const alternatives = product.alternatives ?? {};
        const backend = name === product.package ? product.backend : alternatives[name];
        if (name !== product.package && !Object.hasOwn(alternatives, name)) {
          throw new Error(`Package ${name} is not a public ${engine} Node entry`);
        }
        if (typeof version !== "string" || !version) throw new Error(`Missing version in ${manifest}`);
        if (product.major && version.split(".")[0] !== String(product.major)) {
          throw new Error(`Expected ${name} ${product.major}.x for ${engine}, found ${version}`);
        }
        if (engine === "mermaid-v13" && entry !== realpathSync(resolveImport(name, directory))) {
          throw new Error(`Expected the public Node export of ${name}@${version}, got ${entry}`);
        }
        return { name, version, entry, backend };
      }
    }
    const parent = path.dirname(directory);
    if (parent === directory) throw new Error(`No package manifest for ${entry}`);
    directory = parent;
  }
}

function consumerProject() {
  const cwd = realpathSync(process.cwd());
  const files = ["package.json", "npm-shrinkwrap.json", "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb"];
  const digests = {};
  for (const file of files) {
    const resolved = path.join(cwd, file);
    if (existsSync(resolved)) digests[file] = sha256(readFileSync(resolved));
  }
  return { cwd, digests };
}

function validExpectedLabels(labels) {
  const nonblank = (text) => typeof text === "string" && text.trim().length > 0;
  return Array.isArray(labels) && labels.every((fact) =>
    Array.isArray(fact) ? fact.length > 0 && fact.every(nonblank) : nonblank(fact));
}

function readPlan(file) {
  const raw = readFileSync(file);
  const plan = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(raw));
  if (plan?.schema_version !== 1 || !Array.isArray(plan.scenarios)) {
    throw new Error("Expected a schema_version 1 workload plan with a scenarios array");
  }
  const scenarios = plan.scenarios.map((scenario, index) => {
    if (typeof scenario?.id !== "string" || !scenario.id ||
        !Array.isArray(scenario.requests) || !scenario.requests.length) {
      throw new Error(`Invalid scenario at index ${index}`);
    }
    const ids = new Set();
    const drawingIndices = [];
    const requests = scenario.requests.map((request, position) => {
      const { fixture, source, diagram_id, source_sha256, expected_labels = [] } = request ?? {};
      const label = `scenarios[${index}].requests[${position}]`;
      if (typeof fixture !== "string" || !fixture || typeof source !== "string" ||
          typeof diagram_id !== "string" || !diagram_id || ids.has(diagram_id)) {
        throw new Error(`Expected fixture, source and a distinct diagram_id at ${label}`);
      }
      ids.add(diagram_id);
      const digest = sha256(source);
      if (source_sha256 !== undefined && source_sha256 !== digest) {
        throw new Error(`Source SHA-256 mismatch at ${label}`);
      }
      if (!validExpectedLabels(expected_labels)) {
        throw new Error(`Invalid expected_labels at ${label}`);
      }
      const drawing_required = Object.hasOwn(request, "drawing_facts");
      if (drawing_required) drawingIndices.push(position);
      return { fixture, diagram_id, source, source_sha256: digest, expected_labels, drawing_required };
    });
    return { index, id: scenario.id, origin: scenario.origin, requests, drawingIndices };
  });
  return { sha256: sha256(raw), suite: plan.suite, scenarios };
}

function validateOutputs(jobs) {
  const python = process.platform === "win32" ? "python" : "python3";
  return JSON.parse(execFileSync(python, ["-B", "-X", "utf8", validator], {
    input: JSON.stringify(jobs), encoding: "utf8", maxBuffer: Infinity, stdio: ["pipe", "pipe", "inherit"],
  }));
}

function observedOutput(attempt) {
  const { elapsed_ms, svg } = attempt;
  if (Object.hasOwn(attempt, "error")) {
    const error = attempt.error;
    return {
      elapsed_ms, status: "render-error", code: error?.codeName ?? error?.code ?? null,
      message: errorMessage(error), output_bytes: 0, output_sha256: null,
    };
  }
  return {
    elapsed_ms,
    output_bytes: typeof svg === "string" ? Buffer.byteLength(svg) : 0,
    output_sha256: typeof svg === "string" ? sha256(svg) : null,
  };
}

export async function measureBatch(requests, render, {
  now = () => performance.now(), validate = validateOutputs, cache = requests.map(() => new Map()), asyncSvg = false,
  afterBatch,
} = {}) {
  const attempts = new Array(requests.length);
  const batchStart = now();
  for (let index = 0; index < requests.length; index++) {
    const start = now();
    try {
      const output = render(requests[index]);
      const svg = asyncSvg ? (await output)?.svg : output;
      attempts[index] = { elapsed_ms: now() - start, svg };
    } catch (error) {
      attempts[index] = { elapsed_ms: now() - start, error };
    }
  }
  const elapsed_ms = now() - batchStart;
  const results = attempts.map(observedOutput);
  const pending = [];
  for (const [index, result] of results.entries()) {
    if (result.status === "render-error") continue;
    const cached = cache[index].get(result.output_sha256);
    if (cached) Object.assign(result, cached);
    else pending.push(index);
  }
  if (pending.length) {
    const verdicts = validate(pending.map((index) => ({
      svg: results[index].output_sha256 === null ? null : attempts[index].svg,
      expected_labels: requests[index].expected_labels,
    })));
    if (verdicts.length !== pending.length) throw new Error("SVG validator returned an incomplete batch");
    for (const [offset, index] of pending.entries()) {
      const result = results[index];
      const verdict = verdicts[offset];
      cache[index].set(result.output_sha256, verdict);
      Object.assign(result, verdict);
    }
  }
  afterBatch?.(attempts, results);
  const successes = results.filter((item) => item.status === "ok");
  return {
    elapsed_ms, complete_batch_ms: successes.length === results.length ? elapsed_ms : null,
    successful_requests: successes.length,
    successful_render_ms: successes.length ? successes.reduce((sum, item) => sum + item.elapsed_ms, 0) : null,
    successful_output_bytes: successes.reduce((sum, item) => sum + item.output_bytes, 0),
    results,
  };
}

async function runScenario({ product, engine, scenario, samples, drawingDir }) {
  const merman = engine === "merman";
  const requests = scenario.requests.map(({ source, diagram_id, expected_labels }) => {
    const request = { source, diagram_id, expected_labels };
    if (merman) request.options = { optionsJson: JSON.stringify({ svg: { diagram_id } }) };
    if (engine === "pele") request.options = { idPrefix: diagram_id, now: 0 };
    return request;
  });
  const checked = requests.flatMap((request, index) => request.expected_labels.length ? [index] : []);
  const cache = requests.map(() => new Map());
  const entry = pathToFileURL(product.entry).href;
  const start = performance.now();
  const api = await import(entry);
  const instance = merman ? await api.createNodeEngine() : null;
  const init_ms = performance.now() - start;
  try {
    let render;
    if (merman && typeof instance?.renderSvgSync === "function") {
      render = ({ source, options }) => instance.renderSvgSync(source, options);
    } else if (engine === "pele" && typeof api.render === "function") {
      render = ({ source, options }) => api.render(source, options).svg;
    } else if (engine === "mermaid-v13" && typeof api.default?.render === "function") {
      render = ({ diagram_id, source }) => api.default.render(diagram_id, source);
    } else {
      throw new Error(`Missing ${products[engine].render_api} in ${product.entry}`);
    }
    const options = { cache, asyncSvg: engine === "mermaid-v13" };
    const cold_batch = await measureBatch(requests, render, options);
    const warm_samples = [];
    for (let i = 0; i < samples - 1; i++) warm_samples.push(await measureBatch(requests, render, options));
    if (drawingDir && scenario.drawingIndices.length) {
      options.afterBatch = (attempts, results) => {
        mkdirSync(drawingDir, { recursive: true });
        for (const index of scenario.drawingIndices) {
          if (results[index].output_sha256 === null) continue;
          const basename = `${scenario.index}-${index}.svg`;
          writeFileSync(path.join(drawingDir, basename), attempts[index].svg, { encoding: "utf8", flag: "wx" });
          scenario.requests[index].drawing_artifact = basename;
        }
      };
    }
    warm_samples.push(await measureBatch(requests, render, options));
    const catalog = instance?.runtimeCatalog;
    if (merman && catalog?.package_version !== product.version) {
      throw new Error(`Runtime version ${catalog?.package_version} differs from ${product.name}@${product.version}`);
    }
    const successful = [];
    const comparable = [];
    for (const [index, first] of cold_batch.results.entries()) {
      if (first.status === "ok" && warm_samples.every((batch) => batch.results[index].status === "ok")) {
        successful.push(index);
        if (warm_samples.every((batch) => batch.results[index].output_sha256 === first.output_sha256)) {
          comparable.push(index);
        }
      }
    }
    const first = cold_batch.results[0];
    return {
      index: scenario.index, id: scenario.id, origin: scenario.origin, init_ms,
      first_render_ms: first.status === "ok" ? first.elapsed_ms : null,
      transport_api_version: catalog?.transport_api_version ?? null,
      requests: scenario.requests.map(({ fixture, diagram_id, source_sha256, expected_labels, drawing_required, drawing_artifact }) => ({
        fixture, diagram_id, source_sha256, ...(expected_labels.length ? { expected_labels } : {}),
        ...(drawing_required ? { drawing_required } : {}),
        ...(drawing_artifact ? { drawing_artifact } : {}),
      })),
      cold_batch, warm_samples,
      coverage: {
        planned_requests: requests.length,
        label_reader: checked.length ? "python-xml.etree.ElementTree" : null,
        expected_label_request_indices: checked,
        verified_label_request_indices: successful.filter((index) => requests[index].expected_labels.length),
        successful_request_indices: successful,
        comparable_request_indices: comparable,
        full_batch_comparable: comparable.length === requests.length,
      },
    };
  } finally {
    if (instance) await instance.dispose();
  }
}

function isolatedScenario(input, timeoutSeconds) {
  return new Promise((resolve, reject) => {
    // Package stdout and stderr are diagnostics. Only IPC carries the measured row.
    const child = fork(fileURLToPath(import.meta.url), ["--scenario-worker"], {
      execArgv: [], stdio: ["ignore", 2, 2, "ipc"],
    });
    let reply;
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, Math.ceil(timeoutSeconds * 1000));
    child.once("error", (error) => { clearTimeout(timer); reject(error); });
    child.once("message", (message) => { reply = message; });
    child.once("close", (code, signal) => {
      clearTimeout(timer);
      if (timedOut) {
        const error = new Error(`Scenario ${input.scenario.index} (${input.scenario.id}) timed out after ${timeoutSeconds}s`);
        error.code = "ETIMEDOUT";
        reject(error);
      } else if (code === 0 && reply?.row) {
        resolve(reply.row);
      } else {
        const cause = reply?.error ?? signal ?? `exit ${code}`;
        reject(new Error(`Scenario ${input.scenario.index} (${input.scenario.id}): ${cause}`));
      }
    });
    child.send(input);
  });
}

async function main() {
  const { values } = parseArgs({ options: {
    plan: { type: "string" }, engine: { type: "string" }, module: { type: "string" },
    "drawing-dir": { type: "string" },
    samples: { type: "string", default: "3" }, "timeout-seconds": { type: "string", default: "300" },
  } });
  const samples = Number(values.samples);
  const timeoutSeconds = Number(values["timeout-seconds"]);
  if (!values.plan || !Object.hasOwn(products, values.engine) || values["drawing-dir"] === "" ||
      !/^[1-9]\d*$/u.test(values.samples) || !Number.isSafeInteger(samples) ||
      !(timeoutSeconds > 0 && timeoutSeconds * 1000 <= 2_147_483_647)) {
    throw new Error(`Usage: node tools/bench/workload_node.mjs --plan FILE --engine ${Object.keys(products).join("|")} [--module ENTRY] [--samples N] [--timeout-seconds N] [--drawing-dir DIR]`);
  }
  const drawingDir = values["drawing-dir"] && path.resolve(values["drawing-dir"]);
  const plan = readPlan(values.plan);
  const engine = values.engine;
  const product = installedPackage(values.module ?? products[engine].package, engine);
  const collectorFiles = ["workload_node.mjs", "workload_node_products.json", "workload_svg.py"];
  const report = {
    schema_version: 1, surface: "node", output_format: "svg", engine, backend: product.backend,
    ...(drawingDir ? { drawing_dir: drawingDir } : {}),
    collector_sha256: sha256(collectorFiles.map((name) => sha256(readFileSync(new URL(name, import.meta.url)))).join("\n")),
    package: { name: product.name, version: product.version, entry: product.entry, entry_sha256: sha256(readFileSync(product.entry)) },
    render_api: products[engine].render_api,
    node_project: consumerProject(),
    process_lifecycle: "fresh-per-scenario", init_scope: "package-import-and-engine-creation",
    host: {
      hostname: hostname(), platform: platform(), release: release(), arch: arch(),
      cpu: cpus()[0]?.model, node: process.version,
    },
    suite: plan.suite, plan_sha256: plan.sha256, samples, timeout_seconds: timeoutSeconds, scenarios: [],
  };
  for (const scenario of plan.scenarios) {
    let row;
    try {
      row = await isolatedScenario({ product, engine, scenario, samples, drawingDir }, timeoutSeconds);
    } catch (error) {
      const status = error.code === "ETIMEDOUT" ? "timeout" : "failed";
      return { ...report, status: "failed", failed_scenario: { index: scenario.index, id: scenario.id, status },
        error: errorMessage(error) };
    }
    report.scenarios.push(row);
    if (!row.coverage.full_batch_comparable) {
      const count = row.coverage.comparable_request_indices.length;
      console.error(`Scenario ${row.index} (${row.id}): ${count}/${row.requests.length} requests returned stable SVG in all batches`);
    }
  }
  return report;
}

if (process.argv[1] && pathToFileURL(realpathSync(process.argv[1])).href === import.meta.url) {
  if (process.send && process.argv[2] === "--scenario-worker") {
    process.once("message", async (input) => {
      try {
        process.send({ row: await runScenario(input) });
      } catch (error) {
        process.send({ error: errorMessage(error) });
        process.exitCode = 1;
      }
    });
  } else {
    try {
      const report = await main();
      if (report.status === "failed") {
        console.error(report.error);
        process.exitCode = 1;
      }
      process.stdout.write(`${JSON.stringify(report)}\n`);
    } catch (error) {
      const message = errorMessage(error);
      console.error(message);
      process.stdout.write(`${JSON.stringify({ schema_version: 1, surface: "node", error: message })}\n`);
      process.exitCode = 1;
    }
  }
}
