//! Class measurement projection for Shoal.

#[cfg(feature = "layout-shoal")]
mod shoal;
#[cfg(feature = "layout-shoal")]
pub(super) use shoal::measure_for_shoal;
