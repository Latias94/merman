use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
use std::path::{Path, PathBuf};
use std::process::Command;

const CHILD_OUTPUT_ENV: &str = "MERMAN_DETERMINISTIC_SVG_CHILD_OUTPUT";
const SOURCE: &str = r#"---
config:
  look: handDrawn
  handDrawnSeed: 0
---
flowchart TD
  request[Request] --> response[Response]
"#;

fn render_deterministic_svg() -> String {
    let output = Renderer::new()
        .render(RenderRequest::svg(
            SOURCE,
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect("deterministic render succeeds");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("flowchart is detected");
    };
    svg.into_parts().0
}

fn child_output_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "merman-runtime-determinism-{}-{label}.svg",
        std::process::id()
    ))
}

fn run_child(path: &Path, time_zone: &str) {
    let status = Command::new(std::env::current_exe().expect("current test executable"))
        .arg("--exact")
        .arg("deterministic_svg_child")
        .arg("--nocapture")
        .env(CHILD_OUTPUT_ENV, path)
        .env("TZ", time_zone)
        .status()
        .expect("spawn deterministic render child");
    assert!(status.success(), "child failed for TZ={time_zone}");
}

#[test]
fn deterministic_svg_child() {
    let Some(path) = std::env::var_os(CHILD_OUTPUT_ENV) else {
        return;
    };
    std::fs::write(path, render_deterministic_svg()).expect("write child SVG evidence");
}

#[test]
fn deterministic_svg_is_identical_across_fresh_processes_with_system_adapters_compiled() {
    assert!(
        merman::runtime::compiled_system_adapter_ids()
            .contains(&merman::runtime::RuntimeCapability::SystemClock.id())
    );
    assert!(
        merman::runtime::compiled_system_adapter_ids()
            .contains(&merman::runtime::RuntimeCapability::SystemTimeZone.id())
    );
    assert!(
        merman::runtime::compiled_system_adapter_ids()
            .contains(&merman::runtime::RuntimeCapability::SystemRandom.id())
    );
    assert!(
        merman::runtime::compiled_system_adapter_ids()
            .contains(&merman::runtime::RuntimeCapability::SystemTiming.id())
    );

    let west = child_output_path("west");
    let east = child_output_path("east");
    run_child(&west, "America/New_York");
    run_child(&east, "Asia/Tokyo");

    let west_svg = std::fs::read(&west).expect("read western child SVG");
    let east_svg = std::fs::read(&east).expect("read eastern child SVG");
    let _ = std::fs::remove_file(&west);
    let _ = std::fs::remove_file(&east);

    assert_eq!(west_svg, east_svg);
    assert_eq!(west_svg, render_deterministic_svg().into_bytes());
}
