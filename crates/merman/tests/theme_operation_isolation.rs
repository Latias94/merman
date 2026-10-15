#![cfg(all(
    feature = "svg",
    any(
        feature = "diagram-class",
        feature = "diagram-flowchart",
        feature = "diagram-sequence",
        feature = "diagram-mindmap"
    )
))]

use merman::svg::SvgRenderOptions;
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, RenderedDocument,
    Renderer, SvgRequest,
};
use serde_json::{Value, json};

const FAMILIES: &[(&str, &str)] = &[
    #[cfg(feature = "diagram-class")]
    ("class", "classDiagram\nAlpha --> Beta"),
    #[cfg(feature = "diagram-flowchart")]
    ("flowchart", "flowchart TD\nA[Alpha] --> B[Beta]"),
    #[cfg(feature = "diagram-sequence")]
    ("sequence", "sequenceDiagram\nAlice->>Bob: Hello"),
    #[cfg(feature = "diagram-mindmap")]
    ("mindmap", "mindmap\n  root((Root))\n    Alpha\n    Beta"),
];

fn renderer() -> Renderer {
    Renderer::new()
        .with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "theme": "dark",
                "look": "classic",
                "layout": "dagre",
                "htmlLabels": false,
                "flowchart": { "htmlLabels": false },
                "themeVariables": { "primaryColor": "#123456" }
            }))),
        )
        .with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(0),
        )
}

fn effective_config(renderer: &Renderer, source: &str) -> Value {
    renderer
        .prepare_semantic(source, OperationControl::new())
        .expect("semantic preparation succeeds")
        .expect("fixture contains a diagram")
        .metadata()
        .effective_config
        .as_value()
        .clone()
}

fn request(family: &str) -> SvgRequest {
    SvgRequest {
        options: SvgRenderOptions {
            diagram_id: Some(format!("theme-isolation-{family}")),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn public_svg(renderer: &Renderer, source: &str, family: &str) -> String {
    let output = renderer
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            request(family),
        ))
        .expect("public SVG rendering succeeds");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected public SVG output");
    };
    output.svg().to_owned()
}

fn document(renderer: &Renderer, source: &str, family: &str) -> RenderedDocument {
    let output = renderer
        .render(RenderRequest::document(
            source,
            OperationControl::new(),
            request(family),
        ))
        .expect("native-compatible document rendering succeeds");
    let RenderOutput::Document(Some(output)) = output else {
        panic!("expected document output");
    };
    output
}

#[test]
fn source_theme_layers_do_not_leak_between_families_or_render_targets() {
    let reused = renderer();
    for &(family, body) in FAMILIES {
        let initialized = effective_config(&renderer(), body);
        assert_eq!(initialized["theme"], "dark", "{family}");
        assert_eq!(
            initialized["themeVariables"]["primaryColor"], "#123456",
            "{family}"
        );
        assert_ne!(
            initialized["themeVariables"]["actorBkg"], "#654321",
            "fixture must distinguish retained and rematerialized variables"
        );

        // Repeat the default after each override and reuse one renderer across all families.
        for theme in [
            None,
            Some(json!("base")),
            None,
            Some(json!("null")),
            None,
            Some(Value::Null),
            None,
        ] {
            let source = match &theme {
                None => body.to_owned(),
                Some(theme) => format!(
                    "---\nconfig: {}\n---\n{body}",
                    json!({ "theme": theme, "themeVariables": { "primaryColor": "#654321" } })
                ),
            };
            let context = format!("{family}, source theme {theme:?}");
            let fresh = renderer();
            let actual = effective_config(&reused, &source);
            assert_eq!(actual, effective_config(&fresh, &source), "{context}");
            let expected_theme = match theme.as_ref().and_then(Value::as_str) {
                Some("base") => "base",
                Some("null") => "null",
                _ => "dark",
            };
            assert_eq!(actual["theme"], expected_theme, "{context}");
            assert_eq!(
                actual["themeVariables"]["primaryColor"],
                if theme.is_some() {
                    "#654321"
                } else {
                    "#123456"
                },
                "{context}"
            );
            let expected_actor = if expected_theme == "base" {
                json!("#654321")
            } else {
                initialized["themeVariables"]["actorBkg"].clone()
            };
            assert_eq!(
                actual["themeVariables"]["actorBkg"], expected_actor,
                "{context}"
            );
            if theme.is_none() {
                assert_eq!(actual, initialized, "{context}");
            }

            assert_eq!(
                public_svg(&reused, &source, family),
                public_svg(&fresh, &source, family),
                "public SVG: {context}"
            );
            let reused_document = document(&reused, &source, family);
            let fresh_document = document(&fresh, &source, family);
            assert_eq!(
                reused_document.svg(),
                fresh_document.svg(),
                "document SVG: {context}"
            );
            assert_eq!(
                reused_document.resource_fingerprint(),
                fresh_document.resource_fingerprint(),
                "retained resources: {context}"
            );
            // Native SVG bytes are private; document identity binds both public and native bytes.
            assert_eq!(
                reused_document.document_digest(),
                fresh_document.document_digest(),
                "public/native document identity: {context}"
            );
        }
    }
}
