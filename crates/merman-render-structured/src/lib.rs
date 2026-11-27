//! Layout and SVG emission for Merman's structured diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-cynefin")]
#[doc(hidden)]
pub mod cynefin;

#[cfg(feature = "diagram-gantt")]
#[doc(hidden)]
pub mod gantt;

#[cfg(feature = "diagram-info")]
#[doc(hidden)]
pub mod info;
#[cfg(feature = "diagram-ishikawa")]
#[doc(hidden)]
pub mod ishikawa;
#[cfg(feature = "diagram-journey")]
#[doc(hidden)]
pub mod journey;
#[cfg(feature = "diagram-packet")]
#[doc(hidden)]
pub mod packet;
#[cfg(feature = "diagram-railroad")]
#[doc(hidden)]
pub mod railroad;

#[cfg(feature = "diagram-timeline")]
#[doc(hidden)]
pub mod timeline;
