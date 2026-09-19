use super::super::*;
use crate::diagram_theme::{
    EffectOutsets, ResolvedThemeEffect, SvgFilterRegion, SvgShadowEffect,
    SvgShadowEvidenceRecorder, ThemeResourcePolicy,
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
    Alphabetic,
}

impl<'a> SequenceTextShadow<'a> {
    pub(super) fn resolve(
        options: &'a SvgExecution<'_>,
        role: SequenceTypographyRole,
        typography: &SequenceResolvedTypography,
        receipt: &mut SequenceTypographyThemeReceipt,
    ) -> Self {
        let mut effect = None;
        let mut cleared = false;
        if let Some(theme) = options.resolved_theme() {
            let resolution = typography
                .resolved_style()
                .map(|s| s.effect_resolution().clone())
                .unwrap_or_default();
            if let Some(resolved) = theme.resolve_effect(role.target(), &resolution) {
                let mut binding = false;
                match resolved {
                    ResolvedThemeEffect::ClearedByRule => cleared = true,
                    ResolvedThemeEffect::Rule { graph } => {
                        effect = graph.and_then(SvgShadowEffect::from_graph);
                    }
                    ResolvedThemeEffect::Binding { graph, .. } => {
                        binding = true;
                        effect = graph.and_then(SvgShadowEffect::from_graph);
                    }
                }
                receipt.configure_text_effect(role, binding, !cleared && effect.is_none());
            }
        }
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
            TextShadowBaseline::Middle => y,
            TextShadowBaseline::Alphabetic => y - height / 2.0,
        };
        let Some(materialized) = effect.materialize_user_space(
            &self.resources,
            x - width / 2.0,
            center_y - height / 2.0,
            x + width / 2.0,
            center_y + height / 2.0,
            EffectOutsets {
                top: em,
                right: em,
                bottom: em,
                left: em,
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
        if let Some(application) = application {
            let effect = self
                .effect
                .as_ref()
                .expect("a materialized text shadow has an effect");
            evidence.record_application(effect, &application.id, application.region);
            self.applications.set(self.len().saturating_add(1));
            let [x, y, w, h] = application.region.as_array().map(f64::from);
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
