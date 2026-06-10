import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const generatedPackageJson = path.join(root, "pkg", "package.json");
const presetManifest = path.join(root, "pkg", "merman_wasm_preset.json");
const required = [
  path.join(root, "dist", "index.js"),
  path.join(root, "dist", "index.d.ts"),
  generatedPackageJson,
  presetManifest,
  path.join(root, "pkg", "merman_wasm.js"),
  path.join(root, "pkg", "merman_wasm_bg.wasm"),
];

const missing = required.filter((file) => {
  try {
    return !existsSync(file) || !statSync(file).isFile() || statSync(file).size === 0;
  } catch {
    return true;
  }
});

if (missing.length > 0) {
  console.error(
    [
      "prepack: missing generated web package files.",
      "Run `npm run build --prefix platforms/web` before pack/publish.",
      ...missing.map((file) => `  - ${path.relative(root, file)}`),
    ].join("\n"),
  );
  process.exit(1);
}

try {
  const packageJson = JSON.parse(readFileSync(generatedPackageJson, "utf8"));
  if (packageJson.type !== "module") {
    console.error("prepack: generated pkg/package.json must declare `type: module`.");
    console.error("Run `npm run build --prefix platforms/web` before pack/publish.");
    process.exit(1);
  }
} catch (error) {
  console.error(`prepack: failed to read generated pkg/package.json: ${error.message}`);
  process.exit(1);
}

try {
  const manifest = JSON.parse(readFileSync(presetManifest, "utf8"));
  const allowNonDefaultPreset = process.env.MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET === "1";
  if (manifest.preset !== "browser-full" && !allowNonDefaultPreset) {
    console.error(
      [
        `prepack: generated WASM preset is '${manifest.preset}', expected 'browser-full'.`,
        "The published @mermanjs/web package currently defaults to the full browser artifact.",
        "Rebuild with `npm run build:wasm:full --prefix platforms/web` before pack/publish,",
        "or set MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1 for an intentional local slim package.",
      ].join("\n"),
    );
    process.exit(1);
  }
} catch (error) {
  console.error(`prepack: failed to read pkg/merman_wasm_preset.json: ${error.message}`);
  process.exit(1);
}
