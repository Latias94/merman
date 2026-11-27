//! Layout preparation, geometry, and SVG drawing for Sequence and ZenUML diagrams.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-sequence")]
#[doc(hidden)]
pub mod activation;

#[cfg(feature = "diagram-sequence")]
#[doc(hidden)]
pub mod actor;

#[cfg(feature = "diagram-sequence")]
#[doc(hidden)]
pub mod frame;

#[cfg(feature = "diagram-sequence")]
#[doc(hidden)]
pub mod model;

#[cfg(feature = "diagram-sequence")]
#[doc(hidden)]
pub mod preparation;

#[cfg(feature = "diagram-sequence")]
#[doc(hidden)]
pub mod rect_paint;

#[cfg(all(feature = "diagram-sequence", feature = "svg"))]
#[doc(hidden)]
pub mod svg;

#[cfg(feature = "diagram-zenuml")]
#[doc(hidden)]
pub mod zenuml;
