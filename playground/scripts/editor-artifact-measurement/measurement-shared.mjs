import { createHash } from "node:crypto";
import path from "node:path";

export function normalizeSource(source) {
  return source.replaceAll("\\", "/");
}

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function contentTypeFor(file) {
  switch (path.extname(file).toLowerCase()) {
    case ".css":
      return "text/css; charset=utf-8";
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
    case ".mjs":
      return "text/javascript; charset=utf-8";
    case ".json":
    case ".map":
      return "application/json; charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".wasm":
      return "application/wasm";
    case ".woff":
      return "font/woff";
    case ".woff2":
      return "font/woff2";
    default:
      return "application/octet-stream";
  }
}

export function isCompressible(contentType) {
  return (
    contentType.startsWith("text/") ||
    contentType.startsWith("application/json") ||
    contentType === "application/wasm" ||
    contentType === "image/svg+xml"
  );
}
