import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const { assertSafeSvgForDom } = await import(
  pathToFileURL(path.join(packageRoot, "dist", "svg-safety.js")).href
);

const PNG_1X1 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
const GIF_1X1 = "R0lGODdhAQABAJEAAAAAAPDv9////wAAACH5BAQAAAAALAAAAAABAAEAAAICTAEAOw==";
const WEBP_1X1 = "UklGRiYAAABXRUJQVlA4IBoAAAAwAQCdASoBAAEAAgA0JZwAA3AA/vo8xw8gAA==";
const WEBP_LOSSLESS_1X1 = "UklGRhwAAABXRUJQVlA4TA8AAAAvAAAAAAcQ/Y/+ByKi/wEA";
const JPEG_PROGRESSIVE_2X2 =
  "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wgARCAACAAIDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAb/xAAUAQEAAAAAAAAAAAAAAAAAAAAF/9oADAMBAAIQAxAAAAGsBxX/xAAWEAEBAQAAAAAAAAAAAAAAAAADBAH/2gAIAQEAAQUCjI9i/8QAFxEAAwEAAAAAAAAAAAAAAAAAAAEDMv/aAAgBAwEBPwGm2f/EABcRAAMBAAAAAAAAAAAAAAAAAAABAjL/2gAIAQIBAT8BvTP/xAAbEAACAQUAAAAAAAAAAAAAAAABAgADERNBYf/aAAgBAQAGPwKgSi3xrrk//8QAFhABAQEAAAAAAAAAAAAAAAAAASEA/9oACAEBAAE/IUc0ipsb/9oADAMBAAIAAwAAABAL/8QAFxEAAwEAAAAAAAAAAAAAAAAAAAGhsf/aAAgBAwEBPxCl6f/EABYRAAMAAAAAAAAAAAAAAAAAAAABof/aAAgBAgEBPxCpn//EABcQAQEBAQAAAAAAAAAAAAAAAAERACH/2gAIAQEAAT8QQMsQporOu//Z";
const MAX_IMAGE_BYTES = 16 * 1024 * 1024;
const MAX_ENCODED_IMAGE_BYTES = 24 * 1024 * 1024;
const MAX_SVG_SOURCE_BYTES = 64 * 1024 * 1024;

function rasterDataUrl(format, payload) {
  return `data:image/${format};base64,${payload}`;
}

function svgRaster(dataUrl, element = "image") {
  return `<svg><${element} href="${dataUrl}"/></svg>`;
}

function pngChunk(type, data) {
  const chunk = Buffer.alloc(12 + data.length);
  chunk.writeUInt32BE(data.length, 0);
  chunk.write(type, 4, "ascii");
  data.copy(chunk, 8);
  return chunk;
}

function structuralPng(width, height, extraChunkBytes = 0) {
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

function structuralPngWithDecodedBytes(decodedBytes) {
  const base = structuralPng(1, 1);
  const extraChunkBytes = decodedBytes - base.length - 12;
  assert.ok(extraChunkBytes > 0);
  const result = structuralPng(1, 1, extraChunkBytes);
  assert.equal(result.length, decodedBytes);
  return result;
}

function animatedPng() {
  const source = Buffer.from(PNG_1X1, "base64");
  const firstImageData = source.indexOf(Buffer.from("IDAT", "ascii")) - 4;
  return Buffer.concat([
    source.subarray(0, firstImageData),
    pngChunk("acTL", Buffer.alloc(8)),
    source.subarray(firstImageData),
  ]);
}

function pngWithMetadata(type, data = Buffer.from([0])) {
  const source = structuralPng(1, 1);
  const firstImageData = source.indexOf(Buffer.from("IDAT", "ascii")) - 4;
  return Buffer.concat([
    source.subarray(0, firstImageData),
    pngChunk(type, data),
    source.subarray(firstImageData),
  ]);
}

function animatedGif() {
  const source = Buffer.from(GIF_1X1, "base64");
  const imageStart = source.indexOf(0x2c);
  const trailer = source.lastIndexOf(0x3b);
  assert.ok(imageStart >= 0 && trailer > imageStart);
  return Buffer.concat([
    source.subarray(0, trailer),
    source.subarray(imageStart, trailer),
    source.subarray(trailer),
  ]);
}

function gifWithExtension(extension) {
  const source = Buffer.from(GIF_1X1, "base64");
  const imageStart = source.indexOf(0x2c);
  assert.ok(imageStart >= 0);
  return Buffer.concat([source.subarray(0, imageStart), extension, source.subarray(imageStart)]);
}

function gifWithApplicationExtension(identifier) {
  assert.equal(Buffer.byteLength(identifier, "ascii"), 11);
  return gifWithExtension(
    Buffer.concat([
      Buffer.from([0x21, 0xff, 0x0b]),
      Buffer.from(identifier, "ascii"),
      Buffer.from([0x03, 0x01, 0x00, 0x00, 0x00]),
    ]),
  );
}

function gifWithPlainTextExtension() {
  return gifWithExtension(Buffer.from([0x21, 0x01, 0x0c, ...Buffer.alloc(12), 0x00]));
}

function webpChunk(type, data) {
  const chunk = Buffer.alloc(8 + data.length + (data.length & 1));
  chunk.write(type, 0, "ascii");
  chunk.writeUInt32LE(data.length, 4);
  data.copy(chunk, 8);
  return chunk;
}

function webpFile(chunks) {
  const result = Buffer.concat([Buffer.from("RIFF\0\0\0\0WEBP", "binary"), ...chunks]);
  result.writeUInt32LE(result.length - 8, 4);
  return result;
}

function writeUInt24LE(buffer, value, offset) {
  buffer[offset] = value & 0xff;
  buffer[offset + 1] = (value >> 8) & 0xff;
  buffer[offset + 2] = (value >> 16) & 0xff;
}

function extendedStaticWebp(simplePayload, canvasWidth, canvasHeight, frameWidth, frameHeight) {
  const imageChunk = Buffer.from(Buffer.from(simplePayload, "base64").subarray(12));
  const imageType = imageChunk.toString("ascii", 0, 4);
  if (imageType === "VP8 ") {
    imageChunk.writeUInt16LE((imageChunk.readUInt16LE(14) & 0xc000) | frameWidth, 14);
    imageChunk.writeUInt16LE((imageChunk.readUInt16LE(16) & 0xc000) | frameHeight, 16);
  } else {
    assert.equal(imageType, "VP8L");
    const widthMinusOne = frameWidth - 1;
    const heightMinusOne = frameHeight - 1;
    imageChunk[9] = widthMinusOne & 0xff;
    imageChunk[10] = ((widthMinusOne >> 8) & 0x3f) | ((heightMinusOne & 0x03) << 6);
    imageChunk[11] = (heightMinusOne >> 2) & 0xff;
    imageChunk[12] = (imageChunk[12] & 0x10) | ((heightMinusOne >> 10) & 0x0f);
  }

  const extendedHeader = Buffer.alloc(10);
  if (imageType === "VP8L" && (imageChunk[12] & 0x10) !== 0) {
    extendedHeader[0] = 0x10;
  }
  writeUInt24LE(extendedHeader, canvasWidth - 1, 4);
  writeUInt24LE(extendedHeader, canvasHeight - 1, 7);
  return webpFile([webpChunk("VP8X", extendedHeader), imageChunk]);
}

function animatedWebp() {
  const imageChunk = Buffer.from(Buffer.from(WEBP_1X1, "base64").subarray(12));
  const extendedHeader = Buffer.alloc(10);
  extendedHeader[0] = 0x02;
  const animation = Buffer.alloc(6);
  const frame = webpChunk("ANMF", Buffer.concat([Buffer.alloc(16), imageChunk]));
  return webpFile([
    webpChunk("VP8X", extendedHeader),
    webpChunk("ANIM", animation),
    frame,
    frame,
  ]);
}

assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    '<svg><defs><linearGradient id="fill"></linearGradient><filter id="shadow"></filter></defs><rect fill="url(#fill)" filter="url(#shadow)"/></svg>',
  ),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom('<svg><style>text { fill: url(/* local */ #fill); }</style><text>ok</text></svg>'),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    "<svg><style>/*\u0130*/ text { fill: red; }</StYlE><text>ok</text></svg>",
  ),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    '<svg><foreignObject width="10" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell"><span class="nodeLabel"><p>A</p></span></div></foreignObject></svg>',
  ),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    '<svg><style>div.mermaidTooltip{position:absolute;pointer-events:none;z-index:100;}</style><text>ok</text></svg>',
  ),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom("<!-- generated by test --><svg><text>ok</text></svg><!-- trailing comment -->"),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    `${Array.from({ length: 2_000 }, (_, index) => `<!-- ${index} -->`).join("")}<svg><text>ok</text></svg>`,
  ),
);
assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    `<svg><image href="${rasterDataUrl("png", PNG_1X1)}"/><filter><feImage href="${rasterDataUrl("png", PNG_1X1)}"/></filter></svg>`,
  ),
);
for (const [format, payload] of [
  ["gif", GIF_1X1],
  ["jpeg", JPEG_PROGRESSIVE_2X2],
  ["webp", WEBP_1X1],
  ["webp", WEBP_LOSSLESS_1X1],
]) {
  assert.doesNotThrow(() => assertSafeSvgForDom(svgRaster(rasterDataUrl(format, payload))));
}
for (const payload of [WEBP_1X1, WEBP_LOSSLESS_1X1]) {
  assert.doesNotThrow(() =>
    assertSafeSvgForDom(
      svgRaster(rasterDataUrl("webp", extendedStaticWebp(payload, 1, 1, 1, 1).toString("base64"))),
    ),
  );
}

assert.throws(
  () => assertSafeSvgForDom(svgRaster("data:image/png;base64,iVBORw0KGgo=")),
  /malformed embedded raster image/,
);
assert.throws(
  () => assertSafeSvgForDom(svgRaster(rasterDataUrl("gif", PNG_1X1))),
  /MIME type does not match/,
);
const gifOutsideLogicalScreen = Buffer.from(GIF_1X1, "base64");
const gifImageDescriptor = gifOutsideLogicalScreen.indexOf(0x2c);
gifOutsideLogicalScreen.writeUInt16LE(0xffff, gifImageDescriptor + 5);
gifOutsideLogicalScreen.writeUInt16LE(0xffff, gifImageDescriptor + 7);
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(rasterDataUrl("gif", gifOutsideLogicalScreen.toString("base64"))),
    ),
  /malformed embedded raster image/,
);
for (const payload of [WEBP_1X1, WEBP_LOSSLESS_1X1]) {
  const mismatched = extendedStaticWebp(payload, 1, 1, 8192, 4096);
  assert.throws(
    () => assertSafeSvgForDom(svgRaster(rasterDataUrl("webp", mismatched.toString("base64")))),
    /malformed embedded raster image/,
  );
}
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(rasterDataUrl("png", Buffer.alloc(MAX_IMAGE_BYTES + 1).toString("base64"))),
    ),
  /per-image byte limit/,
);
assert.throws(
  () => assertSafeSvgForDom("€".repeat(Math.floor(MAX_SVG_SOURCE_BYTES / 3) + 1)),
  /source byte limit/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(rasterDataUrl("png", "A".repeat(MAX_ENCODED_IMAGE_BYTES + 4))),
    ),
  /per-image encoded byte limit/,
);
const maximumBytes = structuralPngWithDecodedBytes(MAX_IMAGE_BYTES).toString("base64");
assert.throws(
  () =>
    assertSafeSvgForDom(
      `<svg><image href="${rasterDataUrl("png", maximumBytes)}"/><feImage href="${rasterDataUrl("png", maximumBytes)}"/><image href="${rasterDataUrl("png", PNG_1X1)}"/></svg>`,
    ),
  /aggregate byte limit/,
);
const encodedAggregateOverflow = structuralPngWithDecodedBytes(1024 * 1024 + 1).toString(
  "base64",
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      `<svg><image href="${rasterDataUrl("png", maximumBytes)}"/><feImage href="${rasterDataUrl("png", maximumBytes)}"/><image href="${rasterDataUrl("png", encodedAggregateOverflow)}"/></svg>`,
    ),
  /aggregate encoded byte limit/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(rasterDataUrl("png", structuralPng(0, 1).toString("base64"))),
    ),
  /malformed embedded raster image/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(rasterDataUrl("png", structuralPng(4097, 4096).toString("base64")), "feImage"),
    ),
  /per-image pixel limit/,
);
const maximumPixels = structuralPng(4096, 4096).toString("base64");
assert.throws(
  () =>
    assertSafeSvgForDom(
      `<svg><image href="${rasterDataUrl("png", maximumPixels)}"/><feImage href="${rasterDataUrl("png", maximumPixels)}"/><image href="${rasterDataUrl("png", PNG_1X1)}"/></svg>`,
    ),
  /aggregate pixel limit/,
);
for (const [format, bytes] of [
  ["png", animatedPng()],
  ["gif", animatedGif()],
  ["webp", animatedWebp()],
]) {
  assert.throws(
    () => assertSafeSvgForDom(svgRaster(rasterDataUrl(format, bytes.toString("base64")))),
    /animated or multi-frame/,
  );
}
for (const chunkType of ["fcTL", "fdAT"]) {
  const data = Buffer.alloc(chunkType === "fcTL" ? 26 : 4);
  assert.throws(
    () =>
      assertSafeSvgForDom(
        svgRaster(
          rasterDataUrl("png", pngWithMetadata(chunkType, data).toString("base64")),
        ),
      ),
    /animated or multi-frame/,
  );
}
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(
        rasterDataUrl("gif", gifWithApplicationExtension("NETSCAPE2.0").toString("base64")),
      ),
    ),
  /animated or multi-frame/,
);
for (const gif of [
  gifWithApplicationExtension("XXXXXXXXXXX"),
  gifWithPlainTextExtension(),
]) {
  assert.throws(
    () => assertSafeSvgForDom(svgRaster(rasterDataUrl("gif", gif.toString("base64")))),
    /malformed embedded raster image/,
  );
}
assert.throws(
  () =>
    assertSafeSvgForDom(
      svgRaster(
        rasterDataUrl("png", pngWithMetadata("vpAg", Buffer.alloc(0)).toString("base64")),
      ),
    ),
  /malformed embedded raster image/,
);
const simpleImageChunk = Buffer.from(Buffer.from(WEBP_1X1, "base64").subarray(12));
for (const webp of [
  webpFile([webpChunk("JUNK", Buffer.alloc(0)), simpleImageChunk]),
  webpFile([
    webpChunk("VP8X", Buffer.alloc(10)),
    webpChunk("JUNK", Buffer.alloc(0)),
    simpleImageChunk,
  ]),
]) {
  assert.throws(
    () => assertSafeSvgForDom(svgRaster(rasterDataUrl("webp", webp.toString("base64")))),
    /malformed embedded raster image/,
  );
}
for (const [type, data] of [
  ["iCCP", Buffer.from([0])],
  ["iTXt", Buffer.from([0x6b, 0, 1, 0, 0, 0])],
  ["zTXt", Buffer.from([0])],
]) {
  assert.throws(
    () =>
      assertSafeSvgForDom(
        svgRaster(rasterDataUrl("png", pngWithMetadata(type, data).toString("base64"))),
      ),
    /malformed embedded raster image/,
  );
}
assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    svgRaster(
      rasterDataUrl(
        "png",
        pngWithMetadata("iTXt", Buffer.from([0x6b, 0, 0, 0, 0, 0, 0x6f, 0x6b])).toString(
          "base64",
        ),
      ),
    ),
  ),
);

assert.throws(
  () => assertSafeSvgForDom('<svg><image href="https://example.com/a.png"/></svg>'),
  /external/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg></svg><style>button{color:red}</style>'),
  /malformed/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg></svg><button autofocus>run</button>'),
  /malformed/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><div tabindex="0">run</div></svg>'),
  /unsupported element/,
);
assert.throws(
  () => assertSafeSvgForDom("<svg><g></svg></g>"),
  /malformed/,
);
assert.throws(
  () => assertSafeSvgForDom("text before root<svg></svg>"),
  /non-SVG/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><rect fill="url(https://example.com/fill.svg#x)"/></svg>'),
  /external/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><text style="fill:url(javascript:alert(1))">x</text></svg>'),
  /CSS URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><text style="fill:u&#114l(https://example.com/a.svg#x)">x</text></svg>'),
  /CSS resource|CSS URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><text style="fill:url&lpar;https://example.com/a.svg#x&rpar;">x</text></svg>'),
  /CSS resource|CSS URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>@im&#112ort "https://example.com/a.css";</style></svg>'),
  /CSS resource/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>@im&#x2f;*hidden*&#x2f;port "https://example.com/a.css";</style></svg>'),
  /CSS resource/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>text { fill: u&#x72l(https://example.com/a.svg#x); }</style></svg>'),
  /CSS resource|CSS URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>text { fill: u&#x2f;*hidden*&#x2f;rl(javascript:alert(1)); }</style></svg>'),
  /CSS resource|CSS URL/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      "<svg><style>text { fill: \\75\r\nrl(javascript:alert(1)); }</style></svg>",
    ),
  /CSS resource|CSS URL/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      '<svg><style>text { fill: url(#ok); stroke: /* padding #safe */ url(https://example.com/x.svg#x); }</style></svg>',
    ),
  /CSS resource|CSS URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><a href="java&#115cript:alert(1)">x</a></svg>'),
  /external|unsafe URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><a href="javascript&colon;alert(1)">x</a></svg>'),
  /external|unsafe URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject><div onclick="alert(1)">x</div></foreignObject></svg>'),
  /event/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><text OnClick="alert(1)">x</text></svg>'),
  /event/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg OnLoad="alert(1)"><text>x</text></svg>'),
  /event/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject><img srcset="https://example.com/a.png 1x"/></foreignObject></svg>'),
  /foreignObject/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><image srcset="https://example.com/a.png 1x"/></svg>'),
  /srcset/,
);
assert.throws(
  () => assertSafeSvgForDom("<svg><foreignObject><button>run</button></foreignObject></svg>"),
  /foreignObject/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject><input value="x"/></foreignObject></svg>'),
  /foreignObject/,
);
assert.throws(
  () => assertSafeSvgForDom("<svg><foreignObject><style>button{color:red}</style></foreignObject></svg>"),
  /foreignObject/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject><div tabindex="0">focus</div></foreignObject></svg>'),
  /interactive/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject tabindex="0"><div>focus</div></foreignObject></svg>'),
  /interactive/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><a href="#node" ping="https://example.com/ping">x</a></svg>'),
  /external/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg xml:base="https://example.com/sprite.svg"><use href="#icon"/></svg>'),
  /base/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      '<svg><foreignObject><div style="background-image:image-set(&quot;https://example.com/a.png&quot; 1x)">x</div></foreignObject></svg>',
    ),
  /CSS resource/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>svg{position:fixed;inset:0;z-index:999999}</style></svg>'),
  /viewport-escaping CSS/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg style="position:fixed;inset:0"></svg>'),
  /viewport-escaping CSS/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>svg{position:absolute;inset:0;z-index:999999}</style></svg>'),
  /viewport-escaping CSS/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>@media all{svg{position:absolute;inset:0;z-index:999999}}</style></svg>'),
  /viewport-escaping CSS/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><style>@supports(display:block){svg{position:fixed;inset:0}}</style></svg>'),
  /viewport-escaping CSS/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg style="position:absolute;left:0;top:0"></svg>'),
  /viewport-escaping CSS/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><animate attributeName="href" to="https://example.com/x"/></svg>'),
  /active/,
);

console.log("@mermanjs/web DOM safety smoke passed");
