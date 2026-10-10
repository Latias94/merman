//! Control frames and keyword polygons share lowering, but keep separate terminal evidence.

use super::super::*;
use crate::diagram_theme::{SvgFilterRegion, SvgShadowEffect, SvgShadowEvidenceRecorder};
use crate::sequence::SequenceControlThemeReceipt;
use std::cell::{Cell, RefCell};

pub(super) struct SequenceControlPaint<'a> {
    pub(super) receipt: &'a SequenceControlThemeReceipt,
    effect: Option<SvgShadowEffect>,
    width: Option<f32>,
    baseline_width: f64,
    target: crate::diagram_theme::ThemeTarget,
    options: &'a SvgExecution<'a>,
    emitted: Cell<usize>,
    pub(super) bounds: RefCell<Option<Bounds>>,
}

impl<'a> SequenceControlPaint<'a> {
    pub(super) fn new(
        receipt: &'a SequenceControlThemeReceipt,
        effect: Option<SvgShadowEffect>,
        width: Option<f32>,
        target: crate::diagram_theme::ThemeTarget,
        options: &'a SvgExecution<'a>,
    ) -> Self {
        Self {
            receipt,
            effect,
            width,
            baseline_width: if target == crate::diagram_theme::ThemeTarget::Loop {
                2.0
            } else {
                1.0
            },
            target,
            options,
            emitted: Cell::new(0),
            bounds: RefCell::new(None),
        }
    }

    pub(super) fn needs_bounds(&self) -> bool {
        self.effect.is_some() || self.width.is_some()
    }
    pub(super) fn len(&self) -> usize {
        self.emitted.get()
    }

    pub(super) fn begin_terminal(
        &self,
        out: &mut impl SvgOutput,
        coordinates: [f64; 4],
    ) -> Result<Option<(String, SvgFilterRegion)>> {
        self.receipt.record_surface_candidate();
        if !self.needs_bounds() {
            return Ok(None);
        }
        let [x1, y1, x2, y2] = coordinates.map(crate::number_format::canonicalize_number);
        // Keyword polygons can have acute joins; retain SVG's default miter envelope.
        // Separate frame lines only need their half-stroke envelope.
        let half = self.width.map(f64::from).unwrap_or(self.baseline_width) / 2.0
            * if self.target == crate::diagram_theme::ThemeTarget::LoopLabelBackground {
                4.0
            } else {
                1.0
            };
        let mut bounds = Bounds {
            min_x: x1.min(x2) - half,
            min_y: y1.min(y2) - half,
            max_x: x1.max(x2) + half,
            max_y: y1.max(y2) + half,
        };
        let mut application = None;
        if let Some(effect) = &self.effect {
            self.options
                .work_meter()
                .charge(effect.stages().len().saturating_mul(3))?;
            if let Some(shadow) = effect.materialize_user_space(
                &self.options.theme_resource_policy(),
                bounds.min_x,
                bounds.min_y,
                bounds.max_x,
                bounds.max_y,
                crate::diagram_theme::EffectOutsets::default(),
            )? {
                let region = shadow.region();
                let [x, y, w, h] = region.as_array().map(f64::from);
                bounds = Bounds {
                    min_x: x,
                    min_y: y,
                    max_x: x + w,
                    max_y: y + h,
                };
                let id = format!(
                    "{}-{}-{}-theme-effect-{}",
                    self.options.diagram_id_or("merman"),
                    self.target.id(),
                    self.emitted.get(),
                    effect.id()
                );
                super::super::shadow::write_theme_shadow_application(out, &id, effect, region);
                application = Some((id, region));
            } else {
                self.receipt.effect_unhandled.set(true);
            }
        }
        let mut total = self.bounds.borrow_mut();
        if let Some(total) = total.as_mut() {
            total.min_x = total.min_x.min(bounds.min_x);
            total.min_y = total.min_y.min(bounds.min_y);
            total.max_x = total.max_x.max(bounds.max_x);
            total.max_y = total.max_y.max(bounds.max_y);
        } else {
            *total = Some(bounds);
        }
        out.checkpoint()?;
        Ok(application)
    }

    pub(super) fn write_attributes(
        &self,
        out: &mut impl SvgOutput,
        application: Option<&(String, SvgFilterRegion)>,
        separator: bool,
    ) {
        if self.width.is_none() && application.is_none() {
            if separator {
                out.push_str(r#" style="stroke-dasharray: 3, 3;""#);
            }
            return;
        }
        let mut style = if separator {
            "stroke-dasharray: 3, 3;".to_owned()
        } else {
            String::new()
        };
        if let Some(width) = self.width {
            let _ = write!(style, "stroke-width:{}px;", fmt(f64::from(width)));
        }
        if let Some((id, _)) = application {
            let filter = format!("url(#{id})");
            let _ = write!(out, r#" filter="{}""#, escape_attr(&filter));
            // The keyword stylesheet also declares filter, so the terminal must own its override.
            let _ = write!(style, "filter:{filter};");
        }
        if !style.is_empty() {
            let _ = write!(out, r#" style="{}""#, escape_attr(&style));
        }
    }

    pub(super) fn finish_terminal(
        &self,
        application: Option<&(String, SvgFilterRegion)>,
        evidence: &SvgShadowEvidenceRecorder,
    ) {
        self.receipt.record_surface_emission();
        if let Some((id, region)) = application {
            evidence.record_application(
                self.effect.as_ref().expect("materialized control effect"),
                id,
                *region,
            );
            self.emitted.set(self.emitted.get().saturating_add(1));
            self.receipt.record_effect_emission();
        } else if self.receipt.effect_cleared {
            self.receipt.record_effect_emission();
        }
    }
}
