#![cfg(all(
    feature = "complete-svg-elk",
    feature = "svg",
    feature = "layout-cytoscape",
    feature = "layout-elk",
    feature = "math"
))]

// Keep the resource calibration contract in the ordinary integration-test graph so the
// workspace's default nextest gate catches drift in the evidence tool itself.
#[allow(dead_code)]
#[path = "../examples/layout_work_calibration.rs"]
mod calibration;
