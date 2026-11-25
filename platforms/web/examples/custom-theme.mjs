/**
 * Set canvas and node colors for one declared appearance of an exported preset.
 *
 * @param {import('../src/public-types.js').ThemeDocumentV2} document
 * @param {import('../src/public-types.js').ThemeAppearance} appearance
 * @param {{ background: string, nodeBorder: string, nodeFill: string }} colors
 * @returns {import('../src/public-types.js').ThemeDocumentV2}
 */
export function customizeNodeColors(document, appearance, colors) {
  const layer = document.appearances?.[appearance];
  return {
    ...document,
    appearances: {
      ...document.appearances,
      [appearance]: {
        ...layer,
        roles: {
          ...layer?.roles,
          canvas: colors.background,
          "stroke.border": colors.nodeBorder,
          "surface.node": colors.nodeFill,
        },
      },
    },
  };
}
