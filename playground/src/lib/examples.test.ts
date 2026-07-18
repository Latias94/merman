import assert from "node:assert/strict";
import test from "node:test";

import {
  categories,
  examples,
  filterExamples,
  type ExampleFilter,
} from "./examples.ts";

test("generated catalog covers every full-profile diagram exactly once", () => {
  assert.equal(examples.length, 35);
  assert.equal(new Set(examples.map((example) => example.diagramType)).size, 35);
  assert.ok(examples.every((example) => example.fixture.startsWith("fixtures/")));
});

test("empty projection preserves generated manifest order", () => {
  assert.deepEqual(
    filterExamples().map((example) => example.id),
    examples.map((example) => example.id)
  );
});

test("search matches every indexed field case-insensitively", () => {
  const cases = [
    ["tea shop wardley map", "wardley"],
    ["grammar", "railroad"],
    ["RAILROADABNF", "railroadAbnf"],
    ["fishbone", "ishikawa"],
    ["hello", "sequence"],
  ] as const;

  for (const [query, expectedDiagramType] of cases) {
    assert.ok(
      filterExamples({ query }).some(
        (example) => example.diagramType === expectedDiagramType
      ),
      `${query} should find ${expectedDiagramType}`
    );
  }
});

test("category, search, and ASCII filters compose without reordering", () => {
  const filters: ExampleFilter = {
    category: "Flow",
    query: "diagram",
    asciiDiagramTypes: new Set(["flowchart", "sequence", "state"]),
    asciiOnly: true,
  };
  const result = filterExamples(filters);

  assert.ok(result.length > 0);
  assert.ok(result.every((example) => example.category === "Flow"));
  assert.ok(
    result.every((example) => filters.asciiDiagramTypes?.has(example.diagramType))
  );
  const generatedOffsets = result.map((example) => examples.indexOf(example));
  assert.ok(
    generatedOffsets.every(
      (offset, index) => index === 0 || generatedOffsets[index - 1]! < offset
    )
  );
});

test("categories are stable, unique, and include the all projection", () => {
  assert.equal(categories[0], "All");
  assert.equal(new Set(categories).size, categories.length);
  assert.deepEqual(
    categories.slice(1),
    Array.from(new Set(examples.map((example) => example.category)))
  );
});

test("an unmatched query produces an explicit empty projection", () => {
  assert.deepEqual(filterExamples({ query: "no-such-example-value" }), []);
});
