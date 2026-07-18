import {
  GENERATED_EXAMPLES,
  type GeneratedExample,
} from "../generated/examples.ts";

export type Example = GeneratedExample;

export interface ExampleFilter {
  category?: string;
  query?: string;
  asciiOnly?: boolean;
  asciiDiagramTypes?: ReadonlySet<string>;
}

export const examples: readonly Example[] = GENERATED_EXAMPLES;

export const categories: readonly string[] = [
  "All",
  ...new Set(examples.map((example) => example.category)),
];

export function filterExamples(
  filter: ExampleFilter = {},
  catalog: readonly Example[] = examples
): Example[] {
  const category = filter.category ?? "All";
  const query = filter.query?.trim().toLowerCase() ?? "";

  return catalog.filter((example) => {
    if (category !== "All" && example.category !== category) {
      return false;
    }
    if (
      filter.asciiOnly &&
      !filter.asciiDiagramTypes?.has(example.diagramType)
    ) {
      return false;
    }
    if (query.length === 0) {
      return true;
    }
    return [
      example.title,
      example.category,
      example.diagramType,
      ...example.aliases,
      example.source,
    ].some((value) => value.toLowerCase().includes(query));
  });
}

export function getExampleById(id: string): Example | undefined {
  return examples.find((example) => example.id === id);
}
