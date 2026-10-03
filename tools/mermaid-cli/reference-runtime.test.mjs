import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import vm from "node:vm";
import { prepareReferenceRuntime } from "./reference-runtime.mjs";

const cliRoot = path.dirname(fileURLToPath(import.meta.url));
const compilerPath = createRequire(import.meta.url).resolve("esbuild");
const execute = promisify(execFile);

async function fixture(t, { selected = true, core } = {}) {
  const temporary = await mkdtemp(path.join(os.tmpdir(), "merman-reference-runtime-"));
  t.after(async () => {
    assert.equal(path.dirname(temporary), path.resolve(os.tmpdir()));
    assert.match(path.basename(temporary), /^merman-reference-runtime-/);
    await rm(temporary, { recursive: true, force: true });
  });
  const workspace = path.join(temporary, "repo", "tools", "mermaid-cli");
  async function put(relative, text) {
    const file = path.join(workspace, relative);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, text);
  }
  await put("package.json", JSON.stringify({ devDependencies: selected ? {
    "@mermaid-js/mermaid-zenuml": "1.0.0",
    "@mermaid-js/layout-tidy-tree": "1.0.0",
  } : {} }));
  await put("node_modules/esbuild/package.json", '{"main":"index.cjs"}');
  await put("node_modules/esbuild/index.cjs", `module.exports = require(${JSON.stringify(compilerPath)});`);
  await put("node_modules/mermaid/package.json", '{"type":"module","exports":"./dist/mermaid.core.mjs"}');
  await put("node_modules/mermaid/dist/mermaid.core.mjs", core ?? `
    import sanitizer from "dompurify";
    export default { render: value => sanitizer.sanitize(value) };
  `);
  await put("node_modules/mermaid/dist/mermaid.js", 'throw new Error("Vendored IIFE must not load");');
  await put("node_modules/mermaid/dist/mermaid.esm.mjs", 'throw new Error("Vendored ESM must not load");');
  await put("node_modules/dompurify/package.json", '{"type":"module","exports":"./index.js"}');
  await put("node_modules/dompurify/index.js", '/*! @license Test sanitizer */\nexport default { version: "fixture", sanitize: value => value };');
  for (const [name, source] of [
    ["@mermaid-js/mermaid-zenuml", 'export default { id: "zenuml" };'],
    ["@mermaid-js/layout-tidy-tree", 'export default [{ name: "tidy-tree" }];'],
    ["@mermaid-js/layout-elk", 'throw new Error("Legacy ELK must not load");'],
  ]) {
    await put(`node_modules/${name}/package.json`, '{"type":"module","exports":"./index.js"}');
    await put(`node_modules/${name}/index.js`, source);
  }
  return { temporary, workspace, put };
}

test("builds the installed selected core and sanitizer as one addressed artifact", async () => {
  const descriptor = await prepareReferenceRuntime(cliRoot);
  assert.deepEqual(Object.keys(descriptor).sort(), ["artifact_path", "artifact_sha256", "compiler_version"]);
  const manifest = JSON.parse(await readFile(path.join(cliRoot, "package.json"), "utf8"));
  assert.equal(descriptor.compiler_version, manifest.devDependencies.esbuild);
  assert.equal(path.isAbsolute(descriptor.artifact_path), true);
  assert.equal(path.basename(descriptor.artifact_path), `${descriptor.artifact_sha256}.js`);
  const bytes = await readFile(descriptor.artifact_path);
  assert.equal(createHash("sha256").update(bytes).digest("hex"), descriptor.artifact_sha256);
  const source = bytes.toString("utf8");
  assert.match(source, /node_modules\/mermaid\/dist\/mermaid\.core\.mjs/);
  assert.match(source, /node_modules\/dompurify\/dist\/purify\.es\.mjs/);
  assert.doesNotMatch(source, /node_modules\/mermaid\/dist\/mermaid\.(?:esm\.mjs|js)(?:\s|:)/);
  assert.ok(source.includes(`DOMPurify ${manifest.overrides.dompurify}`));
  assert.match(source, /mermanReferenceRuntime/);
  assert.match(source, /@license/);
});


test("the bare core shares the exposed sanitizer and selected registration arrays", async (t) => {
  const { workspace } = await fixture(t);
  const descriptor = await prepareReferenceRuntime(workspace);
  const source = await readFile(descriptor.artifact_path, "utf8");
  const context = vm.createContext({});
  vm.runInContext(source, context);
  const runtime = context.mermanReferenceRuntime;
  assert.deepEqual(Object.keys(runtime).sort(), ["externalDiagrams", "externalLayouts", "mermaid", "sanitizer"]);
  assert.equal(runtime.externalDiagrams.length, 1);
  assert.equal(runtime.externalDiagrams[0].id, "zenuml");
  assert.equal(runtime.externalLayouts.length, 1);
  assert.equal(runtime.externalLayouts[0].name, "tidy-tree");
  let calls = 0;
  runtime.sanitizer.sanitize = value => { calls += 1; return `clean:${value}`; };
  assert.equal(runtime.mermaid.render("input"), "clean:input");
  assert.equal(calls, 1);
  assert.match(source, /@license Test sanitizer/);
  assert.doesNotMatch(source, /Vendored|Legacy ELK/);
  assert.deepEqual(await prepareReferenceRuntime(workspace), descriptor);
  assert.deepEqual(await readdir(path.dirname(descriptor.artifact_path)), [path.basename(descriptor.artifact_path)]);
});

test("installed but unselected companions are not registered", async (t) => {
  const { workspace } = await fixture(t, { selected: false });
  const descriptor = await prepareReferenceRuntime(workspace);
  const context = vm.createContext({});
  vm.runInContext(await readFile(descriptor.artifact_path, "utf8"), context);
  assert.equal(context.mermanReferenceRuntime.externalDiagrams.length, 0);
  assert.equal(context.mermanReferenceRuntime.externalLayouts.length, 0);
});

test("rejects a runtime external import instead of publishing a partial bundle", async (t) => {
  const { workspace } = await fixture(t, {
    core: 'import external from "https://example.invalid/runtime.js"; export default external;',
  });
  await assert.rejects(prepareReferenceRuntime(workspace), /self-contained without external imports/);
  assert.deepEqual(await readdir(path.resolve(workspace, "../../target/mermaid-reference-runtime")), []);
});

test("rejects CSS side output instead of discarding it", async (t) => {
  const { workspace, put } = await fixture(t, {
    core: 'import "./style.css"; export default {};',
  });
  await put("node_modules/mermaid/dist/style.css", "body { color: red; }");
  await assert.rejects(prepareReferenceRuntime(workspace), /exactly one JavaScript output/);
  assert.deepEqual(await readdir(path.resolve(workspace, "../../target/mermaid-reference-runtime")), []);
});

test("rejects compiler output outside the owned artifact path", async (t) => {
  const { workspace, put } = await fixture(t);
  await put("node_modules/esbuild/index.cjs", `module.exports = {
    version: "fixture",
    build: async () => ({
      outputFiles: [{ path: ${JSON.stringify(path.join(workspace, "escaped.js"))}, contents: Buffer.from("escaped") }],
      metafile: { outputs: { "escaped.js": { imports: [] } } }
    })
  };`);
  await assert.rejects(prepareReferenceRuntime(workspace), /output escapes its owned artifact path/);
});

test("rejects an artifact directory redirected outside repository target", async (t) => {
  const { temporary, workspace } = await fixture(t);
  const target = path.resolve(workspace, "../../target");
  const outside = path.join(temporary, "outside");
  await mkdir(target);
  await mkdir(outside);
  await symlink(outside, path.join(target, "mermaid-reference-runtime"), process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(prepareReferenceRuntime(workspace), /artifact directory escapes target/);
  assert.deepEqual(await readdir(outside), []);
});

test("the CLI writes only the receipt and reports failures without a receipt", async (t) => {
  const { workspace } = await fixture(t);
  const script = path.join(cliRoot, "reference-runtime.mjs");
  const { stdout, stderr } = await execute(process.execPath, [script, "--workspace", workspace]);
  const descriptor = JSON.parse(stdout);
  assert.deepEqual(Object.keys(descriptor).sort(), ["artifact_path", "artifact_sha256", "compiler_version"]);
  assert.equal(stderr, "");
  assert.equal(path.isAbsolute(descriptor.artifact_path), true);
  await assert.rejects(execute(process.execPath, [script, "--unknown"]), error => {
    assert.equal(error.code, 1);
    assert.equal(error.stdout, "");
    assert.match(error.stderr, /Unknown option/);
    return true;
  });
});
