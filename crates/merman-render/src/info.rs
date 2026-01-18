use crate::Result;
use crate::model::{Bounds, InfoDiagramLayout};
use crate::text::TextMeasurer;

const UPSTREAM_MERMAID_VERSION: &str = "11.12.2";

pub fn layout_info_diagram(
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
) -> Result<InfoDiagramLayout> {
    let _ = semantic;

    Ok(InfoDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 400.0,
            max_y: 80.0,
        }),
        version: format!("v{UPSTREAM_MERMAID_VERSION}"),
    })
}
