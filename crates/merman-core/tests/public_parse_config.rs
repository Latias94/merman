#![cfg(feature = "diagram-flowchart")]

use merman_core::presentation::PresentationConfig;
use merman_core::{Engine, ParseOptions};
use serde_json::json;

#[test]
fn public_parse_config_contains_only_admitted_source_overrides() {
    let engine = Engine::new().with_site_config(PresentationConfig::from_value(json!({
        "securityLevel": "strict", "secure": ["securityLevel", "maxTextSize"]
    })));
    for source in [
        "%%{init: {\"securityLevel\":\"loose\",\"maxTextSize\":1,\"flowchart\":{\"htmlLabels\":false}}}%%\nflowchart TD\n A-->B",
        "---\nconfig:\n  securityLevel: loose\n  maxTextSize: 1\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\n A-->B",
    ] {
        let parsed = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed.meta.config.as_value(),
            &json!({"flowchart":{"htmlLabels":false}})
        );
        assert_eq!(
            parsed.meta.effective_config.get_str("securityLevel"),
            Some("strict")
        );
        let typed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            typed.metadata().config.as_value(),
            parsed.meta.config.as_value()
        );
        assert_eq!(
            typed.metadata().effective_config.get_str("securityLevel"),
            Some("strict")
        );
    }
}
