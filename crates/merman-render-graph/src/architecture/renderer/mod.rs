use merman_render_core::text::TextStyle;

mod css_binding;
#[cfg(feature = "layout-cytoscape")]
mod fcose;
pub mod labels;
pub mod metrics;
mod native_theme;
mod svg;
mod theme;

pub use css_binding::ArchitectureCssBinding;
#[cfg(feature = "layout-cytoscape")]
pub use fcose::layout_architecture_diagram_typed;
pub use native_theme::ArchitectureNativeStyle;
#[cfg(feature = "layout-shoal")]
pub use svg::{
    ArchitectureCanonicalEdge, ArchitectureCanonicalGroup, ArchitectureCanonicalSvg,
    ArchitecturePlacedText,
};
pub use svg::{ArchitectureSvgContext, render_architecture_diagram_svg_typed_with_config};
pub use theme::{ArchitectureGroupThemePlan, ArchitectureTypographyThemePlan};

const CYTOSCAPE_DEFAULT_FONT_FAMILY: &str = "Helvetica Neue,Helvetica,sans-serif";

fn architecture_cytoscape_text_style(font_size_px: f64) -> TextStyle {
    TextStyle {
        // Mermaid sets only `font-size` on Architecture nodes, so Cytoscape retains its own
        // default canvas font family rather than inheriting Mermaid's root Trebuchet stack.
        font_family: Some(CYTOSCAPE_DEFAULT_FONT_FAMILY.to_string()),
        font_size: font_size_px,
        font_weight: None,
        font_style: None,
    }
}

#[cfg(test)]
fn presentation_config(value: serde_json::Value) -> merman_model::presentation::PresentationConfig {
    use merman_model::resources::{InputResourcePolicy, ResourceProfile};
    use merman_model::{MermaidConfig, OperationControl, presentation::PresentationConfig};

    PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(value),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[cfg(test)]
fn with_test_svg_id<T>(
    id: &str,
    run: impl FnOnce(
        merman_render_core::svg_css::SvgId<'_>,
        &merman_render_core::resources::OperationWorkMeter,
    ) -> T,
) -> T {
    use merman_render_core::{
        resources::{OperationWorkMeter, RenderResourcePolicy},
        svg_css::{SvgDiagramIdProjection, SvgId},
    };

    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    let projection = SvgDiagramIdProjection::new(&meter);
    let result = run(SvgId::new(id).unwrap().with_projection(&projection), &meter);
    projection.finish().unwrap();
    result
}
