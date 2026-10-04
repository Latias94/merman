/** UTF-16 source boundaries; locating frontmatter does not parse its YAML. */
export interface MermaidFrontmatter {
  indent: string;
  bodyStart: number;
  bodyEnd: number;
  end: number;
}

const whitespace = /\s/;
const isLineBreak = (char: string | undefined): boolean => char === "\n" || char === "\r";

/**
 * Match Mermaid 12.1.0's greedy opening and lazy closing fences in linear time.
 * Source: packages/mermaid/src/diagram-api/regexes.ts at
 * 21f72f07ea22c0af48a3149c550654e80d8e40cb. The public API does not expose this locator.
 */
export function locateMermaidFrontmatter(source: string): MermaidFrontmatter | undefined {
  let cursor = 0;
  while (cursor < source.length && whitespace.test(source[cursor]) && !isLineBreak(source[cursor])) {
    cursor++;
  }
  const indent = source.slice(0, cursor);
  if (!source.startsWith("---", cursor)) return undefined;

  const afterOpeningFence = cursor + 3;
  let firstOpeningEnd = -1;
  let openingWhitespaceEnd = afterOpeningFence;
  while (openingWhitespaceEnd < source.length && whitespace.test(source[openingWhitespaceEnd])) {
    if (firstOpeningEnd === -1 && isLineBreak(source[openingWhitespaceEnd])) {
      firstOpeningEnd = openingWhitespaceEnd;
    }
    openingWhitespaceEnd++;
  }
  if (firstOpeningEnd === -1) return undefined;

  const closingFence = `${indent}---`;
  const closingEnd = (position: number): number => {
    if (!isLineBreak(source[position]) || !source.startsWith(closingFence, position + 1)) return -1;
    let end = -1;
    for (let i = position + 1 + closingFence.length; i < source.length && whitespace.test(source[i]); i++) {
      if (isLineBreak(source[i])) end = i;
    }
    return end;
  };

  // Fence suffixes are disjoint whitespace runs, so these passes cannot backtrack quadratically.
  let lastClosingFence = -1;
  for (let i = firstOpeningEnd + 1; i < source.length; i++) {
    if (closingEnd(i) !== -1) lastClosingFence = i;
  }
  if (lastClosingFence === -1) return undefined;

  // Choose the longest opening that still leaves a closing fence, without retaining an index array.
  let openingEnd = firstOpeningEnd;
  for (let i = firstOpeningEnd + 1; i < Math.min(openingWhitespaceEnd, lastClosingFence); i++) {
    if (isLineBreak(source[i])) openingEnd = i;
  }
  const bodyStart = openingEnd + 1;
  for (let bodyEnd = bodyStart; bodyEnd <= lastClosingFence; bodyEnd++) {
    const end = closingEnd(bodyEnd);
    if (end !== -1) return { indent, bodyStart, bodyEnd, end: end + 1 };
  }
  return undefined;
}
