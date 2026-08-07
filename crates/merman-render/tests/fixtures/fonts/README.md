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
