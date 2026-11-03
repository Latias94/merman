import type { AnalysisByteSpan, SourceAppearanceDeclaration, SourceAppearanceSummary } from "@mermanjs/web";

export interface MermaidAppearanceOverride {
  readonly origin: "config" | "source";
  readonly value: string;
}

export function readMermaidAppearanceOverrides(
  facts: SourceAppearanceSummary | undefined,
  injectedConfigSpan: Readonly<AnalysisByteSpan> | null | undefined,
): { theme?: MermaidAppearanceOverride; font?: MermaidAppearanceOverride; look?: MermaidAppearanceOverride } | null {
  if (facts?.state !== "known") return null;
  const project = (declaration: SourceAppearanceDeclaration | null): MermaidAppearanceOverride | undefined => {
    if (!declaration) return undefined;
    const span = declaration.construct_span;
    return {
      value: declaration.value,
      origin: declaration.origin.kind === "directive" && injectedConfigSpan != null && injectedConfigSpan.start === span.start &&
        injectedConfigSpan.end === span.end ? "config" : "source",
    };
  };
  return { theme: project(facts.theme), font: project(facts.font), look: project(facts.look) };
}
