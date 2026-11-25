#![cfg(not(feature = "wasm-host"))]

use std::process::Command;

#[test]
fn default_xtask_reports_how_to_enable_each_wasm_host_command() {
    for command in [
        "profile-budget",
        "build-typst-package",
        "typst-package-smoke",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args([command, "--help"])
            .output()
            .expect("run xtask");
        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
        let recipe = format!("cargo run -p xtask --features wasm-host -- {command}");
        assert!(stderr.contains(&recipe), "{command} diagnostic: {stderr}");
    }
}
