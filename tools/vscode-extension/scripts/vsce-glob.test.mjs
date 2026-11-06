import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { listFiles, PackageManager } from "@vscode/vsce";

async function extensionFixture(context, files, manifest = {}) {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "merman-vsce-glob-"));
  context.after(() => rm(cwd, { recursive: true, force: true }));
  const contents = {
    "package.json": JSON.stringify({
      name: "glob-compatibility-fixture",
      publisher: "merman",
      version: "0.0.0",
      engines: { vscode: "^1.145.0" },
      ...manifest,
    }),
    "README.md": "# Package filtering fixture\n",
    ...files,
  };
  for (const [relative, source] of Object.entries(contents)) {
    const file = path.join(cwd, relative);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, source);
  }
  return cwd;
}

async function packagedFiles(cwd) {
  const files = await listFiles({ cwd, packageManager: PackageManager.None });
  return files.map((file) => file.replaceAll("\\", "/")).sort();
}

test("VSCE preserves excluded directories, brace patterns, dotfiles, and negated maps", async (context) => {
  const cwd = await extensionFixture(context, {
    ".vscodeignore": [
      "src/",
      "dist/**/*.map",
      "!dist/public.js.map",
      "secret/",
      ".cache/",
      "assets/{draft,backup}.*",
    ].join("\n"),
    "src/extension.ts": "export {};",
    "dist/extension.js": "module.exports = {};",
    "dist/extension.js.map": "{}",
    "dist/public.js.map": "{}",
    "secret/token.txt": "fixture",
    ".cache/tool.log": "fixture",
    "assets/draft.svg": "<svg/>",
    "assets/backup.svg": "<svg/>",
    "assets/final.svg": "<svg/>",
    "media/.preview.svg": "<svg/>",
  });

  assert.deepEqual(await packagedFiles(cwd), [
    "README.md",
    "assets/final.svg",
    "dist/extension.js",
    "dist/public.js.map",
    "media/.preview.svg",
    "package.json",
  ]);
});

test("VSCE retains manifest include patterns and its mandatory package files", async (context) => {
  const cwd = await extensionFixture(
    context,
    {
      "dist/extension.js": "module.exports = {};",
      "dist/nested/helper.js": "module.exports = {};",
      "media/preview.css": "body {}",
      "media/draft.svg": "<svg/>",
      "src/extension.ts": "export {};",
      "secret/token.txt": "fixture",
    },
    { files: ["dist/**", "media/*.css"] },
  );

  assert.deepEqual(await packagedFiles(cwd), [
    "README.md",
    "dist/extension.js",
    "dist/nested/helper.js",
    "media/preview.css",
    "package.json",
  ]);
});
