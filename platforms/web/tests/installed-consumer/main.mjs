import { exercise, require } from "./exercise.mjs";

const cspViolations = [];
addEventListener("securitypolicyviolation", event => {
  cspViolations.push(`${event.effectiveDirective}: ${event.blockedURI}`);
});

function inspectSvg(svg, math) {
  require(!svg.includes("$$"), "SVG retained literal formula delimiters");
  const parsed = new DOMParser().parseFromString(svg, "image/svg+xml");
  require(!parsed.querySelector("parsererror") && parsed.documentElement.localName === "svg",
    "Renderer returned invalid SVG");
  const root = document.importNode(parsed.documentElement, true);
  document.querySelector("#output").append(root);
  const box = root.viewBox.baseVal;
  require([box.x, box.y, box.width, box.height].every(Number.isFinite)
    && box.width > 0 && box.height > 0, "SVG has no finite positive viewport");
  if (math) {
    const formulas = [...root.querySelectorAll("svg")];
    require(formulas.length > 0, "Math SVG has no inline formula viewport");
    for (const formula of formulas) {
      const viewport = formula.viewBox.baseVal;
      require([viewport.x, viewport.y, viewport.width, viewport.height].every(Number.isFinite)
        && viewport.width > 0 && viewport.height > 0, "Formula viewport is empty or invalid");
      require([...formula.querySelectorAll("path[d]")].some(path =>
        path.getAttribute("d").trim() && Number.isFinite(path.getTotalLength()) && path.getTotalLength() > 0),
      "Formula has no meaningful outlined glyph path");
    }
    return { formulaViewports: formulas.length, svgLength: svg.length };
  }
  require(root.textContent.includes("Start") && root.textContent.includes("Done")
    && root.querySelector("path[d]"), "ELK SVG lost nodes or edges");
  return { svgLength: svg.length };
}

async function run() {
  const main = await exercise("main");
  const worker = new Worker(new URL("./worker.mjs", import.meta.url), { type: "module" });
  let message;
  try {
    message = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("Consumer Worker timed out")), 60_000);
      worker.onmessage = event => {
        clearTimeout(timeout);
        if (event.data.error) reject(new Error(event.data.error));
        else resolve(event.data);
      };
      worker.onerror = event => {
        clearTimeout(timeout);
        reject(new Error(event.message));
      };
    });
  } finally {
    worker.terminate();
  }
  for (const result of [main, message.result]) {
    if (__MERMAN_PACKAGE_ID__ === "full" || __MERMAN_PACKAGE_ID__ === "render") {
      require(typeof result.mathSvg === "string" && typeof result.elkSvg === "string",
        "Installed rendering package did not return both SVG documents");
      result.math = inspectSvg(result.mathSvg, true);
      result.elk = inspectSvg(result.elkSvg, false);
      delete result.mathSvg;
      delete result.elkSvg;
    }
  }
  return { main, worker: message.result, cspViolations: [...cspViolations, ...message.cspViolations] };
}

globalThis.__installedConsumer = run();
