#[cfg(not(target_arch = "wasm32"))]
pub(crate) type Duration = std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type Instant = std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub(crate) type Duration = std::time::Duration;

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Instant;

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub(crate) fn now() -> Self {
        Self
    }

    pub(crate) fn elapsed(self) -> Duration {
        let _ = self;
        Duration::ZERO
    }
}
