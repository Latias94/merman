use crate::{BindingError, common};
#[cfg(feature = "analysis")]
use merman_analysis::Analyzer;
#[cfg(feature = "render")]
use std::sync::Arc;

#[derive(Clone)]
pub struct BindingEngine {
    #[cfg(feature = "analysis")]
    analyzer: Analyzer,
    #[cfg(feature = "render")]
    render: crate::render::CachedRenderEngine,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::CachedAsciiEngine,
}

impl BindingEngine {
    pub fn new(options_json: &[u8]) -> Result<Self, BindingError> {
        #[cfg(not(any(feature = "analysis", feature = "render", feature = "ascii")))]
        {
            let _ = options_json;
            Ok(Self {})
        }

        #[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
        {
            let options = common::parse_options(options_json)?;
            Ok(Self {
                #[cfg(feature = "analysis")]
                analyzer: Analyzer::with_options(common::analysis_options(&options)?),
                #[cfg(feature = "render")]
                render: crate::render::CachedRenderEngine::new(&options)?,
                #[cfg(feature = "ascii")]
                ascii: crate::ascii::CachedAsciiEngine::new(&options)?,
            })
        }
    }

    pub fn render_svg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.render_svg(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            Err(common::feature_required_error("SVG rendering", "render"))
        }
    }

    #[cfg(feature = "render")]
    pub fn with_text_measurer(&self, measurer: Arc<dyn crate::TextMeasurer + Send + Sync>) -> Self {
        Self {
            #[cfg(feature = "analysis")]
            analyzer: self.analyzer.clone(),
            render: self.render.with_text_measurer(measurer),
            #[cfg(feature = "ascii")]
            ascii: self.ascii.clone(),
        }
    }

    pub fn render_ascii(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "ascii")]
        {
            self.ascii.render_ascii(source)
        }

        #[cfg(not(feature = "ascii"))]
        {
            let _ = source;
            Err(common::feature_required_error("ASCII rendering", "ascii"))
        }
    }

    pub fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.parse_json(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            Err(common::feature_required_error("parse_json", "render"))
        }
    }

    pub fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.layout_json(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            Err(common::feature_required_error("layout_json", "render"))
        }
    }

    pub fn analyze_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            let source = common::source_text_utf8(source)?;
            self.analyzer
                .analyze_json(source)
                .map_err(common::internal_json_error)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = source;
            Err(common::feature_required_error("analysis", "analysis"))
        }
    }

    pub fn analysis_facts_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            let source = common::source_text_utf8(source)?;
            self.analyzer
                .analyze_facts_json(source)
                .map_err(common::internal_json_error)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = source;
            Err(common::feature_required_error("analysis facts", "analysis"))
        }
    }

    pub fn analyze_document_json(
        &self,
        source: &[u8],
        uri: &[u8],
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            let source = common::source_text_utf8(source)?;
            let uri = common::source_text_utf8(uri)?;
            let descriptor = common::source_descriptor_for_uri(uri);
            merman_analysis::analyze_document(source, &self.analyzer, descriptor)
                .to_json_bytes()
                .map_err(common::internal_json_error)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, uri);
            Err(common::feature_required_error(
                "document analysis",
                "analysis",
            ))
        }
    }

    pub fn analyze_document_facts_json(
        &self,
        source: &[u8],
        uri: &[u8],
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            let source = common::source_text_utf8(source)?;
            let uri = common::source_text_utf8(uri)?;
            let descriptor = common::source_descriptor_for_uri(uri);
            merman_analysis::analyze_document_facts(source, &self.analyzer, descriptor)
                .to_json_bytes()
                .map_err(common::internal_json_error)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, uri);
            Err(common::feature_required_error(
                "document analysis facts",
                "analysis",
            ))
        }
    }

    pub fn validate_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            common::validation_payload_json_from_analysis(&self.analyze_payload(source)?)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = source;
            Err(common::feature_required_error("validation", "analysis"))
        }
    }

    #[cfg(feature = "analysis")]
    fn analyze_payload(
        &self,
        source: &[u8],
    ) -> Result<merman_analysis::AnalysisPayload, BindingError> {
        let source = common::source_text_utf8(source)?;
        Ok(self.analyzer.analyze(source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "analysis")]
    use serde_json::Value;
    use std::sync::Arc;

    #[test]
    fn engine_reuses_options_for_rendering() {
        let engine = BindingEngine::new(
            br#"{
                "layout": { "text_measurer": "deterministic" },
                "svg": { "diagram_id": "cached engine", "pipeline": "readable" }
            }"#,
        )
        .unwrap();

        let svg = engine.render_svg(b"flowchart TD\nA[Hello]");
        if cfg!(feature = "render") {
            let svg = String::from_utf8(svg.unwrap()).unwrap();
            assert!(svg.contains("id=\"cached-engine\""));
            assert!(svg.contains("data-merman-foreignobject"));
        } else {
            assert_eq!(
                svg.unwrap_err().status(),
                crate::BindingStatus::UnsupportedFormat
            );
        }
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn engine_validates_with_cached_renderer() {
        let engine = BindingEngine::new(b"").unwrap();
        let validation: Value =
            serde_json::from_slice(&engine.validate_json(b"").unwrap()).unwrap();

        assert_eq!(validation["valid"], false);
        assert_eq!(validation["code_name"], "MERMAN_NO_DIAGRAM");
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn engine_analyzes_markdown_documents() {
        let engine = BindingEngine::new(b"").unwrap();
        let payload: Value = serde_json::from_slice(
            &engine
                .analyze_document_json(
                    b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
                    b"file:///tmp/example.md",
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(payload["source"]["kind"], "markdown");
        assert!(
            payload["diagnostics"][0]["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|related| related["message"] == "Mermaid fence 1")
        );
    }

    #[cfg(not(feature = "analysis"))]
    #[test]
    fn engine_reports_missing_analysis_feature() {
        let engine = BindingEngine::new(b"").unwrap();

        let err = engine.validate_json(b"flowchart TD\nA").unwrap_err();
        assert_eq!(err.status(), crate::BindingStatus::UnsupportedFormat);
        assert!(err.message().contains("analysis feature"));

        let err = engine
            .analyze_document_json(b"flowchart TD\nA", b"file:///tmp/example.mmd")
            .unwrap_err();
        assert_eq!(err.status(), crate::BindingStatus::UnsupportedFormat);
    }

    #[test]
    fn engine_can_render_concurrently() {
        let engine = Arc::new(BindingEngine::new(b"").unwrap());
        let mut handles = Vec::new();

        for _ in 0..8 {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                for _ in 0..8 {
                    let svg = engine.render_svg(b"flowchart TD\nA[Hello] --> B[World]");
                    if cfg!(feature = "render") {
                        let svg = String::from_utf8(svg.unwrap()).unwrap();
                        assert!(svg.contains("<svg"));
                    } else {
                        let err = svg.unwrap_err();
                        assert_eq!(err.status(), crate::BindingStatus::UnsupportedFormat);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
