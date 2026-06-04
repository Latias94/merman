use crate::common::{BindingError, internal_json_error};

pub const SUPPORTED_DIAGRAMS: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "er",
    "flowchart",
    "gantt",
    "gitgraph",
    "info",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantchart",
    "radar",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "treemap",
    "xychart",
    "zenuml",
];

#[cfg(feature = "ascii")]
pub const ASCII_SUPPORTED_DIAGRAMS: &[&str] = &["class", "er", "flowchart", "sequence", "xychart"];

pub fn supported_themes() -> &'static [&'static str] {
    merman::supported_themes()
}

pub fn supported_diagrams() -> &'static [&'static str] {
    SUPPORTED_DIAGRAMS
}

pub fn ascii_supported_diagrams() -> &'static [&'static str] {
    #[cfg(feature = "ascii")]
    {
        ASCII_SUPPORTED_DIAGRAMS
    }
    #[cfg(not(feature = "ascii"))]
    {
        &[]
    }
}

pub fn supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(supported_diagrams()).map_err(internal_json_error)
}

pub fn ascii_supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(ascii_supported_diagrams()).map_err(internal_json_error)
}

pub fn supported_themes_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(supported_themes()).map_err(internal_json_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn supported_themes_exposes_core_theme_surface() {
        assert_eq!(
            supported_themes(),
            &[
                "default",
                "base",
                "dark",
                "forest",
                "neutral",
                "neo",
                "neo-dark",
                "redux",
                "redux-dark",
                "redux-color",
                "redux-dark-color"
            ]
        );
    }

    #[test]
    fn supported_diagrams_exposes_binding_surface() {
        assert!(supported_diagrams().contains(&"flowchart"));
        assert!(supported_diagrams().contains(&"sequence"));
        assert!(supported_diagrams().contains(&"requirement"));
    }

    #[test]
    fn ascii_supported_diagrams_reflects_feature_surface() {
        if cfg!(feature = "ascii") {
            assert_eq!(
                ascii_supported_diagrams(),
                &["class", "er", "flowchart", "sequence", "xychart"]
            );
        } else {
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn metadata_json_helpers_return_arrays() {
        let diagrams: Value = serde_json::from_slice(&supported_diagrams_json().unwrap()).unwrap();
        let ascii_diagrams: Value =
            serde_json::from_slice(&ascii_supported_diagrams_json().unwrap()).unwrap();
        let themes: Value = serde_json::from_slice(&supported_themes_json().unwrap()).unwrap();

        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        assert!(ascii_diagrams.is_array());
        assert!(
            themes
                .as_array()
                .unwrap()
                .contains(&Value::String("default".to_string()))
        );
    }
}
