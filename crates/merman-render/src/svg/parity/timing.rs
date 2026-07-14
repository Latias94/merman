use web_time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
pub(crate) struct RenderTimings {
    pub total: Duration,
    pub deserialize_model: Duration,
    pub build_ctx: Duration,
    pub viewbox: Duration,
    pub render_svg: Duration,
    pub finalize_svg: Duration,
}

#[derive(Debug)]
pub(crate) struct TimingGuard<'a> {
    dst: &'a mut Duration,
    start: Instant,
}

impl<'a> TimingGuard<'a> {
    pub(crate) fn new(dst: &'a mut Duration) -> Self {
        Self {
            dst,
            start: Instant::now(),
        }
    }
}

impl Drop for TimingGuard<'_> {
    fn drop(&mut self) {
        *self.dst += self.start.elapsed();
    }
}
