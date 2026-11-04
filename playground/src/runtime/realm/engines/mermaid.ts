import type { Mermaid, MermaidConfig } from "mermaid";

import {
  buildMermaidOperationInput,
} from "../../../lib/mermaid-config.ts";
import {
  MERMAID_JS_VERSION,
} from "../../mermaid-requirements.ts";
import { mermaidExternalModuleRegistrar } from "../../external-module-registrar.ts";
import {
  assertRealmSourceBudget,
  assertRealmSvgBudget,
  type CompareOperationStage,
  type CompareRenderOutput,
  type CompareRenderPayload,
} from "../channel-protocol.ts";
import {
  projectError,
  type ErrorProjection,
} from "../../error-projection.ts";

export type MermaidEngineResult = CompareRenderOutput;

export class MermaidEngineError extends Error {
  readonly error: ErrorProjection;
  readonly stage: CompareOperationStage;

  constructor(stage: CompareOperationStage, cause: unknown) {
    const projection = projectError(cause);
    super(projection.summary);
    this.name = "MermaidEngineError";
    this.error = projection;
    this.stage = stage;
  }
}

let mermaidPromise: Promise<Mermaid> | null = null;
let renderSequence = 0;

export async function renderWithMermaid(
  input: CompareRenderPayload,
  presentationHost: HTMLElement,
  onStage: (stage: CompareOperationStage) => void
): Promise<MermaidEngineResult> {
  const operationStartedAt = performance.now();
  const mermaid = await runStage("load", onStage, loadMermaid);
  await runStage("register", onStage, () =>
    mermaidExternalModuleRegistrar.register(
      mermaid,
      input.externalRequirements
    )
  );
  const { configuredSource } = await runStage("initialize", onStage, async () => {
    const configured = buildMermaidOperationInput(
      input.source, input.theme, input.configJson, { diagramFont: input.diagramFont },
    );
    mermaid.initialize({
      ...configured.initializationConfig,
      startOnLoad: false,
      securityLevel: configured.initializationConfig.securityLevel ?? "loose",
    } as MermaidConfig);
    return configured;
  });
  assertRealmSourceBudget(configuredSource);
  const renderStartedAt = performance.now();
  onStage("render");
  // initialize() retains old source directives until render resets them.
  const initializedTheme = mermaid.mermaidAPI.getSiteConfig().theme ?? null;
  let materializedTheme: string | null;
  let result;
  try {
    // The realm queue owns serialization. This API resolves config synchronously
    // before its first await and clears that scope before its promise settles.
    const rendered = mermaid.mermaidAPI.render(nextRenderId(), configuredSource);
    const theme = mermaid.mermaidAPI.getConfig().theme ?? null;
    // The "null" sentinel disables theme rules but retains the initialized palette.
    materializedTheme = theme === "null" ? initializedTheme : theme;
    result = await rendered;
  } catch (error) {
    throw new MermaidEngineError("render", error);
  }

  const svg = result.svg;
  await runStage("svg-budget", onStage, async () => {
    assertRealmSvgBudget(svg);
  });
  const budgetedSvgReadyAt = performance.now();
  const presentationReadyAt = await runStage(
    "presentation",
    onStage,
    () => presentIsolatedSvg(presentationHost, svg)
  );

  return {
    svg,
    materializedTheme,
    prepareTimeMs: renderStartedAt - operationStartedAt,
    renderTimeMs: budgetedSvgReadyAt - renderStartedAt,
    presentationTimeMs: presentationReadyAt - operationStartedAt,
    version: MERMAID_JS_VERSION,
  };
}

async function loadMermaid(): Promise<Mermaid> {
  mermaidPromise ??= import("mermaid")
    .then((module) => module.default)
    .catch((error) => {
      mermaidPromise = null;
      throw error;
    });
  return mermaidPromise;
}

async function runStage<T>(
  stage: CompareOperationStage,
  onStage: (stage: CompareOperationStage) => void,
  run: () => T | Promise<T>
): Promise<T> {
  onStage(stage);
  try {
    return await run();
  } catch (error) {
    if (error instanceof MermaidEngineError) throw error;
    throw new MermaidEngineError(stage, error);
  }
}

function nextRenderId(): string {
  renderSequence += 1;
  return `merman-realm-mermaid-${renderSequence}`;
}

async function presentIsolatedSvg(
  host: HTMLElement,
  svg: string
): Promise<number> {
  try {
    host.innerHTML = svg;
    const element = host.querySelector("svg");
    if (!(element instanceof SVGSVGElement)) {
      throw new Error("Mermaid did not return an SVG root element.");
    }
    const rect = element.getBoundingClientRect();
    if (
      !Number.isFinite(rect.width) ||
      !Number.isFinite(rect.height) ||
      rect.width <= 0 ||
      rect.height <= 0
    ) {
      throw new Error("Mermaid SVG has no finite non-empty layout box.");
    }
    return await new Promise((resolve) => {
      requestAnimationFrame(() => resolve(performance.now()));
    });
  } finally {
    host.replaceChildren();
  }
}
