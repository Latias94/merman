// Mermaid's ZenUML adapter renders native SVG and never uses the bundled neon UI font.
// Keep the no-resource realm self-contained instead of redistributing that optional font.
const ZENUML_OPTIONAL_FONT_FACE = String.raw`
  @font-face{font-family:MS Sans Serif;src:url(/fonts/MS%20Sans%20Serif.ttf) format(\"truetype\")}
`.trim();

export function applyOpaqueRealmSourcePolicy(artifact, source) {
  if (artifact.id !== "mermaid" || artifact.resourcePolicy !== "none-v1") {
    return source;
  }

  const occurrences = source.split(ZENUML_OPTIONAL_FONT_FACE).length - 1;
  if (occurrences !== 1) {
    throw new Error(
      `mermaid engine expected one ZenUML optional font injection; found ${occurrences}.`,
    );
  }

  const sanitized = source.replace(ZENUML_OPTIONAL_FONT_FACE, "");
  if (sanitized.includes("MS%20Sans%20Serif.ttf")) {
    throw new Error("mermaid engine retains the ZenUML optional font resource.");
  }
  return sanitized;
}
