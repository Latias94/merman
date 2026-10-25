# Bundled SDK fonts

These unmodified Cousine fonts are production resources for the optional `bundled-fonts` feature. They are independent copies of the upstream files pinned in `manifest.json`; runtime code does not reference test fixtures.

Regular, bold and italic faces provide a compact Latin, Greek and Cyrillic fallback. This bundle does not claim CJK or emoji coverage. Hosts can supply additional OpenType bytes before these fallback faces.

Distribute `OFL.txt` and the font inventory with SDK artifacts. Font selection for measurement and native painting shares the admitted context; SVG text still requires matching fonts at the display host.
