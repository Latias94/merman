//! Layout and SVG emission for Merman's structured diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-packet")]
#[doc(hidden)]
pub mod packet;
#[cfg(feature = "diagram-railroad")]
#[doc(hidden)]
pub mod railroad;
