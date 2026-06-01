import { existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const required = [
  path.join(root, "dist", "index.js"),
  path.join(root, "dist", "index.d.ts"),
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
