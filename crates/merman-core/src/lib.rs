#![forbid(unsafe_code)]
// LALRPOP generates code that can contain an "empty line after outer attribute" in its output.
// We keep the generated sources as-is and suppress this lint at the crate level.
#![allow(clippy::empty_line_after_outer_attr)]

//! Mermaid parser + semantic model (headless).
//!
//! Design goals:
//! - 1:1 parity with upstream Mermaid (`mermaid@11.12.2`)
//! - deterministic, testable outputs (semantic snapshot goldens)
//! - runtime-agnostic async APIs (no specific executor required)

pub mod common;
pub mod common_db;
pub mod config;
pub mod detect;
pub mod diagram;
pub mod diagrams;
pub mod entities;
pub mod error;
pub mod generated;
pub mod geom;
pub mod preprocess;
pub mod sanitize;
mod theme;
pub mod utils;

pub use config::MermaidConfig;
pub use detect::{Detector, DetectorRegistry};
pub use diagram::{DiagramRegistry, DiagramSemanticParser, ParsedDiagram};
pub use error::{Error, Result};
pub use preprocess::{PreprocessResult, preprocess_diagram};

#[derive(Debug, Clone, Copy, Default)]
pub struct ParseOptions {
    pub suppress_errors: bool,
}

impl ParseOptions {
    /// Strict parsing (errors are returned).
    pub fn strict() -> Self {
        Self {
            suppress_errors: false,
        }
    }

    /// Lenient parsing: on parse failures, return an `error` diagram instead of returning an error.
    pub fn lenient() -> Self {
        Self {
            suppress_errors: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseMetadata {
    pub diagram_type: String,
    /// Parsed config overrides extracted from front-matter and directives.
    /// This mirrors Mermaid's `mermaidAPI.parse()` return shape.
    pub config: MermaidConfig,
    /// The effective config used for detection/parsing after applying site defaults.
    pub effective_config: MermaidConfig,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Engine {
    registry: DetectorRegistry,
    diagram_registry: DiagramRegistry,
    site_config: MermaidConfig,
}

impl Default for Engine {
    fn default() -> Self {
        let site_config = generated::default_site_config();

        Self {
            registry: DetectorRegistry::default_mermaid_11_12_2(),
            diagram_registry: DiagramRegistry::default_mermaid_11_12_2(),
            site_config,
        }
    }
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_site_config(mut self, site_config: MermaidConfig) -> Self {
        // Merge overrides onto Mermaid schema defaults so detectors keep working.
        self.site_config.deep_merge(site_config.as_value());
        self
    }

    pub fn registry(&self) -> &DetectorRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut DetectorRegistry {
        &mut self.registry
    }

    pub fn diagram_registry(&self) -> &DiagramRegistry {
        &self.diagram_registry
    }

    pub fn diagram_registry_mut(&mut self) -> &mut DiagramRegistry {
        &mut self.diagram_registry
    }

    /// Synchronous variant of [`Engine::parse_metadata`].
    ///
    /// This is useful for UI render pipelines that are synchronous (e.g. immediate-mode UI),
    /// where introducing an async executor would be awkward. The parsing work is CPU-bound and
    /// does not perform I/O.
    pub fn parse_metadata_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParseMetadata>> {
        let Some((_, meta)) = self.preprocess_and_detect(text, options)? else {
            return Ok(None);
        };
        Ok(Some(meta))
    }

    pub async fn parse_metadata(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParseMetadata>> {
        self.parse_metadata_sync(text, options)
    }

    /// Synchronous variant of [`Engine::parse_diagram`].
    ///
    /// Note: callers that want “always returns a diagram” behavior can set
    /// [`ParseOptions::suppress_errors`] to `true` to get an `error` diagram on parse failures.
    pub fn parse_diagram_sync(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        let Some((code, meta)) = self.preprocess_and_detect(text, options)? else {
            return Ok(None);
        };

        let mut model = match diagram::parse_or_unsupported(
            &self.diagram_registry,
            &meta.diagram_type,
            &code,
            &meta,
        ) {
            Ok(v) => v,
            Err(err) => {
                if !options.suppress_errors {
                    return Err(err);
                }

                let mut error_meta = meta.clone();
                error_meta.diagram_type = "error".to_string();
                let mut error_model = serde_json::json!({ "type": "error" });
                common_db::apply_common_db_sanitization(
                    &mut error_model,
                    &error_meta.effective_config,
                );
                return Ok(Some(ParsedDiagram {
                    meta: error_meta,
                    model: error_model,
                }));
            }
        };
        common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
        Ok(Some(ParsedDiagram { meta, model }))
    }

    pub async fn parse_diagram(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<ParsedDiagram>> {
        self.parse_diagram_sync(text, options)
    }

    pub async fn parse(&self, text: &str, options: ParseOptions) -> Result<Option<ParseMetadata>> {
        self.parse_metadata(text, options).await
    }

    fn preprocess_and_detect(
        &self,
        text: &str,
        options: ParseOptions,
    ) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram(text, &self.registry)?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let mut effective_config = self.site_config.clone();
        effective_config.deep_merge(pre.config.as_value());

        let diagram_type = match self.registry.detect_type(&pre.code, &mut effective_config) {
            Ok(t) => t.to_string(),
            Err(err) => {
                if options.suppress_errors {
                    return Ok(None);
                }
                return Err(err);
            }
        };
        theme::apply_theme_defaults(&mut effective_config);

        let title = pre
            .title
            .as_ref()
            .map(|t| crate::sanitize::sanitize_text(t, &effective_config))
            .filter(|t| !t.is_empty());

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type,
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use serde_json::json;

    #[test]
    fn parse_graph_defaults_to_flowchart_v2() {
        let engine = Engine::new();
        let res = block_on(engine.parse_metadata("graph TD;A-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.diagram_type, "flowchart-v2");
        assert_eq!(res.config.as_value(), &json!({}));
    }

    #[test]
    fn parse_merges_frontmatter_and_directive_config() {
        let engine = Engine::new();
        let text = r#"---
config:
  theme: forest
  flowchart:
    htmlLabels: true
---
%%{init: { 'theme': 'base' } }%%
graph TD;A-->B;"#;

        let res = block_on(engine.parse_metadata(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.config.as_value(),
            &json!({
                "theme": "base",
                "flowchart": {
                    "htmlLabels": true
                }
            })
        );
    }

    #[test]
    fn parse_sanitizes_frontmatter_title_like_mermaid_common_db() {
        let engine = Engine::new();
        let text = r#"---
title: "Flowchart v2 arrows: graph direction \"<\""
---
graph <
A-->B
"#;

        let meta = block_on(engine.parse_metadata(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        assert_eq!(
            meta.title,
            Some(r#"Flowchart v2 arrows: graph direction "&lt;""#.to_string())
        );
    }

    #[test]
    fn parse_merges_init_directive_numeric_values_like_upstream() {
        let engine = Engine::new();
        let text = r#"%%{init: { 'logLevel': 0 } }%%
sequenceDiagram
Alice->Bob: Hi
"#;

        let res = block_on(engine.parse_metadata(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.diagram_type, "sequence");
        assert_eq!(res.config.as_value(), &json!({ "logLevel": 0 }));
    }

    #[test]
    fn parse_returns_malformed_frontmatter_error_for_unclosed_frontmatter() {
        let engine = Engine::new();
        let err = block_on(engine.parse_metadata(
            "---\ntitle: a malformed YAML front-matter\n",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("Malformed YAML front-matter"));
    }

    #[test]
    fn parse_can_suppress_unknown_diagram_errors() {
        let engine = Engine::new();
        let res = block_on(engine.parse_metadata(
            "this is not a mermaid diagram definition",
            ParseOptions {
                suppress_errors: true,
            },
        ))
        .unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn parse_diagram_pie_basic() {
        let engine = Engine::new();
        let text = r#"pie showData
 "Cats": 2
 'Dogs': 3
 "#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "pie");
        assert_eq!(
            res.model,
            json!({
                "type": "pie",
                "showData": true,
                "title": null,
                "accTitle": null,
                "accDescr": null,
                "sections": [
                    { "label": "Cats", "value": 2.0 },
                    { "label": "Dogs", "value": 3.0 }
                ]
            })
        );
    }

    #[test]
    fn parse_diagram_info_basic() {
        let engine = Engine::new();
        let text = "info showInfo\n";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "info");
        assert_eq!(
            res.model,
            json!({
                "type": "info",
                "showInfo": true
            })
        );
    }

    #[test]
    fn parse_diagram_info_rejects_unsupported_grammar_like_upstream() {
        let engine = Engine::new();
        let text = "info unsupported\n";
        let err = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "Diagram parse error (info): Parsing failed: unexpected character: ->u<- at offset: 5, skipped 11 characters."
        );
    }

    #[test]
    fn parse_diagram_pie_rejects_negative_slice_values_like_upstream() {
        let engine = Engine::new();
        let text = r#"pie title Default text position: Animal adoption
         "dogs" : -60.67
        "rats" : 40.12
        "#;
        let err = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "Diagram parse error (pie): \"dogs\" has invalid value: -60.67. Negative values are not allowed in pie charts. All slice values must be >= 0."
        );
    }

    #[test]
    fn parse_diagram_flowchart_basic_graph() {
        let engine = Engine::new();
        let text = "graph TD;A-->B;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "flowchart-v2");
        assert_eq!(
            res.model,
            json!({
                "type": "flowchart-v2",
                "keyword": "graph",
                "direction": "TB",
                "accTitle": null,
                "accDescr": null,
                "classDefs": {},
                "tooltips": {},
                "edgeDefaults": { "style": [], "interpolate": null },
                "vertexCalls": ["A", "B"],
                "nodes": [
                    { "id": "A", "label": "A", "labelType": "text", "shape": null, "layoutShape": "squareRect", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false },
                    { "id": "B", "label": "B", "labelType": "text", "shape": null, "layoutShape": "squareRect", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false }
                ],
                "edges": [
                    { "from": "A", "to": "B", "id": "L_A_B_0", "isUserDefinedId": false, "arrow": "-->", "type": "arrow_point", "stroke": "normal", "length": 1, "label": null, "labelType": "text", "style": [], "classes": [], "interpolate": null, "animate": null, "animation": null }
                ],
                "subgraphs": []
            })
        );
    }

    #[test]
    fn parse_diagram_flowchart_tolerates_edge_labels() {
        let engine = Engine::new();
        let text = "graph TD;A--x|text including URL space|B;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "flowchart-v2");
        assert_eq!(
            res.model["edges"][0],
            json!({
                "from": "A",
                "to": "B",
                "id": "L_A_B_0",
                "isUserDefinedId": false,
                "arrow": "--x",
                "type": "arrow_cross",
                "stroke": "normal",
                "length": 1,
                "label": "text including URL space",
                "labelType": "text",
                "style": [],
                "classes": [],
                "interpolate": null,
                "animate": null,
                "animation": null
            })
        );
        assert_eq!(res.model["subgraphs"], json!([]));
    }

    #[test]
    fn parse_diagram_flowchart_supports_inline_nodes() {
        let engine = Engine::new();
        let text = "graph TD;A[Start]-->B{Is it?};";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "flowchart-v2");
        assert_eq!(
            res.model,
            json!({
                "type": "flowchart-v2",
                "keyword": "graph",
                "direction": "TB",
                "accTitle": null,
                "accDescr": null,
                "classDefs": {},
                "tooltips": {},
                "edgeDefaults": { "style": [], "interpolate": null },
                "vertexCalls": ["A", "B"],
                "nodes": [
                    { "id": "A", "label": "Start", "labelType": "text", "shape": "square", "layoutShape": "squareRect", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false },
                    { "id": "B", "label": "Is it?", "labelType": "text", "shape": "diamond", "layoutShape": "diamond", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false }
                ],
                "edges": [
                    { "from": "A", "to": "B", "id": "L_A_B_0", "isUserDefinedId": false, "arrow": "-->", "type": "arrow_point", "stroke": "normal", "length": 1, "label": null, "labelType": "text", "style": [], "classes": [], "interpolate": null, "animate": null, "animation": null }
                ],
                "subgraphs": []
            })
        );
    }

    #[test]
    fn parse_diagram_flowchart_edge_stroke_and_type_normal_thick_dotted() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram("graph TD;A-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));

        let res = block_on(engine.parse_diagram("graph TD;A==>B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("thick"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));

        let res = block_on(engine.parse_diagram("graph TD;A-.->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("dotted"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));
    }

    #[test]
    fn parse_diagram_flowchart_double_ended_arrows() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram("graph TD;A<-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("double_arrow_point"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));
    }

    #[test]
    fn parse_diagram_flowchart_edge_text_new_notation() {
        let engine = Engine::new();
        let text = "graph TD;A-- text including URL space and send -->B;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
        assert_eq!(
            res.model["edges"][0]["label"],
            json!("text including URL space and send")
        );
    }

    #[test]
    fn parse_diagram_flowchart_edge_text_new_notation_double_ended() {
        let engine = Engine::new();
        let text = "graph TD;A<-- text -->B;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("double_arrow_point"));
        assert_eq!(res.model["edges"][0]["label"], json!("text"));
    }

    #[test]
    fn parse_diagram_flowchart_invisible_edge() {
        let engine = Engine::new();
        let res = block_on(engine.parse_diagram("graph TD;A~~~B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_open"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("invisible"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));
    }

    #[test]
    fn parse_diagram_flowchart_edges_spec_open_cross_circle() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram("graph TD;A---B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_open"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));

        let res = block_on(engine.parse_diagram("graph TD;A--xB;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_cross"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));

        let res = block_on(engine.parse_diagram("graph TD;A--oB;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_circle"));
        assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
        assert_eq!(res.model["edges"][0]["length"], json!(1));
    }

    #[test]
    fn parse_diagram_flowchart_edges_spec_edge_ids_and_node_metadata_do_not_conflict() {
        let engine = Engine::new();
        let text = "flowchart LR\nA id1@-->B\nA@{ shape: 'rect' }\n";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["id"], json!("id1"));
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
    }

    #[test]
    fn parse_diagram_flowchart_edges_spec_edge_length_matrix() {
        let engine = Engine::new();
        let assert_edge = |diagram: String,
                           expected_type: &str,
                           expected_stroke: &str,
                           expected_length: usize,
                           expected_label: Option<&str>| {
            let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let e = &res.model["edges"][0];
            assert_eq!(e["type"], json!(expected_type), "diagram: {diagram}");
            assert_eq!(e["stroke"], json!(expected_stroke), "diagram: {diagram}");
            assert_eq!(e["length"], json!(expected_length), "diagram: {diagram}");
            match expected_label {
                Some(label) => assert_eq!(e["label"], json!(label), "diagram: {diagram}"),
                None => assert!(e["label"].is_null(), "diagram: {diagram}"),
            }
        };

        for length in 1..=3 {
            assert_edge(
                format!("graph TD;\nA -{}- B;", "-".repeat(length)),
                "arrow_open",
                "normal",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA -- Label -{}- B;", "-".repeat(length)),
                "arrow_open",
                "normal",
                length,
                Some("Label"),
            );
            assert_edge(
                format!("graph TD;\nA -{}> B;", "-".repeat(length)),
                "arrow_point",
                "normal",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA -- Label -{}> B;", "-".repeat(length)),
                "arrow_point",
                "normal",
                length,
                Some("Label"),
            );
            assert_edge(
                format!("graph TD;\nA <-{}> B;", "-".repeat(length)),
                "double_arrow_point",
                "normal",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA <-- Label -{}> B;", "-".repeat(length)),
                "double_arrow_point",
                "normal",
                length,
                Some("Label"),
            );
        }

        for length in 1..=3 {
            assert_edge(
                format!("graph TD;\nA ={}= B;", "=".repeat(length)),
                "arrow_open",
                "thick",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA == Label ={}= B;", "=".repeat(length)),
                "arrow_open",
                "thick",
                length,
                Some("Label"),
            );
            assert_edge(
                format!("graph TD;\nA ={}> B;", "=".repeat(length)),
                "arrow_point",
                "thick",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA == Label ={}> B;", "=".repeat(length)),
                "arrow_point",
                "thick",
                length,
                Some("Label"),
            );
            assert_edge(
                format!("graph TD;\nA <={}> B;", "=".repeat(length)),
                "double_arrow_point",
                "thick",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA <== Label ={}> B;", "=".repeat(length)),
                "double_arrow_point",
                "thick",
                length,
                Some("Label"),
            );
        }

        for length in 1..=3 {
            assert_edge(
                format!("graph TD;\nA -{}- B;", ".".repeat(length)),
                "arrow_open",
                "dotted",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA -. Label {}- B;", ".".repeat(length)),
                "arrow_open",
                "dotted",
                length,
                Some("Label"),
            );
            assert_edge(
                format!("graph TD;\nA -{}-> B;", ".".repeat(length)),
                "arrow_point",
                "dotted",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA -. Label {}-> B;", ".".repeat(length)),
                "arrow_point",
                "dotted",
                length,
                Some("Label"),
            );
            assert_edge(
                format!("graph TD;\nA <-{}-> B;", ".".repeat(length)),
                "double_arrow_point",
                "dotted",
                length,
                None,
            );
            assert_edge(
                format!("graph TD;\nA <-. Label {}-> B;", ".".repeat(length)),
                "double_arrow_point",
                "dotted",
                length,
                Some("Label"),
            );
        }
    }

    #[test]
    fn parse_diagram_flowchart_edges_spec_keywords_as_edge_labels_in_double_ended_edges() {
        let engine = Engine::new();

        let keywords = [
            "graph",
            "flowchart",
            "flowchart-elk",
            "style",
            "default",
            "linkStyle",
            "interpolate",
            "classDef",
            "class",
            "href",
            "call",
            "click",
            "_self",
            "_blank",
            "_parent",
            "_top",
            "end",
            "subgraph",
            "kitty",
        ];

        let edges = [
            ("x--", "--x", "normal", "double_arrow_cross"),
            ("x==", "==x", "thick", "double_arrow_cross"),
            ("x-.", ".-x", "dotted", "double_arrow_cross"),
            ("o--", "--o", "normal", "double_arrow_circle"),
            ("o==", "==o", "thick", "double_arrow_circle"),
            ("o-.", ".-o", "dotted", "double_arrow_circle"),
            ("<--", "-->", "normal", "double_arrow_point"),
            ("<==", "==>", "thick", "double_arrow_point"),
            ("<-.", ".->", "dotted", "double_arrow_point"),
        ];

        for (edge_start, edge_end, stroke, edge_type) in edges {
            for keyword in keywords {
                let diagram = format!("graph TD;\nA {edge_start} {keyword} {edge_end} B;");
                let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
                    .unwrap()
                    .unwrap();
                let e = &res.model["edges"][0];
                assert_eq!(e["type"], json!(edge_type), "diagram: {diagram}");
                assert_eq!(e["stroke"], json!(stroke), "diagram: {diagram}");
                assert_eq!(e["label"], json!(keyword), "diagram: {diagram}");
                assert_eq!(e["labelType"], json!("text"), "diagram: {diagram}");
            }
        }
    }

    #[test]
    fn parse_diagram_flowchart_node_data_basic_shape_data_statements() {
        let engine = Engine::new();

        let res = block_on(
            engine.parse_diagram("flowchart TB\nD@{ shape: rounded}", ParseOptions::default()),
        )
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["id"], json!("D"));
        assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
        assert_eq!(nodes[0]["label"], json!("D"));

        let res = block_on(engine.parse_diagram(
            "flowchart TB\nD@{ shape: rounded }",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
        assert_eq!(nodes[0]["label"], json!("D"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_shape_data_with_amp_and_edges() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            "flowchart TB\nD@{ shape: rounded } & E",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0]["id"], json!("D"));
        assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
        assert_eq!(nodes[0]["label"], json!("D"));
        assert_eq!(nodes[1]["id"], json!("E"));
        assert_eq!(nodes[1]["label"], json!("E"));

        let res = block_on(engine.parse_diagram(
            "flowchart TB\nD@{ shape: rounded } --> E",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0]["id"], json!("D"));
        assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
        assert_eq!(nodes[1]["id"], json!("E"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_shape_data_whitespace_variants() {
        let engine = Engine::new();

        for diagram in [
            "flowchart TB\nD@{shape: rounded}",
            "flowchart TB\nD@{       shape: rounded}",
            "flowchart TB\nD@{ shape: rounded         }",
        ] {
            let res = block_on(engine.parse_diagram(diagram, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let nodes = res.model["nodes"].as_array().unwrap();
            assert_eq!(nodes.len(), 1, "diagram: {diagram}");
            assert_eq!(nodes[0]["id"], json!("D"), "diagram: {diagram}");
            assert_eq!(
                nodes[0]["layoutShape"],
                json!("rounded"),
                "diagram: {diagram}"
            );
            assert_eq!(nodes[0]["label"], json!("D"), "diagram: {diagram}");
        }
    }

    #[test]
    fn parse_diagram_flowchart_node_data_shape_data_amp_and_edge_matrix() {
        let engine = Engine::new();

        let cases = [
            (
                "flowchart TB\nD@{ shape: rounded } & E --> F",
                3usize,
                "D",
                "rounded",
            ),
            (
                "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F",
                3usize,
                "D",
                "rounded",
            ),
            (
                "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F & G@{ shape: rounded }",
                4usize,
                "D",
                "rounded",
            ),
            (
                "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F@{ shape: rounded } & G@{ shape: rounded }",
                4usize,
                "D",
                "rounded",
            ),
            (
                "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F{ shape: rounded } & G{ shape: rounded }    ",
                4usize,
                "D",
                "rounded",
            ),
        ];

        for (diagram, expected_nodes, first_id, first_layout) in cases {
            let res = block_on(engine.parse_diagram(diagram, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let nodes = res.model["nodes"].as_array().unwrap();
            assert_eq!(nodes.len(), expected_nodes, "diagram: {diagram}");
            assert_eq!(nodes[0]["id"], json!(first_id), "diagram: {diagram}");
            assert_eq!(
                nodes[0]["layoutShape"],
                json!(first_layout),
                "diagram: {diagram}"
            );
        }
    }

    #[test]
    fn parse_diagram_flowchart_node_data_shape_data_allows_brace_in_multiline_string() {
        let engine = Engine::new();

        let text = r#"flowchart TB
A@{
  label: "This is }"
  other: "clock"
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["label"], json!("This is }"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_multiple_properties_same_line() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            "flowchart TB\nD@{ shape: rounded , label: \"DD\"}",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["id"], json!("D"));
        assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
        assert_eq!(nodes[0]["label"], json!("DD"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_link_to_node_with_more_data_multiline_yaml() {
        let engine = Engine::new();

        let text = r#"flowchart TB
A --> D@{
  shape: circle
  other: "clock"
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0]["id"], json!("A"));
        assert_eq!(nodes[0]["layoutShape"], json!("squareRect"));
        assert_eq!(nodes[0]["label"], json!("A"));
        assert_eq!(nodes[1]["id"], json!("D"));
        assert_eq!(nodes[1]["layoutShape"], json!("circle"));
        assert_eq!(nodes[1]["label"], json!("D"));
        assert_eq!(res.model["edges"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn parse_diagram_flowchart_node_data_nodes_after_each_other() {
        let engine = Engine::new();
        let text = r#"flowchart TB
A[hello]
B@{
  shape: circle
  other: "clock"
}
C[Hello]@{
  shape: circle
  other: "clock"
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0]["id"], json!("A"));
        assert_eq!(nodes[0]["label"], json!("hello"));
        assert_eq!(nodes[0]["layoutShape"], json!("squareRect"));
        assert_eq!(nodes[1]["id"], json!("B"));
        assert_eq!(nodes[1]["label"], json!("B"));
        assert_eq!(nodes[1]["layoutShape"], json!("circle"));
        assert_eq!(nodes[2]["id"], json!("C"));
        assert_eq!(nodes[2]["label"], json!("Hello"));
        assert_eq!(nodes[2]["layoutShape"], json!("circle"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_shape_data_allows_brace_and_at_in_strings() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            "flowchart TB\nA@{ label: \"This is }\" }",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["layoutShape"], json!("squareRect"));
        assert_eq!(nodes[0]["label"], json!("This is }"));

        let res = block_on(engine.parse_diagram(
            "flowchart TB\nA@{ label: \"This is a string with @\" }",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["label"], json!("This is a string with @"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_shape_validation_errors() {
        let engine = Engine::new();

        let err = block_on(engine.parse_diagram(
            "flowchart TB\nA@{ shape: this-shape-does-not-exist }",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("No such shape: this-shape-does-not-exist.")
        );

        let err = block_on(engine.parse_diagram(
            "flowchart TB\nA@{ shape: rect_left_inv_arrow }",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("No such shape: rect_left_inv_arrow. Shape names should be lowercase.")
        );
    }

    #[test]
    fn parse_diagram_flowchart_node_data_multiline_strings_match_mermaid() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"flowchart TB
A@{
  label: |
    This is a
    multiline string
  other: "clock"
}
"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["label"], json!("This is a\nmultiline string\n"));

        let res = block_on(engine.parse_diagram(
            r#"flowchart TB
A@{
  label: "This is a
    multiline string"
  other: "clock"
}
"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["label"], json!("This is a<br/>multiline string"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_labels_across_multi_nodes_and_edges() {
        let engine = Engine::new();

        let text = r#"flowchart TB
n2["label for n2"] & n4@{ label: "label for n4"} & n5@{ label: "label for n5"}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0]["label"], json!("label for n2"));
        assert_eq!(nodes[1]["label"], json!("label for n4"));
        assert_eq!(nodes[2]["label"], json!("label for n5"));

        let text = r#"flowchart TD
A["A"] --> B["for B"] & C@{ label: "for c"} & E@{label : "for E"}
D@{label: "for D"}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes[0]["label"], json!("A"));
        assert_eq!(nodes[1]["label"], json!("for B"));
        assert_eq!(nodes[2]["label"], json!("for c"));
        assert_eq!(nodes[3]["label"], json!("for E"));
        assert_eq!(nodes[4]["label"], json!("for D"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_allows_at_in_labels_across_shapes() {
        let engine = Engine::new();

        let text = r#"flowchart TD
A["@A@"] --> B["@for@ B@"] & C@{ label: "@for@ c@"} & E{"`@for@ E@`"} & D(("@for@ D@"))
H1{{"@for@ H@"}}
H2{{"`@for@ H@`"}}
Q1{"@for@ Q@"}
Q2{"`@for@ Q@`"}
AS1>"@for@ AS@"]
AS2>"`@for@ AS@`"]
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 11);
        for (i, node) in nodes.iter().enumerate() {
            assert!(
                node["label"].as_str().unwrap().contains("@for@") || node["label"] == json!("@A@"),
                "node {i}: {:?}",
                node
            );
        }
        assert_eq!(nodes[0]["label"], json!("@A@"));
        assert_eq!(nodes[1]["label"], json!("@for@ B@"));
        assert_eq!(nodes[2]["label"], json!("@for@ c@"));
        assert_eq!(nodes[3]["label"], json!("@for@ E@"));
        assert_eq!(nodes[4]["label"], json!("@for@ D@"));
        assert_eq!(nodes[5]["label"], json!("@for@ H@"));
        assert_eq!(nodes[6]["label"], json!("@for@ H@"));
        assert_eq!(nodes[7]["label"], json!("@for@ Q@"));
        assert_eq!(nodes[8]["label"], json!("@for@ Q@"));
        assert_eq!(nodes[9]["label"], json!("@for@ AS@"));
        assert_eq!(nodes[10]["label"], json!("@for@ AS@"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_unique_edge_ids_with_groups() {
        let engine = Engine::new();

        let text = r#"flowchart TD
A & B e1@--> C & D
A1 e2@--> C1 & D1
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["nodes"].as_array().unwrap().len(), 7);
        let edges = res.model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 6);
        assert_eq!(edges[0]["id"], json!("L_A_C_0"));
        assert_eq!(edges[1]["id"], json!("L_A_D_0"));
        assert_eq!(edges[2]["id"], json!("e1"));
        assert_eq!(edges[3]["id"], json!("L_B_D_0"));
        assert_eq!(edges[4]["id"], json!("e2"));
        assert_eq!(edges[5]["id"], json!("L_A1_D1_0"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_redefined_edge_id_becomes_auto_id() {
        let engine = Engine::new();

        let text = r#"flowchart TD
A & B e1@--> C & D
A1 e1@--> C1 & D1
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let edges = res.model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 6);
        assert_eq!(edges[0]["id"], json!("L_A_C_0"));
        assert_eq!(edges[1]["id"], json!("L_A_D_0"));
        assert_eq!(edges[2]["id"], json!("e1"));
        assert_eq!(edges[3]["id"], json!("L_B_D_0"));
        assert_eq!(edges[4]["id"], json!("L_A1_C1_0"));
        assert_eq!(edges[5]["id"], json!("L_A1_D1_0"));
    }

    #[test]
    fn parse_diagram_flowchart_node_data_overrides_edge_animate() {
        let engine = Engine::new();

        let text = r#"flowchart TD
A e1@--> B
C e2@--> D
E e3@--> F
e1@{ animate: true }
e2@{ animate: false }
e3@{ animate: true }
e3@{ animate: false }
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let edges = res.model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0]["id"], json!("e1"));
        assert_eq!(edges[0]["animate"], json!(true));
        assert_eq!(edges[1]["id"], json!("e2"));
        assert_eq!(edges[1]["animate"], json!(false));
        assert_eq!(edges[2]["id"], json!("e3"));
        assert_eq!(edges[2]["animate"], json!(false));
    }

    #[test]
    fn parse_diagram_flowchart_markdown_strings_in_nodes_and_edges() {
        let engine = Engine::new();
        let text = "flowchart\nA[\"`The cat in **the** hat`\"]-- \"`The *bat* in the chat`\" -->B[\"The dog in the hog\"] -- \"The rat in the mat\" -->C;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let nodes = res.model["nodes"].as_array().unwrap();
        let find_node = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();
        let node_a = find_node("A");
        let node_b = find_node("B");

        assert_eq!(node_a["label"], json!("The cat in **the** hat"));
        assert_eq!(node_a["labelType"], json!("markdown"));
        assert_eq!(node_b["label"], json!("The dog in the hog"));
        assert_eq!(node_b["labelType"], json!("string"));

        let edges = res.model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0]["from"], json!("A"));
        assert_eq!(edges[0]["to"], json!("B"));
        assert_eq!(edges[0]["type"], json!("arrow_point"));
        assert_eq!(edges[0]["label"], json!("The *bat* in the chat"));
        assert_eq!(edges[0]["labelType"], json!("markdown"));
        assert_eq!(edges[1]["from"], json!("B"));
        assert_eq!(edges[1]["to"], json!("C"));
        assert_eq!(edges[1]["type"], json!("arrow_point"));
        assert_eq!(edges[1]["label"], json!("The rat in the mat"));
        assert_eq!(edges[1]["labelType"], json!("string"));
    }

    #[test]
    fn parse_diagram_flowchart_markdown_strings_in_subgraphs() {
        let engine = Engine::new();
        let text = r#"flowchart LR
subgraph "One"
  a("`The **cat**
  in the hat`") -- "1o" --> b{{"`The **dog** in the hog`"}}
end
subgraph "`**Two**`"
  c("`The **cat**
  in the hat`") -- "`1o **ipa**`" --> d("The dog in the hog")
end"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let subgraphs = res.model["subgraphs"].as_array().unwrap();
        assert_eq!(subgraphs.len(), 2);
        assert_eq!(subgraphs[0]["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(subgraphs[0]["title"], json!("One"));
        assert_eq!(subgraphs[0]["labelType"], json!("text"));
        assert_eq!(subgraphs[1]["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(subgraphs[1]["title"], json!("**Two**"));
        assert_eq!(subgraphs[1]["labelType"], json!("markdown"));
    }

    #[test]
    fn parse_diagram_flowchart_header_direction_shorthand() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram("graph >;A-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["direction"], json!("LR"));

        let res = block_on(engine.parse_diagram("graph <;A-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["direction"], json!("RL"));

        let res = block_on(engine.parse_diagram("graph ^;A-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["direction"], json!("BT"));

        let res = block_on(engine.parse_diagram("graph v;A-->B;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["direction"], json!("TB"));
    }

    #[test]
    fn parse_diagram_flowchart_v_is_node_id_not_direction() {
        let engine = Engine::new();
        let res =
            block_on(engine.parse_diagram("graph TD;A--xv(my text);", ParseOptions::default()))
                .unwrap()
                .unwrap();

        let v = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("v"))
            .unwrap();
        assert_eq!(v["label"], json!("my text"));
        assert_eq!(v["shape"], json!("round"));
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_cross"));
    }

    #[test]
    fn parse_diagram_flowchart_v_in_node_ids_variants_from_flow_text_spec() {
        let engine = Engine::new();
        let text = "graph TD;A--xv(my text);A--xcsv(my text);A--xava(my text);A--xva(my text);";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        assert_eq!(res.model["edges"].as_array().unwrap().len(), 4);
        for edge in res.model["edges"].as_array().unwrap() {
            assert_eq!(edge["type"], json!("arrow_cross"));
        }

        let nodes = res.model["nodes"].as_array().unwrap();
        let find = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();

        assert_eq!(find("v")["label"], json!("my text"));
        assert_eq!(find("csv")["label"], json!("my text"));
        assert_eq!(find("ava")["label"], json!("my text"));
        assert_eq!(find("va")["label"], json!("my text"));
    }

    #[test]
    fn parse_diagram_flowchart_edge_label_supports_quoted_strings() {
        let engine = Engine::new();
        let res = block_on(engine.parse_diagram(
            "graph TD;V-- \"test string()\" -->a[v]",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(res.model["edges"][0]["label"], json!("test string()"));
        assert_eq!(res.model["edges"][0]["labelType"], json!("string"));
    }

    #[test]
    fn parse_diagram_flowchart_edge_label_old_notation_without_spaces() {
        let engine = Engine::new();
        let res = block_on(engine.parse_diagram(
            "graph TD;A--text including URL space and send-->B;",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["edges"][0]["label"],
            json!("text including URL space and send")
        );
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
    }

    #[test]
    fn parse_diagram_flowchart_edge_labels_can_span_multiple_lines() {
        let engine = Engine::new();
        let text = "graph TD;A--o|text space|B;\n B-->|more text with space|C;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"].as_array().unwrap().len(), 2);
        assert_eq!(res.model["edges"][0]["type"], json!("arrow_circle"));
        assert_eq!(res.model["edges"][1]["type"], json!("arrow_point"));
        assert_eq!(
            res.model["edges"][1]["label"],
            json!("more text with space")
        );
    }

    #[test]
    fn parse_diagram_flowchart_vertex_shapes_from_flow_text_spec() {
        let engine = Engine::new();
        let text = r#"graph TD;
A_node-->B[This is square];
A_node-->C(Chimpansen hoppar);
A_node-->D{Diamond};
A_node-->E((Circle));
A_node-->F(((Double circle)));
A_node-->G{{Hex}};
A_node-->H[[Subroutine]];
A_node-->I(-Ellipse-);
A_node-->J([Stadium]);
A_node-->K[(Cylinder)];
A_node-->L>Odd];
A_node-->M[/Lean right/];
A_node-->N[\Lean left\];
A_node-->O[/Trapezoid\];
A_node-->P[\Inv trapezoid/];
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let nodes = res.model["nodes"].as_array().unwrap();
        let find = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();

        assert_eq!(find("B")["shape"], json!("square"));
        assert_eq!(find("C")["shape"], json!("round"));
        assert_eq!(find("D")["shape"], json!("diamond"));
        assert_eq!(find("E")["shape"], json!("circle"));
        assert_eq!(find("F")["shape"], json!("doublecircle"));
        assert_eq!(find("G")["shape"], json!("hexagon"));
        assert_eq!(find("H")["shape"], json!("subroutine"));
        assert_eq!(find("I")["shape"], json!("ellipse"));
        assert_eq!(find("J")["shape"], json!("stadium"));
        assert_eq!(find("K")["shape"], json!("cylinder"));
        assert_eq!(find("L")["shape"], json!("odd"));
        assert_eq!(find("M")["shape"], json!("lean_right"));
        assert_eq!(find("N")["shape"], json!("lean_left"));
        assert_eq!(find("O")["shape"], json!("trapezoid"));
        assert_eq!(find("P")["shape"], json!("inv_trapezoid"));
    }

    #[test]
    fn parse_diagram_flowchart_rect_border_syntax_sets_rect_shape() {
        let engine = Engine::new();
        let text = "graph TD;A_node-->B[|borders:lt|This node has a graph as text];";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let b = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("B"))
            .unwrap();
        assert_eq!(b["shape"], json!("rect"));
        assert_eq!(b["label"], json!("This node has a graph as text"));
    }

    #[test]
    fn parse_diagram_flowchart_odd_vertex_allows_id_ending_with_minus() {
        let engine = Engine::new();
        let text = "graph TD;A_node-->odd->Vertex Text];";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let odd = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("odd-"))
            .unwrap();
        assert_eq!(odd["shape"], json!("odd"));
        assert_eq!(odd["label"], json!("Vertex Text"));
    }

    #[test]
    fn parse_diagram_flowchart_allows_brackets_inside_quoted_square_labels() {
        let engine = Engine::new();
        let text = "graph TD;A[\"chimpansen hoppar ()[]\"] --> C;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["shape"], json!("square"));
        assert_eq!(a["label"], json!("chimpansen hoppar ()[]"));
        assert_eq!(a["labelType"], json!("string"));
    }

    #[test]
    fn parse_diagram_flowchart_flow_text_error_cases_from_upstream_spec() {
        let engine = Engine::new();

        let err = block_on(engine.parse_diagram(
            "graph TD; A[This is a () in text];",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string().contains(
                "Invalid text label: contains structural characters; quote it to use them"
            )
        );

        let err = block_on(engine.parse_diagram(
            "graph TD;A(this node has \"string\" and text)-->|this link has \"string\" and text|C;",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string().contains(
                "Invalid text label: contains structural characters; quote it to use them"
            )
        );

        let err = block_on(engine.parse_diagram(
            "graph TD; A[This is a \\\"()\\\" in text];",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("Unterminated node label (missing `]`)")
        );

        let err = block_on(engine.parse_diagram(
            "graph TD; A[\"This is a \"()\" in text\"];",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("Invalid string label: contains nested quotes")
        );

        let err = block_on(engine.parse_diagram(
            "graph TD; node[hello ) world] --> works",
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(
            err.to_string().contains(
                "Invalid text label: contains structural characters; quote it to use them"
            )
        );

        let err = block_on(engine.parse_diagram("graph\nX(- My Text (", ParseOptions::default()))
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Unterminated node label (missing `-)`)")
        );
    }

    #[test]
    fn parse_diagram_flowchart_keywords_in_vertex_text_across_shapes() {
        let engine = Engine::new();

        let keywords = [
            "graph",
            "flowchart",
            "flowchart-elk",
            "style",
            "default",
            "linkStyle",
            "interpolate",
            "classDef",
            "class",
            "href",
            "call",
            "click",
            "_self",
            "_blank",
            "_parent",
            "_top",
            "end",
            "subgraph",
            "kitty",
        ];

        let shapes: [(&str, &str, &str); 14] = [
            ("[", "]", "square"),
            ("(", ")", "round"),
            ("{", "}", "diamond"),
            ("(-", "-)", "ellipse"),
            ("([", "])", "stadium"),
            (">", "]", "odd"),
            ("[(", ")]", "cylinder"),
            ("(((", ")))", "doublecircle"),
            ("[/", "\\]", "trapezoid"),
            ("[\\", "/]", "inv_trapezoid"),
            ("[/", "/]", "lean_right"),
            ("[\\", "\\]", "lean_left"),
            ("[[", "]]", "subroutine"),
            ("{{", "}}", "hexagon"),
        ];

        for keyword in keywords {
            for (open, close, shape) in shapes {
                let text = format!(
                    "graph TD;A_{keyword}_node-->B{open}This node has a {keyword} as text{close};"
                );
                let res = block_on(engine.parse_diagram(&text, ParseOptions::default()))
                    .unwrap()
                    .unwrap();
                let b = res
                    .model
                    .get("nodes")
                    .and_then(|v| v.as_array())
                    .unwrap()
                    .iter()
                    .find(|n| n["id"] == json!("B"))
                    .unwrap();
                assert_eq!(b["shape"], json!(shape));
                assert_eq!(
                    b["label"],
                    json!(format!("This node has a {keyword} as text"))
                );
            }

            let rect_text = format!(
                "graph TD;A_{keyword}_node-->B[|borders:lt|This node has a {keyword} as text];"
            );
            let res = block_on(engine.parse_diagram(&rect_text, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let b = res
                .model
                .get("nodes")
                .and_then(|v| v.as_array())
                .unwrap()
                .iter()
                .find(|n| n["id"] == json!("B"))
                .unwrap();
            assert_eq!(b["shape"], json!("rect"));
            assert_eq!(
                b["label"],
                json!(format!("This node has a {keyword} as text"))
            );
        }
    }

    #[test]
    fn parse_diagram_flowchart_allows_slashes_in_lean_vertices() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            "graph TD;A_node-->B[/This node has a / as text/];",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let b = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("B"))
            .unwrap();
        assert_eq!(b["shape"], json!("lean_right"));
        assert_eq!(b["label"], json!("This node has a / as text"));

        let res = block_on(engine.parse_diagram(
            r#"graph TD;A_node-->B[\This node has a \ as text\];"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let b = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("B"))
            .unwrap();
        assert_eq!(b["shape"], json!("lean_left"));
        assert_eq!(b["label"], json!(r#"This node has a \ as text"#));
    }

    #[test]
    fn parse_diagram_flowchart_misc_vertex_text_cases_from_flow_text_spec() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            "graph TD;A-->C{Chimpansen hoppar ???-???};",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let c = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("C"))
            .unwrap();
        assert_eq!(c["shape"], json!("diamond"));
        assert_eq!(c["label"], json!("Chimpansen hoppar ???-???"));

        let res = block_on(engine.parse_diagram(
            "graph TD;A-->C(Chimpansen hoppar ???  <br> -  ???);",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let c = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("C"))
            .unwrap();
        assert_eq!(c["shape"], json!("round"));
        assert_eq!(c["label"], json!("Chimpansen hoppar ???  <br> -  ???"));

        let res = block_on(
            engine.parse_diagram("graph TD;A-->C(妖忘折忘抖抉);", ParseOptions::default()),
        )
        .unwrap()
        .unwrap();
        let c = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("C"))
            .unwrap();
        assert_eq!(c["label"], json!("妖忘折忘抖抉"));

        let res = block_on(
            engine.parse_diagram(r#"graph TD;A-->C(c:\windows);"#, ParseOptions::default()),
        )
        .unwrap()
        .unwrap();
        let c = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("C"))
            .unwrap();
        assert_eq!(c["label"], json!(r#"c:\windows"#));
    }

    #[test]
    fn parse_diagram_flowchart_ellipse_vertex_text_and_unterminated_ellipse_errors() {
        let engine = Engine::new();

        let ok = block_on(engine.parse_diagram(
            "graph TD\nA(-this is an ellipse-)-->B",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let a = ok.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["shape"], json!("ellipse"));
        assert_eq!(a["label"], json!("this is an ellipse"));

        let bad = block_on(engine.parse_diagram("graph\nX(- My Text (", ParseOptions::default()));
        assert!(bad.is_err());
    }

    #[test]
    fn parse_diagram_flowchart_question_and_unicode_in_node_and_edge_text() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram("graph TD;A(?)-->|?|C;", ParseOptions::default()))
            .unwrap()
            .unwrap();
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["label"], json!("?"));
        assert_eq!(res.model["edges"][0]["label"], json!("?"));

        let res = block_on(engine.parse_diagram(
            "graph TD;A(谷豕那角??)-->|谷豕那角??|C;",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["label"], json!("谷豕那角??"));
        assert_eq!(res.model["edges"][0]["label"], json!("谷豕那角??"));

        let res = block_on(
            engine.parse_diagram("graph TD;A(,.?!+-*)-->|,.?!+-*|C;", ParseOptions::default()),
        )
        .unwrap()
        .unwrap();
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["label"], json!(",.?!+-*"));
        assert_eq!(res.model["edges"][0]["label"], json!(",.?!+-*"));
    }

    #[test]
    fn parse_diagram_flowchart_node_label_invalid_mixed_text_and_quotes_errors() {
        let engine = Engine::new();

        let bad = block_on(engine.parse_diagram(
            "graph TD; A[This is a () in text];",
            ParseOptions::default(),
        ));
        assert!(bad.is_err());

        let bad = block_on(engine.parse_diagram(
            "graph TD;A(this node has \"string\" and text)-->|this link has \"string\" and text|C;",
            ParseOptions::default(),
        ));
        assert!(bad.is_err());

        let bad = block_on(engine.parse_diagram(
            "graph TD; A[This is a \\\"()\\\" in text];",
            ParseOptions::default(),
        ));
        assert!(bad.is_err());

        let bad = block_on(engine.parse_diagram(
            "graph TD; A[\"This is a \"()\" in text\"];",
            ParseOptions::default(),
        ));
        assert!(bad.is_err());

        let bad = block_on(engine.parse_diagram(
            "graph TD; node[hello ) world] --> works",
            ParseOptions::default(),
        ));
        assert!(bad.is_err());
    }

    #[test]
    fn parse_diagram_flowchart_supports_subgraph_block() {
        let engine = Engine::new();
        let text = "graph TD;subgraph S;A-->B;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "flowchart-v2");
        assert_eq!(
            res.model["subgraphs"],
            json!([{
                "id": "S",
                "nodes": ["B", "A"],
                "title": "S",
                "classes": [],
                "styles": [],
                "dir": null,
                "labelType": "text"
            }])
        );
    }

    #[test]
    fn parse_diagram_flowchart_supports_nested_subgraphs() {
        let engine = Engine::new();
        let text = "graph TD;subgraph Outer;subgraph Inner;A-->B;end;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["subgraphs"],
            json!([{
                "id": "Inner",
                "nodes": ["B", "A"],
                "title": "Inner",
                "classes": [],
                "styles": [],
                "dir": null,
                "labelType": "text"
            }, {
                "id": "Outer",
                "nodes": ["Inner"],
                "title": "Outer",
                "classes": [],
                "styles": [],
                "dir": null,
                "labelType": "text"
            }])
        );
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_supports_explicit_id_and_title() {
        let engine = Engine::new();
        let text = "graph TD;subgraph ide1[one];A-->B;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["subgraphs"],
            json!([{
                "id": "ide1",
                "nodes": ["B", "A"],
                "title": "one",
                "classes": [],
                "styles": [],
                "dir": null,
                "labelType": "text"
            }])
        );
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_title_with_spaces_uses_auto_id() {
        let engine = Engine::new();
        let text = "graph TD;subgraph number as labels;A-->B;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["subgraphs"],
            json!([{
                "id": "subGraph0",
                "nodes": ["B", "A"],
                "title": "number as labels",
                "classes": [],
                "styles": [],
                "dir": null,
                "labelType": "text"
            }])
        );
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_direction_statement_sets_dir() {
        let engine = Engine::new();
        let text = "graph LR;subgraph TOP;direction TB;A-->B;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["subgraphs"],
            json!([{
                "id": "TOP",
                "nodes": ["B", "A"],
                "title": "TOP",
                "classes": [],
                "styles": [],
                "dir": "TB",
                "labelType": "text"
            }])
        );
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_inherits_global_direction_when_enabled() {
        let mut site = MermaidConfig::empty_object();
        site.set_value("flowchart.inheritDir", json!(true));
        let engine = Engine::new().with_site_config(site);
        let text = "graph LR;subgraph TOP;A-->B;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["subgraphs"][0]["dir"], json!("LR"));
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_tab_indentation_matches_mermaid_membership_order() {
        let engine = Engine::new();
        let text = "graph TB\nsubgraph One\n\ta1-->a2\nend";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["subgraphs"],
            json!([{
                "id": "One",
                "nodes": ["a2", "a1"],
                "title": "One",
                "classes": [],
                "styles": [],
                "dir": null,
                "labelType": "text"
            }])
        );
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_chain_membership_order_matches_mermaid() {
        let engine = Engine::new();
        let text = "graph TB\nsubgraph One\n\ta1-->a2-->a3\nend";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["subgraphs"][0]["nodes"],
            json!(["a3", "a2", "a1"])
        );
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_title_with_spaces_in_quotes_uses_auto_id() {
        let engine = Engine::new();
        let text = "graph TB\nsubgraph \"Some Title\"\n\ta1-->a2\nend";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["subgraphs"][0]["title"], json!("Some Title"));
        assert_eq!(res.model["subgraphs"][0]["id"], json!("subGraph0"));
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_id_and_title_notation() {
        let engine = Engine::new();
        let text = "graph TB\nsubgraph some-id[Some Title]\n\ta1-->a2\nend";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["subgraphs"][0]["id"], json!("some-id"));
        assert_eq!(res.model["subgraphs"][0]["title"], json!("Some Title"));
        assert_eq!(res.model["subgraphs"][0]["labelType"], json!("text"));
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_bracket_quoted_title_sets_label_type_string() {
        let engine = Engine::new();
        let text = "graph TD;subgraph uid2[\"text of doom\"];c-->d;end;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["subgraphs"][0]["id"], json!("uid2"));
        assert_eq!(res.model["subgraphs"][0]["title"], json!("text of doom"));
        assert_eq!(res.model["subgraphs"][0]["labelType"], json!("string"));
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_markdown_title_sets_label_type_markdown() {
        let engine = Engine::new();
        let text = "graph TD\nsubgraph \"`**Two**`\"\nA-->B\nend";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["subgraphs"][0]["title"], json!("**Two**"));
        assert_eq!(res.model["subgraphs"][0]["labelType"], json!("markdown"));
    }

    #[test]
    fn parse_diagram_flowchart_subgraph_supports_amp_group_syntax_minimally() {
        let engine = Engine::new();
        let text = "graph TD\nsubgraph myTitle\na & b --> c & e\nend";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["subgraphs"][0]["nodes"].as_array().unwrap();
        let as_set: std::collections::HashSet<String> = nodes
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(as_set.contains("a"));
        assert!(as_set.contains("b"));
        assert!(as_set.contains("c"));
        assert!(as_set.contains("e"));
    }

    #[test]
    fn parse_diagram_flowchart_style_statement_applies_vertex_styles() {
        let engine = Engine::new();
        let text = "graph TD;style Q background:#fff;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let q = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("Q"))
            .unwrap();
        assert_eq!(q["styles"], json!(["background:#fff"]));
    }

    #[test]
    fn parse_diagram_flowchart_classdef_and_class_assign_work() {
        let engine = Engine::new();
        let text = "graph TD;classDef exClass background:#bbb,border:1px solid red;a-->b;class a,b exClass;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["classDefs"]["exClass"],
            json!(["background:#bbb", "border:1px solid red"])
        );
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("a"))
            .unwrap();
        let b = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("b"))
            .unwrap();
        assert_eq!(a["classes"][0], json!("exClass"));
        assert_eq!(b["classes"][0], json!("exClass"));
    }

    #[test]
    fn parse_diagram_flowchart_inline_vertex_class_via_style_separator() {
        let engine = Engine::new();
        // Mermaid `encodeEntities(...)` treats `#bbb;` as an entity placeholder when semicolons
        // are used as statement separators. Use newlines to match upstream parsing behavior.
        let text = "graph TD\nclassDef exClass background:#bbb\nA-->B[test]:::exClass\n";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let b = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("B"))
            .unwrap();
        assert_eq!(b["classes"][0], json!("exClass"));
    }

    #[test]
    fn parse_diagram_flowchart_linkstyle_applies_edge_style_and_validates_bounds() {
        let engine = Engine::new();
        let ok = "graph TD\nA-->B\nlinkStyle 0 stroke-width:1px;";
        let res = block_on(engine.parse_diagram(ok, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["style"][0], json!("stroke-width:1px"));

        let bad = "graph TD\nA-->B\nlinkStyle 1 stroke-width:1px;";
        let err = block_on(engine.parse_diagram(bad, ParseOptions::default())).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Diagram parse error (flowchart-v2): The index 1 for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and 0. (Help: Ensure that the index is within the range of existing edges.)"
        );
    }

    #[test]
    fn parse_diagram_flowchart_linkstyle_default_interpolate_sets_edge_defaults() {
        let engine = Engine::new();
        let text = "graph TD\nA-->B\nlinkStyle default interpolate basis";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edgeDefaults"]["interpolate"], json!("basis"));
    }

    #[test]
    fn parse_diagram_flowchart_linkstyle_numbered_interpolate_sets_edges() {
        let engine = Engine::new();
        let text = "graph TD\nA-->B\nA-->C\nlinkStyle 0 interpolate basis\nlinkStyle 1 interpolate cardinal";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["interpolate"], json!("basis"));
        assert_eq!(res.model["edges"][1]["interpolate"], json!("cardinal"));
    }

    #[test]
    fn parse_diagram_flowchart_linkstyle_multi_numbered_interpolate_sets_edges() {
        let engine = Engine::new();
        let text = "graph TD\nA-->B\nA-->C\nlinkStyle 0,1 interpolate basis";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["interpolate"], json!("basis"));
        assert_eq!(res.model["edges"][1]["interpolate"], json!("basis"));
    }

    #[test]
    fn parse_diagram_flowchart_edge_curve_properties_using_edge_id() {
        let engine = Engine::new();
        let text = "graph TD\nA e1@-->B\nA uniqueName@-->C\ne1@{curve: basis}\nuniqueName@{curve: cardinal}";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edges"][0]["id"], json!("e1"));
        assert_eq!(res.model["edges"][1]["id"], json!("uniqueName"));
        assert_eq!(res.model["edges"][0]["interpolate"], json!("basis"));
        assert_eq!(res.model["edges"][1]["interpolate"], json!("cardinal"));
    }

    #[test]
    fn parse_diagram_flowchart_edge_curve_properties_does_not_override_default() {
        let engine = Engine::new();
        let text = "graph TD\nA e1@-->B\nA-->C\nlinkStyle default interpolate linear\ne1@{curve: stepAfter}";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edgeDefaults"]["interpolate"], json!("linear"));
        assert_eq!(res.model["edges"][0]["interpolate"], json!("stepAfter"));
    }

    #[test]
    fn parse_diagram_flowchart_edge_curve_properties_mixed_with_line_interpolation() {
        let engine = Engine::new();
        let text = "graph TD\nA e1@-->B-->D\nA-->C e4@-->D-->E\nlinkStyle default interpolate linear\nlinkStyle 1 interpolate basis\ne1@{curve: monotoneX}\ne4@{curve: stepBefore}";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["edgeDefaults"]["interpolate"], json!("linear"));
        assert_eq!(res.model["edges"][0]["interpolate"], json!("monotoneX"));
        assert_eq!(res.model["edges"][1]["interpolate"], json!("basis"));
        assert_eq!(res.model["edges"][3]["interpolate"], json!("stepBefore"));
    }

    #[test]
    fn parse_diagram_flowchart_click_link_sets_link_and_tooltip_and_clickable_class() {
        let engine = Engine::new();
        let text = "graph TD\nA-->B\nclick A href \"click.html\" \"tooltip\" _blank";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["link"], json!("click.html"));
        assert_eq!(a["linkTarget"], json!("_blank"));
        assert_eq!(res.model["tooltips"]["A"], json!("tooltip"));
        assert_eq!(a["classes"][0], json!("clickable"));
    }

    #[test]
    fn parse_diagram_flowchart_click_link_sanitizes_javascript_urls_when_not_loose() {
        let engine = Engine::new();
        let text = "graph TD\nA-->B\nclick A href \"javascript:alert(1)\" \"tooltip\" _blank";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let a = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("A"))
            .unwrap();
        assert_eq!(a["link"], json!("about:blank"));
        assert_eq!(a["linkTarget"], json!("_blank"));
    }

    #[test]
    fn parse_diagram_flowchart_style_statement_supports_multiple_styles() {
        let engine = Engine::new();
        let text = "graph TD;style R background:#fff,border:1px solid red;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let r = res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("R"))
            .unwrap();
        assert_eq!(
            r["styles"],
            json!(["background:#fff", "border:1px solid red"])
        );
    }

    #[test]
    fn parse_diagram_flowchart_classdef_supports_multiple_classes() {
        let engine = Engine::new();
        let text = "graph TD;classDef firstClass,secondClass background:#bbb,border:1px solid red;";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["classDefs"]["firstClass"],
            json!(["background:#bbb", "border:1px solid red"])
        );
        assert_eq!(
            res.model["classDefs"]["secondClass"],
            json!(["background:#bbb", "border:1px solid red"])
        );
    }

    #[test]
    fn parse_diagram_flowchart_inline_vertex_class_in_groups_matches_mermaid_style_spec() {
        let engine = Engine::new();
        let text = r#"
graph TD
  classDef C1 stroke-dasharray:4
  classDef C2 stroke-dasharray:6
  A & B:::C1 & D:::C1 --> E:::C2
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let find = |id: &str| {
            res.model["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == json!(id))
                .unwrap()
                .clone()
        };
        assert!(find("A")["classes"].as_array().unwrap().is_empty());
        assert_eq!(find("B")["classes"][0], json!("C1"));
        assert_eq!(find("D")["classes"][0], json!("C1"));
        assert_eq!(find("E")["classes"][0], json!("C2"));
    }

    #[cfg(feature = "large-features")]
    #[test]
    fn full_build_detects_mindmap() {
        let engine = Engine::new();
        let res = block_on(engine.parse_metadata("mindmap\n  root", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.diagram_type, "mindmap");
    }

    #[cfg(not(feature = "large-features"))]
    #[test]
    fn tiny_build_does_not_detect_mindmap() {
        let engine = Engine::new();
        let err = block_on(engine.parse_metadata("mindmap\n  root", ParseOptions::default()))
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("No diagram type detected matching given configuration")
        );
    }

    #[test]
    fn parse_diagram_flowchart_keyword_flowchart() {
        let engine = Engine::new();
        let res = block_on(engine.parse_diagram("flowchart TD\nA-->B", ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "flowchart-v2");
        assert_eq!(res.model["keyword"], json!("flowchart"));
        assert_eq!(res.model["direction"], json!("TB"));
        assert_eq!(res.model["subgraphs"], json!([]));
    }

    #[cfg(feature = "large-features")]
    #[test]
    fn full_build_detects_flowchart_elk_and_sets_layout() {
        let engine = Engine::new();
        let res =
            block_on(engine.parse_metadata("flowchart-elk TD\nA-->B", ParseOptions::default()))
                .unwrap()
                .unwrap();
        assert_eq!(res.diagram_type, "flowchart-elk");
        assert_eq!(res.effective_config.get_str("layout"), Some("elk"));
    }

    #[cfg(not(feature = "large-features"))]
    #[test]
    fn tiny_build_flowchart_elk_falls_back_to_flowchart_v2() {
        let engine = Engine::new();
        let res =
            block_on(engine.parse_metadata("flowchart-elk TD\nA-->B", ParseOptions::default()))
                .unwrap()
                .unwrap();
        assert_eq!(res.diagram_type, "flowchart-v2");
        assert_eq!(res.effective_config.get_str("layout"), None);
    }

    #[test]
    fn parse_diagram_sequence_basic_messages_and_notes() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
Alice->Bob:Hello Bob, how are you?
Note right of Bob: Bob thinks
Bob-->Alice: I am good thanks!"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "sequence");

        let msgs = res.model["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["from"], json!("Alice"));
        assert_eq!(msgs[0]["to"], json!("Bob"));
        assert_eq!(msgs[0]["message"], json!("Hello Bob, how are you?"));
        assert_eq!(msgs[0]["type"], json!(5));
        assert_eq!(msgs[0]["wrap"], json!(false));

        assert_eq!(msgs[1]["type"], json!(2));
        assert_eq!(msgs[1]["placement"], json!(1));
        assert_eq!(msgs[1]["from"], json!("Bob"));
        assert_eq!(msgs[1]["to"], json!("Bob"));
        assert_eq!(msgs[1]["message"], json!("Bob thinks"));

        assert_eq!(msgs[2]["from"], json!("Bob"));
        assert_eq!(msgs[2]["to"], json!("Alice"));
        assert_eq!(msgs[2]["message"], json!("I am good thanks!"));
        assert_eq!(msgs[2]["type"], json!(6));
    }

    #[test]
    fn parse_diagram_sequence_is_stateless_across_multiple_parses() {
        let engine = Engine::new();
        let first = r#"sequenceDiagram
Alice->Bob:Hello Bob, how are you?
Bob-->Alice:I am good thanks!"#;
        let second = r#"sequenceDiagram
Alice->John:Hello John, how are you?
John-->Alice:I am good thanks!"#;

        let a = block_on(engine.parse_diagram(first, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let b = block_on(engine.parse_diagram(second, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let a_msgs = a.model["messages"].as_array().unwrap();
        let b_msgs = b.model["messages"].as_array().unwrap();

        assert_eq!(a_msgs.len(), 2);
        assert_eq!(a_msgs[0]["id"], json!("0"));
        assert_eq!(a_msgs[1]["id"], json!("1"));
        assert_eq!(a_msgs[0]["from"], json!("Alice"));
        assert_eq!(a_msgs[0]["to"], json!("Bob"));
        assert_eq!(a_msgs[1]["from"], json!("Bob"));
        assert_eq!(a_msgs[1]["to"], json!("Alice"));

        assert_eq!(b_msgs.len(), 2);
        assert_eq!(b_msgs[0]["id"], json!("0"));
        assert_eq!(b_msgs[1]["id"], json!("1"));
        assert_eq!(b_msgs[0]["from"], json!("Alice"));
        assert_eq!(b_msgs[0]["to"], json!("John"));
        assert_eq!(b_msgs[1]["from"], json!("John"));
        assert_eq!(b_msgs[1]["to"], json!("Alice"));
    }

    #[test]
    fn parse_diagram_sequence_title_and_accessibility_fields() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
title: Diagram Title
accTitle: Accessible Title
accDescr: Accessible Description
Alice->Bob:Hello"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        assert_eq!(res.model["title"], json!("Diagram Title"));
        assert_eq!(res.model["accTitle"], json!("Accessible Title"));
        assert_eq!(res.model["accDescr"], json!("Accessible Description"));
    }

    #[test]
    fn parse_sanitizes_common_db_fields_in_strict_mode() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
title: <script>alert(1)</script><b>t</b>
accTitle: <script>alert(1)</script><b>a</b>
accDescr: <script>alert(1)</script><b>d</b>
Alice->Bob:Hello"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        assert_eq!(res.model["title"], json!("<b>t</b>"));
        assert_eq!(res.model["accTitle"], json!("<b>a</b>"));
        assert_eq!(res.model["accDescr"], json!("<b>d</b>"));
    }

    #[test]
    fn parse_diagram_sequence_wrap_directive_controls_default_wrap() {
        let engine = Engine::new();
        let text = r#"%%{wrap}%%
sequenceDiagram
Alice->Bob:Hello
Alice->Bob:nowrap:Hello again"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let msgs = res.model["messages"].as_array().unwrap();

        assert_eq!(msgs[0]["wrap"], json!(true));
        assert_eq!(msgs[1]["wrap"], json!(false));
        assert_eq!(msgs[1]["message"], json!("Hello again"));
    }

    #[test]
    fn parse_diagram_sequence_links() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant a as Alice
participant b as Bob
participant c as Charlie
links a: { "Repo": "https://repo.contoso.com/", "Dashboard": "https://dashboard.contoso.com/" }
links b: { "Dashboard": "https://dashboard.contoso.com/" }
links a: { "On-Call": "https://oncall.contoso.com/?svc=alice" }
link a: Endpoint @ https://alice.contoso.com
link a: Swagger @ https://swagger.contoso.com
link a: Tests @ https://tests.contoso.com/?svc=alice@contoso.com
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let actors = res.model["actors"].as_object().unwrap();
        assert_eq!(
            actors["a"]["links"]["Repo"],
            json!("https://repo.contoso.com/")
        );
        assert_eq!(actors["b"]["links"].get("Repo"), None);
        assert_eq!(
            actors["a"]["links"]["Dashboard"],
            json!("https://dashboard.contoso.com/")
        );
        assert_eq!(
            actors["b"]["links"]["Dashboard"],
            json!("https://dashboard.contoso.com/")
        );
        assert_eq!(
            actors["a"]["links"]["On-Call"],
            json!("https://oncall.contoso.com/?svc=alice")
        );
        assert_eq!(actors["c"]["links"].get("Dashboard"), None);
        assert_eq!(
            actors["a"]["links"]["Endpoint"],
            json!("https://alice.contoso.com")
        );
        assert_eq!(
            actors["a"]["links"]["Swagger"],
            json!("https://swagger.contoso.com")
        );
        assert_eq!(
            actors["a"]["links"]["Tests"],
            json!("https://tests.contoso.com/?svc=alice@contoso.com")
        );
    }

    #[test]
    fn parse_diagram_sequence_properties() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant a as Alice
participant b as Bob
participant c as Charlie
properties a: {"class": "internal-service-actor", "icon": "@clock"}
properties b: {"class": "external-service-actor", "icon": "@computer"}
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let actors = res.model["actors"].as_object().unwrap();
        assert_eq!(
            actors["a"]["properties"]["class"],
            json!("internal-service-actor")
        );
        assert_eq!(
            actors["b"]["properties"]["class"],
            json!("external-service-actor")
        );
        assert_eq!(actors["a"]["properties"]["icon"], json!("@clock"));
        assert_eq!(actors["b"]["properties"]["icon"], json!("@computer"));
        assert_eq!(actors["c"]["properties"].get("class"), None);
    }

    #[test]
    fn parse_diagram_sequence_box_color_and_membership() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
box green Group 1
participant a as Alice
participant b as Bob
end
participant c as Charlie
links a: { "Repo": "https://repo.contoso.com/", "Dashboard": "https://dashboard.contoso.com/" }
links b: { "Dashboard": "https://dashboard.contoso.com/" }
links a: { "On-Call": "https://oncall.contoso.com/?svc=alice" }
link a: Endpoint @ https://alice.contoso.com
link a: Swagger @ https://swagger.contoso.com
link a: Tests @ https://tests.contoso.com/?svc=alice@contoso.com
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let boxes = res.model["boxes"].as_array().unwrap();
        assert_eq!(boxes[0]["name"], json!("Group 1"));
        assert_eq!(boxes[0]["actorKeys"], json!(["a", "b"]));
        assert_eq!(boxes[0]["fill"], json!("green"));
    }

    #[test]
    fn parse_diagram_sequence_box_without_color_defaults_to_transparent() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
box Group 1
participant a as Alice
participant b as Bob
end
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let boxes = res.model["boxes"].as_array().unwrap();
        assert_eq!(boxes[0]["name"], json!("Group 1"));
        assert_eq!(boxes[0]["actorKeys"], json!(["a", "b"]));
        assert_eq!(boxes[0]["fill"], json!("transparent"));
    }

    #[test]
    fn parse_diagram_sequence_box_without_description_has_falsy_name() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
box aqua
participant a as Alice
participant b as Bob
end
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let boxes = res.model["boxes"].as_array().unwrap();
        assert!(boxes[0]["name"].is_null());
        assert_eq!(boxes[0]["actorKeys"], json!(["a", "b"]));
        assert_eq!(boxes[0]["fill"], json!("aqua"));
    }

    #[test]
    fn parse_diagram_sequence_box_rgb_color() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
box rgb(34, 56, 0) Group1
participant a as Alice
participant b as Bob
end
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let boxes = res.model["boxes"].as_array().unwrap();
        assert_eq!(boxes[0]["name"], json!("Group1"));
        assert_eq!(boxes[0]["fill"], json!("rgb(34, 56, 0)"));
        assert_eq!(boxes[0]["actorKeys"], json!(["a", "b"]));
    }

    #[test]
    fn parse_diagram_sequence_create_participant_and_actor() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant a as Alice
a ->>b: Hello Bob?
create participant c
b-->>c: Hello c!
c ->> b: Hello b?
create actor d as Donald
a ->> d: Hello Donald?
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let actors = res.model["actors"].as_object().unwrap();
        let created = res.model["createdActors"].as_object().unwrap();

        assert_eq!(actors["c"]["name"], json!("c"));
        assert_eq!(actors["c"]["description"], json!("c"));
        assert_eq!(actors["c"]["type"], json!("participant"));
        assert_eq!(created["c"], json!(1));

        assert_eq!(actors["d"]["name"], json!("d"));
        assert_eq!(actors["d"]["description"], json!("Donald"));
        assert_eq!(actors["d"]["type"], json!("actor"));
        assert_eq!(created["d"], json!(3));
    }

    #[test]
    fn parse_diagram_sequence_destroy_participant_marks_destroyed_actor_index() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant a as Alice
a ->>b: Hello Bob?
destroy a
b-->>a: Hello Alice!
b ->> c: Where is Alice?
destroy c
b ->> c: Where are you?
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let destroyed = res.model["destroyedActors"].as_object().unwrap();
        assert_eq!(destroyed["a"], json!(1));
        assert_eq!(destroyed["c"], json!(3));
    }

    #[test]
    fn parse_diagram_sequence_create_and_destroy_same_actor() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
a ->>b: Hello Bob?
create participant c
b ->>c: Hello c!
c ->> b: Hello b?
destroy c
b ->> c : Bye c !
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let created = res.model["createdActors"].as_object().unwrap();
        let destroyed = res.model["destroyedActors"].as_object().unwrap();
        assert_eq!(created["c"], json!(1));
        assert_eq!(destroyed["c"], json!(3));
    }

    #[test]
    fn parse_diagram_sequence_extended_participant_syntax_parses_type_override() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant Alice@{ "type" : "database" }
participant Bob@{ "type" : "database" }
participant Carl@{ type: "database" }
participant David@{ "type" : 'database' }
participant Eve@{ type: 'database' }
participant Favela@{ "type" : "database"    }
Bob->>+Alice: Hi Alice
Alice->>+Bob: Hi Bob
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let actors = res.model["actors"].as_object().unwrap();

        for id in ["Alice", "Bob", "Carl", "David", "Eve", "Favela"] {
            assert_eq!(actors[id]["type"], json!("database"));
            assert_eq!(actors[id]["description"], json!(id));
        }
    }

    #[test]
    fn parse_diagram_sequence_extended_participant_syntax_mixed_types_and_implicit_participants() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant lead
participant dsa@{ "type" : "queue" }
API->>+Database: getUserb
Database-->>-API: userb
dsa --> Database: hello
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let actors = res.model["actors"].as_object().unwrap();

        assert_eq!(actors["lead"]["type"], json!("participant"));
        assert_eq!(actors["lead"]["description"], json!("lead"));
        assert_eq!(actors["dsa"]["type"], json!("queue"));
        assert_eq!(actors["dsa"]["description"], json!("dsa"));

        assert_eq!(actors["API"]["type"], json!("participant"));
        assert_eq!(actors["Database"]["type"], json!("participant"));
    }

    #[test]
    fn parse_diagram_sequence_extended_participant_syntax_invalid_config_fails() {
        let engine = Engine::new();
        let bad_json = r#"sequenceDiagram
participant D@{ "type: "entity" }
participant E@{ "type": "dat
abase }
"#;
        assert!(block_on(engine.parse_diagram(bad_json, ParseOptions::default())).is_err());

        let missing_colon = r#"sequenceDiagram
participant C@{ "type" "control" }
C ->> C: action
"#;
        assert!(block_on(engine.parse_diagram(missing_colon, ParseOptions::default())).is_err());

        let missing_brace = r#"sequenceDiagram
participant E@{ "type": "entity"
E ->> E: process
"#;
        assert!(block_on(engine.parse_diagram(missing_brace, ParseOptions::default())).is_err());
    }

    #[test]
    fn parse_diagram_sequence_deactivate_inactive_participant_fails_like_upstream() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
participant user as End User
participant Server as Server
participant System as System
participant System2 as System2

user->>+Server: Test
user->>+Server: Test2
user->>System: Test
Server->>-user: Test
Server->>-user: Test2

%% The following deactivation of Server will fail
Server->>-user: Test3"#;

        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        assert!(
            err.to_string()
                .contains("Trying to inactivate an inactive participant (Server)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_diagram_class_text_label_member_annotation_and_css_classes() {
        let engine = Engine::new();
        let text = r#"classDiagram
class C1["Class 1 with text label"]
<<interface>> C1
C1: member1
cssClass "C1" styleClass
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "classDiagram");

        let c1 = &res.model["classes"]["C1"];
        assert_eq!(c1["label"], json!("Class 1 with text label"));
        assert_eq!(c1["cssClasses"], json!("default styleClass"));
        assert_eq!(c1["annotations"][0], json!("interface"));
        assert_eq!(c1["members"][0]["displayText"], json!("member1"));
    }

    #[test]
    fn parse_diagram_class_css_class_shorthand() {
        let engine = Engine::new();
        let text = r#"classDiagram
class C1["Class 1 with text label"]:::styleClass
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let c1 = &res.model["classes"]["C1"];
        assert_eq!(c1["label"], json!("Class 1 with text label"));
        assert_eq!(c1["cssClasses"], json!("default styleClass"));
    }

    #[test]
    fn parse_diagram_class_namespace_and_generic_methods() {
        let engine = Engine::new();
        let text = r#"classDiagram
namespace Company.Project {
  class User {
    +login(username: String, password: String)
    +logout()
  }
}
namespace Company.Project.Module {
  class GenericClass~T~ {
    +addItem(item: T)
    +getItem() T
  }
}
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let user = &res.model["classes"]["User"];
        assert_eq!(user["parent"], json!("Company.Project"));
        assert_eq!(
            user["methods"][0]["displayText"],
            json!("+login(username: String, password: String)")
        );
        assert_eq!(user["methods"][1]["displayText"], json!("+logout()"));

        let generic = &res.model["classes"]["GenericClass"];
        assert_eq!(generic["type"], json!("T"));
        assert_eq!(generic["parent"], json!("Company.Project.Module"));
        assert_eq!(
            generic["methods"][0]["displayText"],
            json!("+addItem(item: T)")
        );
        assert_eq!(
            generic["methods"][1]["displayText"],
            json!("+getItem() : T")
        );
    }

    #[test]
    fn parse_diagram_class_relation_with_label_and_direction() {
        let engine = Engine::new();
        let text = r#"classDiagram
direction LR
class Admin
class Report
Admin --> Report : generates
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["direction"], json!("LR"));

        let rels = res.model["relations"].as_array().unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["id1"], json!("Admin"));
        assert_eq!(rels[0]["id2"], json!("Report"));
        assert_eq!(rels[0]["title"], json!("generates"));
        assert_eq!(rels[0]["relation"]["lineType"], json!(0));
        assert_eq!(rels[0]["relation"]["type1"], json!(-1));
        assert_eq!(rels[0]["relation"]["type2"], json!(3));
    }

    #[test]
    fn parse_diagram_class_style_statement_sets_node_styles() {
        let engine = Engine::new();
        let text = r#"classDiagram
class Class01
style Class01 fill:#f9f,stroke:#333,stroke-width:4px
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let c = &res.model["classes"]["Class01"];
        assert_eq!(
            c["styles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn parse_diagram_class_classdef_applies_styles_to_css_classes() {
        let engine = Engine::new();
        let text = r#"classDiagram
class Class01
cssClass "Class01" pink
classDef pink fill:#f9f
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let c = &res.model["classes"]["Class01"];
        assert_eq!(c["cssClasses"], json!("default pink"));
        assert_eq!(c["styles"], json!(["fill:#f9f"]));
    }

    #[test]
    fn parse_diagram_class_multiple_classdefs_merge_styles() {
        let engine = Engine::new();
        let text = r#"classDiagram
class Class01:::pink
cssClass "Class01" bold
classDef pink fill:#f9f
classDef bold stroke:#333,stroke-width:6px
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let c = &res.model["classes"]["Class01"];
        assert_eq!(c["cssClasses"], json!("default pink bold"));
        assert_eq!(
            c["styles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:6px"])
        );
    }

    #[test]
    fn parse_diagram_class_link_and_click_statements_set_clickable_and_metadata() {
        let engine = Engine::new();
        let text = r#"classDiagram
class Class1
link Class1 "google.com" "A tooltip" _self
click Class1 href "example.com" "B tooltip" _blank
click Class1 call functionCall(test, test1, test2) "C tooltip"
callback Class1 "otherCall" "D tooltip"
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let c = &res.model["classes"]["Class1"];
        assert!(c["cssClasses"].as_str().unwrap().contains("clickable"));
        assert_eq!(c["link"], json!("example.com"));
        assert_eq!(c["linkTarget"], json!("_blank"));
        assert_eq!(c["tooltip"], json!("D tooltip"));
        assert_eq!(c["haveCallback"], json!(true));
        assert_eq!(c["callback"]["function"], json!("otherCall"));
        assert_eq!(c["callbackEffective"], json!(false));
    }

    #[test]
    fn parse_diagram_class_href_sanitizes_javascript_urls_when_not_loose() {
        let engine = Engine::new();
        let res = block_on(engine.parse_diagram(
            r#"classDiagram
class Class1
click Class1 href "javascript:alert(1)" "A tooltip" _self"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        let c = &res.model["classes"]["Class1"];
        assert_eq!(c["link"], json!("about:blank"));
        assert_eq!(c["linkTarget"], json!("_self"));
        assert_eq!(c["tooltip"], json!("A tooltip"));
    }

    #[test]
    fn parse_diagram_class_security_level_sandbox_forces_link_target_top() {
        let engine = Engine::new().with_site_config({
            let mut cfg = MermaidConfig::empty_object();
            cfg.set_value("securityLevel", json!("sandbox"));
            cfg
        });

        let text = r#"classDiagram
class Class1
click Class1 href "google.com" "A tooltip" _self
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let c = &res.model["classes"]["Class1"];
        assert_eq!(c["link"], json!("google.com"));
        assert_eq!(c["linkTarget"], json!("_top"));
    }

    #[test]
    fn parse_diagram_class_security_level_loose_marks_callback_effective() {
        let engine = Engine::new().with_site_config({
            let mut cfg = MermaidConfig::empty_object();
            cfg.set_value("securityLevel", json!("loose"));
            cfg
        });

        let text = r#"classDiagram
class Class1
click Class1 call functionCall() "A tooltip"
"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let c = &res.model["classes"]["Class1"];
        assert_eq!(c["haveCallback"], json!(true));
        assert_eq!(c["callback"]["function"], json!("functionCall"));
        assert!(c["callback"].get("args").is_none());
        assert_eq!(c["callbackEffective"], json!(true));
    }

    #[test]
    fn engine_with_site_config_preserves_default_renderer_for_detection() {
        let engine = Engine::new().with_site_config({
            let mut cfg = MermaidConfig::empty_object();
            cfg.set_value("securityLevel", json!("sandbox"));
            cfg
        });

        let text = r#"classDiagram
class Class1
"#;
        let res = block_on(engine.parse_metadata(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.diagram_type, "classDiagram");
    }

    #[test]
    fn parse_diagram_er_allows_standalone_entities() {
        let engine = Engine::new();
        let text = "erDiagram\nISLAND\nMAINLAND\n";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.meta.diagram_type, "er");
        assert_eq!(res.model["relationships"].as_array().unwrap().len(), 0);
        assert_eq!(res.model["entities"].as_object().unwrap().len(), 2);
        assert!(res.model["entities"].get("ISLAND").is_some());
        assert!(res.model["entities"].get("MAINLAND").is_some());
    }

    #[test]
    fn parse_diagram_er_parses_alias_and_attributes() {
        let engine = Engine::new();
        let text = r#"erDiagram
foo["bar"] {
  string title PK, FK "comment"
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        let e = &res.model["entities"]["foo"];
        assert_eq!(e["alias"], json!("bar"));
        assert_eq!(e["attributes"][0]["type"], json!("string"));
        assert_eq!(e["attributes"][0]["name"], json!("title"));
        assert_eq!(e["attributes"][0]["keys"], json!(["PK", "FK"]));
        assert_eq!(e["attributes"][0]["comment"], json!("comment"));
    }

    #[test]
    fn parse_diagram_er_empty_quoted_entity_name_is_error() {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram("erDiagram\n\"\"\n", ParseOptions::default()))
            .unwrap_err()
            .to_string();
        assert!(err.contains("DiagramParse") || err.contains("Unsupported") || !err.is_empty());
    }

    #[test]
    fn parse_diagram_er_rejects_percent_and_backslash_in_quoted_entity_name() {
        let engine = Engine::new();
        assert!(
            block_on(engine.parse_diagram("erDiagram\n\"Blo%rf\"\n", ParseOptions::default()))
                .is_err()
        );
        assert!(
            block_on(engine.parse_diagram("erDiagram\n\"Blo\\\\rf\"\n", ParseOptions::default()))
                .is_err()
        );
    }

    #[test]
    fn parse_diagram_er_supports_empty_attribute_blocks_and_multiple_blocks() {
        let engine = Engine::new();
        let text = r#"erDiagram
BOOK {}
BOOK {
  string title
}
BOOK{
  string author
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let e = &res.model["entities"]["BOOK"];
        assert_eq!(e["attributes"].as_array().unwrap().len(), 2);
        assert_eq!(e["attributes"][0]["name"], json!("title"));
        assert_eq!(e["attributes"][1]["name"], json!("author"));
    }

    #[test]
    fn parse_diagram_er_alias_applies_even_when_relationship_is_defined_first() {
        let engine = Engine::new();
        let text = r#"erDiagram
foo ||--o{ bar : rel
foo["batman"]
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["entities"]["foo"]["alias"], json!("batman"));
        assert_eq!(res.model["entities"]["bar"]["alias"], json!(""));
    }

    #[test]
    fn parse_diagram_er_allows_multiple_statements_without_newlines_like_upstream_jison() {
        let engine = Engine::new();
        let text = r#"erDiagram
foo ||--o{ bar : rel
buzz foo["batman"]
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        assert_eq!(res.model["entities"]["foo"]["alias"], json!("batman"));
        assert_eq!(res.model["entities"]["bar"]["alias"], json!(""));
        assert!(res.model["entities"].get("buzz").is_some());
    }

    #[test]
    fn parse_diagram_er_self_relationship_does_not_duplicate_entity() {
        let engine = Engine::new();
        let text = r#"erDiagram
NODE ||--o{ NODE : "leads to"
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["entities"].as_object().unwrap().len(), 1);
        let rels = res.model["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["entityA"], rels[0]["entityB"]);
    }

    #[test]
    fn parse_diagram_er_inline_class_assignment_applies_css_classes() {
        let engine = Engine::new();
        let text = r#"erDiagram
FOO:::pink
classDef pink fill:#f9f
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let e = &res.model["entities"]["FOO"];
        assert_eq!(e["cssClasses"], json!("default pink"));
        assert_eq!(res.model["classes"]["pink"]["styles"], json!(["fill:#f9f"]));
    }

    #[test]
    fn parse_diagram_er_direction_statement_sets_direction() {
        let engine = Engine::new();
        let text = r#"erDiagram
direction LR
A ||--o{ B : has
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["direction"], json!("LR"));
    }

    #[test]
    fn parse_diagram_er_allows_hyphen_and_underscore_in_unquoted_entity_name() {
        let engine = Engine::new();
        let text = "erDiagram\nDUCK-BILLED_PLATYPUS\n";
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert!(res.model["entities"].get("DUCK-BILLED_PLATYPUS").is_some());
    }

    #[test]
    fn parse_diagram_er_rejects_unquoted_entity_names_with_non_identifier_punctuation() {
        let engine = Engine::new();
        assert!(
            block_on(engine.parse_diagram("erDiagram\nBlo@rf\n", ParseOptions::default())).is_err()
        );
        assert!(
            block_on(engine.parse_diagram("erDiagram\nBlo?rf\n", ParseOptions::default())).is_err()
        );
    }

    #[test]
    fn parse_diagram_er_supports_attribute_name_brackets_and_parens() {
        let engine = Engine::new();
        let text = r#"erDiagram
BOOK {
  string author-ref[name](1)
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let attrs = res.model["entities"]["BOOK"]["attributes"]
            .as_array()
            .unwrap();
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0]["name"], json!("author-ref[name](1)"));
    }

    #[test]
    fn parse_diagram_er_allows_asterisk_at_start_of_attribute_name() {
        let engine = Engine::new();
        let text = r#"erDiagram
BOOK {
  string *title
  id *the_Primary_Key
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let attrs = res.model["entities"]["BOOK"]["attributes"]
            .as_array()
            .unwrap();
        assert_eq!(attrs.len(), 2);
    }

    #[test]
    fn parse_diagram_er_rejects_attribute_names_with_leading_numbers_dashes_or_brackets() {
        let engine = Engine::new();
        for bad in ["0author", "-author", "[author", "(author"] {
            let text = format!("erDiagram\nBOOK {{\n  string {bad}\n}}\n");
            assert!(block_on(engine.parse_diagram(&text, ParseOptions::default())).is_err());
        }
    }

    #[test]
    fn parse_diagram_er_supports_generic_and_array_and_limited_length_types() {
        let engine = Engine::new();
        let text = r#"erDiagram
BOOK {
  type~T~ type
  option~T~ readable "comment"
  string[] readers FK
  character(10) isbn FK
  varchar(5) postal_code "Five digits"
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let attrs = res.model["entities"]["BOOK"]["attributes"]
            .as_array()
            .unwrap();
        assert_eq!(attrs.len(), 5);
        assert_eq!(attrs[3]["type"], json!("character(10)"));
        assert_eq!(attrs[4]["type"], json!("varchar(5)"));
        assert_eq!(attrs[4]["comment"], json!("Five digits"));
    }

    #[test]
    fn parse_diagram_er_parses_many_constraints_and_comments() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER {
  int customer_number PK, FK "comment1"
  datetime customer_status_start_datetime PK,UK, FK
  datetime customer_status_end_datetime PK , UK "comment3"
  string customer_firstname
  string customer_lastname "comment5"
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let attrs = res.model["entities"]["CUSTOMER"]["attributes"]
            .as_array()
            .unwrap();
        assert_eq!(attrs[0]["keys"], json!(["PK", "FK"]));
        assert_eq!(attrs[0]["comment"], json!("comment1"));
        assert_eq!(attrs[1]["keys"], json!(["PK", "UK", "FK"]));
        assert_eq!(attrs[2]["keys"], json!(["PK", "UK"]));
        assert_eq!(attrs[2]["comment"], json!("comment3"));
        assert_eq!(attrs[3]["keys"], json!([]));
        assert_eq!(attrs[4]["keys"], json!([]));
        assert_eq!(attrs[4]["comment"], json!("comment5"));
    }

    #[test]
    fn parse_diagram_er_allows_multiple_relationships_between_same_two_entities() {
        let engine = Engine::new();
        let text = r#"erDiagram
CAR ||--o{ PERSON : "insured for"
CAR }o--|| PERSON : "owned by"
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["entities"].as_object().unwrap().len(), 2);
        assert_eq!(res.model["relationships"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn parse_diagram_er_supports_one_or_more_cardinality_markers() {
        let engine = Engine::new();
        let text = r#"erDiagram
A ||--|{ B : has
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let rels = res.model["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["relSpec"]["cardA"], json!("ONE_OR_MORE"));
        assert_eq!(rels[0]["relSpec"]["cardB"], json!("ONLY_ONE"));
    }

    #[test]
    fn parse_diagram_er_relationship_matrix_matches_upstream_spec_minimally() {
        let engine = Engine::new();
        let cases: &[(&str, &str, &str, &str)] = &[
            ("A ||--|{ B : has", "ONE_OR_MORE", "ONLY_ONE", "IDENTIFYING"),
            (
                "A ||..o{ B : has",
                "ZERO_OR_MORE",
                "ONLY_ONE",
                "NON_IDENTIFYING",
            ),
            (
                "A |o..o{ B : has",
                "ZERO_OR_MORE",
                "ZERO_OR_ONE",
                "NON_IDENTIFYING",
            ),
            (
                "A |o--|{ B : has",
                "ONE_OR_MORE",
                "ZERO_OR_ONE",
                "IDENTIFYING",
            ),
            ("A }|--|| B : has", "ONLY_ONE", "ONE_OR_MORE", "IDENTIFYING"),
            (
                "A }o--|| B : has",
                "ONLY_ONE",
                "ZERO_OR_MORE",
                "IDENTIFYING",
            ),
            (
                "A }o..o| B : has",
                "ZERO_OR_ONE",
                "ZERO_OR_MORE",
                "NON_IDENTIFYING",
            ),
            (
                "A }|..o| B : has",
                "ZERO_OR_ONE",
                "ONE_OR_MORE",
                "NON_IDENTIFYING",
            ),
            (
                "A |o..|| B : has",
                "ONLY_ONE",
                "ZERO_OR_ONE",
                "NON_IDENTIFYING",
            ),
            (
                "A ||..|| B : has",
                "ONLY_ONE",
                "ONLY_ONE",
                "NON_IDENTIFYING",
            ),
            ("A ||--o| B : has", "ZERO_OR_ONE", "ONLY_ONE", "IDENTIFYING"),
            (
                "A |o..o| B : has",
                "ZERO_OR_ONE",
                "ZERO_OR_ONE",
                "NON_IDENTIFYING",
            ),
            (
                "A }o--o{ B : has",
                "ZERO_OR_MORE",
                "ZERO_OR_MORE",
                "IDENTIFYING",
            ),
            (
                "A }|..|{ B : has",
                "ONE_OR_MORE",
                "ONE_OR_MORE",
                "NON_IDENTIFYING",
            ),
            (
                "A }o--|{ B : has",
                "ONE_OR_MORE",
                "ZERO_OR_MORE",
                "IDENTIFYING",
            ),
            (
                "A }|..o{ B : has",
                "ZERO_OR_MORE",
                "ONE_OR_MORE",
                "NON_IDENTIFYING",
            ),
            // relType variants
            (
                "A ||.-o{ B : has",
                "ZERO_OR_MORE",
                "ONLY_ONE",
                "NON_IDENTIFYING",
            ),
            (
                "A ||-.o{ B : has",
                "ZERO_OR_MORE",
                "ONLY_ONE",
                "NON_IDENTIFYING",
            ),
        ];

        for (line, card_a, card_b, rel_type) in cases {
            let text = format!("erDiagram\n{line}\n");
            let res = block_on(engine.parse_diagram(&text, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let rels = res.model["relationships"].as_array().unwrap();
            assert_eq!(rels.len(), 1, "{line}");
            assert_eq!(rels[0]["relSpec"]["cardA"], json!(*card_a), "{line}");
            assert_eq!(rels[0]["relSpec"]["cardB"], json!(*card_b), "{line}");
            assert_eq!(rels[0]["relSpec"]["relType"], json!(*rel_type), "{line}");
        }
    }

    #[test]
    fn parse_diagram_er_relationship_word_aliases_match_upstream_spec_minimally() {
        let engine = Engine::new();
        let cases: &[(&str, &str, &str, &str)] = &[
            (
                "A one or zero to many B : has",
                "ZERO_OR_MORE",
                "ZERO_OR_ONE",
                "IDENTIFYING",
            ),
            (
                "A one or many optionally to zero or one B : has",
                "ZERO_OR_ONE",
                "ONE_OR_MORE",
                "NON_IDENTIFYING",
            ),
            (
                "A zero or more to zero or many B : has",
                "ZERO_OR_MORE",
                "ZERO_OR_MORE",
                "IDENTIFYING",
            ),
            (
                "A many(0) to many(1) B : has",
                "ONE_OR_MORE",
                "ZERO_OR_MORE",
                "IDENTIFYING",
            ),
            (
                "A many optionally to one B : has",
                "ONLY_ONE",
                "ZERO_OR_MORE",
                "NON_IDENTIFYING",
            ),
            (
                "A only one optionally to 1+ B : has",
                "ONE_OR_MORE",
                "ONLY_ONE",
                "NON_IDENTIFYING",
            ),
            (
                "A 0+ optionally to 1 B : has",
                "ONLY_ONE",
                "ZERO_OR_MORE",
                "NON_IDENTIFYING",
            ),
            (
                "HOUSE one to one ROOM : contains",
                "ONLY_ONE",
                "ONLY_ONE",
                "IDENTIFYING",
            ),
        ];

        for (line, card_a, card_b, rel_type) in cases {
            let text = format!("erDiagram\n{line}\n");
            let res = block_on(engine.parse_diagram(&text, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let rels = res.model["relationships"].as_array().unwrap();
            assert_eq!(rels.len(), 1, "{line}");
            assert_eq!(rels[0]["relSpec"]["cardA"], json!(*card_a), "{line}");
            assert_eq!(rels[0]["relSpec"]["cardB"], json!(*card_b), "{line}");
            assert_eq!(rels[0]["relSpec"]["relType"], json!(*rel_type), "{line}");
        }
    }

    #[test]
    fn parse_diagram_er_rejects_invalid_relationship_syntax() {
        let engine = Engine::new();
        assert!(
            block_on(engine.parse_diagram("erDiagram\nA xxx B : has\n", ParseOptions::default()))
                .is_err()
        );
    }

    #[test]
    fn parse_diagram_er_style_statement_applies_styles() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER
style CUSTOMER color:red,stroke:blue,fill:#f9f
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["entities"]["CUSTOMER"]["cssStyles"],
            json!(["color:red", "stroke:blue", "fill:#f9f"])
        );
    }

    #[test]
    fn parse_diagram_er_style_statements_append_across_multiple_lines() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER
style CUSTOMER color:red
style CUSTOMER fill:#f9f
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["entities"]["CUSTOMER"]["cssStyles"],
            json!(["color:red", "fill:#f9f"])
        );
    }

    #[test]
    fn parse_diagram_er_class_statement_assigns_classes() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER
class CUSTOMER firstClass, secondClass, thirdClass
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["entities"]["CUSTOMER"]["cssClasses"],
            json!("default firstClass secondClass thirdClass")
        );
    }

    #[test]
    fn parse_diagram_er_class_statement_appends_across_multiple_lines() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER
class CUSTOMER firstClass
class CUSTOMER secondClass
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["entities"]["CUSTOMER"]["cssClasses"],
            json!("default firstClass secondClass")
        );
    }

    #[test]
    fn parse_diagram_er_classdef_defines_styles_and_text_styles() {
        let engine = Engine::new();
        let text = r#"erDiagram
classDef myClass fill:#f9f, stroke: red, color: pink
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["classes"]["myClass"],
            json!({
                "id": "myClass",
                "styles": ["fill:#f9f", "stroke:red", "color:pink"],
                "textStyles": ["color:pink"]
            })
        );
    }

    #[test]
    fn parse_diagram_er_classdef_supports_multiple_classes_in_one_statement() {
        let engine = Engine::new();
        let text = r#"erDiagram
classDef firstClass,secondClass fill:#f9f, stroke: red, color: pink
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["classes"]["firstClass"]["styles"][1],
            json!("stroke:red")
        );
        assert_eq!(
            res.model["classes"]["secondClass"]["styles"][2],
            json!("color:pink")
        );
    }

    #[test]
    fn parse_diagram_er_shorthand_class_assignment_variants_work() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER:::myClass
CUSTOMER2:::myClass {}
CUSTOMER3:::myClass {
  string name
}
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["entities"]["CUSTOMER"]["cssClasses"],
            json!("default myClass")
        );
        assert_eq!(
            res.model["entities"]["CUSTOMER2"]["cssClasses"],
            json!("default myClass")
        );
        assert_eq!(
            res.model["entities"]["CUSTOMER3"]["cssClasses"],
            json!("default myClass")
        );
    }

    #[test]
    fn parse_diagram_er_shorthand_assignment_supports_multiple_classes_and_alias() {
        let engine = Engine::new();
        let text = r#"erDiagram
c[CUSTOMER]:::firstClass,secondClass
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["entities"]["c"]["alias"], json!("CUSTOMER"));
        assert_eq!(
            res.model["entities"]["c"]["cssClasses"],
            json!("default firstClass secondClass")
        );
    }

    #[test]
    fn parse_diagram_er_shorthand_assignment_works_in_relationships() {
        let engine = Engine::new();
        let text = r#"erDiagram
CUSTOMER:::myClass ||--o{ PERSON:::myClass : allows
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(
            res.model["entities"]["CUSTOMER"]["cssClasses"],
            json!("default myClass")
        );
        assert_eq!(
            res.model["entities"]["PERSON"]["cssClasses"],
            json!("default myClass")
        );
    }

    #[test]
    fn parse_diagram_er_relationship_labels_allow_empty_quoted_and_unquoted() {
        let engine = Engine::new();
        let res_empty = block_on(engine.parse_diagram(
            "erDiagram\nCUSTOMER ||--|{ ORDER : \"\"\n",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(res_empty.model["relationships"][0]["roleA"], json!(""));

        let res_unquoted = block_on(engine.parse_diagram(
            "erDiagram\nCUSTOMER ||--|{ ORDER : places\n",
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res_unquoted.model["relationships"][0]["roleA"],
            json!("places")
        );
    }

    #[test]
    fn parse_diagram_er_parent_child_relationship_sets_md_parent_cardinality() {
        let engine = Engine::new();
        let text = r#"erDiagram
PROJECT u--o{ TEAM_MEMBER : "parent"
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let rel = &res.model["relationships"][0];
        assert_eq!(rel["relSpec"]["cardB"], json!("MD_PARENT"));
        assert_eq!(rel["relSpec"]["cardA"], json!("ZERO_OR_MORE"));
    }

    #[test]
    fn parse_diagram_er_allows_prototype_like_entity_names() {
        let engine = Engine::new();
        for name in ["__proto__", "constructor", "prototype"] {
            let text = format!("erDiagram\n{name} ||--|{{ ORDER : place\n");
            assert!(block_on(engine.parse_diagram(&text, ParseOptions::default())).is_ok());
        }
    }

    #[test]
    fn parse_diagram_er_parses_relationship_cardinality_and_type() {
        let engine = Engine::new();
        let text = r#"erDiagram
CAR ||--o{ DRIVER : "insured for"
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();

        assert_eq!(res.model["entities"].as_object().unwrap().len(), 2);
        let rels = res.model["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["roleA"], json!("insured for"));
        assert_eq!(rels[0]["relSpec"]["cardA"], json!("ZERO_OR_MORE"));
        assert_eq!(rels[0]["relSpec"]["cardB"], json!("ONLY_ONE"));
        assert_eq!(rels[0]["relSpec"]["relType"], json!("IDENTIFYING"));
    }

    #[test]
    fn parse_diagram_er_supports_numeric_cardinality_shorthands() {
        let engine = Engine::new();
        let text = r#"erDiagram
A 1+--0+ B : has
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let rels = res.model["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["relSpec"]["cardA"], json!("ZERO_OR_MORE"));
        assert_eq!(rels[0]["relSpec"]["cardB"], json!("ONE_OR_MORE"));
    }

    #[test]
    fn parse_diagram_er_acc_title_and_multiline_description() {
        let engine = Engine::new();
        let text = r#"erDiagram
accTitle: graph title
accDescr { this graph is
  about
  stuff
}
A ||--o{ B : has
"#;
        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        assert_eq!(res.model["accTitle"], json!("graph title"));
        assert_eq!(res.model["accDescr"], json!("this graph is\nabout\nstuff"));
    }

    #[test]
    fn parse_diagram_sequence_alt_multiple_elses_inserts_control_messages() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?

%% Comment
Note right of Bob: Bob thinks
alt isWell

Bob-->Alice: I am good thanks!
else isSick
Bob-->Alice: Feel sick...
else default
Bob-->Alice: :-)
end"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let messages = res.model["messages"].as_array().unwrap();

        assert_eq!(messages.len(), 9);
        assert_eq!(messages[1]["from"], json!("Bob"));
        assert_eq!(messages[2]["type"], json!(12));
        assert_eq!(messages[3]["from"], json!("Bob"));
        assert_eq!(messages[4]["type"], json!(13));
        assert_eq!(messages[5]["from"], json!("Bob"));
        assert_eq!(messages[6]["type"], json!(13));
        assert_eq!(messages[7]["from"], json!("Bob"));
        assert_eq!(messages[8]["type"], json!(14));
    }

    #[test]
    fn parse_diagram_sequence_critical_without_options() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
critical Establish a connection to the DB
Service-->DB: connect
end"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let messages = res.model["messages"].as_array().unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["type"], json!(27));
        assert_eq!(messages[1]["from"], json!("Service"));
        assert_eq!(messages[2]["type"], json!(29));
    }

    #[test]
    fn parse_diagram_sequence_critical_with_options() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
critical Establish a connection to the DB
Service-->DB: connect
option Network timeout
Service-->Service: Log error
option Credentials rejected
Service-->Service: Log different error
end"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let messages = res.model["messages"].as_array().unwrap();

        assert_eq!(messages.len(), 7);
        assert_eq!(messages[0]["type"], json!(27));
        assert_eq!(messages[1]["from"], json!("Service"));
        assert_eq!(messages[2]["type"], json!(28));
        assert_eq!(messages[3]["from"], json!("Service"));
        assert_eq!(messages[4]["type"], json!(28));
        assert_eq!(messages[5]["from"], json!("Service"));
        assert_eq!(messages[6]["type"], json!(29));
    }

    #[test]
    fn parse_diagram_sequence_break_block_inserts_control_messages() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
Consumer-->API: Book something
API-->BookingService: Start booking process
break when the booking process fails
API-->Consumer: show failure
end
API-->BillingService: Start billing process"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let messages = res.model["messages"].as_array().unwrap();

        assert_eq!(messages.len(), 6);
        assert_eq!(messages[0]["from"], json!("Consumer"));
        assert_eq!(messages[1]["from"], json!("API"));
        assert_eq!(messages[2]["type"], json!(30));
        assert_eq!(messages[3]["from"], json!("API"));
        assert_eq!(messages[4]["type"], json!(31));
        assert_eq!(messages[5]["from"], json!("API"));
    }

    #[test]
    fn parse_diagram_sequence_par_over_block() {
        let engine = Engine::new();
        let text = r#"sequenceDiagram
par_over Parallel overlap
Alice ->> Bob: Message
Note left of Alice: Alice note
Note right of Bob: Bob note
end"#;

        let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let messages = res.model["messages"].as_array().unwrap();

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0]["type"], json!(32));
        assert_eq!(messages[0]["message"], json!("Parallel overlap"));
        assert_eq!(messages[1]["from"], json!("Alice"));
        assert_eq!(messages[2]["from"], json!("Alice"));
        assert_eq!(messages[3]["from"], json!("Bob"));
        assert_eq!(messages[4]["type"], json!(21));
    }

    #[test]
    fn parse_diagram_sequence_special_characters_in_loop_opt_alt_par() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
loop -:<>,;# comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!("-:<>,"));

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
opt -:<>,;# comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!("-:<>,"));

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
alt -:<>,;# comment
Bob-->Alice: I am good thanks!
else ,<>:-#; comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!("-:<>,"));
        assert_eq!(messages[3]["message"], json!(",<>:-"));

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
par -:<>,;# comment
Bob-->Alice: I am good thanks!
and ,<>:-#; comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!("-:<>,"));
        assert_eq!(messages[3]["message"], json!(",<>:-"));
    }

    #[test]
    fn parse_diagram_sequence_no_label_loop_opt_alt_par() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
loop
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!(""));
        assert_eq!(messages[2]["message"], json!("I am good thanks!"));

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
opt # comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!(""));
        assert_eq!(messages[2]["message"], json!("I am good thanks!"));

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
alt;Bob-->Alice: I am good thanks!
else # comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!(""));
        assert_eq!(messages[2]["message"], json!("I am good thanks!"));
        assert_eq!(messages[3]["message"], json!(""));
        assert_eq!(messages[4]["message"], json!("I am good thanks!"));

        let res = block_on(engine.parse_diagram(
            r#"sequenceDiagram
Alice->Bob: Hello Bob, how are you?
par;Bob-->Alice: I am good thanks!
and # comment
Bob-->Alice: I am good thanks!
end"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let messages = res.model["messages"].as_array().unwrap();
        assert_eq!(messages[1]["message"], json!(""));
        assert_eq!(messages[2]["message"], json!("I am good thanks!"));
        assert_eq!(messages[3]["message"], json!(""));
        assert_eq!(messages[4]["message"], json!("I am good thanks!"));
    }

    #[test]
    fn parse_diagram_state_v2_alias_and_colon_description() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
state "Small State 1" as namedState1"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(res.meta.diagram_type, "stateDiagram");
        assert_eq!(
            res.model["states"]["namedState1"]["descriptions"][0],
            json!("Small State 1")
        );

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
namedState1 : Small State 1"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["states"]["namedState1"]["descriptions"][0],
            json!("Small State 1")
        );

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
namedState1:Small State 1"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["states"]["namedState1"]["descriptions"][0],
            json!("Small State 1")
        );
    }

    #[test]
    fn parse_diagram_state_v2_groups_and_unsafe_ids() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
state "Small State 1" as namedState1
state "Big State 1" as bigState1 {
  bigState1InternalState
}
namedState1 --> bigState1: should point to \nBig State 1 container

state "Small State 2" as namedState2
state bigState2 {
  bigState2InternalState
}
namedState2 --> bigState2: should point to \nbigState2 container"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        assert_eq!(
            res.model["states"]["bigState1"]["doc"][0]["id"],
            json!("bigState1InternalState")
        );
        assert_eq!(
            res.model["states"]["bigState2"]["doc"][0]["id"],
            json!("bigState2InternalState")
        );
        assert_eq!(res.model["relations"][0]["id1"], json!("namedState1"));
        assert_eq!(res.model["relations"][0]["id2"], json!("bigState1"));
        assert_eq!(res.model["relations"][1]["id1"], json!("namedState2"));
        assert_eq!(res.model["relations"][1]["id2"], json!("bigState2"));

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
[*] --> __proto__
__proto__ --> [*]"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert!(res.model["states"]["__proto__"].is_object());
        assert!(res.model["states"]["root_start"].is_object());
        assert!(res.model["states"]["root_end"].is_object());
    }

    #[test]
    fn parse_diagram_state_v2_classdef_class_and_shorthand() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
classDef exampleStyleClass background:#bbb,border:1.5px solid red;
a --> b:::exampleStyleClass
class a exampleStyleClass"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        assert_eq!(
            res.model["styleClasses"]["exampleStyleClass"]["styles"][0],
            json!("background:#bbb")
        );
        assert_eq!(
            res.model["styleClasses"]["exampleStyleClass"]["styles"][1],
            json!("border:1.5px solid red")
        );
        assert_eq!(
            res.model["states"]["a"]["classes"][0],
            json!("exampleStyleClass")
        );
        assert_eq!(
            res.model["states"]["b"]["classes"][0],
            json!("exampleStyleClass")
        );

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
classDef exampleStyleClass background:#bbb,border:1px solid red;
[*]:::exampleStyleClass --> b"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["states"]["root_start"]["classes"][0],
            json!("exampleStyleClass")
        );

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
classDef exampleStyleClass background:#bbb,border:1px solid red;
a-->b
class a,b,c, d, e exampleStyleClass"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        for id in ["a", "b", "c", "d", "e"] {
            assert_eq!(
                res.model["states"][id]["classes"][0],
                json!("exampleStyleClass")
            );
        }
    }

    #[test]
    fn parse_diagram_state_v2_style_statement_sets_node_styles_and_ignores_comments() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
id1
id2
style id1,id2 background:#bbb, font-weight:bold, font-style:italic;"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        assert_eq!(
            res.model["nodes"][0]["cssStyles"],
            json!(["background:#bbb", "font-weight:bold", "font-style:italic"])
        );
        assert_eq!(
            res.model["nodes"][1]["cssStyles"],
            json!(["background:#bbb", "font-weight:bold", "font-style:italic"])
        );

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
[*] --> Moving
Moving --> Still
Moving --> Crash
state Moving {
%% comment inside state
slow  --> fast
}"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        assert_eq!(
            res.model["states"]["Moving"]["doc"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn parse_diagram_state_v2_click_and_href_store_links() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
S1
click S1 "https://example.com" "Go to Example""#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["links"]["S1"]["url"],
            json!("https://example.com")
        );
        assert_eq!(res.model["links"]["S1"]["tooltip"], json!("Go to Example"));

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
S2
click S2 href "https://example.com""#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["links"]["S2"]["url"],
            json!("https://example.com")
        );
        assert_eq!(res.model["links"]["S2"]["tooltip"], json!(""));
    }

    #[test]
    fn parse_diagram_state_v2_note_right_of_and_block_note() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
[*] --> A
note right of A : This is a note"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["states"]["A"]["note"]["position"],
            json!("right of")
        );
        assert_eq!(
            res.model["states"]["A"]["note"]["text"],
            json!("This is a note")
        );

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
[*] --> A
note right of A
  line1
  line2
end note"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        let note_text = res.model["states"]["A"]["note"]["text"].as_str().unwrap();
        assert_eq!(note_text, "line1\nline2");

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram
foo: bar
note "This is a floating note" as N1"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            res.model["states"]["N1"]["note"]["text"],
            json!("This is a floating note")
        );
    }

    #[test]
    fn parse_diagram_state_v2_getdata_edges_and_note_edges() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
A --> B: hello"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        assert_eq!(res.model["edges"][0]["start"], json!("A"));
        assert_eq!(res.model["edges"][0]["end"], json!("B"));
        assert_eq!(res.model["edges"][0]["label"], json!("hello"));
        assert_eq!(res.model["edges"][0]["arrowhead"], json!("normal"));

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
[*] --> A
note left of A : note text"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        let note_edge = res.model["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["arrowhead"] == json!("none"))
            .unwrap();
        assert_eq!(note_edge["classes"], json!("transition note-edge"));
    }

    #[test]
    fn parse_diagram_state_v2_sanitizes_edge_labels_like_mermaid_common() {
        let engine = Engine::new();
        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
A --> B: hello<script>alert(1)</script>world"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(res.model["edges"][0]["label"], json!("helloworld"));
    }

    #[test]
    fn parse_diagram_state_v2_getdata_dom_id_counter_and_note_padding_match_mermaid() {
        let engine = Engine::new();

        let res = block_on(engine.parse_diagram(
            r#"stateDiagram-v2
A --> B
note right of A : note text"#,
            ParseOptions::default(),
        ))
        .unwrap()
        .unwrap();

        let nodes = res.model["nodes"].as_array().unwrap();
        let node_a = nodes.iter().find(|n| n["id"] == json!("A")).unwrap();
        let node_b = nodes.iter().find(|n| n["id"] == json!("B")).unwrap();
        let note_group = nodes
            .iter()
            .find(|n| n["id"] == json!("A----parent"))
            .unwrap();
        let note_node = nodes
            .iter()
            .find(|n| n["id"] == json!("A----note-1"))
            .unwrap();

        assert_eq!(node_a["domId"], json!("state-A-1"));
        assert_eq!(node_b["domId"], json!("state-B-0"));
        assert_eq!(note_group["domId"], json!("state-A----parent-1"));
        assert_eq!(note_node["domId"], json!("state-A----note-1"));
        assert_eq!(note_group["padding"], json!(16));
        assert_eq!(note_node["padding"], json!(15));
        assert_eq!(note_node["parentId"], json!("A----parent"));
    }
}
