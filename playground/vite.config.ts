import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import wasm from "vite-plugin-wasm";
import path from "path";

export default defineConfig({
  plugins: [react(), wasm()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./"),
    },
  },
  // GitHub Pages 部署时使用仓库名作为 base
  // 本地开发时使用 /
  base: process.env.NODE_ENV === "production" ? "/merman/" : "/",
  build: {
    outDir: "dist",
    target: "esnext",
    rolldownOptions: {
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
      // 允许访问 WASM 文件
      allow: [".."],
    },
  },
});
