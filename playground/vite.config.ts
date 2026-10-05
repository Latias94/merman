import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import {
  createOpaqueRealmCspPlugin,
  loadOpaqueRealmCspHashes,
} from "./scripts/opaque-realm-csp.mjs";
import { OPAQUE_REALM_ARTIFACT_PLAN } from "./scripts/opaque-realm-artifact-plan.mjs";
import { readBuildIdentity } from "./scripts/build-identity.mjs";

const playgroundRoot = import.meta.dirname;
const buildIdentity = readBuildIdentity(path.resolve(playgroundRoot, ".."));
const opaqueRealmCspHashes = loadOpaqueRealmCspHashes(playgroundRoot);

export default defineConfig({
  define: {
    __PLAYGROUND_BUILD__: JSON.stringify(buildIdentity),
  },
  plugins: [
    createOpaqueRealmCspPlugin(opaqueRealmCspHashes),
    react(),
    {
      name: "playground-build-identity",
      generateBundle() {
        this.emitFile({
          type: "asset",
          fileName: "build-info.json",
          source: `${JSON.stringify(buildIdentity, null, 2)}\n`,
        });
      },
    },
  ],
  resolve: {
    alias: {
      "@": path.resolve(playgroundRoot, "./"),
    },
  },
  // GitHub Pages uses the repository name as its production base.
  // Local development remains rooted at `/`.
  base: process.env.NODE_ENV === "production" ? "/merman/" : "/",
  build: {
    manifest: true,
    outDir: "dist",
    target: "esnext",
    rolldownOptions: {
      input: Object.fromEntries(
        OPAQUE_REALM_ARTIFACT_PLAN.pages.map(
          (page: Readonly<{ key: string; source: string }>) => [
            page.key,
            path.resolve(playgroundRoot, page.source),
          ],
        ),
      ),
      output: {
        codeSplitting: true,
      },
    },
  },
  optimizeDeps: {
    exclude: ["@mermanjs/web"],
  },
  server: {
    fs: {
      // Keep package-relative WASM artifacts reachable during development.
      allow: [".."],
    },
  },
});
