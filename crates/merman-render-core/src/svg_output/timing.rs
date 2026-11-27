use merman_model::runtime::{OperationTimer, OperationTiming};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RenderTiming {
    authority: Option<OperationTiming>,
}

impl RenderTiming {
    pub const fn disabled() -> Self {
        Self { authority: None }
    }

    pub const fn enabled(authority: OperationTiming) -> Self {
        Self {
            authority: Some(authority),
        }
    }

    pub const fn is_enabled(self) -> bool {
        self.authority.is_some()
    }

    pub fn start(self) -> Option<OperationTimer> {
        self.authority.map(OperationTiming::start)
    }

    pub fn section<'duration>(
        self,
        dst: &'duration mut Duration,
    ) -> Option<TimingGuard<'duration>> {
        self.start().map(|timer| TimingGuard {
            dst,
            timer: Some(timer),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct RenderTimings {
    pub total: Duration,
    pub deserialize_model: Duration,
    pub build_ctx: Duration,
    pub viewbox: Duration,
    pub render_svg: Duration,
    pub finalize_svg: Duration,
}

#[derive(Debug)]
pub struct TimingGuard<'a> {
    dst: &'a mut Duration,
    timer: Option<OperationTimer>,
}

impl Drop for TimingGuard<'_> {
    fn drop(&mut self) {
        let timer = self
            .timer
            .take()
            .expect("timing guard owns one timer until drop");
        *self.dst += timer.elapsed();
    }
}
