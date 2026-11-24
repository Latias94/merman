import type {
  HostTextMeasurer,
  SvgRenderResult,
  ThemeResolution,
  ThemeResolveOptions,
} from "@mermanjs/web";

export * from "@mermanjs/web";

export const browserRenderFixture = {
  calls: [] as {
    mode: "browser" | "headless" | "legacy";
    source: string;
    optionsJson: string | null | undefined;
    measurer?: HostTextMeasurer;
  }[],
  themeCalls: [] as { document: string; options?: ThemeResolveOptions }[],
  failure: null as unknown,
  result: {
    svg: '<svg xmlns="http://www.w3.org/2000/svg" />',
    metadata: {
      version: 1,
      operation_id: "svg",
      media_type: "image/svg+xml",
      runtime_policy: "default",
      byte_length: 0,
      theme_outcome: { mode: "mermaid_parity", notices: [] },
    },
  } as SvgRenderResult,
};

export function packageVersion(): string {
  return "test-runtime";
}

export function resolveTheme(document: string, options?: ThemeResolveOptions): ThemeResolution {
  browserRenderFixture.themeCalls.push({ document, options });
  return { ok: false, line: 3, column: 8, path: "seed.accent", message: "A color is required." };
}

export function renderSvg(source: string, optionsJson?: string | null): string {
  browserRenderFixture.calls.push({ mode: "legacy", source, optionsJson });
  if (browserRenderFixture.failure) throw browserRenderFixture.failure;
  return browserRenderFixture.result.svg;
}

export function renderSvgWithTextMeasurer(source: string, measurer: HostTextMeasurer, optionsJson?: string | null): string {
  browserRenderFixture.calls.push({ mode: "legacy", source, optionsJson, measurer });
  if (browserRenderFixture.failure) throw browserRenderFixture.failure;
  return browserRenderFixture.result.svg;
}

export function renderSvgResult(source: string, optionsJson?: string | null): SvgRenderResult {
  browserRenderFixture.calls.push({ mode: "headless", source, optionsJson });
  if (browserRenderFixture.failure) throw browserRenderFixture.failure;
  return browserRenderFixture.result;
}

export function renderSvgResultWithTextMeasurer(source: string, measurer: HostTextMeasurer, optionsJson?: string | null): SvgRenderResult {
  browserRenderFixture.calls.push({ mode: "browser", source, optionsJson, measurer });
  if (browserRenderFixture.failure) throw browserRenderFixture.failure;
  return browserRenderFixture.result;
}
