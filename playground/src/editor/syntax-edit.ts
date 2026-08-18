import type { Edit, Point } from "web-tree-sitter";

export function singleEdit(
  previous: string,
  next: string,
): ConstructorParameters<typeof Edit>[0] {
  let startIndex = commonPrefix(previous, next);
  let suffix = commonSuffix(previous, next, startIndex);
  startIndex = safeCodePointBoundary(previous, next, startIndex);
  suffix = safeSuffix(previous, next, startIndex, suffix);
  const oldEndIndex = previous.length - suffix;
  const newEndIndex = next.length - suffix;
  const startPosition = pointAt(previous, startIndex);
  return {
    startIndex,
    oldEndIndex,
    newEndIndex,
    startPosition,
    oldEndPosition: advancePoint(previous, startIndex, oldEndIndex, startPosition),
    newEndPosition: advancePoint(next, startIndex, newEndIndex, startPosition),
  };
}

function commonPrefix(left: string, right: string): number {
  const limit = Math.min(left.length, right.length);
  let index = 0;
  while (index < limit && left.charCodeAt(index) === right.charCodeAt(index)) {
    index += 1;
  }
  return index;
}

function commonSuffix(left: string, right: string, prefix: number): number {
  const limit = Math.min(left.length, right.length) - prefix;
  let count = 0;
  while (
    count < limit &&
    left.charCodeAt(left.length - count - 1) ===
      right.charCodeAt(right.length - count - 1)
  ) {
    count += 1;
  }
  return count;
}

function safeCodePointBoundary(
  left: string,
  right: string,
  index: number,
): number {
  if (splitsSurrogate(left, index) || splitsSurrogate(right, index)) {
    return index - 1;
  }
  return index;
}

function safeSuffix(
  left: string,
  right: string,
  prefix: number,
  suffix: number,
): number {
  const leftEnd = left.length - suffix;
  const rightEnd = right.length - suffix;
  if (
    suffix > 0 &&
    (splitsSurrogate(left, leftEnd) || splitsSurrogate(right, rightEnd)) &&
    leftEnd >= prefix &&
    rightEnd >= prefix
  ) {
    return suffix - 1;
  }
  return suffix;
}

function splitsSurrogate(source: string, index: number): boolean {
  return (
    index > 0 &&
    index < source.length &&
    isHighSurrogate(source.charCodeAt(index - 1)) &&
    isLowSurrogate(source.charCodeAt(index))
  );
}

function pointAt(source: string, offset: number): Point {
  let row = 0;
  let lineStart = 0;
  for (let index = 0; index < offset; index += 1) {
    if (source.charCodeAt(index) === 10) {
      row += 1;
      lineStart = index + 1;
    }
  }
  return { row, column: offset - lineStart };
}

function advancePoint(
  source: string,
  start: number,
  end: number,
  initial: Point,
): Point {
  let row = initial.row;
  let column = initial.column;
  for (let index = start; index < end; index += 1) {
    if (source.charCodeAt(index) === 10) {
      row += 1;
      column = 0;
    } else {
      column += 1;
    }
  }
  return { row, column };
}

function isHighSurrogate(value: number): boolean {
  return value >= 0xd800 && value <= 0xdbff;
}

function isLowSurrogate(value: number): boolean {
  return value >= 0xdc00 && value <= 0xdfff;
}
