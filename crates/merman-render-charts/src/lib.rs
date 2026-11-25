//! Layout, geometry, and presentation for Merman's chart families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-pie")]
#[doc(hidden)]
pub mod pie;

#[cfg(feature = "diagram-xychart")]
#[doc(hidden)]
pub mod xychart;
