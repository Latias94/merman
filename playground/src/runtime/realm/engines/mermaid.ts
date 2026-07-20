import type { Mermaid, MermaidConfig } from "mermaid";

import {
  buildMermaidConfig,
  sourceWithMermaidConfig,
} from "../../../lib/mermaid-config.ts";
import {
  MERMAID_JS_VERSION,
} from "../../mermaid-requirements.ts";
import { mermaidExternalModuleRegistrar } from "../../external-module-registrar.ts";
import {
  assertRealmSourceBudget,
  assertRealmSvgBudget,
  type CompareOperationStage,
  type CompareRenderPayload,
} from "../channel-protocol.ts";
import {
  projectError,
  type ErrorProjection,
} from "../../error-projection.ts";

export interface MermaidEngineResult {
  readonly prepareTimeMs: number;
  readonly presentationTimeMs: number;
  readonly renderTimeMs: number;
  readonly svg: string;
  readonly version: string;
}

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
  const config = await runStage("initialize", onStage, async () => {
    const config = buildMermaidConfig(input.configJson, input.theme, {
      diagramFont: input.diagramFont,
    });
    mermaid.initialize({
      ...config,
      startOnLoad: false,
      securityLevel: config.securityLevel ?? "loose",
    } as MermaidConfig);
    return config;
  });

  const configuredSource = sourceWithMermaidConfig(input.source, config);
  assertRealmSourceBudget(configuredSource);
  const renderStartedAt = performance.now();
  onStage("render");
  let result;
  try {
    result = await mermaid.render(nextRenderId(), configuredSource);
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
