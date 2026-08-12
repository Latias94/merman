use std::cell::RefCell;

use crate::__private::{NativeSvgFilterReceipt, NativeSvgHardShadow};

use super::StateSvgEffect;

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
    pub(crate) fn record_application(&self, effect: &StateSvgEffect, scoped_filter_id: &str) {
        let region = effect.region();
        let Some(application) = NativeSvgHardShadow::new(
            scoped_filter_id,
            [region.x, region.y, region.width, region.height],
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
        let state = self.state.borrow();
        if state.invalid {
            return None;
        }
        NativeSvgFilterReceipt::from_hard_shadows(state.applications.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::diagram_theme::{
        EffectGraph, EffectInput, EffectPrimitive, FilterRegion, ThemeColorValue,
    };

    use super::*;

    fn effect() -> StateSvgEffect {
        StateSvgEffect::from_graph_for_test(
            &EffectGraph::new(
                "hard-shadow",
                FilterRegion::bounded(-0.2, -0.2, 1.4, 1.4),
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
        let recorder = StateSvgEffectEvidenceRecorder::default();
        assert!(recorder.finish().is_none());

        recorder.record_application(&effect, "diagram-state-ready-theme-effect-hard-shadow");
        recorder.record_application(&effect, "diagram-state-done-theme-effect-hard-shadow");
        let receipt = recorder.finish().expect("complete receipt");
        assert_eq!(receipt.hard_shadow_count(), 2);
        assert_eq!(receipt.reference_count(), 2);
    }

    #[test]
    fn duplicate_filter_ids_invalidate_the_receipt() {
        let effect = effect();
        let recorder = StateSvgEffectEvidenceRecorder::default();
        recorder.record_application(&effect, "diagram-state-theme-effect-hard-shadow");
        recorder.record_application(&effect, "diagram-state-theme-effect-hard-shadow");

        assert!(recorder.finish().is_none());
    }
}
