#![forbid(unsafe_code)]

pub use merman_core::*;

#[cfg(feature = "render")]
pub mod render {
    pub use merman_render::model::LayoutedDiagram;
    pub use merman_render::svg::SvgRenderOptions;
    pub use merman_render::text::{
        DeterministicTextMeasurer, TextMeasurer, VendoredFontMetricsTextMeasurer,
    };
    pub use merman_render::{LayoutOptions, layout_parsed};

    #[derive(Debug, thiserror::Error)]
    pub enum HeadlessError {
        #[error(transparent)]
        Parse(#[from] merman_core::Error),
        #[error(transparent)]
        Render(#[from] merman_render::Error),
    }

    pub type Result<T> = std::result::Result<T, HeadlessError>;

    pub async fn layout_diagram(
        engine: &merman_core::Engine,
        text: &str,
        parse_options: merman_core::ParseOptions,
        layout_options: &LayoutOptions,
    ) -> Result<Option<LayoutedDiagram>> {
        let Some(parsed) = engine.parse_diagram(text, parse_options).await? else {
            return Ok(None);
        };
        Ok(Some(merman_render::layout_parsed(&parsed, layout_options)?))
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

    pub async fn render_svg(
        engine: &merman_core::Engine,
        text: &str,
        parse_options: merman_core::ParseOptions,
        layout_options: &LayoutOptions,
        svg_options: &SvgRenderOptions,
    ) -> Result<Option<String>> {
        let Some(diagram) = layout_diagram(engine, text, parse_options, layout_options).await?
        else {
            return Ok(None);
        };
        let svg =
            render_layouted_svg(&diagram, layout_options.text_measurer.as_ref(), svg_options)?;
        Ok(Some(svg))
    }
}
