import assert from "node:assert/strict";
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
const viteConfig = await readFile(
  path.resolve(import.meta.dirname, "..", "vite.config.ts"),
  "utf8",
);

test("dev, build, and test fail closed on the selected full browser artifact", () => {
  assert.match(npmrc, /^ignore-scripts=true$/mu);
  assert.equal(packageJson.packageManager, "npm@11.17.0");
  assert.equal(packageJson.engines.node, "^22.13.0 || >=24.0.0");
  assert.equal(packageJson.engines.npm, ">=11.17.0");
  assert.equal(packageJson.scripts.predev, undefined);
  assert.equal(packageJson.scripts.prebuild, undefined);
  assert.equal(packageJson.scripts.pretest, undefined);
  assert.equal(packageJson.scripts.postbuild, undefined);
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
  assert.match(packageJson.scripts["prepare:browser-runtime"], /build:opaque-realm/);
  assert.match(packageJson.scripts["prepare:browser-runtime"], /verify:opaque-realm/);
  for (const packageName of Object.keys(packageJson.dependencies).filter((name) =>
    name.startsWith("@mermanjs/web"),
  )) {
    assert.match(
      viteConfig,
      new RegExp(`exclude:[\\s\\S]*["']${packageName}["']`, "u"),
      `${packageName} must retain its package-relative WASM URL`,
    );
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
    "4.12.1"
  );
  assert.equal(
    browserTestsPackageJson.devDependencies["@playwright/test"],
    "1.61.1"
  );
  assert.equal(browserTestsPackageJson.devDependencies.playwright, "1.61.1");
  assert.equal(browserTestsPackageJson.dependencies?.["@zenuml/core"], undefined);
  assert.equal(
    browserTestsPackageJson.dependencies?.["@mermaid-js/mermaid-zenuml"],
    undefined
  );
  assert.match(browserTestsNpmrc, /^ignore-scripts=true$/mu);
  assert.match(packageJson.scripts["test:browser:typecheck"], /--prefix tests/u);
  assert.match(packageJson.scripts["test:browser:chromium"], /--prefix tests/u);
  assert.match(packageJson.scripts["test:browser:smoke:built"], /--prefix tests/u);

  for (const packagePath of [
    "node_modules/@playwright/test",
    "node_modules/playwright",
    "node_modules/playwright-core",
  ]) {
    assert.equal(runtimeLock.packages[packagePath], undefined);
  }
  assert.equal(
    browserTestsLock.packages["node_modules/@playwright/test"].version,
    "1.61.1"
  );
  assert.equal(
    browserTestsLock.packages["node_modules/playwright"].version,
    "1.61.1"
  );
  assert.equal(
    browserTestsLock.packages["node_modules/playwright-core"].version,
    "1.61.1"
  );
  for (const packagePath of [
    "node_modules/@mermaid-js/mermaid-zenuml",
    "node_modules/@zenuml/core",
    "node_modules/mermaid",
  ]) {
    assert.equal(browserTestsLock.packages[packagePath], undefined);
  }
});
