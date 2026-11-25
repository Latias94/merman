//! Layout, geometry, and presentation for Merman's chart families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-pie")]
#[doc(hidden)]
pub mod pie;

#[cfg(feature = "diagram-quadrant-chart")]
#[doc(hidden)]
pub mod quadrantchart;

#[cfg(feature = "diagram-radar")]
#[doc(hidden)]
pub mod radar;

#[cfg(feature = "diagram-sankey")]
#[doc(hidden)]
pub mod sankey;

#[cfg(feature = "diagram-treemap")]
#[doc(hidden)]
pub mod treemap;

#[cfg(feature = "diagram-venn")]
#[doc(hidden)]
pub mod venn;

#[cfg(feature = "diagram-wardley")]
#[doc(hidden)]
pub mod wardley;

#[cfg(feature = "diagram-xychart")]
#[doc(hidden)]
pub mod xychart;
