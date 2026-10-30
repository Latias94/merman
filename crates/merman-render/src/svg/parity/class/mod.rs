use super::*;

// Class diagram SVG renderer implementation (split from parity.rs).

type Rect = crate::geom::Box2;

mod bounds;

mod context;
use context::{ClassRenderDetails, ClassRenderLookups, emit_class_render_timing};

mod css;
use css::write_class_css;

mod defs;
use defs::{class_marker_paint_bindings, class_markers};

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
pub(super) use render::render_class_diagram_svg_model_with_config;
