const UTF8_CHUNK_CODE_UNITS = 16_000;
const UTF8_ENCODER = new TextEncoder();
const UTF8_SCRATCH = new Uint8Array(64 * 1024);

export function utf8ByteLength(value: string): number {
  return countUtf8Bytes(value, Number.POSITIVE_INFINITY);
}

export function exceedsUtf8ByteBudget(value: string, limit: number): boolean {
  if (value.length > limit) return true;
  return countUtf8Bytes(value, limit) > limit;
}

function countUtf8Bytes(value: string, stopAfter: number): number {
  let bytes = 0;
  let offset = 0;
  while (offset < value.length) {
    let end = Math.min(value.length, offset + UTF8_CHUNK_CODE_UNITS);
    if (
      end < value.length &&
      isHighSurrogate(value.charCodeAt(end - 1)) &&
      isLowSurrogate(value.charCodeAt(end))
    ) {
      end += 1;
    }
    const chunk = value.slice(offset, end);
    const encoded = UTF8_ENCODER.encodeInto(chunk, UTF8_SCRATCH);
    if (encoded.read !== chunk.length) {
      throw new Error("UTF-8 byte counter exhausted its scratch buffer.");
    }
    bytes += encoded.written;
    if (bytes > stopAfter) return bytes;
    offset = end;
  }
  return bytes;
}

function isHighSurrogate(value: number): boolean {
  return value >= 0xd800 && value <= 0xdbff;
}

function isLowSurrogate(value: number): boolean {
  return value >= 0xdc00 && value <= 0xdfff;
}
