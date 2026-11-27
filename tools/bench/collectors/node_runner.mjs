// Run an expanded workload plan against one installed Node SVG renderer.
import { execFileSync, fork, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { arch, constants, cpus, hostname, platform, release, tmpdir } from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath, pathToFileURL } from "node:url";
import { isDeepStrictEqual, parseArgs } from "node:util";

const { node_products: products, node_fixed_layouts: fixedLayouts } =
  JSON.parse(readFileSync(new URL("../workloads.json", import.meta.url), "utf8"));
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const errorMessage = (error) => String(error?.message ?? error);
const validator = fileURLToPath(new URL("../merman_bench/svg.py", import.meta.url));
const DEFAULT_WARMUP = { max_calls: 20, max_ms: 150 };
const VALIDATION_BATCH_BYTES = 32 * 1024 * 1024;
const SITE_LAYOUTS = ["dagre", "elk", "shoal"];
const posixProcessGroups = process.platform !== "win32";
const SCENARIO_LIFETIME_FD = 4;

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
      const { name, version, optionalDependencies } = JSON.parse(readFileSync(manifest, "utf8"));
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
        const nativePackages = backend === "napi"
          ? Object.keys(optionalDependencies ?? {}).filter((dep) => dep.startsWith("@mermanjs/node-")) : [];
        const scope = backend === "javascript" ? "node-entry" : "node-entry-and-transport";
        return { name, version, entry, backend, scope, directory, nativePackages };
      }
    }
    const parent = path.dirname(directory);
    if (parent === directory) throw new Error(`No package manifest for ${entry}`);
    directory = parent;
  }
}

function fileStamp(file) {
  try {
    const { dev, ino, size, mtimeNs, ctimeNs } = statSync(file, { bigint: true });
    return [dev, ino, size, mtimeNs, ctimeNs].join(":");
  } catch {
    return null;
  }
}

async function fileEvidence(role, file, stamp) {
  const filename = realpathSync(file);
  const digest = createHash("sha256");
  let bytes = 0;
  for await (const chunk of createReadStream(filename)) {
    bytes += chunk.length;
    digest.update(chunk);
  }
  if (stamp !== undefined && (stamp === null || stamp !== fileStamp(file))) {
    throw new Error(`Product file changed or was unavailable before import: ${file}`);
  }
  return { role, path: filename, bytes, sha256: digest.digest("hex") };
}

// wasm-bindgen reads the binary through fs, so only its CJS glue appears in require.cache.
// Check the packaged sibling against the compiled module while the measured engine is still live.
async function matchesLoadedWasm(file) {
  const { Session } = await import("node:inspector/promises");
  const session = new Session();
  const ids = [];
  session.connect();
  try {
    session.on("Debugger.scriptParsed", ({ params }) => {
      if (params.scriptLanguage === "WebAssembly" && params.length === file.bytes) ids.push(params.scriptId);
    });
    await session.post("Debugger.enable");
    if (ids.length !== 1) return false;
    const { bytecode } = await session.post("Debugger.getScriptSource", { scriptId: ids[0] });
    return typeof bytecode === "string" && sha256(Buffer.from(bytecode, "base64")) === file.sha256;
  } finally {
    session.disconnect();
  }
}

function productFiles(product) {
  const entryStamp = fileStamp(product.entry);
  const addons = new Map();
  const require = createRequire(product.entry);
  for (const name of product.nativePackages) {
    try {
      const file = require.resolve(name);
      if (file.endsWith(".node")) addons.set(file, fileStamp(file));
    } catch {
      // Optional platform packages may be absent from this installation.
    }
  }
  const glue = path.join(product.directory, "artifact", "merman_node.cjs");
  const glueStamp = product.backend === "node-wasm" ? fileStamp(glue) : null;
  return { entryStamp, addons: [...addons], glue, glueStamp };
}

async function productArtifacts(product, previousModules) {
  const { scope, entryStamp, addons, glue, glueStamp } = product;
  const require = createRequire(product.entry);
  const loaded = (file) => require.cache[file]?.loaded && !previousModules.has(file);
  const files = [];
  try {
    files.push(await fileEvidence("entry", product.entry, entryStamp));
    if (product.backend === "napi") {
      const observed = addons.filter(([file]) => loaded(file));
      if (observed.length !== 1) throw new Error("Could not identify one loaded Merman platform addon");
      const [[file, stamp]] = observed;
      files.push(await fileEvidence("native-addon", file, stamp));
    } else if (product.backend === "node-wasm") {
      if (!loaded(realpathSync(glue))) throw new Error("The public Merman WASM glue was not observed in the worker");
      files.push(await fileEvidence("wasm-glue", glue, glueStamp));
      const wasm = await fileEvidence("wasm-module", path.join(product.directory, "artifact", "merman_node_bg.wasm"));
      if (!await matchesLoadedWasm(wasm)) throw new Error("Packaged WASM bytes could not be matched to the loaded module");
      files.push(wasm);
    }
    return { scope, status: "recorded", files };
  } catch (error) {
    return { scope, status: "unknown", files, reason: errorMessage(error) };
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

function readPlan(file, retainAllDrawings) {
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
    const drawingIndices = [];
    const requests = scenario.requests.map((request, position) => {
      const { fixture, source, diagram_id, source_sha256, expected_labels = [], expected_outcome = "draw" } = request ?? {};
      const label = `scenarios[${index}].requests[${position}]`;
      if (typeof fixture !== "string" || !fixture || typeof source !== "string" ||
          typeof diagram_id !== "string" || !diagram_id) {
        throw new Error(`Expected fixture, source and a nonempty diagram_id at ${label}`);
      }
      const digest = sha256(source);
      if (source_sha256 !== undefined && source_sha256 !== digest) {
        throw new Error(`Source SHA-256 mismatch at ${label}`);
      }
      if (!validExpectedLabels(expected_labels)) {
        throw new Error(`Invalid expected_labels at ${label}`);
      }
      if (!["draw", "allow-empty", "reject", "unreviewed"].includes(expected_outcome)) {
        throw new Error(`Invalid expected_outcome at ${label}`);
      }
      const drawing_required = Object.hasOwn(request, "drawing_facts");
      if (drawing_required || retainAllDrawings) drawingIndices.push(position);
      return { fixture, diagram_id, source, source_sha256: digest, expected_labels, expected_outcome, drawing_required };
    });
    return { index, id: scenario.id, origin: scenario.origin, requests, drawingIndices };
  });
  return { sha256: sha256(raw), suite: plan.suite, scenarios };
}

function scenarioIdentity({ index, id, origin, requests }) {
  const identities = requests.map(({ fixture, diagram_id, source_sha256, expected_outcome, expected_labels, drawing_required, drawing_artifact }) => ({
    fixture, diagram_id, source_sha256, expected_outcome,
    ...(expected_labels.length ? { expected_labels } : {}),
    ...(drawing_required ? { drawing_required } : {}),
    ...(drawing_artifact ? { drawing_artifact } : {}),
  }));
  return { index, id, origin, requests: identities };
}

function validateOutputs(jobs, lifetimeFd) {
  const args = ["-B", "-X", "utf8", validator];
  const options = {
    input: JSON.stringify(jobs), encoding: "utf8", maxBuffer: Infinity, stdio: ["pipe", "pipe", "inherit"],
  };
  if (posixProcessGroups && lifetimeFd !== undefined) options.stdio.push(lifetimeFd);

  const interpreter = process.env.MERMAN_BENCH_PYTHON;
  const defaults = posixProcessGroups ? [["python3"], ["python"]] : [["python"], ["python3"], ["py", "-3"]];
  const commands = interpreter ? [[interpreter]] : defaults;
  for (const [command, ...prefix] of commands) {
    // Reap Python before returning. Its exit status takes precedence over a secondary stdin EPIPE.
    const { status, signal, error, stdout } = spawnSync(command, [...prefix, ...args], options);
    if (!interpreter && error?.code === "ENOENT") continue;
    if (signal || (status !== null && status !== 0)) {
      throw new Error(`SVG validator failed (${signal ?? `exit ${status}`})`);
    }
    if (error) throw error;
    return JSON.parse(stdout);
  }
  throw new Error("SVG validator needs Python. Set MERMAN_BENCH_PYTHON to the interpreter path.");
}

// Only assert literal root IDs in the subset that Merman's SVG ID sanitizer preserves.
function preservesRootId(id) {
  return /^[A-Za-z](?:[A-Za-z0-9_]|-(?=[A-Za-z0-9_]))*$/u.test(id) && id !== "m";
}

function rootId(svg) {
  const match = /^<svg(?:\s+[^\s=<>]+\s*=\s*(?:"[^"]*"|'[^']*'))*?\s+id\s*=\s*(?:"([^"]*)"|'([^']*)')/u.exec(svg.trimStart());
  return match?.[1] ?? match?.[2] ?? null;
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

async function measureBatch(requests, render, { now, asyncSvg, importStart }) {
  const attempts = new Array(requests.length);
  let firstReturnMs;
  const batchStart = now();
  for (let index = 0; index < requests.length; index++) {
    const start = now();
    try {
      const output = render(requests[index]);
      const svg = asyncSvg ? (await output)?.svg : output;
      const end = now();
      attempts[index] = { elapsed_ms: end - start, svg };
      if (index === 0 && importStart !== undefined) firstReturnMs = end - importStart;
    } catch (error) {
      attempts[index] = { elapsed_ms: now() - start, error };
    }
  }
  return { elapsed_ms: now() - batchStart, firstReturnMs, attempts };
}

async function warmRenderer(requests, render, { now, asyncSvg, max_calls, max_ms }) {
  let calls = 0;
  let elapsed_ms = 0;
  if (max_calls === 0 || max_ms === 0) return { calls, elapsed_ms };
  const start = now();
  while (calls < max_calls && elapsed_ms < max_ms) {
    try {
      const output = render(requests[calls % requests.length]);
      if (asyncSvg) await output;
    } catch {
      // Incomplete diagrams can be part of the workload. Measured calls record their errors.
    }
    calls++;
    elapsed_ms = now() - start;
  }
  return { calls, elapsed_ms };
}

function deferredValidation(requests, validate, directory, maxBytes) {
  const resultsByCheck = new Map();
  const files = [];
  const checks = requests.map(({ expected_labels, expected_outcome = "draw" }) => {
    const policy = JSON.stringify([expected_outcome, expected_labels ?? []]);
    return { expected_labels, expected_outcome, digest: sha256(policy), bytes: Buffer.byteLength(policy) };
  });
  let jobs = [];
  let bytes = 0;

  // Bound the validator's input batches. Oversized jobs spill on their own.
  function spill() {
    const file = path.join(directory, `${files.length}.json`);
    writeFileSync(file, JSON.stringify(jobs), { encoding: "utf8", flag: "wx" });
    files.push(file);
    jobs = [];
    bytes = 0;
  }

  return {
    observe({ attempts, ...batch }) {
      const results = attempts.map((attempt, index) => {
        const result = observedOutput(attempt);
        const check = checks[index];
        const { expected_labels, expected_outcome } = check;
        const failed = result.status === "render-error";
        if (failed && expected_outcome !== "reject") return result;

        const error = failed ? { code: result.code, message: result.message } : null;
        const diagnostic = error && JSON.stringify(error);
        const svg = result.output_sha256 === null ? null : attempt.svg;
        if (svg !== null && (requests[index].expected_root_id !== undefined || requests[index].omits_diagram_id)) {
          result.svg_root_id = rootId(svg);
        }
        // The validator sees lone surrogates in JSON. The report's UTF-8 hash replaces them.
        const svgKey = svg !== null && !svg.isWellFormed()
          ? `escaped:${sha256(JSON.stringify(svg))}` : result.output_sha256;
        const key = diagnostic ? `error:${sha256(diagnostic)}` : `${check.digest}:${svgKey}`;
        const matching = resultsByCheck.get(key);
        if (matching) {
          matching.push(result);
          return result;
        }

        resultsByCheck.set(key, [result]);
        const jobBytes = check.bytes + (diagnostic ? Buffer.byteLength(diagnostic) : result.output_bytes);
        if (jobs.length && bytes + jobBytes > maxBytes) spill();
        if (error) {
          jobs.push({ error, expected_outcome });
        } else {
          jobs.push({ svg, expected_labels, expected_outcome });
        }
        bytes += jobBytes;
        if (bytes > maxBytes) spill();
        return result;
      });
      return { ...batch, results };
    },
    finish() {
      const targets = resultsByCheck.values();
      const apply = (batch) => {
        if (!batch.length) return;
        const verdicts = validate(batch);
        if (verdicts.length !== batch.length) throw new Error("SVG validator returned an incomplete batch");
        for (const verdict of verdicts) {
          for (const result of targets.next().value) Object.assign(result, verdict);
        }
      };
      for (const file of files) apply(JSON.parse(readFileSync(file, "utf8")));
      apply(jobs);
    },
  };
}

export async function measureScenario(requests, render, {
  samples, now = () => performance.now(), asyncSvg = false, importStart, afterLastBatch,
  warmup = DEFAULT_WARMUP, validate = validateOutputs, validationDirectory, validationBatchBytes = VALIDATION_BATCH_BYTES,
}) {
  const timing = { now, asyncSvg };
  const cold = await measureBatch(requests, render, { ...timing, importStart });
  const observedWarmup = await warmRenderer(requests, render, { ...timing, ...warmup });
  const warm = [];
  for (let i = 0; i < samples; i++) {
    warm.push(await measureBatch(requests, render, timing));
  }
  const lastAttempts = afterLastBatch ? warm.at(-1).attempts : undefined;
  const validation = deferredValidation(requests, validate, validationDirectory, validationBatchBytes);
  const observedCold = validation.observe(cold);
  for (let i = 0; i < warm.length; i++) warm[i] = validation.observe(warm[i]);
  validation.finish();
  afterLastBatch?.(lastAttempts, warm.at(-1).results);
  return {
    cold_batch: summarizeBatch(requests, observedCold, importStart),
    warmup: observedWarmup,
    warm_samples: warm.map((batch) => summarizeBatch(requests, batch)),
  };
}

function summarizeBatch(requests, { elapsed_ms, firstReturnMs, results }, importStart) {
  for (const [index, result] of results.entries()) {
    if (result.status !== "ok") continue;
    const { expected_root_id, omits_diagram_id } = requests[index];
    if (omits_diagram_id) {
      result.status = "diagram-id-not-applied";
      result.message = "This no-options bridge call omits the frozen diagram_id.";
    } else if (expected_root_id !== undefined && result.svg_root_id !== expected_root_id) {
      result.status = "diagram-id-mismatch";
      result.message = `Expected SVG root id ${JSON.stringify(expected_root_id)}, found ${JSON.stringify(result.svg_root_id)}.`;
    }
  }
  const successes = results.filter((item) => item.status === "ok");
  const first = results[0];
  const firstDrew = first?.status === "ok" &&
    (first.output_class === "draw" || (first.output_class === undefined && (requests[0].expected_outcome ?? "draw") === "draw"));
  return {
    ...(importStart === undefined ? {} : { init_through_first_svg_ms: firstDrew ? firstReturnMs : null }),
    elapsed_ms, complete_batch_ms: successes.length === results.length ? elapsed_ms : null,
    successful_requests: successes.length,
    expected_rejections: results.filter((item) => item.status === "expected-rejection").length,
    successful_render_ms: successes.length ? successes.reduce((sum, item) => sum + item.elapsed_ms, 0) : null,
    successful_output_bytes: successes.reduce((sum, item) => sum + item.output_bytes, 0),
    results,
  };
}

function noOptionsBridge(engine, variant) {
  return engine === "merman" && variant?.options_json === null && !variant?.direct_id;
}

function expectedLayoutCapabilities(diagramType, siteLayout) {
  const fixed = fixedLayouts.find(({ diagram_types, site_layouts }) =>
    diagram_types.includes(diagramType) && (!site_layouts || site_layouts.includes(siteLayout)));
  return fixed?.required_capability_ids ?? [`layout-${siteLayout}`];
}

function fieldValueEnd(text, objectStart, key, expectedToken) {
  // The template is already valid JSON. Locate the field without rewriting the option bytes.
  let depth = 0;
  const tokens = /"(?:[^"\\]|\\.)*"|[{}\[\]]/gu;
  tokens.lastIndex = objectStart;
  const value = /[ \t\r\n]*:[ \t\r\n]*(\{|null)/uy;
  for (const { 0: token, index } of text.matchAll(tokens)) {
    if (token === "{" || token === "[") depth++;
    else if (token === "}" || token === "]") {
      if (--depth === 0) break;
    } else if (depth === 1 && JSON.parse(token) === key) {
      value.lastIndex = index + token.length;
      if (value.exec(text)?.[1] === expectedToken) return value.lastIndex;
    }
  }
  throw new Error(`Could not locate the ${key} Options JSON field`);
}

function optionsWithPlanId(text = "{}") {
  const options = JSON.parse(text);
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new Error("A Merman variant requires an Options JSON object");
  }
  const { svg } = options;
  if (svg != null && (typeof svg !== "object" || Array.isArray(svg))) {
    throw new Error("A Merman variant requires svg options to be an object or null");
  }
  if (svg && Object.hasOwn(svg, "diagram_id") && svg.diagram_id !== null) {
    return (id) => {
      if (svg.diagram_id !== id) throw new Error(`svg.diagram_id conflicts with frozen diagram_id ${JSON.stringify(id)}`);
      return { optionsJson: text };
    };
  }
  const root = text.indexOf("{");
  const offset = svg === undefined ? root + 1 : fieldValueEnd(text, root, "svg", svg === null ? "null" : "{");
  const insertId = (prefix, suffix) => (id) => ({ optionsJson: prefix + JSON.stringify(id) + suffix });
  if (svg === null) return insertId(text.slice(0, offset - 4) + '{"diagram_id":', "}" + text.slice(offset));
  if (svg?.diagram_id === null) {
    const idEnd = fieldValueEnd(text, offset - 1, "diagram_id", "null");
    return insertId(text.slice(0, idEnd - 4), text.slice(idEnd));
  }
  const comma = Object.keys(svg ?? options).length ? "," : "";
  const prefix = text.slice(0, offset) + (svg ? '"diagram_id":' : '"svg":{"diagram_id":');
  const suffix = (svg ? "" : "}") + comma + text.slice(offset);
  return insertId(prefix, suffix);
}

async function runScenario({ product, engine, variant, scenario, samples, warmup, drawingDir, validationDirectory }) {
  const merman = engine === "merman";
  const siteLayout = variant?.site_layout;
  const directId = variant?.direct_id;
  const omitId = noOptionsBridge(engine, variant);
  const planOptions = merman && !omitId && !directId ? optionsWithPlanId(variant?.options_json) : null;
  const requests = scenario.requests.map(({ source, diagram_id, expected_labels, expected_outcome }) => {
    const request = { source, diagram_id, expected_labels, expected_outcome };
    if (directId || planOptions) {
      request.options = directId ? { diagramId: diagram_id } : planOptions(diagram_id);
      if (preservesRootId(diagram_id)) request.expected_root_id = diagram_id;
    }
    if (omitId) request.omits_diagram_id = true;
    if (engine === "pele") request.options = { idPrefix: diagram_id, now: 0 };
    return request;
  });
  const checked = requests.flatMap((request, index) => request.expected_labels.length ? [index] : []);
  const entry = pathToFileURL(product.entry).href;
  const previousModules = merman ? new Set(Object.keys(createRequire(entry).cache)) : null;
  const start = performance.now();
  const api = await import(entry);
  const instance = merman
    ? await (siteLayout ? api.createNodeEngine({ bindingOptions: { site_config: { layout: siteLayout } } }) : api.createNodeEngine())
    : null;
  const init_ms = performance.now() - start;
  try {
    let render;
    if (merman && typeof instance?.renderSvgSync === "function") {
      render = omitId
        ? ({ source }) => instance.renderSvgSync(source)
        : ({ source, options }) => instance.renderSvgSync(source, options);
    } else if (engine === "pele" && typeof api.render === "function") {
      render = ({ source, options }) => api.render(source, options).svg;
    } else if (engine === "mermaid-v13" && typeof api.default?.render === "function") {
      render = ({ diagram_id, source }) => api.default.render(diagram_id, source);
    } else {
      throw new Error(`Missing ${products[engine].render_api} in ${product.entry}`);
    }
    const options = {
      samples, warmup, validationDirectory, asyncSvg: engine === "mermaid-v13", importStart: start,
      validate: (jobs) => validateOutputs(jobs, posixProcessGroups ? SCENARIO_LIFETIME_FD : undefined),
    };
    if (drawingDir && scenario.drawingIndices.length) {
      options.afterLastBatch = (attempts, results) => {
        mkdirSync(drawingDir, { recursive: true });
        for (const index of scenario.drawingIndices) {
          if (results[index].output_sha256 === null) continue;
          const basename = `${scenario.index}-${index}.svg`;
          writeFileSync(path.join(drawingDir, basename), attempts[index].svg, { encoding: "utf8", flag: "wx" });
          scenario.requests[index].drawing_artifact = basename;
        }
      };
    }
    const measured = await measureScenario(requests, render, options);
    const { cold_batch, warm_samples } = measured;
    const peakRss = process.resourceUsage().maxRSS;
    const layoutPlans = siteLayout && requests.map(({ source, options, diagram_id }, index) => {
      if (!["ok", "diagram-id-not-applied"].includes(cold_batch.results[index].status)) return null;
      const planJson = directId ? instance.svgPlanJsonSync(source) : instance.svgPlanJsonSync(source, options);
      const { ready, diagram_type, required_capability_ids } = JSON.parse(planJson);
      const layouts = required_capability_ids.filter((id) => id.startsWith("layout-"));
      if (!ready || !isDeepStrictEqual(layouts, expectedLayoutCapabilities(diagram_type, siteLayout))) {
        const capabilities = required_capability_ids.join(", ") || "no required capabilities";
        throw new Error(`${diagram_id}: requested ${siteLayout}. SVG plan reports ${capabilities}.`);
      }
      return { diagram_type, required_capability_ids };
    });
    const catalog = instance?.runtimeCatalog;
    if (merman && catalog?.package_version !== product.version) {
      throw new Error(`Runtime version ${catalog?.package_version} differs from ${product.name}@${product.version}`);
    }
    const successful = [];
    const comparable = [];
    const rejections = [];
    for (const [index, first] of cold_batch.results.entries()) {
      const outcome = requests[index].expected_outcome;
      if (!warm_samples.every((batch) => batch.results[index].status === first.status)) continue;
      if (outcome === "reject" && first.status === "expected-rejection") rejections.push(index);
      if (first.status !== "ok") continue;
      successful.push(index);
      const reviewed = outcome === "draw" || (outcome === "allow-empty" && ["draw", "empty"].includes(first.output_class));
      if (reviewed && warm_samples.every((batch) => batch.results[index].output_sha256 === first.output_sha256)) {
        comparable.push(index);
      }
    }
    const first = cold_batch.results[0];
    return {
      ...scenarioIdentity(scenario), init_ms,
      peak_rss_kib: peakRss,
      ...(layoutPlans ? { layout_plans: layoutPlans } : {}),
      artifact_identity: await productArtifacts(product, previousModules),
      first_render_ms: first.status === "ok" ? first.elapsed_ms : null,
      transport_api_version: catalog?.transport_api_version ?? null,
      ...measured,
      coverage: {
        planned_requests: requests.length,
        label_reader: checked.length ? "python-xml.etree.ElementTree" : null,
        expected_label_request_indices: checked,
        verified_label_request_indices: successful.filter((index) => requests[index].expected_labels.length),
        successful_request_indices: successful,
        expected_rejection_request_indices: rejections,
        comparable_request_indices: comparable,
        full_batch_comparable: comparable.length === requests.length && requests.every((r, i) =>
          r.expected_outcome === requests[0].expected_outcome && cold_batch.results[i].output_class === first.output_class),
      },
    };
  } finally {
    if (instance) await instance.dispose();
  }
}

function killProcessGroup(pid) {
  try {
    process.kill(-pid, "SIGKILL");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

function forkScenario(mode, { detached = false, lifetimePipe } = {}) {
  // Package stdout and stderr are diagnostics. Only IPC carries the measured row.
  const stdio = ["ignore", 2, 2, "ipc"];
  if (lifetimePipe !== undefined) stdio[SCENARIO_LIFETIME_FD] = lifetimePipe;
  return fork(fileURLToPath(import.meta.url), [mode], { execArgv: [], stdio, detached });
}

function superviseScenario() {
  // The group leader stays alive through worker failure, so signaling the group cannot hit a reused ID.
  const stopGroup = () => killProcessGroup(process.pid);
  process.once("disconnect", stopGroup);
  process.once("message", (input) => {
    const worker = forkScenario("--scenario-worker", { lifetimePipe: SCENARIO_LIFETIME_FD });
    let reply;
    let workerError;
    worker.once("message", (message) => { reply = message; });
    worker.once("error", (error) => { workerError = errorMessage(error); });
    worker.once("close", (code, signal) => {
      if (code === 0 && reply?.row) {
        process.send(reply, (error) => {
          if (error) stopGroup();
          else process.off("disconnect", stopGroup);
        });
      } else {
        process.send({ error: reply?.error ?? workerError ?? signal ?? `exit ${code}` }, stopGroup);
      }
    });
    worker.send(input);
  });
}

function isolatedScenario(input, timeoutSeconds) {
  const validationDirectory = mkdtempSync(path.join(tmpdir(), "merman-node-validation-"));
  let onInterrupt;
  let interruptedBy;
  return new Promise((resolve, reject) => {
    const child = posixProcessGroups
      ? forkScenario("--scenario-supervisor", { detached: true, lifetimePipe: "pipe" })
      : forkScenario("--scenario-worker");
    // The inherited pipe keeps `close` pending until any in-flight validator has also exited.
    child.stdio[SCENARIO_LIFETIME_FD]?.resume();
    const stopWorker = () => {
      if (!posixProcessGroups) child.kill("SIGKILL");
      else if (child.pid !== undefined && child.exitCode === null && child.signalCode === null) {
        killProcessGroup(child.pid);
      }
    };
    if (posixProcessGroups) {
      // A terminal interrupt reaches the collector instead of the detached scenario group.
      onInterrupt = (signal) => {
        interruptedBy ??= signal;
        stopWorker();
      };
      process.on("SIGINT", onInterrupt);
      process.on("SIGTERM", onInterrupt);
    }
    let reply;
    let childError;
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      stopWorker();
    }, Math.ceil(timeoutSeconds * 1000));
    child.once("error", (error) => {
      if (posixProcessGroups) {
        childError = error;
        stopWorker();
      } else {
        clearTimeout(timer);
        reject(error);
      }
    });
    child.once("message", (message) => { reply = message; });
    child.once("close", (code, signal) => {
      clearTimeout(timer);
      if (timedOut) {
        const error = new Error(`Scenario ${input.scenario.index} (${input.scenario.id}) timed out after ${timeoutSeconds}s`);
        error.code = "ETIMEDOUT";
        reject(error);
      } else if (childError) {
        reject(childError);
      } else if (code === 0 && reply?.row) {
        resolve(reply.row);
      } else {
        const cause = reply?.error ?? signal ?? `exit ${code}`;
        reject(new Error(`Scenario ${input.scenario.index} (${input.scenario.id}): ${cause}`));
      }
    });
    child.send({ ...input, validationDirectory });
  }).finally(() => {
    if (onInterrupt) {
      process.off("SIGINT", onInterrupt);
      process.off("SIGTERM", onInterrupt);
    }
    try {
      rmSync(validationDirectory, { recursive: true, force: true, maxRetries: 3 });
    } catch (error) {
      console.error(`Could not remove the SVG validation scratch directory ${validationDirectory}: ${errorMessage(error)}`);
    }
    if (interruptedBy) process.exit(128 + constants.signals[interruptedBy]);
  });
}

async function main() {
  const { values } = parseArgs({ options: {
    plan: { type: "string" }, engine: { type: "string" }, module: { type: "string" },
    "variant-label": { type: "string" }, "options-json": { type: "string" }, "direct-id": { type: "boolean" },
    "site-layout": { type: "string" },
    "drawing-dir": { type: "string" }, "all-drawings": { type: "boolean" },
    samples: { type: "string", default: "3" }, "timeout-seconds": { type: "string", default: "300" },
    "warmup-calls": { type: "string", default: String(DEFAULT_WARMUP.max_calls) },
    "warmup-ms": { type: "string", default: String(DEFAULT_WARMUP.max_ms) },
  } });
  const samples = Number(values.samples);
  const timeoutSeconds = Number(values["timeout-seconds"]);
  const warmup = { max_calls: Number(values["warmup-calls"]), max_ms: Number(values["warmup-ms"]) };
  if (!values.plan || !Object.hasOwn(products, values.engine) || values["drawing-dir"] === "" ||
      !/^[1-9]\d*$/u.test(values.samples) || !Number.isSafeInteger(samples) ||
      !["warmup-calls", "warmup-ms"].every((key) => /^\d+$/u.test(values[key]) && Number.isSafeInteger(Number(values[key]))) ||
      !(timeoutSeconds > 0 && timeoutSeconds * 1000 <= 2_147_483_647)) {
    throw new Error(`Usage: node tools/bench/collectors/node_runner.mjs --plan FILE --engine ${Object.keys(products).join("|")} [--module ENTRY] [--variant-label LABEL [--options-json JSON | --direct-id] [--site-layout dagre|elk|shoal]] [--samples N] [--warmup-calls N] [--warmup-ms N] [--timeout-seconds N] [--drawing-dir DIR [--all-drawings]]\nPass --options-json={} or --direct-id to match the plan IDs in a named Merman variant. With neither flag, a named Merman variant collects a no-options diagnostic without a timing ratio.\nWarmup stops after ${DEFAULT_WARMUP.max_calls} calls or ${DEFAULT_WARMUP.max_ms} ms by default, whichever comes first. Set either limit to 0 to skip warmup.`);
  }
  if (values["all-drawings"] && !values["drawing-dir"]) throw new Error("--all-drawings requires --drawing-dir");
  const label = values["variant-label"];
  const optionsJson = values["options-json"];
  const directId = values["direct-id"];
  const siteLayout = values["site-layout"];
  if (label !== undefined && (!/^[A-Za-z0-9](?:[A-Za-z0-9_.-]{0,62}[A-Za-z0-9_-])?$/u.test(label) ||
      /^(CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])$/iu.test(label.split(".")[0]))) {
    throw new Error("--variant-label must be a portable label of up to 64 characters");
  }
  if (optionsJson !== undefined && (label === undefined || values.engine !== "merman" || !optionsJson.trim())) {
    throw new Error("--options-json requires a Merman variant and nonempty Options JSON text");
  }
  if (directId && (label === undefined || values.engine !== "merman" || optionsJson !== undefined)) {
    throw new Error("--direct-id requires a Merman variant without --options-json");
  }
  if (siteLayout !== undefined && (label === undefined || values.engine !== "merman" || !SITE_LAYOUTS.includes(siteLayout))) {
    throw new Error("--site-layout requires a Merman variant and dagre, elk, or shoal");
  }
  const variant = label === undefined ? undefined : {
    label, options_json: optionsJson ?? null,
    ...(siteLayout ? { site_layout: siteLayout } : {}),
    ...(directId ? { direct_id: true } : {}),
  };
  const drawingDir = values["drawing-dir"] && path.resolve(values["drawing-dir"]);
  const plan = readPlan(values.plan, values["all-drawings"]);
  const engine = values.engine;
  const product = installedPackage(values.module ?? products[engine].package, engine);
  // Resolve candidates in the parent to keep the worker's loader caches cold for init_ms.
  Object.assign(product, productFiles(product));
  const collectorFiles = ["node_runner.mjs", "../workloads.json", "../merman_bench/svg.py"];
  const report = {
    schema_version: 1, surface: "node", output_format: "svg", engine, backend: product.backend,
    diagram_id_mode: noOptionsBridge(engine, variant) ? "omitted" : "plan",
    ...(variant ? { variant } : {}),
    ...(drawingDir ? { drawing_dir: drawingDir } : {}),
    // The existing comparison gate uses this fingerprint to withhold ratios across collection recipes.
    collector_sha256: sha256([
      ...collectorFiles.map((name) => sha256(readFileSync(new URL(name, import.meta.url)))), JSON.stringify(warmup),
    ].join("\n")),
    package: { name: product.name, version: product.version, entry: product.entry, entry_sha256: sha256(readFileSync(product.entry)) },
    render_api: products[engine].render_api,
    node_project: consumerProject(),
    process_lifecycle: "fresh-per-scenario", init_scope: "package-import-and-engine-creation",
    host: {
      hostname: hostname(), platform: platform(), release: release(), arch: arch(),
      cpu: cpus()[0]?.model, node: process.version,
    },
    suite: plan.suite, plan_sha256: plan.sha256, samples, warmup, timeout_seconds: timeoutSeconds,
    planned_scenarios: plan.scenarios.length, scenarios: [],
  };
  for (const scenario of plan.scenarios) {
    let row;
    try {
      row = await isolatedScenario({ product, engine, variant, scenario, samples, warmup, drawingDir }, timeoutSeconds);
    } catch (error) {
      const status = error.code === "ETIMEDOUT" ? "timeout" : "failed";
      const message = errorMessage(error);
      console.error(message);
      report.scenarios.push({ ...scenarioIdentity(scenario), status, error: message,
        unmeasured_requests: scenario.requests.length, init_ms: null, first_render_ms: null, cold_batch: null, warm_samples: [] });
      report.status = "failed";
      continue;
    }
    report.scenarios.push(row);
    if (!row.coverage.full_batch_comparable) {
      const count = row.coverage.comparable_request_indices.length;
      console.error(`Scenario ${row.index} (${row.id}) has ${count}/${row.requests.length} requests eligible for timing within their output class`);
    }
  }
  const measured = report.scenarios.filter((row) => row.artifact_identity);
  const first = measured[0]?.artifact_identity;
  if (first && measured.every((row) => isDeepStrictEqual(row.artifact_identity, first))) {
    report.artifact_identity = first;
    for (const row of measured) delete row.artifact_identity;
  } else {
    report.artifact_identity = {
      scope: product.scope,
      status: "unknown", files: [], reason: first ? "Measured workers observed different product files" : "No measured worker",
    };
  }
  const entry = report.artifact_identity.files.find((file) => file.role === "entry");
  if (entry && entry.sha256 !== report.package.entry_sha256) {
    report.artifact_identity.status = "unknown";
    report.artifact_identity.reason = "The measured entry differs from the selected package entry";
  }
  return report;
}

if (process.argv[1] && pathToFileURL(realpathSync(process.argv[1])).href === import.meta.url) {
  if (process.send && process.argv[2] === "--scenario-supervisor") {
    superviseScenario();
  } else if (process.send && process.argv[2] === "--scenario-worker") {
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
      if (report.status === "failed") process.exitCode = 1;
      process.stdout.write(`${JSON.stringify(report)}\n`);
    } catch (error) {
      const message = errorMessage(error);
      console.error(message);
      process.stdout.write(`${JSON.stringify({ schema_version: 1, surface: "node", error: message })}\n`);
      process.exitCode = 1;
    }
  }
}
