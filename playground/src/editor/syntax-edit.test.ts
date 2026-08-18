import assert from "node:assert/strict";
import test from "node:test";
import { singleEdit } from "./syntax-edit.ts";

test("single edit keeps UTF-16 surrogate pairs and CRLF points intact", () => {
  const edit = singleEdit(
    "flowchart TD\r\n😀A --> B",
    "flowchart TD\r\n😀node --> B",
  );

  assert.deepEqual(edit, {
    startIndex: 16,
    oldEndIndex: 17,
    newEndIndex: 20,
    startPosition: { row: 1, column: 2 },
    oldEndPosition: { row: 1, column: 3 },
    newEndPosition: { row: 1, column: 6 },
  });
});

test("single edit expands a boundary that would split an astral character", () => {
  const edit = singleEdit("A😀B", "A😁B");
  assert.equal(edit.startIndex, 1);
  assert.equal(edit.oldEndIndex, 3);
  assert.equal(edit.newEndIndex, 3);
});
