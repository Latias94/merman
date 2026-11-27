use merman_render_core::number_format::canonical_number as fmt;
use merman_render_core::root_svg;
use merman_render_core::svg_css::{SvgDiagramIdValue, SvgId as SvgDiagramId};
use merman_render_core::svg_output::{BoundedSvgOutput, SvgOutput};
use merman_render_core::text::{TextMeasurer, TextStyle};
use merman_render_core::xml::{escape_attr_display, escape_xml_display, escape_xml_into};
use merman_render_core::{Bounds, Result};
use std::fmt::Write as _;

fn fmt_into(out: &mut impl SvgOutput, value: f64) {
    let _ = write!(out, "{}", fmt(value));
}
use super::ClassDiagramLayout;

// Class diagram SVG renderer implementation (split from parity.rs).

type Rect = super::Rect;

mod bounds;

mod context;
use context::{ClassRenderDetails, ClassRenderLookups, emit_class_render_timing};

mod css;
use css::write_class_css;

mod defs;
#[cfg(feature = "layout-shoal")]
pub use defs::class_canonical_marker;
use defs::class_markers;

mod edge;

mod groups;

mod interface;

mod label;

mod namespace;

mod node;

mod nodes;

mod note;

mod rough;

mod root;

mod viewbox;

type ClassSvgModel = merman_model::families::class::ClassDiagram;
type ClassSvgNode<'a> = merman_model::families::class::ClassNodeRenderView<'a>;
type ClassSvgRelation<'a> = merman_model::families::class::ClassRelationRenderView<'a>;
type ClassSvgNote<'a> = merman_model::families::class::ClassNoteRenderView<'a>;
type ClassSvgInterface<'a> = merman_model::families::class::ClassInterfaceRenderView<'a>;

mod render;
#[cfg(feature = "layout-shoal")]
pub use render::{ClassCanonicalEdge, ClassCanonicalRoutes};
pub use render::{ClassSvgContext, render_class_diagram_svg_model_with_config};
