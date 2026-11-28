use kurbo::{Point, Rect, Shape};
use merman_model::OperationPhase;
use merman_render_core::{
    Error, Result,
    resources::OperationWorkMeter,
    rough_svg::{controlled, ops_to_compact_svg_path_d},
    svg_bounds::svg_path_bounds_from_d,
    svg_output::SvgOutput,
};
use roughr::core::{OpType, OptionsBuilder, RoughRandomness};

pub(super) struct HandDrawn<'a> {
    pub(super) work_meter: &'a OperationWorkMeter,
    pub(super) randomness: &'a RoughRandomness,
}

pub(super) struct Outline {
    pub(super) d: String,
    pub(super) bounds: Rect,
}

impl HandDrawn<'_> {
    pub(super) fn outline(&self, rect: Rect, radius: f64) -> Result<Outline> {
        let base = if radius > 0.0 {
            rect.to_rounded_rect(radius).to_path(0.1)
        } else {
            rect.to_path(0.1)
        };
        // Pin the joins and keep straight sides within half a pixel of their route contacts.
        let mut options = OptionsBuilder::default()
            .randomness(self.randomness.clone())
            .roughness(0.7)
            .bowing(0.0)
            .max_randomness_offset(0.9)
            .preserve_vertices(true)
            .disable_multi_stroke(true)
            .build()
            .expect("valid ER hand-drawn outline options");
        let d = controlled::generate(self.work_meter, OperationPhase::Emit, |control| {
            let segments =
                roughr::points_on_path::normalized_path_with_control(&base.to_svg(), control)?;
            let mut outline = roughr::renderer::svg_normalized_segments_with_control::<f64>(
                &segments,
                &mut options,
                control,
            )?;
            // Use the same closed contour to fill, clip rows and stroke the table frame.
            let mut first = true;
            outline
                .ops
                .retain(|op| std::mem::take(&mut first) || op.op != OpType::Move);
            let mut d = ops_to_compact_svg_path_d(&outline);
            d.push('Z');
            Ok(d)
        })?;
        self.work_meter.charge_emit_work(d.len())?;
        let bounds = svg_path_bounds_from_d(&d).ok_or_else(|| Error::InvalidModel {
            message: "ER entity has no hand-drawn paint bounds".into(),
        })?;
        Ok(Outline {
            d,
            bounds: Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
        })
    }

    pub(super) fn write_divider(
        &self,
        out: &mut impl SvgOutput,
        from: Point,
        to: Point,
    ) -> Result<()> {
        let mut options = OptionsBuilder::default()
            .randomness(self.randomness.clone())
            .roughness(0.7)
            .preserve_vertices(true)
            .build()
            .expect("valid ER hand-drawn divider options");
        let stroke = controlled::generate(self.work_meter, OperationPhase::Emit, |control| {
            roughr::renderer::line_with_control(from.x, from.y, to.x, to.y, &mut options, control)
        })?;
        out.push_str(&ops_to_compact_svg_path_d(&stroke));
        out.checkpoint()
    }
}
