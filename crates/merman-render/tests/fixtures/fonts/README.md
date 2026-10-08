# Font Container Fixtures

`FontAwesome-4.6.3.otf` is a CFF-flavored OpenType test fixture copied from the pinned local
`cytoscape.js` checkout at revision `22716bfb75834b56fa6679648b0abb06f4ae691c`:

- Source path: `documentation/font/FontAwesome.otf`
- SHA-256: `ecd72f31910a8ee2726fd17bd459be26f230779f3f3ed5f69ebf829e4b12e768`
- Upstream declaration: `documentation/css/font-awesome.css`
- Declaration SHA-256: `9a5e39a1ecc5d4c9331216d47e0ed4b14a71549291a39da09478a06c9a5aad00`
- Font license: SIL Open Font License 1.1
- License text: `fixtures/themes/licenses/OFL-1.1.txt`

The fixture is retained only to exercise the CFF OpenType admission path. Tests derive TrueType
and TrueType Collection inputs from the already licensed WOFF2 theme fixtures so they do not
depend on fonts installed on the host machine.

`DejaVuSerif-NativeFilter.ttf` is a 11,788-byte subset of DejaVu Serif 2.37 from
Debian's `fonts-dejavu-core_2.37-8_all.deb`. It retains only the characters in
`Request VolumeDocumentation`, the two labels whose native shadows exceeded the
previous fixed-em text allocation on Linux. Native export receipt tests load it
explicitly, so the regression is independent of the host's installed fonts.

- Font license: Bitstream Vera font license; DejaVu changes are public domain.
- License text: `DejaVu-LICENSE.txt` in this directory.
- SHA-256: `108e66f8d1c7d2580dc8245286bd6290b815d0a002a74322322b542a832ccb7c`.
- Subset tool: fontTools `pyftsubset`, with `--text='Request VolumeDocumentation'`,
  `--name-IDs='*'`, and `--name-languages='*'`.
