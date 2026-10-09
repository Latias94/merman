use super::super::*;
use crate::diagram_theme::{
    EffectOutsets, SvgFilterRegion, SvgShadowEffect, SvgShadowEvidenceRecorder, ThemeResourcePolicy,
};
use crate::sequence::{
    SequenceResolvedTypography, SequenceTextSurface, SequenceTypographyRole,
    SequenceTypographyThemeReceipt,
};
use std::cell::{Cell, RefCell};

/// A role's text-only shadow, materialized from the actual writer's final line.
/// It retains bounds and counts, not a second copy of every label or layout.
pub(super) struct SequenceTextShadow<'a> {
    effect: Option<SvgShadowEffect>,
    role: SequenceTypographyRole,
    cleared: bool,
    diagram_id: SvgDiagramId<'a>,
    resources: std::sync::Arc<ThemeResourcePolicy>,
    work_meter: &'a crate::resources::OperationWorkMeter,
    applications: Cell<usize>,
    pub(super) bounds: RefCell<Option<Bounds>>,
}

pub(super) struct SequenceTextShadowApplication {
    id: String,
    region: SvgFilterRegion,
    pub(super) filter: String,
}

#[derive(Clone, Copy)]
pub(super) enum TextShadowBaseline {
    NoteMiddle,
    Middle,
    MiddleStart,
    Alphabetic,
}

impl<'a> SequenceTextShadow<'a> {
    pub(super) fn from_prepared(
        options: &'a SvgExecution<'_>,
        role: SequenceTypographyRole,
        typography: &SequenceResolvedTypography,
        receipt: &mut SequenceTypographyThemeReceipt,
    ) -> Self {
        let prepared = typography.text_effect();
        prepared.configure_receipt(role, receipt);
        let effect = prepared.effect().cloned();
        let cleared = prepared.cleared();
        Self {
            effect,
            role,
            cleared,
            diagram_id: options.diagram_id_or("merman"),
            resources: options.theme_resource_policy(),
            work_meter: options.work_meter(),
            applications: Cell::new(0),
            bounds: RefCell::new(None),
        }
    }

    pub(super) fn needs_bounds(&self) -> bool {
        self.effect.is_some()
    }
    pub(super) fn is_paintless(&self, text: &str) -> bool {
        self.needs_bounds()
            && text
                .chars()
                .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{200B}'))
    }

    pub(super) fn len(&self) -> usize {
        self.applications.get()
    }

    /// These writers use middle anchors with distinct baselines. One em of
    /// additional paint space bounds the allocation, not arbitrary host font ink;
    /// native export independently checks actual glyph containment.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_definition(
        &self,
        out: &mut impl SvgOutput,
        text: &str,
        x: f64,
        y: f64,
        baseline: TextShadowBaseline,
        style: &TextStyle,
        measurer: &dyn TextMeasurer,
    ) -> Result<Option<SequenceTextShadowApplication>> {
        let Some(effect) = &self.effect else {
            return Ok(None);
        };
        self.work_meter.charge(
            text.len()
                .max(1)
                .saturating_add(effect.stages().len().saturating_mul(3)),
        )?;
        let width = measurer.measure_svg_tspan_text_bbox_width_px(text, style);
        let height = measurer.measure_svg_tspan_text_bbox_height_px(text, style);
        let em = style.font_size;
        if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
            return Ok(None);
        }
        // Serialize coordinates before materializing so bounds use the terminal's
        // rounded placement rather than an unobservable higher-precision position.
        let x = crate::number_format::canonicalize_number(x);
        let y = crate::number_format::canonicalize_number(y);
        let center_y = match baseline {
            TextShadowBaseline::NoteMiddle => y + em,
            TextShadowBaseline::Middle | TextShadowBaseline::MiddleStart => y,
            TextShadowBaseline::Alphabetic => y - height / 2.0,
        };
        let left = if matches!(baseline, TextShadowBaseline::MiddleStart) {
            x
        } else {
            x - width / 2.0
        };
        // A native fallback face can change advances across the whole line. Allocate one
        // additional measured line width on each side, rather than a fixed em allowance that
        // shrinks relative to longer labels. This is bounded allocation, not certified ink;
        // native export still verifies actual outlines and rejects underestimated metrics.
        let horizontal_reserve = width.max(em);
        let Some(materialized) = effect.materialize_user_space(
            &self.resources,
            left,
            center_y - height / 2.0,
            left + width,
            center_y + height / 2.0,
            EffectOutsets {
                top: em,
                right: horizontal_reserve,
                bottom: em,
                left: horizontal_reserve,
            },
        )?
        else {
            return Ok(None);
        };
        let region = materialized.region();
        let id = format!(
            "{}-{}-text-{}-theme-effect-{}",
            self.diagram_id,
            match self.role {
                SequenceTypographyRole::Note => "note",
                SequenceTypographyRole::Loop => "loop",
                SequenceTypographyRole::Actor => "actor",
                SequenceTypographyRole::Message => "message",
            },
            self.len(),
            effect.id()
        );
        let filter = super::super::shadow::write_theme_shadow_application(out, &id, effect, region);
        out.checkpoint()?;
        Ok(Some(SequenceTextShadowApplication { id, region, filter }))
    }

    pub(super) fn record_terminal(
        &self,
        application: Option<&SequenceTextShadowApplication>,
        paintless: bool,
        evidence: &SvgShadowEvidenceRecorder,
        receipt: &SequenceTypographyThemeReceipt,
        surface: SequenceTextSurface,
    ) {
        self.record_translated_terminal(application, paintless, evidence, receipt, surface, 0.0);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_translated_terminal(
        &self,
        application: Option<&SequenceTextShadowApplication>,
        paintless: bool,
        evidence: &SvgShadowEvidenceRecorder,
        receipt: &SequenceTypographyThemeReceipt,
        surface: SequenceTextSurface,
        translate_y: f64,
    ) {
        if let Some(application) = application {
            let effect = self
                .effect
                .as_ref()
                .expect("a materialized text shadow has an effect");
            evidence.record_application(effect, &application.id, application.region);
            self.applications.set(self.len().saturating_add(1));
            let [x, y, w, h] = application.region.as_array().map(f64::from);
            let y = y + translate_y;
            let mut total = self.bounds.borrow_mut();
            if let Some(total) = total.as_mut() {
                total.min_x = total.min_x.min(x);
                total.min_y = total.min_y.min(y);
                total.max_x = total.max_x.max(x + w);
                total.max_y = total.max_y.max(y + h);
            } else {
                *total = Some(Bounds {
                    min_x: x,
                    min_y: y,
                    max_x: x + w,
                    max_y: y + h,
                });
            }
            receipt.record_text_effect(surface, true);
        } else if self.cleared || (paintless && self.effect.is_some()) {
            receipt.record_text_effect(surface, false);
        }
    }
}

#[cfg(test)]
mod prepared_tests {
    use super::*;

    #[test]
    fn text_shadow_instances_keep_application_counts_and_bounds_local() {
        crate::svg::parity::with_test_svg_execution(
            crate::DiagramFamilyId::SEQUENCE,
            &crate::svg::SvgRenderOptions::default(),
            |execution| {
                let config = merman_core::MermaidConfig::default();
                let typography = crate::sequence::SequenceTypographyPlan::resolve(
                    &config,
                    None,
                    execution.work_meter(),
                )
                .unwrap();
                let mut first_receipt = SequenceTypographyThemeReceipt::from_plan(&typography);
                let mut second_receipt = SequenceTypographyThemeReceipt::from_plan(&typography);
                let first = SequenceTextShadow::from_prepared(
                    execution,
                    SequenceTypographyRole::Note,
                    typography.note(),
                    &mut first_receipt,
                );
                let second = SequenceTextShadow::from_prepared(
                    execution,
                    SequenceTypographyRole::Note,
                    typography.note(),
                    &mut second_receipt,
                );
                first.applications.set(1);
                *first.bounds.borrow_mut() = Some(Bounds {
                    min_x: 0.0,
                    min_y: 0.0,
                    max_x: 1.0,
                    max_y: 1.0,
                });
                assert_eq!(second.len(), 0);
                assert!(second.bounds.borrow().is_none());
            },
        );
    }
}
