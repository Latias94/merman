//! Shape vocabulary and geometry shared by the flow diagram renderers.

#![forbid(unsafe_code)]

#[doc(hidden)]
pub mod bpmn;
#[cfg(feature = "layout-dagre")]
#[doc(hidden)]
pub mod dagre;
#[doc(hidden)]
pub mod shapes;
