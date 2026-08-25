export const MERMAID_SYNTAX_TOKEN_TYPES = Object.freeze([
  "comment",
  "string",
  "number",
  "keyword",
  "operator",
  "namespace",
  "function",
  "variable",
  "property",
  "type",
  "macro",
  "decorator",
  "enumMember",
] as const);

export const MERMAID_SYNTAX_TOKEN_LEGEND = Object.freeze({
  tokenTypes: MERMAID_SYNTAX_TOKEN_TYPES,
  tokenModifiers: Object.freeze([] as string[]),
});

export interface MermaidSyntaxCapture {
  readonly capture: string;
  readonly endIndex: number;
  readonly patternIndex: number;
  readonly startIndex: number;
}

interface LineCapture extends MermaidSyntaxCapture {
  readonly end: number;
  readonly line: number;
  readonly start: number;
  readonly tokenType: number;
}

const CAPTURE_TOKEN_TYPE: Readonly<Record<string, string>> = Object.freeze({
  attribute: "decorator",
  boolean: "enumMember",
  comment: "comment",
  constant: "enumMember",
  function: "function",
  keyword: "keyword",
  namespace: "namespace",
  number: "number",
  operator: "operator",
  property: "property",
  punctuation: "operator",
  string: "string",
  type: "type",
  variable: "variable",
  "function.macro": "macro",
  "keyword.operator": "operator",
  "variable.member": "property",
});

export function projectMermaidSyntaxTokens(
  source: string,
  captures: readonly MermaidSyntaxCapture[],
): Uint32Array {
  const lines = lineStarts(source);
  const segments = captures.flatMap((capture) =>
    splitCapture(source, lines, capture),
  );
  const resolved = resolveOverlaps(segments);
  const packed = new Uint32Array(resolved.length * 5);
  let packedIndex = 0;
  let previousLine = 0;
  let previousStart = 0;

  for (const segment of resolved) {
    const deltaLine = segment.line - previousLine;
    const deltaStart = deltaLine === 0 ? segment.start - previousStart : segment.start;
    packed[packedIndex++] = deltaLine;
    packed[packedIndex++] = deltaStart;
    packed[packedIndex++] = segment.end - segment.start;
    packed[packedIndex++] = segment.tokenType;
    packed[packedIndex++] = 0;
    previousLine = segment.line;
    previousStart = segment.start;
  }

  return packed;
}

function splitCapture(
  source: string,
  lines: readonly number[],
  capture: MermaidSyntaxCapture,
): LineCapture[] {
  const tokenType = tokenTypeIndex(capture.capture);
  if (tokenType === null) return [];
  const start = boundedOffset(capture.startIndex, source.length);
  const end = boundedOffset(capture.endIndex, source.length);
  if (start >= end) return [];

  const segments: LineCapture[] = [];
  let current = start;
  while (current < end) {
    const line = lineForOffset(lines, current);
    const lineStart = lines[line];
    const nextLineStart = lines[line + 1];
    let contentEnd = nextLineStart === undefined ? source.length : nextLineStart - 1;
    if (contentEnd > lineStart && source.charCodeAt(contentEnd - 1) === 13) {
      contentEnd -= 1;
    }
    const segmentEnd = Math.min(end, contentEnd);
    if (current < segmentEnd) {
      segments.push({
        ...capture,
        end: segmentEnd - lineStart,
        line,
        start: current - lineStart,
        tokenType,
      });
    }
    if (nextLineStart === undefined || nextLineStart <= current) break;
    current = nextLineStart;
  }
  return segments;
}

function resolveOverlaps(segments: readonly LineCapture[]): LineCapture[] {
  const resolved: LineCapture[] = [];
  const lines = new Map<number, LineCapture[]>();
  for (const segment of segments) {
    const line = lines.get(segment.line) ?? [];
    line.push(segment);
    lines.set(segment.line, line);
  }
  for (const [line, candidates] of [...lines].sort(([left], [right]) => left - right)) {
    const boundaries = [...new Set(candidates.flatMap(({ start, end }) => [start, end]))].sort(
      (left, right) => left - right,
    );
    for (let index = 0; index + 1 < boundaries.length; index += 1) {
      const start = boundaries[index];
      const end = boundaries[index + 1];
      if (start === undefined || end === undefined || start >= end) continue;
      const winner = candidates
        .filter((candidate) => candidate.start <= start && candidate.end >= end)
        .sort(compareCapturePriority)
        .at(-1);
      if (!winner) continue;
      const previous = resolved.at(-1);
      if (
        previous &&
        previous.line === line &&
        previous.end === start &&
        previous.tokenType === winner.tokenType
      ) {
        resolved[resolved.length - 1] = { ...previous, end };
      } else {
        resolved.push({ ...winner, line, start, end });
      }
    }
  }
  return resolved;
}

function compareCapturePriority(left: LineCapture, right: LineCapture): number {
  return (
    left.capture.split(".").length - right.capture.split(".").length ||
    left.patternIndex - right.patternIndex ||
    right.end - right.start - (left.end - left.start) ||
    left.capture.localeCompare(right.capture)
  );
}

function tokenTypeIndex(capture: string): number | null {
  let candidate = capture;
  while (candidate.length > 0) {
    const tokenType = CAPTURE_TOKEN_TYPE[candidate];
    if (tokenType) {
      const index = MERMAID_SYNTAX_TOKEN_TYPES.indexOf(
        tokenType as (typeof MERMAID_SYNTAX_TOKEN_TYPES)[number],
      );
      return index < 0 ? null : index;
    }
    const separator = candidate.lastIndexOf(".");
    if (separator < 0) return null;
    candidate = candidate.slice(0, separator);
  }
  return null;
}

function lineStarts(source: string): number[] {
  const starts = [0];
  for (let index = 0; index < source.length; index += 1) {
    if (source.charCodeAt(index) === 10) starts.push(index + 1);
  }
  return starts;
}

function lineForOffset(lines: readonly number[], offset: number): number {
  let low = 0;
  let high = lines.length;
  while (low + 1 < high) {
    const middle = Math.floor((low + high) / 2);
    if ((lines[middle] ?? 0) <= offset) low = middle;
    else high = middle;
  }
  return low;
}

function boundedOffset(value: number, length: number): number {
  if (!Number.isSafeInteger(value)) return 0;
  return Math.max(0, Math.min(value, length));
}
