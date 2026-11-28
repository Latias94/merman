use crate::resources::{OperationWorkMeter, ResourceLimitPhase};
use crate::{Error, Result};
use merman_model::OperationPhase;

#[derive(Clone, Copy)]
pub(super) struct PostprocessWork<'a>(pub(super) &'a OperationWorkMeter);

impl PostprocessWork<'_> {
    pub(super) fn checkpoint(self) -> Result<()> {
        self.0.checkpoint(OperationPhase::Postprocess)?;
        Ok(())
    }

    pub(super) fn preflight_svg_byte_count(self, bytes: usize) -> Result<()> {
        self.0.preflight_svg_byte_count(
            bytes,
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Postprocess,
        )?;
        Ok(())
    }

    pub(super) fn svg_byte_count_overflow(self) -> Error {
        self.0
            .terminate_svg_byte_count_overflow(
                ResourceLimitPhase::SvgPostprocess,
                OperationPhase::Postprocess,
            )
            .into()
    }
}
