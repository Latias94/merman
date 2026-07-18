import type { Mermaid, MermaidConfig } from "mermaid";
import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

import {
  buildMermaidConfig,
  sourceWithMermaidConfig,
} from "../../../lib/mermaid-config.ts";
import {
  MERMAID_JS_VERSION,
  type MermaidExternalRequirements,
} from "../../mermaid-requirements.ts";
import {
  assertRealmSourceBudget,
  assertRealmSvgBudget,
  type CompareOperationStage,
  type CompareRenderPayload,
} from "../channel-protocol.ts";

export interface MermaidEngineResult {
  readonly prepareTimeMs: number;
  readonly presentationTimeMs: number;
  readonly renderTimeMs: number;
  readonly svg: string;
  readonly version: string;
}

export class MermaidEngineError extends Error {
  readonly stage: CompareOperationStage;

  constructor(stage: CompareOperationStage, cause: unknown) {
    super(errorMessage(cause));
    this.name = "MermaidEngineError";
    this.stage = stage;
  }
}

const EXTERNAL_DIAGRAM_LOAD_ERROR = /^Failed to load \d+ external diagrams$/;

let mermaidPromise: Promise<Mermaid> | null = null;
let zenumlPromise: Promise<Awaited<ReturnType<typeof importZenUml>>> | null = null;
let elkLayoutsPromise: Promise<Awaited<ReturnType<typeof importElkLayouts>>> | null =
  null;
let registeredZenUml: Mermaid | null = null;
let registeredElkLayouts: Mermaid | null = null;
let renderSequence = 0;

export async function renderWithMermaid(
  input: CompareRenderPayload,
  presentationHost: HTMLElement,
  onStage: (stage: CompareOperationStage) => void
): Promise<MermaidEngineResult> {
  const operationStartedAt = performance.now();
  const mermaid = await runStage("load", onStage, loadMermaid);
  await runStage("register", onStage, () =>
    registerExternalRequirements(mermaid, input.externalRequirements)
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
    if (
      !input.externalRequirements.zenuml ||
      !(error instanceof Error) ||
      !EXTERNAL_DIAGRAM_LOAD_ERROR.test(error.message)
    ) {
      throw new MermaidEngineError("render", error);
    }
    await runStage("zenuml-recovery", onStage, async () => {
      registeredZenUml = null;
      await ensureZenUmlRegistered(mermaid);
    });
    try {
      result = await mermaid.render(nextRenderId(), configuredSource);
    } catch (retryError) {
      throw new MermaidEngineError("zenuml-recovery", retryError);
    }
  }

  const svg = result.svg;
  await runStage("svg-validation", onStage, async () => {
    assertRealmSvgBudget(svg);
    assertSafeSvgForDom(svg);
  });
  const safeSvgReadyAt = performance.now();
  const presentationReadyAt = await runStage(
    "presentation",
    onStage,
    () => presentSafeSvg(presentationHost, svg)
  );

  return {
    svg,
    prepareTimeMs: renderStartedAt - operationStartedAt,
    renderTimeMs: safeSvgReadyAt - renderStartedAt,
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

async function registerExternalRequirements(
  mermaid: Mermaid,
  requirements: MermaidExternalRequirements
): Promise<void> {
  if (requirements.elkLayouts && registeredElkLayouts !== mermaid) {
    elkLayoutsPromise ??= importElkLayouts().catch((error) => {
      elkLayoutsPromise = null;
      throw error;
    });
    mermaid.registerLayoutLoaders(await elkLayoutsPromise);
    registeredElkLayouts = mermaid;
  }
  if (requirements.zenuml) {
    await ensureZenUmlRegistered(mermaid);
  }
}

async function ensureZenUmlRegistered(mermaid: Mermaid): Promise<void> {
  if (registeredZenUml === mermaid) return;
  zenumlPromise ??= importZenUml().catch((error) => {
    zenumlPromise = null;
    throw error;
  });
  await mermaid.registerExternalDiagrams([await zenumlPromise], {
    lazyLoad: true,
  });
  registeredZenUml = mermaid;
}

async function importZenUml() {
  return (await import("@mermaid-js/mermaid-zenuml")).default;
}

async function importElkLayouts() {
  return (await import("@mermaid-js/layout-elk")).default;
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

async function presentSafeSvg(host: HTMLElement, svg: string): Promise<number> {
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

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
