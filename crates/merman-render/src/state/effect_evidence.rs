use std::cell::RefCell;

use crate::__private::{NativeSvgFilterReceipt, NativeSvgHardShadow};

use super::{StateSvgEffect, StateSvgFilterRegion};

#[derive(Debug, Default)]
struct StateSvgEffectEvidenceState {
    applications: Vec<NativeSvgHardShadow>,
    invalid: bool,
}

/// Records hard-shadow applications only after the State writer appended both the definition and
/// the matching `filter` reference.
#[derive(Debug, Default)]
pub(crate) struct StateSvgEffectEvidenceRecorder {
    state: RefCell<StateSvgEffectEvidenceState>,
}

impl StateSvgEffectEvidenceRecorder {
    pub(crate) fn record_application(
        &self,
        effect: &StateSvgEffect,
        scoped_filter_id: &str,
        region: StateSvgFilterRegion,
    ) {
        let region = region.as_array();
        let Some(application) = NativeSvgHardShadow::new(
            scoped_filter_id,
            region,
            [effect.offset_x(), effect.offset_y()],
            &effect.color().as_css(),
            1,
        ) else {
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
        let receipt = NativeSvgFilterReceipt::from_hard_shadows(applications);
        if had_applications && receipt.is_none() {
            state.invalid = true;
        }
        receipt
    }
}

#[cfg(test)]
mod tests {
    use crate::diagram_theme::{EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue};
    use crate::state::StateSvgFilterRegion;

    use super::*;

    fn effect() -> StateSvgEffect {
        StateSvgEffect::from_graph_for_test(
            &EffectGraph::new(
                "hard-shadow",
                [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 5.0,
                    offset_y: 5.0,
                    blur_radius: 0.0,
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
        let effect = effect();
        let region = StateSvgFilterRegion::try_bounded(-0.02, -0.05, 1.14, 1.4)
            .expect("valid effect region");
        let recorder = StateSvgEffectEvidenceRecorder::default();
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
        assert_eq!(receipt.hard_shadow_count(), 2);
        assert_eq!(receipt.reference_count(), 2);
        assert!(recorder.finish().is_none());
    }

    #[test]
    fn duplicate_filter_ids_invalidate_the_receipt() {
        let effect = effect();
        let region = StateSvgFilterRegion::try_bounded(-0.02, -0.05, 1.14, 1.4)
            .expect("valid effect region");
        let recorder = StateSvgEffectEvidenceRecorder::default();
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
        let effect = effect();
        let region = StateSvgFilterRegion::try_bounded(-0.02, -0.05, 1.14, 1.4)
            .expect("valid effect region");
        let forward = StateSvgEffectEvidenceRecorder::default();
        forward.record_application(&effect, "alpha", region);
        forward.record_application(&effect, "beta", region);
        let reverse = StateSvgEffectEvidenceRecorder::default();
        reverse.record_application(&effect, "beta", region);
        reverse.record_application(&effect, "alpha", region);

        assert_eq!(forward.finish(), reverse.finish());
    }
}
