use std::cell::RefCell;

use crate::__private::{NativeSvgFilterApplication, NativeSvgFilterReceipt, NativeSvgShadowStage};

use crate::diagram_theme::{SvgFilterRegion, SvgShadowEffect};

#[derive(Debug, Default)]
struct SvgShadowEvidenceState {
    applications: Vec<NativeSvgFilterApplication>,
    invalid: bool,
}

/// Records drop-shadow applications only after a family writer appended both the definition and
/// the matching `filter` reference.
#[derive(Debug, Default)]
pub(crate) struct SvgShadowEvidenceRecorder {
    state: RefCell<SvgShadowEvidenceState>,
}

impl SvgShadowEvidenceRecorder {
    pub(crate) fn record_application(
        &self,
        effect: &SvgShadowEffect,
        scoped_filter_id: &str,
        region: SvgFilterRegion,
    ) {
        let stages = effect
            .stages()
            .iter()
            .map(|stage| {
                NativeSvgShadowStage::new(
                    stage.input,
                    [stage.offset_x, stage.offset_y],
                    [stage.std_deviation; 2],
                    &stage.color.as_css(),
                )
            })
            .collect::<Option<Vec<_>>>();
        let application = stages.and_then(|stages| {
            NativeSvgFilterApplication::new(
                scoped_filter_id,
                region.as_array(),
                effect.color_space(),
                stages,
                1,
            )
        });
        let Some(application) = application else {
            self.state.borrow_mut().invalid = true;
            return;
        };
        self.state.borrow_mut().applications.push(application);
    }

    pub(crate) fn finish(&self) -> Option<NativeSvgFilterReceipt> {
        let mut state = self.state.borrow_mut();
        let applications = std::mem::take(&mut state.applications);
        if state.invalid {
            return None;
        }
        let had_applications = !applications.is_empty();
        let receipt = NativeSvgFilterReceipt::from_applications(applications);
        if had_applications && receipt.is_none() {
            state.invalid = true;
        }
        receipt
    }
}

#[cfg(test)]
mod tests {
    use crate::diagram_theme::SvgFilterRegion;
    use crate::diagram_theme::{EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue};

    use super::*;

    fn effect(std_deviation: f32) -> SvgShadowEffect {
        SvgShadowEffect::from_graph(
            &EffectGraph::new(
                "hard-shadow",
                [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 5.0,
                    offset_y: 5.0,
                    blur_radius: std_deviation,
                    spread: 0.0,
                    color: ThemeColorValue::parse("#111827").unwrap(),
                }],
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn receipt_requires_unique_actual_filter_applications() {
        let effect = effect(0.0);
        let region =
            SvgFilterRegion::try_bounded(-0.02, -0.05, 1.14, 1.4).expect("valid effect region");
        let recorder = SvgShadowEvidenceRecorder::default();
        assert!(recorder.finish().is_none());

        recorder.record_application(
            &effect,
            "diagram-state-ready-theme-effect-hard-shadow",
            region,
        );
        recorder.record_application(
            &effect,
            "diagram-state-done-theme-effect-hard-shadow",
            region,
        );
        let receipt = recorder.finish().expect("complete receipt");
        assert_eq!(receipt.drop_shadow_count(), 2);
        assert_eq!(receipt.reference_count(), 2);
        assert!(recorder.finish().is_none());
    }

    #[test]
    fn duplicate_filter_ids_invalidate_the_receipt() {
        let effect = effect(0.0);
        let region =
            SvgFilterRegion::try_bounded(-0.02, -0.05, 1.14, 1.4).expect("valid effect region");
        let recorder = SvgShadowEvidenceRecorder::default();
        recorder.record_application(&effect, "diagram-state-theme-effect-hard-shadow", region);
        recorder.record_application(&effect, "diagram-state-theme-effect-hard-shadow", region);

        assert!(recorder.finish().is_none());
        recorder.record_application(
            &effect,
            "diagram-state-other-theme-effect-hard-shadow",
            region,
        );
        assert!(recorder.finish().is_none());
    }

    #[test]
    fn receipt_order_is_independent_of_application_order() {
        let effect = effect(0.0);
        let region =
            SvgFilterRegion::try_bounded(-0.02, -0.05, 1.14, 1.4).expect("valid effect region");
        let forward = SvgShadowEvidenceRecorder::default();
        forward.record_application(&effect, "alpha", region);
        forward.record_application(&effect, "beta", region);
        let reverse = SvgShadowEvidenceRecorder::default();
        reverse.record_application(&effect, "beta", region);
        reverse.record_application(&effect, "alpha", region);

        assert_eq!(forward.finish(), reverse.finish());
    }

    #[test]
    fn receipt_is_sensitive_to_the_emitted_standard_deviation() {
        let region = SvgFilterRegion::try_bounded(-0.66, -1.65, 2.32, 4.3)
            .expect("valid soft-shadow region");
        let hard = SvgShadowEvidenceRecorder::default();
        hard.record_application(&effect(0.0), "drop-shadow", region);
        let soft = SvgShadowEvidenceRecorder::default();
        soft.record_application(&effect(8.0), "drop-shadow", region);

        assert_ne!(hard.finish(), soft.finish());
    }
}
