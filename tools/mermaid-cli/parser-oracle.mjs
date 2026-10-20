import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const require = createRequire(import.meta.url);
const MAX_SOURCE_BYTES = 4096;
const MAX_BATCH_CASES = 10064;
const MAX_REQUEST_BYTES = 48 * 1024 * 1024;
// The selected 12.2 package exports these actual helpers from this shared chunk.
// A changed selected package must admit its new seam explicitly instead of guessing a decoder.
const decoderModule = "mermaid/dist/chunks/mermaid.core/chunk-7S6XSTFO.mjs";

export async function prepareObservationAdapter(directory) {
  const compiler = require("esbuild");
  const sourcePath = require.resolve(decoderModule);
  const sourceSha256 = createHash("sha256").update(await readFile(sourcePath)).digest("hex");
  const result = await compiler.build({
    stdin: { contents: `import { decodeEntities, utils_default } from '${decoderModule}'; globalThis.mermanParserObservation = { decodeEntities, entityDecode: utils_default.entityDecode };`, resolveDir: path.dirname(modulePath), sourcefile: "parser-observation.mjs" },
    bundle: true, format: "iife", platform: "browser", target: "es2022", write: false, legalComments: "none", logLevel: "warning",
  });
  if (result.outputFiles.length !== 1) throw new Error("Decoder must be one bundled artifact");
  const bytes = result.outputFiles[0].contents;
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  await mkdir(directory, { recursive: true });
  const artifactPath = path.join(directory, `parser-observation-${sha256}.js`);
  await writeFile(artifactPath, bytes);
  return { artifact_path: artifactPath, artifact_sha256: sha256, source_sha256: sourceSha256 };
}

export function validateRequest(input) {
  if (!input || input.schema_version !== 1) throw new Error("Unknown parser oracle request schema");
  if (Object.keys(input).some((key) => !["schema_version", "runtime", "cases"].includes(key))) throw new Error("Unknown request field");
  if (!input.runtime || typeof input.runtime.artifact_path !== "string" || !/^[a-f0-9]{64}$/.test(input.runtime.artifact_sha256) || typeof input.runtime.browser_executable !== "string") throw new Error("Invalid runtime descriptor");
  if (!input.runtime.decoder || typeof input.runtime.decoder.artifact_path !== "string" || !/^[a-f0-9]{64}$/.test(input.runtime.decoder.artifact_sha256) || !/^[a-f0-9]{64}$/.test(input.runtime.decoder.source_sha256)) throw new Error("Invalid selected decoder descriptor");
  if (!Array.isArray(input.cases) || !input.cases.length || input.cases.length > MAX_BATCH_CASES) throw new Error("Invalid batch size");
  const ids = new Set();
  for (const item of input.cases) {
    if (typeof item.id !== "string" || !item.id || ids.has(item.id)) throw new Error("Invalid or duplicate case ID");
    ids.add(item.id);
    if (!["flowchart", "sequence"].includes(item.family)) throw new Error("Unsupported family");
    if (!["grammar", "public"].includes(item.lane)) throw new Error("Unsupported lane");
    if (typeof item.source !== "string" || Buffer.byteLength(item.source) > MAX_SOURCE_BYTES) throw new Error("Source exceeds 4096 byte bound");
    if (!item.site_config || !["strict", "loose"].includes(item.site_config.securityLevel)) throw new Error("Explicit strict/loose site configuration required");
    if (item.lane === "grammar" && (/^\s*---/.test(item.source) || item.source.includes("%%{"))) throw new Error("Frontmatter and directives require the public lane");
  }
  return input;
}

// This function executes in the selected browser realm. It observes real family DB accessors.
async function observeCase(item) {
  const mermaid = globalThis.mermanReferenceRuntime?.mermaid;
  if (!mermaid) throw new Error("Selected Mermaid runtime is unavailable");
  mermaid.initialize({ ...item.site_config, startOnLoad: false });
  let diagram;
  try {
    diagram = item.lane === "public"
      ? await mermaid.parse(item.source, { suppressErrors: false })
      : await mermaid.mermaidAPI.getDiagramFromText(item.source);
  } catch (error) {
    if (error instanceof TypeError || error instanceof ReferenceError || error instanceof RangeError) throw error;
    return { id: item.id, status: "rejected", error: String(error?.message ?? error) };
  }
  const diagramType = item.lane === "public" ? diagram.diagramType : diagram.type;
  if (!(item.family === "flowchart" ? ["flowchart", "flowchart-v2", "flowchart-elk"].includes(diagramType) : diagramType === "sequence")) throw new Error(`Detected family ${diagramType} disagrees with ${item.family}`);
  if (item.lane === "public") return { id: item.id, status: "accepted", observation: { diagramType, config: diagram.config } };
  const db = diagram.db;
  const decoder = globalThis.mermanParserObservation;
  if (!decoder) throw new Error("Selected label decoder is unavailable");
  const flowLabel = (text) => text == null ? null : decoder.entityDecode(decoder.decodeEntities(text));
  const common = { title: db.getDiagramTitle() || null, accTitle: db.getAccTitle() || null, accDescr: db.getAccDescription() || null };
  let observation;
  if (item.family === "flowchart") {
    observation = {
      ...common,
      direction: db.getDirection() ?? null,
      nodes: [...db.getVertices().values()].map((node) => ({
        id: node.id, label: flowLabel(node.text), labelType: node.labelType ?? null, shape: node.type ?? null,
        classes: node.classes, styles: node.styles, link: node.link ?? null, linkTarget: node.linkTarget ?? null,
        haveCallback: node.haveCallback ?? false, tooltip: db.getTooltip(node.id) ?? null,
        icon: node.icon ?? null, form: node.form ?? null, pos: node.pos ?? null, img: node.img ?? null,
        constraint: node.constraint ?? null, assetWidth: node.assetWidth ?? null, assetHeight: node.assetHeight ?? null,
      })),
      edges: db.getEdges().map((edge) => ({
        from: edge.start, to: edge.end, label: edge.text ? flowLabel(edge.text) : null, labelType: edge.labelType ?? null,
        type: edge.type ?? null, stroke: edge.stroke ?? null, length: edge.length,
        id: edge.isUserDefinedId ? edge.id : null, isUserDefinedId: edge.isUserDefinedId,
        classes: edge.classes, style: edge.style ?? [], interpolate: edge.interpolate ?? null,
        animate: edge.animate ?? null, animation: edge.animation ?? null,
      })),
      edgeDefaults: { style: db.getEdges().defaultStyle ?? [], interpolate: db.getEdges().defaultInterpolate ?? null },
      subgraphs: db.getSubGraphs().map((group) => ({ id: group.id, title: flowLabel(group.title), dir: group.dir ?? null, labelType: group.labelType ?? null, classes: group.classes, nodes: group.nodes, metadata: group.metadata ?? null })),
      classDefs: Object.fromEntries([...db.getClasses()].map(([id, value]) => [id, value.styles])),
    };
  } else {
    observation = {
      ...common,
      actorOrder: db.getActorKeys(),
      actors: Object.fromEntries([...db.getActors()].map(([id, actor]) => [id, {
        name: actor.name, description: actor.description, type: actor.type, wrap: actor.wrap,
        links: actor.links, properties: actor.properties,
      }])),
      messages: db.getMessages().map((message) => ({
        from: message.from ?? null, to: message.to ?? null, type: message.type ?? null,
        message: message.message, wrap: message.wrap, activate: message.activate ?? false,
        placement: message.placement ?? null, centralConnection: message.centralConnection ?? 0,
      })),
      boxes: db.getBoxes().map((box) => ({ actorKeys: box.actorKeys, name: box.name ?? null, fill: box.fill, wrap: box.wrap })),
      createdActors: Object.fromEntries(db.getCreatedActors()), destroyedActors: Object.fromEntries(db.getDestroyedActors()),
    };
  }
  // Detach every record from mutable singleton state before the next parse.
  return { id: item.id, status: "accepted", observation: JSON.parse(JSON.stringify(observation)) };
}

export async function runOracle(input, requestSha256) {
  validateRequest(input);
  const bytes = await readFile(input.runtime.artifact_path);
  const artifactSha256 = createHash("sha256").update(bytes).digest("hex");
  if (artifactSha256 !== input.runtime.artifact_sha256) throw new Error("Runtime artifact digest changed");
  const decoderBytes = await readFile(input.runtime.decoder.artifact_path);
  const decoderSha256 = createHash("sha256").update(decoderBytes).digest("hex");
  const decoderSourceSha256 = createHash("sha256").update(await readFile(require.resolve(decoderModule))).digest("hex");
  if (decoderSha256 !== input.runtime.decoder.artifact_sha256 || decoderSourceSha256 !== input.runtime.decoder.source_sha256) throw new Error("Selected decoder artifact/source digest changed");
  const puppeteer = require("puppeteer");
  const browser = await puppeteer.launch({ executablePath: input.runtime.browser_executable, browser: "chrome", headless: "shell", detached: false, args: ["--no-sandbox", "--disable-setuid-sandbox"] });
  try {
    const page = await browser.newPage();
    page.setDefaultTimeout(10_000);
    await page.addScriptTag({ content: bytes.toString("utf8") });
    await page.addScriptTag({ content: decoderBytes.toString("utf8") });
    const outcomes = [];
    for (const item of input.cases) {
      let timer;
      try {
        outcomes.push(await Promise.race([
          page.evaluate(observeCase, item),
          new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`Oracle case ${item.id} timed out`)), 10_000); }),
        ]));
      } finally { clearTimeout(timer); }
    }
    return { schema_version: 1, request_sha256: requestSha256, artifact_sha256: artifactSha256, browser_version: await browser.version(), decoder_sha256: decoderSha256, decoder_source_sha256: decoderSourceSha256, outcomes };
  } finally {
    await browser.close();
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  try {
    if (process.argv[2] === "--prepare-adapter") {
      if (process.argv.length !== 4) throw new Error("Adapter preparation needs its owned output directory");
      process.stdout.write(`${JSON.stringify(await prepareObservationAdapter(process.argv[3]))}\n`);
    } else {
    if (process.argv.length !== 3) throw new Error("usage: node parser-oracle.mjs REQUEST_JSON");
    const bytes = await readFile(process.argv[2]);
    if (bytes.length > MAX_REQUEST_BYTES) throw new Error("Oracle request exceeds bounded input");
    const input = JSON.parse(bytes);
    const output = await runOracle(input, createHash("sha256").update(bytes).digest("hex"));
    process.stdout.write(`${JSON.stringify(output)}\n`);
    }
  } catch (error) {
    console.error(error?.stack ?? String(error));
    process.exitCode = 1;
  }
}
