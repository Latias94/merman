import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const packageJson = JSON.parse(
  await readFile(path.resolve(import.meta.dirname, "..", "package.json"), "utf8")
);
const npmrc = await readFile(
  path.resolve(import.meta.dirname, "..", ".npmrc"),
  "utf8"
);
const webPackageJson = JSON.parse(
  await readFile(
    path.resolve(
      import.meta.dirname,
      "..",
      "..",
      "platforms/web/packages/full/package.json",
    ),
    "utf8",
  ),
);

test("dev, build, and test fail closed on the selected full browser artifact", () => {
  assert.match(npmrc, /^ignore-scripts=true$/mu);
  assert.equal(packageJson.packageManager, "npm@12.2.2");
  assert.equal(packageJson.engines.node, "^22.22.2 || ^24.15.0 || >=26.0.0");
  assert.equal(packageJson.engines.npm, ">=12.2.2");
  assert.equal(packageJson.scripts.predev, undefined);
  assert.equal(packageJson.scripts.prebuild, undefined);
  assert.equal(packageJson.scripts.pretest, undefined);
  assert.equal(packageJson.scripts.postbuild, undefined);
  assert.equal(packageJson.devDependencies["vite-plugin-wasm"], undefined);
  for (const script of ["dev", "build", "test"]) {
    assert.match(
      packageJson.scripts[script],
      /^npm run prepare:browser-runtime(?: && |$)/u
    );
  }
  assert.match(packageJson.scripts.build, /npm run build:prepared$/u);
  assert.match(packageJson.scripts["build:prepared"], /npm run verify:dist$/u);
  assert.match(packageJson.scripts["verify:wasm-inputs"], /verify-wasm-inputs\.mjs/);
  assert.match(packageJson.scripts["verify:wasm-inputs"], /--package full/);
  assert.match(packageJson.scripts["verify:wasm-inputs"], /--assembled/);
  assert.deepEqual(
    Object.fromEntries(
      Object.entries(packageJson.dependencies).filter(([name]) =>
        name.startsWith("@mermanjs/web"),
      ),
    ),
    {
      "@mermanjs/web": "file:../platforms/web/packages/full",
    },
  );
  assert.deepEqual(Object.keys(webPackageJson.exports ?? {}).sort(), [".", "./wasm"]);
  assert.equal(webPackageJson.exports["./wasm"], "./artifacts/wasm/merman_wasm_bg.wasm");
  assert.deepEqual(Object.keys(webPackageJson.exports["."]).sort(), [
    "import",
    "types",
  ]);
  assert.match(packageJson.scripts["prepare:browser-runtime"], /build:opaque-realm/);
  assert.match(packageJson.scripts["prepare:browser-runtime"], /verify:opaque-realm/);
});

test("Shoal preview opts into the experimental artifact through its own entrypoints", () => {
  const scripts = packageJson.scripts;
  assert.match(scripts["assemble:shoal"], /run check:contracts -- --local-shoal/u);
  assert.match(scripts["assemble:shoal"], /run build:packages -- --package full --local-shoal$/u);
  assert.match(scripts["prepare:browser-runtime:shoal"], /^npm run verify:wasm-inputs -- --local-shoal && /u);
  assert.match(scripts["dev:shoal"], /^npm run prepare:browser-runtime:shoal && vite$/u);
  assert.match(scripts["build:shoal"], /^npm run prepare:browser-runtime:shoal && npm run build:prepared$/u);
  assert.doesNotMatch(scripts["verify:wasm-inputs"], /--local-shoal/);
});

test("Grid and Layered use the same explicit local Playground entrypoints", () => {
  for (const [script, action] of [
    ["assemble:layout", "assemble"],
    ["prepare:browser-runtime:layout", "prepare"],
    ["dev:layout", "dev"],
    ["build:layout", "build"],
  ]) {
    assert.equal(packageJson.scripts[script], `node scripts/local-layout.mjs ${action}`);
  }
  const entry = path.join(import.meta.dirname, "local-layout.mjs");
  for (const args of [[], ["--local-shoal"], ["--local-layout", "grid", "--local-layout=layered"]]) {
    const result = spawnSync(process.execPath, [entry, "prepare", ...args], { encoding: "utf8" });
    assert.equal(result.status, 2, result.stderr);
    assert.match(result.stderr, /--local-layout/u);
  }
});

test("browser test tooling is isolated from the companion runtime tree", async () => {
  const testsRoot = path.resolve(import.meta.dirname, "..", "tests");
  const browserTestsPackageJson = JSON.parse(
    await readFile(path.join(testsRoot, "package.json"), "utf8")
  );
  const browserTestsNpmrc = await readFile(
    path.join(testsRoot, ".npmrc"),
    "utf8"
  );
  const runtimeLock = JSON.parse(
    await readFile(path.resolve(import.meta.dirname, "..", "package-lock.json"), "utf8")
  );
  const browserTestsLock = JSON.parse(
    await readFile(path.join(testsRoot, "package-lock.json"), "utf8")
  );

  for (const dependency of [
    "@axe-core/playwright",
    "@playwright/test",
    "playwright",
    "playwright-core",
  ]) {
    assert.equal(packageJson.dependencies?.[dependency], undefined);
    assert.equal(packageJson.devDependencies?.[dependency], undefined);
  }

  assert.equal(browserTestsPackageJson.private, true);
  assert.equal(
    browserTestsPackageJson.packageManager,
    packageJson.packageManager
  );
  assert.deepEqual(browserTestsPackageJson.engines, packageJson.engines);
  assert.equal(
    browserTestsPackageJson.devDependencies["@axe-core/playwright"],
    "4.14.0"
  );
  assert.equal(
    browserTestsPackageJson.devDependencies["@playwright/test"],
    "1.65.0"
  );
  assert.equal(browserTestsPackageJson.devDependencies.playwright, "1.65.0");
  assert.equal(browserTestsPackageJson.dependencies?.["@zenuml/core"], undefined);
  assert.equal(
    browserTestsPackageJson.dependencies?.["@mermaid-js/mermaid-zenuml"],
    undefined
  );
  assert.match(browserTestsNpmrc, /^ignore-scripts=true$/mu);
  assert.match(packageJson.scripts["test:browser:typecheck"], /--prefix tests/u);
  assert.match(
    packageJson.scripts["test:browser:chromium:desktop:built"],
    /--prefix tests/u,
  );
  assert.match(
    packageJson.scripts["test:browser:mobile:built"],
    /--prefix tests/u,
  );
  assert.match(
    packageJson.scripts["test:browser:smoke:non-chromium:built"],
    /--prefix tests/u,
  );

  for (const packagePath of [
    "node_modules/@playwright/test",
    "node_modules/playwright",
    "node_modules/playwright-core",
  ]) {
    assert.equal(runtimeLock.packages[packagePath], undefined);
  }
  assert.equal(
    browserTestsLock.packages["node_modules/@playwright/test"].version,
    "1.65.0"
  );
  assert.equal(
    browserTestsLock.packages["node_modules/playwright"].version,
    "1.65.0"
  );
  assert.equal(
    browserTestsLock.packages["node_modules/playwright-core"].version,
    "1.65.0"
  );
  for (const packagePath of [
    "node_modules/@mermaid-js/mermaid-zenuml",
    "node_modules/@zenuml/core",
    "node_modules/mermaid",
  ]) {
    assert.equal(browserTestsLock.packages[packagePath], undefined);
  }
});
