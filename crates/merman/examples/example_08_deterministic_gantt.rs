mod support;

use merman::{Engine, ParseOptions, runtime::RuntimePolicy, time::CivilDate};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = support::read_mermaid_or_default(
        "example_08_deterministic_gantt",
        r#"gantt
dateFormat MM-DD
section Release
Parser freeze: done, p1, 02-20, 2d
Alpha polish: active, p2, after p1, 3d
"#,
    )?;

    // Relative Gantt parsing depends on "today" and local offset; fix both for snapshots.
    let runtime_policy = RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)?
        .with_fixed_today(Some(CivilDate::new(2026, 2, 15).expect("valid fixed date")));
    let engine = Engine::new().with_runtime_policy(runtime_policy);
    let Some(parsed) = engine.parse_diagram_sync(&input, ParseOptions::strict())? else {
        return Err("no Mermaid diagram detected".into());
    };

    let output = json!({
        "fixedToday": "2026-02-15",
        "fixedLocalOffsetMinutes": 0,
        "diagramType": parsed.meta.diagram_type,
        "model": parsed.model,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
