# merman-export

`merman-export` is the bounded binary-export layer behind Merman's PNG, JPEG, and PDF output.

Most applications should depend on [`merman`](https://crates.io/crates/merman) and call its
high-level `HeadlessRenderer` methods. This crate exists so SVG conversion backends stay separate
from Mermaid parsing, semantic analysis, layout, and SVG construction as an implementation
responsibility.

The public API accepts only `merman_render::svg::ResvgCompatibleSvg`, a terminally validated SVG
artifact. It deliberately does not accept Mermaid source strings or an engine, so callers cannot
bypass the source-to-SVG safety pipeline.

That sealed producer contract intentionally keeps `merman-render` in this crate's resolved
dependency closure. `merman-export` is therefore not a lightweight arbitrary-SVG conversion
library, and its crate boundary alone is not evidence that parsing or rendering dependencies were
removed. Exact PNG and PDF closure claims are verified from the repository's artifact profiles.

Enable only the formats needed by the host:

```toml
[dependencies]
merman-export = { version = "0.8.0-alpha.3", features = ["png"] }
```

`png` and `jpeg` share private bitmap preparation. `pdf` is a separate vector export capability.
All formats keep explicit allocation and embedded-image limits; see the main project's
[output documentation](https://github.com/Latias94/merman/blob/main/docs/rendering/RASTER_OUTPUT.md).

## License

Licensed under either of Apache License, Version 2.0 or MIT at your option.
