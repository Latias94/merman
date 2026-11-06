import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { ESLint } from "eslint";
import tseslint from "typescript-eslint";

async function sourceFixture(context, files) {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "merman-eslint-glob-"));
  context.after(() => rm(cwd, { recursive: true, force: true }));
  for (const [relative, source] of Object.entries(files)) {
    const file = path.join(cwd, relative);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, source);
  }
  return cwd;
}

test("ESLint discovers brace globs while preserving ignored directories and dotfiles", async (context) => {
  const cwd = await sourceFixture(context, {
    "src/plain.ts": "export const value: number = 1;",
    "src/view.tsx": "export const view = <div />;",
    "src/.hidden.ts": "export const hidden: number = 1;",
    "src/nested/child.ts": "export const child: number = 1;",
    "src/generated/skip.ts": "invalid source",
    "dist/skip.ts": "invalid source",
    "node_modules/skip.ts": "invalid source",
  });
  const eslint = new ESLint({
    cwd,
    overrideConfigFile: true,
    overrideConfig: [
      { ignores: ["dist/**", "src/generated/**"] },
      {
        files: ["**/*.{ts,tsx}"],
        languageOptions: { parser: tseslint.parser },
      },
    ],
  });

  const results = await eslint.lintFiles(["**/*.{ts,tsx}"]);
  assert.deepEqual(
    results.map((result) => path.relative(cwd, result.filePath).replaceAll("\\", "/")).sort(),
    ["src/.hidden.ts", "src/nested/child.ts", "src/plain.ts", "src/view.tsx"],
  );
  assert.ok(results.every((result) => result.messages.length === 0));
});

test("TypeScript ESLint project service admits explicit default-project globs with type information", async (context) => {
  const cwd = await sourceFixture(context, {
    "tsconfig.json": JSON.stringify({
      compilerOptions: { strict: true, target: "ESNext" },
      include: ["src/**/*.ts"],
    }),
    "src/in-project.ts": "Promise.resolve(42);",
    "standalone.ts": "Promise.resolve(42);",
  });
  const eslint = new ESLint({
    cwd,
    overrideConfigFile: true,
    overrideConfig: [
      {
        files: ["**/*.ts"],
        languageOptions: {
          parser: tseslint.parser,
          parserOptions: {
            tsconfigRootDir: cwd,
            projectService: { allowDefaultProject: ["*.ts"] },
          },
        },
        plugins: { "@typescript-eslint": tseslint.plugin },
        rules: { "@typescript-eslint/no-floating-promises": "error" },
      },
    ],
  });

  const results = await eslint.lintFiles(["**/*.ts"]);
  assert.equal(results.length, 2);
  for (const result of results) {
    assert.equal(result.fatalErrorCount, 0, JSON.stringify(result.messages));
    assert.deepEqual(
      result.messages.map((message) => message.ruleId),
      ["@typescript-eslint/no-floating-promises"],
      result.filePath,
    );
  }
});
