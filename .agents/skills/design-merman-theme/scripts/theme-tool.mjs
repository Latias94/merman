import { readFile, writeFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const HELP = `Merman theme design tool (Node.js 22+)

  catalog
  export <preset-id>
  materialize <design.json>
  support <query.json>
  render <theme.json> <diagram.mmd> --out <preview.svg> [--options <options.json>]

Options:
  --engine <module>  ESM module exporting createNodeEngine; default @mermanjs/node
  --out <path>       Write JSON instead of stdout; required for render
  --options <path>   Render options without theme; schema version defaults to 3
  --force            Intentionally replace existing output files
  --help            Show usage without loading an engine

Render writes <preview>.metadata.json from the same operation. Materialize takes
the design payload, not the recipe envelope. Rust owns validation and admission.
`;

function argumentsFor(argv) {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      engine: { type: "string", default: "@mermanjs/node" },
      out: { type: "string" },
      options: { type: "string" },
      force: { type: "boolean", default: false },
      help: { type: "boolean", default: false },
    },
  });
  if (values.help) return { help: true };
  const [command, ...inputs] = positionals;
  const arity = { catalog: 0, export: 1, materialize: 1, support: 1, render: 2 };
  if (!Object.hasOwn(arity, command) || inputs.length !== arity[command]) {
    throw new Error(`Invalid command or arguments.\n${HELP}`);
  }
  if (values.options && command !== "render") throw new Error("--options is only valid for render.");
  if (command === "render" && (!values.out || !values.out.endsWith(".svg"))) {
    throw new Error("render requires --out <preview.svg>.");
  }
  return { ...values, command, inputs };
}

async function loadEngine(specifier) {
  const isPath = path.isAbsolute(specifier) || specifier.startsWith(".");
  const module = await import(isPath ? pathToFileURL(path.resolve(specifier)).href : specifier);
  if (typeof module.createNodeEngine !== "function") {
    throw new Error("The engine module must export createNodeEngine().");
  }
  return module.createNodeEngine();
}

// Resolve aliases before publishing so --force cannot overwrite a theme or source through a link.
async function outputIdentity(file) {
  try { return await realpath(file); }
  catch (error) {
    if (error.code !== "ENOENT") throw error;
    return path.join(await realpath(path.dirname(file)), path.basename(file));
  }
}

async function checkOutputs(outputs, inputs, force) {
  const inputPaths = await Promise.all(inputs.map((file) => realpath(file)));
  const identities = new Set();
  const existingOutputs = [];
  for (const file of outputs) {
    const identity = await outputIdentity(file);
    if (inputPaths.includes(identity) || identities.has(identity)) {
      throw new Error(`Output collides with an input or another output: ${file}`);
    }
    identities.add(identity);
    try {
      const existing = await stat(file);
      if (!force || !existing.isFile()) throw new Error(`Output already exists: ${file}`);
      if (existingOutputs.some((other) => existing.dev === other.dev && existing.ino === other.ino)) {
        throw new Error(`Outputs are linked to the same file: ${file}`);
      }
      existingOutputs.push(existing);
      for (const input of inputs) {
        const source = await stat(input);
        if (existing.dev === source.dev && existing.ino === source.ino) {
          throw new Error(`Output is linked to an input: ${file}`);
        }
      }
    } catch (error) { if (error.code !== "ENOENT") throw error; }
  }
}

export async function run(argv, { createEngine = loadEngine, stdout = (text) => process.stdout.write(text) } = {}) {
  const args = argumentsFor(argv);
  if (args.help) { stdout(HELP); return; }
  const { command, inputs } = args;
  const files = command === "export" || command === "catalog" ? [] : [...inputs];
  if (args.options) files.push(args.options);
  const outputs = args.out ? [path.resolve(args.out)] : [];
  if (command === "render") outputs.push(outputs[0].slice(0, -4) + ".metadata.json");
  await checkOutputs(outputs, files, args.force);

  let source = command === "export" ? inputs[0] : command === "catalog" ? undefined : await readFile(inputs[0], "utf8");
  let optionsJson;
  if (command === "render") {
    const recipe = source;
    JSON.parse(recipe);
    const options = args.options ? JSON.parse(await readFile(args.options, "utf8")) : {};
    if (!options || typeof options !== "object" || Array.isArray(options) || Object.hasOwn(options, "theme")) {
      throw new Error("Render options must be an object without theme; pass the theme as its own file.");
    }
    if (!Object.hasOwn(options, "version")) options.version = 3;
    // Keep original recipe text: normalizing it in JavaScript would conceal duplicate-key errors.
    optionsJson = `${JSON.stringify(options).slice(0, -1)},"theme":${recipe}}`;
    source = await readFile(inputs[1], "utf8");
  }
  const engine = await createEngine(args.engine);
  try {
    if (command === "catalog") {
      await publish(engine.metadataJson("theme-catalog"));
    } else {
      const operationId = { export: "export-theme-preset-json", materialize: "materialize-theme-json", support: "describe-theme-support-json", render: "svg" }[command];
      const result = await engine.executeOperation({ operationId, source, ...(optionsJson ? { optionsJson } : {}) });
      if (command === "render") {
        if (result.media_type !== "image/svg+xml" || typeof result.metadata_json !== "string") {
          throw new Error("Render did not return SVG and metadata from one execution.");
        }
        const metadata = JSON.parse(result.metadata_json);
        await writeFile(outputs[0], result.data, { flag: args.force ? "w" : "wx" });
        await writeFile(outputs[1], JSON.stringify(metadata, null, 2) + "\n", { flag: args.force ? "w" : "wx" });
        stdout(JSON.stringify({ svg: outputs[0], metadata: outputs[1] }) + "\n");
      } else {
        if (result.media_type !== "application/json") throw new Error("Expected JSON from the theme operation.");
        await publish(result.data);
      }
    }
  } finally { await engine.dispose(); }

  async function publish(json) {
    const readable = JSON.stringify(JSON.parse(json), null, 2) + "\n";
    if (outputs.length) await writeFile(outputs[0], readable, { flag: args.force ? "w" : "wx" });
    else stdout(readable);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { await run(process.argv.slice(2)); }
  catch (error) {
    process.stderr.write(JSON.stringify({ message: error.message, ...(error.codeName ? { code: error.codeName } : {}), ...(error.details ? { details: error.details } : {}) }) + "\n");
    process.exitCode = 1;
  }
}
