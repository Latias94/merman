#![forbid(unsafe_code)]

//! `merman` is a headless, parity-focused Mermaid implementation in Rust.
//!
//! It is pinned to Mermaid `@11.12.2`; upstream Mermaid is treated as the spec. See:
//! - `docs/adr/0014-upstream-parity-policy.md`
//! - `docs/alignment/STATUS.md`
//!
//! # Features
//!
//! - `render`: enable layout + SVG rendering (`merman::render`)
//! - `raster`: enable PNG/JPG/PDF output via pure-Rust SVG rasterization/conversion

pub use merman_core::*;

#[cfg(feature = "render")]
pub mod render {
    pub use merman_render::model::LayoutedDiagram;
    pub use merman_render::svg::{SvgRenderOptions, foreign_object_label_fallback_svg_text};
    pub use merman_render::text::{
        DeterministicTextMeasurer, TextMeasurer, VendoredFontMetricsTextMeasurer,
    };
    pub use merman_render::{LayoutOptions, layout_parsed};

    #[cfg(feature = "raster")]
    pub mod raster;

    #[derive(Debug, thiserror::Error)]
    pub enum HeadlessError {
        #[error(transparent)]
        Parse(#[from] merman_core::Error),
        #[error(transparent)]
        Render(#[from] merman_render::Error),
    }

    pub type Result<T> = std::result::Result<T, HeadlessError>;

    /// Converts an arbitrary string into a conservative SVG `id` token suitable for embedding
    /// multiple Mermaid diagrams in the same UI tree.
    ///
    /// Mermaid uses the root `<svg id="...">` value as a prefix for internal ids like
    /// `chart-title-<id>` and marker ids under `<defs>`. If you inline multiple SVGs with the same
    /// id, those internal ids may collide.
    ///
    /// This helper:
    /// - trims whitespace
    /// - replaces unsupported characters with `-`
    /// - ensures the id starts with an ASCII letter by prefixing `m-` when needed
    pub fn sanitize_svg_id(raw: &str) -> String {
        let raw = raw.trim();
        if raw.is_empty() {
            return "m-untitled".to_string();
        }

        let mut out = String::with_capacity(raw.len() + 4);
        for ch in raw.chars() {
            let ok = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ':' || ch == '.';
            out.push(if ok { ch } else { '-' });
        }

        let starts_ok = out.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        if !starts_ok {
            out.insert_str(0, "m-");
        }

        while out.contains("--") {
            out = out.replace("--", "-");
        }
        let out = out.trim_matches('-');
        if out.is_empty() || out == "m" {
            return "m-untitled".to_string();
        }
        out.to_string()
    }

    /// Synchronous layout helper (executor-free).
    pub fn layout_diagram_sync(
        engine: &merman_core::Engine,
        text: &str,
        parse_options: merman_core::ParseOptions,
        layout_options: &LayoutOptions,
    ) -> Result<Option<LayoutedDiagram>> {
        let Some(parsed) = engine.parse_diagram_sync(text, parse_options)? else {
            return Ok(None);
        };
        Ok(Some(merman_render::layout_parsed(&parsed, layout_options)?))
    }

    /// Returns layout defaults intended for UI integrations that render headless SVG.
    ///
    /// This is a convenience wrapper around [`LayoutOptions::headless_svg_defaults`].
    pub fn headless_layout_options() -> LayoutOptions {
        LayoutOptions::headless_svg_defaults()
    }

    pub async fn layout_diagram(
        engine: &merman_core::Engine,
        text: &str,
        parse_options: merman_core::ParseOptions,
        layout_options: &LayoutOptions,
    ) -> Result<Option<LayoutedDiagram>> {
        layout_diagram_sync(engine, text, parse_options, layout_options)
    }

    pub fn render_layouted_svg(
        diagram: &LayoutedDiagram,
        measurer: &dyn TextMeasurer,
        svg_options: &SvgRenderOptions,
    ) -> Result<String> {
        Ok(merman_render::svg::render_layouted_svg(
            diagram,
            measurer,
            svg_options,
        )?)
    }

    /// Synchronous SVG render helper (executor-free).
    pub fn render_svg_sync(
        engine: &merman_core::Engine,
        text: &str,
        parse_options: merman_core::ParseOptions,
        layout_options: &LayoutOptions,
        svg_options: &SvgRenderOptions,
    ) -> Result<Option<String>> {
        let Some(diagram) = layout_diagram_sync(engine, text, parse_options, layout_options)?
        else {
            return Ok(None);
        };
        let svg =
            render_layouted_svg(&diagram, layout_options.text_measurer.as_ref(), svg_options)?;
        Ok(Some(svg))
    }

    pub async fn render_svg(
        engine: &merman_core::Engine,
        text: &str,
        parse_options: merman_core::ParseOptions,
        layout_options: &LayoutOptions,
        svg_options: &SvgRenderOptions,
    ) -> Result<Option<String>> {
        render_svg_sync(engine, text, parse_options, layout_options, svg_options)
    }

    /// Convenience wrapper that bundles an [`Engine`] and common options for headless rendering.
    ///
    /// This is intended for UI integrations where passing 4-5 separate parameters per call is
    /// noisy. It stays runtime-agnostic: all work is CPU-bound and does not perform I/O.
    #[derive(Clone)]
    pub struct HeadlessRenderer {
        pub engine: merman_core::Engine,
        pub parse: merman_core::ParseOptions,
        pub layout: LayoutOptions,
        pub svg: SvgRenderOptions,
    }

    impl Default for HeadlessRenderer {
        fn default() -> Self {
            Self {
                engine: merman_core::Engine::new(),
                parse: merman_core::ParseOptions::default(),
                layout: LayoutOptions::headless_svg_defaults(),
                svg: SvgRenderOptions::default(),
            }
        }
    }

    impl HeadlessRenderer {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_site_config(mut self, site_config: merman_core::MermaidConfig) -> Self {
            self.engine = self.engine.with_site_config(site_config);
            self
        }

        pub fn parse_metadata_sync(
            &self,
            text: &str,
        ) -> Result<Option<merman_core::ParseMetadata>> {
            Ok(self.engine.parse_metadata_sync(text, self.parse)?)
        }

        pub fn parse_diagram_sync(&self, text: &str) -> Result<Option<merman_core::ParsedDiagram>> {
            Ok(self.engine.parse_diagram_sync(text, self.parse)?)
        }

        pub fn layout_diagram_sync(&self, text: &str) -> Result<Option<LayoutedDiagram>> {
            layout_diagram_sync(&self.engine, text, self.parse, &self.layout)
        }

        pub fn render_svg_sync(&self, text: &str) -> Result<Option<String>> {
            render_svg_sync(&self.engine, text, self.parse, &self.layout, &self.svg)
        }

        /// Renders SVG and applies a best-effort readability fallback for `<foreignObject>` labels.
        ///
        /// Many headless SVG renderers and rasterizers do not fully support HTML inside
        /// `<foreignObject>`. This helper overlays extracted label text as `<text>/<tspan>` so
        /// consumers can still display something readable.
        pub fn render_svg_readable_sync(&self, text: &str) -> Result<Option<String>> {
            let Some(svg) = self.render_svg_sync(text)? else {
                return Ok(None);
            };
            Ok(Some(foreign_object_label_fallback_svg_text(&svg)))
        }

        pub fn render_svg_readable_sync_with_diagram_id(
            &self,
            text: &str,
            diagram_id: &str,
        ) -> Result<Option<String>> {
            let Some(svg) = self.render_svg_sync_with_diagram_id(text, diagram_id)? else {
                return Ok(None);
            };
            Ok(Some(foreign_object_label_fallback_svg_text(&svg)))
        }

        pub fn render_svg_sync_with(
            &self,
            text: &str,
            svg: &SvgRenderOptions,
        ) -> Result<Option<String>> {
            render_svg_sync(&self.engine, text, self.parse, &self.layout, svg)
        }

        pub fn render_svg_sync_with_diagram_id(
            &self,
            text: &str,
            diagram_id: &str,
        ) -> Result<Option<String>> {
            let mut svg = self.svg.clone();
            svg.diagram_id = Some(sanitize_svg_id(diagram_id));
            self.render_svg_sync_with(text, &svg)
        }

        #[cfg(feature = "raster")]
        pub fn render_png_sync(
            &self,
            text: &str,
            raster: &raster::RasterOptions,
        ) -> raster::Result<Option<Vec<u8>>> {
            raster::render_png_sync(
                &self.engine,
                text,
                self.parse,
                &self.layout,
                &self.svg,
                raster,
            )
        }

        #[cfg(feature = "raster")]
        pub fn render_jpeg_sync(
            &self,
            text: &str,
            raster: &raster::RasterOptions,
        ) -> raster::Result<Option<Vec<u8>>> {
            raster::render_jpeg_sync(
                &self.engine,
                text,
                self.parse,
                &self.layout,
                &self.svg,
                raster,
            )
        }

        #[cfg(feature = "raster")]
        pub fn render_pdf_sync(&self, text: &str) -> raster::Result<Option<Vec<u8>>> {
            raster::render_pdf_sync(&self.engine, text, self.parse, &self.layout, &self.svg)
        }
    }
}
