import { rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
rmSync(path.join(webRoot, "dist"), { recursive: true, force: true });
