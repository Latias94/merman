mod edges;
#[cfg(test)]
mod foreign_object;
mod geometry;
mod icons;
mod labels;
mod nodes;
mod render;
mod root;
mod settings;
#[cfg(all(test, feature = "layout-shoal"))]
mod shoal_tests;
mod viewport;

/// Non-billing terminal replay shared by Architecture SVG emission passes.
#[derive(Clone, Copy)]
pub(super) struct ArchitectureEmitCheckpoints<'a> {
    work_meter: &'a crate::resources::OperationWorkMeter,
}

impl<'a> ArchitectureEmitCheckpoints<'a> {
    pub(super) const fn new(work_meter: &'a crate::resources::OperationWorkMeter) -> Self {
        Self { work_meter }
    }

    pub(super) fn checkpoint(self) -> crate::Result<()> {
        self.work_meter
            .checkpoint(merman_model::OperationPhase::Emit)
            .map_err(Into::into)
    }
}

pub(super) use render::render_architecture_diagram_svg_typed_with_config;

#[cfg(feature = "layout-shoal")]
fn write_geometry_attrs(out: &mut impl super::SvgOutput, kind: &str, id: &str) {
    let _ = write!(out, r#" data-et="{kind}" data-id=""#);
    super::escape_attr_into(out, id);
    out.push('"');
}
