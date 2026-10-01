import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, realpath, rename, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const modulePath = fileURLToPath(import.meta.url);

function isInside(parent, child) {
  const relative = path.relative(parent, child);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

function validateReferenceRuntimeBuild(result, workspace, outfile) {
  const outputs = Object.entries(result.metafile?.outputs ?? {});
  if (result.outputFiles?.length !== 1 || outputs.length !== 1) {
    throw new Error("Reference runtime must be self-contained in exactly one JavaScript output.");
  }
  const [outputPath, outputMetadata] = outputs[0];
  if (path.resolve(workspace, outputPath) !== outfile || path.resolve(result.outputFiles[0].path) !== outfile) {
    throw new Error("Reference runtime compiler output escapes its owned artifact path.");
  }
  if (outputMetadata.imports.length > 0) {
    throw new Error("Reference runtime must be self-contained without external imports.");
  }
  return result.outputFiles[0];
}

/** Build the selected CLI module graph into one owned browser artifact. */
export async function prepareReferenceRuntime(cliRoot) {
  const workspace = await realpath(path.resolve(cliRoot));
  const manifest = JSON.parse(await readFile(path.join(workspace, "package.json"), "utf8"));
  const requireFromCli = createRequire(path.join(workspace, "package.json"));
  const compilerPath = await realpath(requireFromCli.resolve("esbuild"));
  if (!isInside(path.join(workspace, "node_modules"), compilerPath)) {
    throw new Error("Reference runtime compiler must come from the CLI workspace node_modules.");
  }
  const compiler = requireFromCli(compilerPath);

  const imports = [
    "import mermaid from 'mermaid';",
    "import sanitizer from 'dompurify';",
  ];
  const diagrams = [];
  let layouts = "[]";
  if (manifest.devDependencies?.["@mermaid-js/mermaid-zenuml"]) {
    imports.push("import zenuml from '@mermaid-js/mermaid-zenuml';");
    diagrams.push("zenuml");
  }
  if (manifest.devDependencies?.["@mermaid-js/layout-tidy-tree"]) {
    imports.push("import tidyLayouts from '@mermaid-js/layout-tidy-tree';");
    layouts = "tidyLayouts";
  }
  const source = `${imports.join("\n")}\nglobalThis.mermanReferenceRuntime = { mermaid, sanitizer, externalDiagrams: [${diagrams.join(", ")}], externalLayouts: ${layouts} };\n`;

  const repository = await realpath(path.resolve(workspace, "../.."));
  const targetPath = path.join(repository, "target");
  await mkdir(targetPath, { recursive: true });
  const target = await realpath(targetPath);
  if (!isInside(repository, target)) {
    throw new Error("Reference runtime target directory escapes the repository.");
  }
  const artifactDirectory = path.join(target, "mermaid-reference-runtime");
  await mkdir(artifactDirectory, { recursive: true });
  const directory = await realpath(artifactDirectory);
  if (!isInside(target, directory)) {
    throw new Error("Reference runtime artifact directory escapes target.");
  }
  const outfile = path.join(directory, "reference-runtime.js");
  const result = await compiler.build({
    absWorkingDir: workspace,
    stdin: { contents: source, resolveDir: workspace, sourcefile: "merman-reference-runtime.mjs", loader: "js" },
    outfile,
    bundle: true,
    format: "iife",
    platform: "browser",
    target: "es2022",
    legalComments: "eof",
    metafile: true,
    write: false,
    logLevel: "warning",
  });
  const file = validateReferenceRuntimeBuild(result, workspace, outfile);

  const digest = createHash("sha256").update(file.contents).digest("hex");
  const artifactPath = path.join(directory, `${digest}.js`);
  const temporaryPath = path.join(directory, `.${digest}.${randomUUID()}.tmp`);
  try {
    await writeFile(temporaryPath, file.contents, { flag: "wx" });
    await rename(temporaryPath, artifactPath);
  } finally {
    await rm(temporaryPath, { force: true });
  }
  return {
    artifact_path: artifactPath,
    artifact_sha256: digest,
    compiler_version: compiler.version,
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  try {
    const { values } = parseArgs({ options: { workspace: { type: "string" } } });
    const descriptor = await prepareReferenceRuntime(values.workspace ?? path.dirname(modulePath));
    process.stdout.write(`${JSON.stringify(descriptor)}\n`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
