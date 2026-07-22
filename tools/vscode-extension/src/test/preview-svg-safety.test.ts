import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { assertSafePreviewSvg } from "../preview-svg-safety.js";

const PNG_1X1 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
const GIF_1X1 = "R0lGODdhAQABAJEAAAAAAPDv9////wAAACH5BAQAAAAALAAAAAABAAEAAAICTAEAOw==";
const WEBP_1X1 = "UklGRiYAAABXRUJQVlA4IBoAAAAwAQCdASoBAAEAAgA0JZwAA3AA/vo8xw8gAA==";
const JPEG_1X1 =
  "/9j/4AAQSkZJRgABAQAASABIAAD/4QBMRXhpZgAATU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAAaADAAQAAAABAAAAAQAAAAD/7QA4UGhvdG9zaG9wIDMuMAA4QklNBAQAAAAAAAA4QklNBCUAAAAAABDUHYzZjwCyBOmACZjs+EJ+/8AAEQgAAQABAwEiAAIRAQMRAf/EAB8AAAEFAQEBAQEBAAAAAAAAAAABAgMEBQYHCAkKC//EALUQAAIBAwMCBAMFBQQEAAABfQECAwAEEQUSITFBBhNRYQcicRQygZGhCCNCscEVUtHwJDNicoIJChYXGBkaJSYnKCkqNDU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6g4SFhoeIiYqSk5SVlpeYmZqio6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2drh4uPk5ebn6Onq8fLz9PX29/j5+v/EAB8BAAMBAQEBAQEBAQEAAAAAAAABAgMEBQYHCAkKC//EALURAAIBAgQEAwQHBQQEAAECdwABAgMRBAUhMQYSQVEHYXETIjKBCBRCkaGxwQkjM1LwFWJy0QoWJDThJfEXGBkaJicoKSo1Njc4OTpDREVGR0hJSlNUVVZXWFlaY2RlZmdoaWpzdHV2d3h5eoKDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uLj5OXm5+jp6vLz9PX29/j5+v/bAEMAAgICAgICAwICAwUDAwMFBgUFBQUGCAYGBgYGCAoICAgICAgKCgoKCgoKCgwMDAwMDA4ODg4ODw8PDw8PDw8PD//bAEMBAgICBAQEBwQEBxALCQsQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEP/dAAQAAf/aAAwDAQACEQMRAD8A/cCiiivQA//Z";
const WEBP_LOSSLESS_1X1 = "UklGRhwAAABXRUJQVlA4TA8AAAAvAAAAAAcQ/Y/+ByKi/wEA";
const JPEG_PROGRESSIVE_2X2 =
  "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wgARCAACAAIDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAb/xAAUAQEAAAAAAAAAAAAAAAAAAAAF/9oADAMBAAIQAxAAAAGsBxX/xAAWEAEBAQAAAAAAAAAAAAAAAAADBAH/2gAIAQEAAQUCjI9i/8QAFxEAAwEAAAAAAAAAAAAAAAAAAAEDMv/aAAgBAwEBPwGm2f/EABcRAAMBAAAAAAAAAAAAAAAAAAABAjL/2gAIAQIBAT8BvTP/xAAbEAACAQUAAAAAAAAAAAAAAAABAgADERNBYf/aAAgBAQAGPwKgSi3xrrk//8QAFhABAQEAAAAAAAAAAAAAAAAAASEA/9oACAEBAAE/IUc0ipsb/9oADAMBAAIAAwAAABAL/8QAFxEAAwEAAAAAAAAAAAAAAAAAAAGhsf/aAAgBAwEBPxCl6f/EABYRAAMAAAAAAAAAAAAAAAAAAAABof/aAAgBAgEBPxCpn//EABcQAQEBAQAAAAAAAAAAAAAAAAERACH/2gAIAQEAAT8QQMsQporOu//Z";
const MAX_IMAGE_BYTES = 16 * 1024 * 1024;
const MAX_ENCODED_IMAGE_BYTES = 24 * 1024 * 1024;
const MAX_SVG_SOURCE_BYTES = 64 * 1024 * 1024;

function rasterDataUrl(format: string, payload: string): string {
  return `data:image/${format};base64,${payload}`;
}

function svgRaster(dataUrl: string, element = "image"): string {
  return `<svg><${element} href="${dataUrl}"/></svg>`;
}

function pngChunk(type: string, data: Buffer): Buffer {
  const chunk = Buffer.alloc(12 + data.length);
  chunk.writeUInt32BE(data.length, 0);
  chunk.write(type, 4, "ascii");
  data.copy(chunk, 8);
  return chunk;
}

function structuralPng(width: number, height: number, extraChunkBytes = 0): Buffer {
  const signature = Buffer.from("89504e470d0a1a0a", "hex");
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 6, 0, 0, 0], 8);
  const chunks = [pngChunk("IHDR", header)];
  if (extraChunkBytes > 0) {
    chunks.push(pngChunk("tEXt", Buffer.alloc(extraChunkBytes)));
  }
  chunks.push(pngChunk("IDAT", Buffer.from([0])), pngChunk("IEND", Buffer.alloc(0)));
  return Buffer.concat([signature, ...chunks]);
}

function structuralPngWithDecodedBytes(decodedBytes: number): Buffer {
  const base = structuralPng(1, 1);
  const extraChunkBytes = decodedBytes - base.length - 12;
  assert.ok(extraChunkBytes > 0);
  const result = structuralPng(1, 1, extraChunkBytes);
  assert.equal(result.length, decodedBytes);
  return result;
}

function animatedPng(): Buffer {
  const source = Buffer.from(PNG_1X1, "base64");
  const firstImageData = source.indexOf(Buffer.from("IDAT", "ascii")) - 4;
  const animationControl = Buffer.alloc(8);
  animationControl.writeUInt32BE(1, 0);
  return Buffer.concat([
    source.subarray(0, firstImageData),
    pngChunk("acTL", animationControl),
    source.subarray(firstImageData),
  ]);
}

function pngWithMetadata(type: string, data = Buffer.from([0])): Buffer {
  const source = structuralPng(1, 1);
  const firstImageData = source.indexOf(Buffer.from("IDAT", "ascii")) - 4;
  return Buffer.concat([
    source.subarray(0, firstImageData),
    pngChunk(type, data),
    source.subarray(firstImageData),
  ]);
}

function animatedGif(): Buffer {
  const source = Buffer.from(GIF_1X1, "base64");
  const imageStart = source.indexOf(0x2c);
  const trailer = source.lastIndexOf(0x3b);
  assert.ok(imageStart >= 0 && trailer > imageStart);
  const image = source.subarray(imageStart, trailer);
  return Buffer.concat([source.subarray(0, trailer), image, source.subarray(trailer)]);
}

function webpChunk(type: string, data: Buffer): Buffer {
  const chunk = Buffer.alloc(8 + data.length + (data.length & 1));
  chunk.write(type, 0, "ascii");
  chunk.writeUInt32LE(data.length, 4);
  data.copy(chunk, 8);
  return chunk;
}

function webpFile(chunks: readonly Buffer[]): Buffer {
  const result = Buffer.concat([Buffer.from("RIFF\0\0\0\0WEBP", "binary"), ...chunks]);
  result.writeUInt32LE(result.length - 8, 4);
  return result;
}

function extendedStaticWebp(
  simplePayload: string,
  canvasWidth: number,
  canvasHeight: number,
  frameWidth: number,
  frameHeight: number,
): Buffer {
  const imageChunk = Buffer.from(Buffer.from(simplePayload, "base64").subarray(12));
  const imageType = imageChunk.toString("ascii", 0, 4);
  if (imageType === "VP8 ") {
    imageChunk.writeUInt16LE((imageChunk.readUInt16LE(14) & 0xc000) | frameWidth, 14);
    imageChunk.writeUInt16LE((imageChunk.readUInt16LE(16) & 0xc000) | frameHeight, 16);
  } else {
    assert.equal(imageType, "VP8L");
    const widthMinusOne = frameWidth - 1;
    const heightMinusOne = frameHeight - 1;
    imageChunk.writeUInt8(widthMinusOne & 0xff, 9);
    imageChunk.writeUInt8(
      ((widthMinusOne >> 8) & 0x3f) | ((heightMinusOne & 0x03) << 6),
      10,
    );
    imageChunk.writeUInt8((heightMinusOne >> 2) & 0xff, 11);
    imageChunk.writeUInt8(
      (imageChunk.readUInt8(12) & 0x10) | ((heightMinusOne >> 10) & 0x0f),
      12,
    );
  }

  const extendedHeader = Buffer.alloc(10);
  if (imageType === "VP8L" && (imageChunk.readUInt8(12) & 0x10) !== 0) {
    extendedHeader[0] = 0x10;
  }
  extendedHeader.writeUIntLE(canvasWidth - 1, 4, 3);
  extendedHeader.writeUIntLE(canvasHeight - 1, 7, 3);
  return webpFile([webpChunk("VP8X", extendedHeader), imageChunk]);
}

function animatedWebp(): Buffer {
  const imageChunk = Buffer.from(Buffer.from(WEBP_1X1, "base64").subarray(12));
  const extendedHeader = Buffer.alloc(10);
  extendedHeader[0] = 0x02;
  const frame = webpChunk("ANMF", Buffer.concat([Buffer.alloc(16), imageChunk]));
  return webpFile([
    webpChunk("VP8X", extendedHeader),
    webpChunk("ANIM", Buffer.alloc(6)),
    frame,
    frame,
  ]);
}

describe("preview SVG safety", () => {
  it("accepts local inert SVG output", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        '<svg viewBox="0 0 100 50"><defs><marker id="arrow"></marker></defs><a href="#node"><text>ok</text></a></svg>',
      ),
    );
  });

  it("accepts inert Mermaid HTML labels inside foreignObject", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        '<svg viewBox="0 0 100 50"><foreignObject width="10" height="24" overflow="visible"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"><p>A</p></span></div></foreignObject></svg>',
      ),
    );
  });

  it("accepts local fragment and raster data URL references", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        `<svg><defs><linearGradient id="fill"></linearGradient><filter id="shadow"><feImage href="${rasterDataUrl("png", PNG_1X1)}"/></filter><clipPath id="clip"></clipPath><mask id="mask"></mask><marker id="arrow"></marker></defs><rect fill="url(#fill)" filter="url(#shadow)" clip-path="url(#clip)" mask="url(#mask)" marker-end="url(#arrow)"/><a href="#node">x</a><image href="${rasterDataUrl("png", PNG_1X1)}"/></svg>`,
      ),
    );
    for (const [format, payload] of [
      ["gif", GIF_1X1],
      ["jpeg", JPEG_1X1],
      ["jpg", JPEG_1X1],
      ["jpeg", JPEG_PROGRESSIVE_2X2],
      ["webp", WEBP_1X1],
      ["webp", WEBP_LOSSLESS_1X1],
    ] as const) {
      assert.doesNotThrow(() => assertSafePreviewSvg(svgRaster(rasterDataUrl(format, payload))));
    }
    for (const payload of [WEBP_1X1, WEBP_LOSSLESS_1X1]) {
      assert.doesNotThrow(() =>
        assertSafePreviewSvg(
          svgRaster(
            rasterDataUrl(
              "webp",
              extendedStaticWebp(payload, 1, 1, 1, 1).toString("base64"),
            ),
          ),
        ),
      );
    }
    assert.doesNotThrow(() =>
      assertSafePreviewSvg('<svg><style>text { fill: url(/* local */ #fill); }</style><text>ok</text></svg>'),
    );
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        '<svg><style>div.mermaidTooltip{position:absolute;pointer-events:none;z-index:100;}</style><text>ok</text></svg>',
      ),
    );
  });

  it("rejects malformed base64, malformed raster headers, and MIME mismatches", () => {
    assert.throws(
      () => assertSafePreviewSvg(svgRaster("data:image/png;base64,AB==")),
      /malformed embedded raster data URL/,
    );
    assert.throws(
      () => assertSafePreviewSvg(svgRaster("data:image/png;base64,iVBORw0KGgo=")),
      /malformed embedded raster image/,
    );
    assert.throws(
      () => assertSafePreviewSvg(svgRaster(rasterDataUrl("gif", PNG_1X1))),
      /MIME type does not match/,
    );
  });

  it("rejects raster frame dimensions that disagree with their container", () => {
    const gifOutsideLogicalScreen = Buffer.from(GIF_1X1, "base64");
    const imageDescriptor = gifOutsideLogicalScreen.indexOf(0x2c);
    gifOutsideLogicalScreen.writeUInt16LE(0xffff, imageDescriptor + 5);
    gifOutsideLogicalScreen.writeUInt16LE(0xffff, imageDescriptor + 7);
    assert.throws(
      () =>
        assertSafePreviewSvg(
          svgRaster(rasterDataUrl("gif", gifOutsideLogicalScreen.toString("base64"))),
        ),
      /malformed embedded raster image/,
    );

    for (const payload of [WEBP_1X1, WEBP_LOSSLESS_1X1]) {
      const mismatched = extendedStaticWebp(payload, 1, 1, 8192, 4096);
      assert.throws(
        () =>
          assertSafePreviewSvg(
            svgRaster(rasterDataUrl("webp", mismatched.toString("base64"))),
          ),
        /malformed embedded raster image/,
      );
    }
  });

  it("enforces source, encoded, and decoded byte budgets before normalization or allocation", () => {
    assert.throws(
      () => assertSafePreviewSvg("€".repeat(Math.floor(MAX_SVG_SOURCE_BYTES / 3) + 1)),
      /source byte limit/,
    );

    assert.throws(
      () =>
        assertSafePreviewSvg(
          svgRaster(rasterDataUrl("png", "A".repeat(MAX_ENCODED_IMAGE_BYTES + 4))),
        ),
      /per-image encoded byte limit/,
    );

    const oversized = Buffer.alloc(MAX_IMAGE_BYTES + 1).toString("base64");
    assert.throws(
      () => assertSafePreviewSvg(svgRaster(rasterDataUrl("png", oversized))),
      /per-image byte limit/,
    );

    const maximum = structuralPngWithDecodedBytes(MAX_IMAGE_BYTES).toString("base64");
    assert.throws(
      () =>
        assertSafePreviewSvg(
          `<svg><image href="${rasterDataUrl("png", maximum)}"/><feImage href="${rasterDataUrl("png", maximum)}"/><image href="${rasterDataUrl("png", PNG_1X1)}"/></svg>`,
        ),
      /aggregate byte limit/,
    );

    const encodedAggregateOverflow = structuralPngWithDecodedBytes(1024 * 1024 + 1).toString(
      "base64",
    );
    assert.throws(
      () =>
        assertSafePreviewSvg(
          `<svg><image href="${rasterDataUrl("png", maximum)}"/><feImage href="${rasterDataUrl("png", maximum)}"/><image href="${rasterDataUrl("png", encodedAggregateOverflow)}"/></svg>`,
        ),
      /aggregate encoded byte limit/,
    );
  });

  it("enforces intrinsic dimension and aggregate pixel budgets for image and feImage", () => {
    const zeroWidth = structuralPng(0, 1).toString("base64");
    const oversized = structuralPng(4097, 4096).toString("base64");
    const maximum = structuralPng(4096, 4096).toString("base64");
    assert.throws(
      () => assertSafePreviewSvg(svgRaster(rasterDataUrl("png", zeroWidth))),
      /malformed embedded raster image/,
    );
    assert.throws(
      () => assertSafePreviewSvg(svgRaster(rasterDataUrl("png", oversized), "feImage")),
      /per-image pixel limit/,
    );
    assert.throws(
      () =>
        assertSafePreviewSvg(
          `<svg><image href="${rasterDataUrl("png", maximum)}"/><feImage href="${rasterDataUrl("png", maximum)}"/><image href="${rasterDataUrl("png", PNG_1X1)}"/></svg>`,
        ),
      /aggregate pixel limit/,
    );
  });

  it("rejects APNG, multi-image GIF, and animated WebP payloads", () => {
    for (const [format, bytes] of [
      ["png", animatedPng()],
      ["gif", animatedGif()],
      ["webp", animatedWebp()],
    ] as const) {
      assert.throws(
        () => assertSafePreviewSvg(svgRaster(rasterDataUrl(format, bytes.toString("base64")))),
        /animated or multi-frame/,
      );
    }
  });

  it("rejects PNG chunks with independently compressed metadata", () => {
    for (const [type, data] of [
      ["iCCP", Buffer.from([0])],
      ["iTXt", Buffer.from([0x6b, 0, 1, 0, 0, 0])],
      ["zTXt", Buffer.from([0])],
    ] as const) {
      assert.throws(
        () =>
          assertSafePreviewSvg(
            svgRaster(rasterDataUrl("png", pngWithMetadata(type, data).toString("base64"))),
          ),
        /malformed embedded raster image/,
      );
    }
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        svgRaster(
          rasterDataUrl(
            "png",
            pngWithMetadata(
              "iTXt",
              Buffer.from([0x6b, 0, 0, 0, 0, 0, 0x6f, 0x6b]),
            ).toString("base64"),
          ),
        ),
      ),
    );
  });

  it("accepts comments around a single SVG root", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        "<!-- generated by test --><svg><text>ok</text></svg><!-- trailing comment -->",
      ),
    );
  });

  it("accepts many ignorable chunks without recursive parsing", () => {
    const prefix = Array.from({ length: 2_000 }, (_, index) => `<!-- ${index} -->`).join("");
    assert.doesNotThrow(() => assertSafePreviewSvg(`${prefix}<svg><text>ok</text></svg>`));
  });

  it("rejects non-SVG renderer output", () => {
    assert.throws(() => assertSafePreviewSvg("<html></html>"), /non-SVG/);
    assert.throws(() => assertSafePreviewSvg("text before root<svg></svg>"), /non-SVG/);
  });

  it("rejects active or interactive content after the SVG root closes", () => {
    assert.throws(
      () => assertSafePreviewSvg("<svg></svg><style>button{color:red}</style>"),
      /malformed/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg></svg><button autofocus>run</button>"),
      /malformed/,
    );
  });

  it("rejects unsupported elements and mismatched SVG tags", () => {
    assert.throws(
      () => assertSafePreviewSvg('<svg><div tabindex="0">run</div></svg>'),
      /unsupported element/,
    );
    assert.throws(() => assertSafePreviewSvg("<svg><g></svg></g>"), /malformed/);
  });

  it("rejects active embedded SVG content", () => {
    assert.throws(() => assertSafePreviewSvg("<svg><script>alert(1)</script></svg>"), /active/);
    assert.throws(() => assertSafePreviewSvg("<svg><iframe></iframe></svg>"), /active/);
    assert.throws(() => assertSafePreviewSvg('<svg><animate attributeName="href" to="https://example.com/x"/></svg>'), /active/);
    assert.throws(() => assertSafePreviewSvg("<svg><set attributeName=\"fill\" to=\"url(https://example.com/x)\"/></svg>"), /active/);
  });

  it("rejects interactive or non-label foreignObject content", () => {
    assert.throws(
      () => assertSafePreviewSvg("<svg><foreignObject><button>run</button></foreignObject></svg>"),
      /foreignObject/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><input value="x"/></foreignObject></svg>'),
      /foreignObject/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><foreignObject><style>button{color:red}</style></foreignObject></svg>"),
      /foreignObject/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><div tabindex="0">focus</div></foreignObject></svg>'),
      /interactive/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject tabindex="0"><div>focus</div></foreignObject></svg>'),
      /interactive/,
    );
  });

  it("rejects event handlers and unsafe URL attributes", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><text onclick="alert(1)">x</text></svg>'), /event/);
    assert.throws(() => assertSafePreviewSvg('<svg><text OnClick="alert(1)">x</text></svg>'), /event/);
    assert.throws(() => assertSafePreviewSvg('<svg OnLoad="alert(1)"><text>x</text></svg>'), /event/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><div onclick="alert(1)">x</div></foreignObject></svg>'),
      /event/,
    );
    assert.throws(() => assertSafePreviewSvg('<svg><a href="javascript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="java&#115;cript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="java&#115cript:alert(1)">x</a></svg>'), /external|unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="javascript&colon;alert(1)">x</a></svg>'), /external|unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a xlink:href="JavaScript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><image href="data:text/html,hello"/></svg>'),
      /malformed embedded raster data URL/,
    );
    assert.throws(() => assertSafePreviewSvg('<svg><image href="file:///etc/passwd"/></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="command:workbench.action.openSettings">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="vscode://file/path">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="foo:bar">x</a></svg>'), /unsafe URL/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><img srcset="https://example.com/a.png 1x"/></foreignObject></svg>'),
      /foreignObject/,
    );
    assert.throws(() => assertSafePreviewSvg('<svg><image srcset="https://example.com/a.png 1x"/></svg>'), /srcset/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><button formaction="https://example.com/post">x</button></foreignObject></svg>'),
      /foreignObject/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><a href="#node" ping="https://example.com/ping">x</a></svg>'),
      /external/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg xml:base="https://example.com/sprite.svg"><use href="#icon"/></svg>'),
      /base/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><image href="data:image/svg+xml,%3Csvg%20onload%3Dalert(1)%3E"/></svg>'),
      /malformed embedded raster data URL/,
    );
  });

  it("rejects external resource references", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><image href="https://example.com/a.png"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><use href="//example.com/sprite.svg#x"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><image href="images/a.png"/></svg>'), /external/);
  });

  it("rejects external resources in SVG URL-bearing attributes", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><rect fill="url(https://example.com/fill.svg#x)"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect stroke="url(file:///tmp/stroke.svg#x)"/></svg>'), /unsafe/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect filter="url(data:image/svg+xml,%3Csvg%3E)"/></svg>'), /unsafe/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect clip-path="url(//example.com/clip.svg#x)"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect mask="url(images/mask.svg#x)"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><path marker-end="url(javascript:alert(1))"/></svg>'), /unsafe/);
  });

  it("rejects unsafe CSS references", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(javascript:alert(1))">x</text></svg>'), /CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:u&#114l(https://example.com/a.svg#x)">x</text></svg>'), /CSS resource|CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url&lpar;https://example.com/a.svg#x&rpar;">x</text></svg>'), /CSS resource|CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(jav\\61script:alert(1))">x</text></svg>'), /CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(file:///tmp/a.svg)">x</text></svg>'), /CSS URL/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><text style="fill:url(data:image/svg+xml,%3Csvg%3E)">x</text></svg>'),
      /unsafe embedded resource references/,
    );
    assert.throws(
      () =>
        assertSafePreviewSvg(
          '<svg><foreignObject><div style="background-image:image-set(&quot;https://example.com/a.png&quot; 1x)">x</div></foreignObject></svg>',
        ),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { background-image: -webkit-image-set("https://example.com/a.png" 1x); }</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>svg{position:fixed;inset:0;z-index:999999}</style></svg>"),
      /viewport-escaping CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg style="position:fixed;inset:0"></svg>'),
      /viewport-escaping CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>svg{position:absolute;inset:0;z-index:999999}</style></svg>"),
      /viewport-escaping CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>@media all{svg{position:absolute;inset:0;z-index:999999}}</style></svg>"),
      /viewport-escaping CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>@supports(display:block){svg{position:fixed;inset:0}}</style></svg>"),
      /viewport-escaping CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg style="position:absolute;left:0;top:0"></svg>'),
      /viewport-escaping CSS/,
    );
    assert.throws(() => assertSafePreviewSvg('<svg><style>@import "https://example.com/a.css";</style></svg>'), /CSS resource/);
    assert.throws(() => assertSafePreviewSvg('<svg><style>@im&#112ort "https://example.com/a.css";</style></svg>'), /CSS resource/);
    assert.throws(() => assertSafePreviewSvg('<svg><style>@im&#x2f;*hidden*&#x2f;port "https://example.com/a.css";</style></svg>'), /CSS resource/);
    assert.throws(() => assertSafePreviewSvg('<svg><style>text { fill: url(//example.com/a.svg#x); }</style></svg>'), /CSS resource/);
    assert.throws(() => assertSafePreviewSvg('<svg><style>text { fill: u&#x72l(https://example.com/a.svg#x); }</style></svg>'), /CSS resource|CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><style>text { fill: u&#x2f;*hidden*&#x2f;rl(javascript:alert(1)); }</style></svg>'), /CSS resource|CSS URL/);
    assert.throws(
      () =>
        assertSafePreviewSvg(
          '<svg><style>text { fill: url(#ok); stroke: /* padding #safe */ url(https://example.com/x.svg#x); }</style></svg>',
        ),
      /CSS resource|CSS URL/,
    );
  });

  it("rejects shadow-scoping CSS selectors", () => {
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>:host{position:fixed;inset:0}</style></svg>"),
      /shadow CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>:host-context(body){z-index:999999}</style></svg>"),
      /shadow CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>::slotted(*){display:block}</style></svg>"),
      /shadow CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>:h\\6fst{position:fixed}</style></svg>"),
      /shadow CSS/,
    );
    assert.throws(
      () => assertSafePreviewSvg("<svg><style>:h/*hidden*/ost{position:fixed}</style></svg>"),
      /shadow CSS/,
    );
  });

  it("rejects CSS resource keywords hidden behind CSS escapes", () => {
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>@im\\70ort "https://example.com/a.css";</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { fill: u\\72l(//example.com/a.svg#x); }</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { fill: u\\000072 l(javascript:alert(1)); }</style></svg>'),
      /CSS URL/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>@im/*hidden*/port "https://example.com/a.css";</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { fill: u/*hidden*/rl(//example.com/a.svg#x); }</style></svg>'),
      /CSS resource/,
    );
  });
});
