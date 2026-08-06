import path from "node:path";

import {
  defineConfig,
  normalizePath,
  type Plugin,
  type UserConfig,
} from "vite";

import productionConfig from "./vite.config";

const playgroundRoot = import.meta.dirname;
const repositoryRoot = path.resolve(playgroundRoot, "..");
const variant = process.env.MERMAN_EDITOR_ARTIFACT_VARIANT;
const outDir = process.env.MERMAN_EDITOR_ARTIFACT_OUT_DIR;

if (variant !== "full" && variant !== "editor") {
  throw new Error(
    "MERMAN_EDITOR_ARTIFACT_VARIANT must be either full or editor.",
  );
}
if (!outDir || !path.isAbsolute(outDir)) {
  throw new Error("MERMAN_EDITOR_ARTIFACT_OUT_DIR must be an absolute path.");
}

const base = productionConfig as UserConfig;
const productionInputs = base.build?.rolldownOptions?.input;

if (
  !productionInputs ||
  typeof productionInputs !== "object" ||
  Array.isArray(productionInputs)
) {
  throw new Error(
    "The production Vite config must expose named HTML inputs for artifact measurement.",
  );
}

export default defineConfig({
  ...base,
  base: "/",
  build: {
    ...base.build,
    emptyOutDir: true,
    outDir,
    rolldownOptions: {
      ...base.build?.rolldownOptions,
      input: {
        ...productionInputs,
        editorSemanticEquivalence: path.join(
          playgroundRoot,
          "scripts/editor-artifact-measurement/semantic-equivalence.html",
        ),
      },
    },
  },
  optimizeDeps: {
    ...base.optimizeDeps,
    exclude: ["@mermanjs/web", "@mermanjs/web-editor"],
  },
  plugins: base.plugins,
  worker: {
    ...base.worker,
    plugins: () => [
      editorArtifactVariantPlugin(),
      ...(base.worker?.plugins?.() ?? []),
    ],
  },
});

function editorArtifactVariantPlugin(): Plugin {
  const editorEntry = path.join(
    repositoryRoot,
    "platforms/web/packages/editor/dist/package-entries/editor.js",
  );
  const editorSourceRoot = `${normalizePath(
    path.join(playgroundRoot, "src/editor"),
  )}/`;

  return {
    name: "merman-editor-artifact-measurement",
    enforce: "pre",
    resolveId(source, importer) {
      if (
        variant === "editor" &&
        source === "@mermanjs/web" &&
        importer &&
        normalizeImporter(importer).startsWith(editorSourceRoot)
      ) {
        return editorEntry;
      }
      return null;
    },
  };
}

function normalizeImporter(importer: string): string {
  return normalizePath(importer.split("?", 1)[0].split("#", 1)[0]);
}
