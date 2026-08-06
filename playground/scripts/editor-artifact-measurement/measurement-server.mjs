import { createServer } from "node:http";

import { EDITOR_ARTIFACT_VARIANTS } from "./contract.mjs";

export async function createMeasurementServers(builds) {
  const servers = {};
  try {
    for (const variant of EDITOR_ARTIFACT_VARIANTS) {
      servers[variant] = await createMeasurementServer(
        builds[variant].staticFiles,
      );
    }
    return servers;
  } catch (error) {
    await Promise.all(Object.values(servers).map((server) => server.close()));
    throw error;
  }
}

async function createMeasurementServer(files) {
  let observation = null;
  const server = createServer((request, response) => {
    let pathname;
    try {
      pathname = decodeURIComponent(
        new URL(request.url ?? "/", "http://local").pathname,
      );
    } catch {
      response.writeHead(400).end("Bad request");
      return;
    }
    if (pathname === "/") pathname = "/index.html";
    const file = files.get(pathname);
    if (!file) {
      response.writeHead(404).end("Not found");
      return;
    }
    const acceptsGzip = /(?:^|,)\s*gzip\s*(?:,|$)/iu.test(
      request.headers["accept-encoding"] ?? "",
    );
    const body = acceptsGzip && file.gzip ? file.gzip : file.body;
    const headers = {
      "Cache-Control": file.immutable
        ? "public, max-age=31536000, immutable"
        : "no-cache",
      "Content-Length": String(body.byteLength),
      "Content-Type": file.contentType,
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Resource-Policy": "same-origin",
    };
    if (acceptsGzip && file.gzip) headers["Content-Encoding"] = "gzip";
    response.writeHead(200, headers);
    if (observation) {
      const bodyBytes = request.method === "HEAD" ? 0 : body.byteLength;
      const observedRequest = {
        bodyBytes,
        cacheControl: headers["Cache-Control"],
        contentEncoding: headers["Content-Encoding"] ?? "identity",
        finishedWallTimeMs: null,
        method: request.method ?? "GET",
        pathname,
      };
      observation.bodyBytes += bodyBytes;
      observation.requests.push(observedRequest);
      response.once("finish", () => {
        observedRequest.finishedWallTimeMs = Date.now();
      });
    }
    if (request.method !== "HEAD") response.end(body);
    else response.end();
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Measurement server did not expose a TCP address.");
  }

  return {
    beginObservation() {
      if (observation) throw new Error("Server measurement is already active.");
      observation = { bodyBytes: 0, requests: [] };
    },
    close: () => new Promise((resolve) => server.close(resolve)),
    endObservation() {
      if (!observation) throw new Error("Server measurement is not active.");
      const result = observation;
      observation = null;
      return result;
    },
    url: `http://127.0.0.1:${address.port}/`,
  };
}

export function artifactReadyAtMs(requests, artifactFile, timeOrigin, mode) {
  const normalized = `/${artifactFile.replaceAll("\\", "/")}`;
  const completions = requests
    .filter((request) => request.pathname === normalized)
    .map((request) => request.finishedWallTimeMs)
    .filter(Number.isFinite);
  if (completions.length > 0) {
    return Math.max(0, Math.max(...completions) - timeOrigin);
  }
  if (mode === "warm") return 0;
  throw new Error(
    `Cold run did not observe artifact completion for ${artifactFile}.`,
  );
}
