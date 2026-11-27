use std::sync::Arc;

use merman_model::{families::sequence::SequenceDiagramModel, presentation::PresentationConfig};
use merman_render_core::{
    resources::OperationWorkMeter, root_svg::RootViewportContext, svg_css::SvgId,
    text::TextMeasurer,
};
use merman_theme_contract::v2::Color;

use crate::preparation::SequencePreparedArtifact;

mod actor_effect;
mod actor_man;
mod actor_man_glyphs;
mod actor_popup;
mod actor_shapes;
mod actors;
mod arrowhead;
mod block_text;
mod blocks;
#[cfg(feature = "native-compact")]
mod compact;
mod control_paint;
mod css;
mod frames;
mod geometry;
mod interactions;
mod math_label;
mod messages;
mod model;
mod notes;
mod render;
mod root;
mod settings;

#[cfg(feature = "native-compact")]
pub use compact::{SequenceCompactError, SequenceCompactPolicy, render_compact};
pub use render::render_parity;

use crate::preparation::SequenceOperationCheckpoints as SequenceEmitCheckpoints;

/// Borrowed inputs for one emission of a paired Sequence artifact.
///
/// The viewport and diagram ID describe the same validated scope. The measurer and meter come
/// from the operation that owns the prepared artifact.
pub struct SequenceSvgRequest<'a> {
    pub prepared: &'a SequencePreparedArtifact,
    pub semantic: &'a SequenceDiagramModel,
    pub config: &'a PresentationConfig,
    pub measurer: &'a dyn TextMeasurer,
    pub diagram_id: SvgId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub work_meter: &'a Arc<OperationWorkMeter>,
    pub canvas_background: Option<Color>,
}

mod prelude {
    pub(super) use merman_render_core::{
        Bounds, Error, LayoutNode, Result,
        number_format::canonical_number as fmt,
        root_svg,
        svg_css::{SvgId as SvgDiagramId, scoped_drop_shadow, scoped_svg_url},
        svg_output::{BoundedSvgOutput, SvgOutput},
        text::{TextMeasurer, TextStyle},
        xml::{escape_attr, escape_attr_display, escape_xml, escape_xml_display},
    };
    pub(super) use std::fmt::Write;

    pub(super) use crate::model::SequenceDiagramLayout;
}

/// Escape a prepared text node without interpreting another entity layer.
fn escape_prepared_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    merman_render_core::xml::escape_xml_raw_into(&mut escaped, text);
    escaped
}

#[cfg(test)]
fn presentation_config(value: serde_json::Value) -> PresentationConfig {
    use merman_model::{
        MermaidConfig, OperationControl,
        resources::{InputResourcePolicy, ResourceProfile},
    };

    PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(value),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}
