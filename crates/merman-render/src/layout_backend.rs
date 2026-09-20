//! Resolve registered graph layouts before execution.
//!
//! Mermaid 12's `getRegisteredLayoutAlgorithm` falls back when a loader is absent,
//! never after a selected backend fails. Host capability policy remains a separate
//! admission check: a compiled backend denied by the caller is not an absent loader.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphLayoutBackend {
    Dagre,
    Elk,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphLayoutSelection<'a> {
    pub requested: &'a str,
    pub backend: GraphLayoutBackend,
}

/// Selection for the graph families currently sharing the Dagre/ELK adapters.
pub(crate) fn resolve_graph_layout(config: &Value) -> GraphLayoutSelection<'_> {
    let requested = config.get("layout").and_then(Value::as_str).unwrap_or("");
    let backend = match requested {
        "elk" if cfg!(feature = "layout-elk") => GraphLayoutBackend::Elk,
        _ => GraphLayoutBackend::Dagre,
    };
    GraphLayoutSelection { requested, backend }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn graph_layout_selection_retains_request_and_resolves_available_loader() {
        for requested in ["dagre", "elk", "ELK", "unknown", ""] {
            let config = json!({"layout": requested});
            let selected = resolve_graph_layout(&config);
            assert_eq!(selected.requested, requested);
            let expected = if requested == "elk" && cfg!(feature = "layout-elk") {
                GraphLayoutBackend::Elk
            } else {
                GraphLayoutBackend::Dagre
            };
            assert_eq!(selected.backend, expected);
        }
        assert_eq!(
            resolve_graph_layout(&Value::Null).backend,
            GraphLayoutBackend::Dagre
        );
    }
}
