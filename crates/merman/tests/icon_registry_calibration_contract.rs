#![cfg(feature = "svg")]

// Keep the calibration accounting contract in the ordinary integration-test graph so the
// workspace's default nextest gate catches drift in the measurement tool itself.
#[allow(dead_code)]
#[path = "../examples/icon_registry_calibration.rs"]
mod calibration;
