use crate::resources::OperationWorkMeter;
use merman_model::OperationPhase;
use roughr::work::{WorkControl, WorkError};

pub(in crate::svg::parity) struct RoughWorkControl<'a> {
    meter: &'a OperationWorkMeter,
    phase: OperationPhase,
}

impl WorkControl for RoughWorkControl<'_> {
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        let result = match self.phase {
            OperationPhase::Emit => self.meter.charge_emit_work(units),
            _ => self.meter.charge(units),
        };
        result.map_err(|_| WorkError::Interrupted)
    }
}

pub(in crate::svg::parity) fn generate<T>(
    meter: &OperationWorkMeter,
    phase: OperationPhase,
    generate: impl FnOnce(&mut RoughWorkControl<'_>) -> Result<T, WorkError>,
) -> crate::Result<T> {
    meter.checkpoint(phase)?;
    let result = generate(&mut RoughWorkControl { meter, phase });
    meter.checkpoint(phase)?;
    result.map_err(|error| match error {
        WorkError::ArithmeticOverflow => meter.arithmetic_overflow().into(),
        error => crate::Error::InvalidModel {
            message: format!("RoughJS generation failed: {error}"),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkError, RenderResourcePolicy, ResourceLimitId};
    use merman_model::OperationControl;
    use roughr::core::{FillStyle, OptionsBuilder, RoughJsSeed, RoughMathRandom, RoughRandomness};

    fn options() -> roughr::core::Options {
        OptionsBuilder::default()
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(0.0),
                RoughMathRandom::new(7),
            ))
            .fill(roughr::Srgba::new(0.2, 0.3, 0.4, 1.0))
            .fill_style(FillStyle::Hachure)
            .hachure_gap(5.0)
            .roughness(0.7)
            .build()
            .unwrap()
    }

    #[test]
    fn cancellation_inside_rough_generation_preserves_phase_and_sticky_cause() {
        for phase in [OperationPhase::Emit, OperationPhase::Layout] {
            for (primitive, checkpoints) in [(0, 8), (1, 128), (2, 32)] {
                let control = OperationControl::new();
                control.cancel_after_checkpoints(checkpoints);
                let meter = OperationWorkMeter::new_with_control(
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                    control,
                );
                let result = generate(&meter, phase, |work| {
                    let generator = roughr::generator::Generator::default();
                    match primitive {
                        0 => {
                            generator.circle_with_control(0.0, 0.0, 4096.0, &Some(options()), work)
                        }
                        1 => generator.rectangle_with_control(
                            0.0,
                            0.0,
                            4096.0,
                            4096.0,
                            &Some(options()),
                            work,
                        ),
                        _ => generator.path_with_control::<f64>(
                            "M0 0 C0 4096 4096 4096 4096 0 Z",
                            &Some(options()),
                            work,
                        ),
                    }
                    .map(|_| ())
                });
                let error = result.expect_err("cancellation must be observed inside generation");
                assert!(
                    matches!(error, crate::Error::Cancelled(ref details) if details.phase == phase)
                );
                let used = meter.used();
                assert!(used > 0);
                assert!(
                    matches!(meter.checkpoint(phase), Err(OperationWorkError::Cancelled(details)) if details.phase == phase)
                );
                let repeated = generate(&meter, phase, |_| -> Result<(), WorkError> {
                    panic!("a terminal operation must not resume RoughR generation")
                })
                .unwrap_err();
                assert!(
                    matches!(repeated, crate::Error::Cancelled(details) if details.phase == phase)
                );
                assert_eq!(meter.used(), used);
            }
        }
    }

    #[test]
    fn rough_generation_uses_the_existing_cumulative_work_meter() {
        let meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 512)
                .unwrap(),
        );
        meter.charge(100).unwrap();
        let result = generate(&meter, OperationPhase::Emit, |work| {
            roughr::generator::Generator::default()
                .rectangle_with_control(0.0, 0.0, 1000.0, 1000.0, &Some(options()), work)
                .map(|_| ())
        });
        assert!(
            matches!(result, Err(crate::Error::ResourceLimitExceeded(details)) if details.limit == "max_layout_work_units")
        );
        assert!(meter.used() > 100 && meter.used() <= 512);
        assert!(
            matches!(meter.control().terminal_checkpoint_at(OperationPhase::Layout),
            Err(merman_model::OperationLedgerError::ResourceLimitExceeded(details)) if details.phase == OperationPhase::Emit)
        );
    }
}
