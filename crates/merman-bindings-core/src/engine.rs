use crate::{BindingError, BindingRuntimePolicy, common};
#[cfg(feature = "analysis")]
use merman_analysis::Analyzer;
use std::sync::Arc;

#[derive(Clone)]
pub struct BindingEngine {
    runtime_policy_id: &'static str,
    runtime_policy: merman::runtime::RuntimePolicy,
    base_options_json: Arc<[u8]>,
    #[cfg(feature = "svg")]
    host_text_measurer: Option<Arc<dyn crate::HostTextMeasurer>>,
    semantic: SemanticOperationEngine,
    #[cfg(feature = "analysis")]
    analyzer: Analyzer,
    #[cfg(feature = "svg")]
    render: crate::render::CachedRenderEngine,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::CachedAsciiEngine,
}

impl BindingEngine {
    /// Creates a deterministic engine that never consults ambient host state.
    pub fn new(options_json: &[u8]) -> Result<Self, BindingError> {
        let options = common::parse_options(options_json)?;
        ensure_selected_runtime_policy(&options, BindingRuntimePolicy::Deterministic)?;
        Self::with_parsed_options(
            &options,
            merman::runtime::RuntimePolicy::deterministic(),
            BindingRuntimePolicy::Deterministic.id(),
            Arc::from(options_json),
        )
    }

    /// Creates an engine from the explicit `runtime_policy` binding option.
    ///
    /// An omitted policy selects deterministic behavior even when system adapters are compiled.
    pub fn from_options(options_json: &[u8]) -> Result<Self, BindingError> {
        let options = common::parse_options(options_json)?;
        let (selection, runtime_policy) = common::selected_runtime_policy(&options)?;
        Self::with_parsed_options(
            &options,
            runtime_policy,
            selection.id(),
            Arc::from(options_json),
        )
    }

    /// Creates an engine that explicitly selects the compiled native runtime adapters.
    pub fn try_native(options_json: &[u8]) -> Result<Self, BindingError> {
        let options = common::parse_options(options_json)?;
        ensure_selected_runtime_policy(&options, BindingRuntimePolicy::Native)?;
        let runtime_policy =
            merman::runtime::RuntimePolicy::try_native().map_err(common::runtime_policy_error)?;
        Self::with_parsed_options(
            &options,
            runtime_policy,
            BindingRuntimePolicy::Native.id(),
            Arc::from(options_json),
        )
    }

    pub fn with_operation_context(
        options_json: &[u8],
        context: merman::runtime::OperationContext,
    ) -> Result<Self, BindingError> {
        let options = common::parse_options(options_json)?;
        common::reject_selected_runtime_policy(&options, "operation-context")?;
        Self::with_parsed_options(
            &options,
            merman::runtime::RuntimePolicy::from_operation_context(context),
            "operation-context",
            Arc::from(options_json),
        )
    }

    pub fn with_runtime_policy(
        options_json: &[u8],
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let options = common::parse_options(options_json)?;
        common::reject_selected_runtime_policy(&options, "custom")?;
        Self::with_parsed_options(&options, runtime_policy, "custom", Arc::from(options_json))
    }

    fn with_parsed_options(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        runtime_policy_id: &'static str,
        base_options_json: Arc<[u8]>,
    ) -> Result<Self, BindingError> {
        Ok(Self {
            runtime_policy_id,
            runtime_policy: runtime_policy.clone(),
            base_options_json,
            #[cfg(feature = "svg")]
            host_text_measurer: None,
            semantic: SemanticOperationEngine::with_runtime_policy(
                options,
                runtime_policy.clone(),
            )?,
            #[cfg(feature = "analysis")]
            analyzer: Analyzer::with_options(
                common::artifact_analysis_options(options)?.with_runtime_policy(
                    common::binding_runtime_policy_from(options, runtime_policy.clone())?,
                ),
            ),
            #[cfg(feature = "svg")]
            render: crate::render::CachedRenderEngine::with_runtime_policy(
                options,
                runtime_policy.clone(),
            )?,
            #[cfg(feature = "ascii")]
            ascii: crate::ascii::CachedAsciiEngine::with_runtime_policy(options, runtime_policy)?,
        })
    }

    pub(crate) fn for_request_options(
        &self,
        options_json: &[u8],
    ) -> Result<Option<Self>, BindingError> {
        if options_json.is_empty() {
            return Ok(None);
        }

        let merged_json = common::merge_request_options(&self.base_options_json, options_json)?;
        let options = common::parse_options(&merged_json)?;
        let engine = Self::with_parsed_options(
            &options,
            self.runtime_policy.clone(),
            self.runtime_policy_id,
            Arc::from(merged_json),
        )?;
        #[cfg(feature = "svg")]
        let engine = match &self.host_text_measurer {
            Some(measurer) => engine.with_host_text_measurer(Arc::clone(measurer)),
            None => engine,
        };
        Ok(Some(engine))
    }

    #[must_use]
    pub const fn runtime_policy_id(&self) -> &'static str {
        self.runtime_policy_id
    }

    pub fn render_svg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render.render_svg(source)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = source;
            Err(common::feature_required_error("SVG rendering", "svg"))
        }
    }

    pub fn render_png(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "png")]
        {
            self.render.render_png(source)
        }

        #[cfg(not(feature = "png"))]
        {
            let _ = source;
            Err(common::feature_required_error("PNG rendering", "png"))
        }
    }

    pub fn render_jpeg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "jpeg")]
        {
            self.render.render_jpeg(source)
        }

        #[cfg(not(feature = "jpeg"))]
        {
            let _ = source;
            Err(common::feature_required_error("JPEG rendering", "jpeg"))
        }
    }

    pub fn render_pdf(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "pdf")]
        {
            self.render.render_pdf(source)
        }

        #[cfg(not(feature = "pdf"))]
        {
            let _ = source;
            Err(common::feature_required_error("PDF rendering", "pdf"))
        }
    }

    #[cfg(feature = "svg")]
    pub fn with_host_text_measurer(&self, measurer: Arc<dyn crate::HostTextMeasurer>) -> Self {
        Self {
            runtime_policy_id: self.runtime_policy_id,
            runtime_policy: self.runtime_policy.clone(),
            base_options_json: Arc::clone(&self.base_options_json),
            host_text_measurer: Some(Arc::clone(&measurer)),
            semantic: self.semantic.clone(),
            #[cfg(feature = "analysis")]
            analyzer: self.analyzer.clone(),
            render: self.render.with_host_text_measurer(measurer),
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
        self.semantic.parse_json(source)
    }

    pub fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render.layout_json(source)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = source;
            Err(common::feature_required_error("layout_json", "svg"))
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

fn ensure_selected_runtime_policy(
    options: &common::BindingOptions,
    expected: BindingRuntimePolicy,
) -> Result<(), BindingError> {
    if let Some(actual) = options.runtime_policy
        && actual != expected
    {
        return Err(BindingError::new(
            crate::BindingStatus::InvalidArgument,
            format!(
                "runtime_policy `{}` conflicts with the `{}` engine constructor",
                actual.id(),
                expected.id()
            ),
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct SemanticOperationEngine {
    engine: merman::Engine,
    parse_options: merman::ParseOptions,
    resources: merman::resources::InputResourcePolicy,
}

impl SemanticOperationEngine {
    fn with_runtime_policy(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let runtime_policy = common::binding_runtime_policy_from(options, runtime_policy)?;
        let mut engine = merman::Engine::new().with_runtime_policy(runtime_policy);
        if let Some(site_config) = common::binding_site_config(options)? {
            engine = engine.with_site_config(site_config);
        }

        let parse_options = if options
            .parse
            .as_ref()
            .and_then(|parse| parse.suppress_errors)
            .unwrap_or(false)
        {
            merman::ParseOptions::lenient()
        } else {
            merman::ParseOptions::strict()
        };
        let resources = common::binding_input_resource_policy(
            options.analysis.resources.as_ref(),
            common::InputResourceOperation::ArtifactUnion,
        )?;

        Ok(Self {
            engine,
            parse_options,
            resources,
        })
    }

    fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = common::source_text(source)?;
        self.resources.check_source_bytes(source).map_err(|error| {
            BindingError::new(
                crate::BindingStatus::ResourceLimitExceeded,
                error.to_string(),
            )
        })?;
        let parsed = self
            .engine
            .parse_diagram_sync(source, self.parse_options)
            .map_err(classify_semantic_error)?
            .ok_or_else(common::no_diagram_error)?;

        serde_json::to_vec(&parsed.model).map_err(common::internal_json_error)
    }
}

fn classify_semantic_error(error: merman::Error) -> BindingError {
    match error {
        merman::Error::RuntimePolicy(error) => common::runtime_policy_error(error),
        error => BindingError::new(crate::BindingStatus::ParseError, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(not(feature = "svg"), feature = "analysis"))]
    use serde_json::Value;
    use std::sync::Arc;

    #[cfg(not(feature = "svg"))]
    #[test]
    fn semantic_parse_is_available_without_renderer() {
        let engine = BindingEngine::new(b"").expect("semantic bindings engine");
        let model: Value = serde_json::from_slice(
            &engine
                .parse_json(b"flowchart TD\nA[Start] --> B[Done]")
                .expect("semantic JSON without SVG"),
        )
        .expect("semantic model JSON");

        assert_eq!(model["type"], "flowchart-v2");
    }

    #[test]
    fn engine_reuses_options_for_rendering() {
        let engine = BindingEngine::new(
            br#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "cached engine", "pipeline": "readable" }
            }"#,
        )
        .unwrap();

        let svg = engine.render_svg(b"flowchart TD\nA[Hello]");
        if cfg!(feature = "svg") {
            let svg = String::from_utf8(svg.unwrap()).unwrap();
            assert!(svg.contains("id=\"cached-engine\""));
            assert!(svg.contains("data-merman-foreignobject"));
        } else {
            assert_eq!(
                svg.unwrap_err().status(),
                crate::BindingStatus::UnsupportedOperation
            );
        }
    }

    #[cfg(all(feature = "svg", feature = "ascii"))]
    #[test]
    fn cached_engine_accepts_render_only_resource_limits() {
        let engine = BindingEngine::new(
            br#"{
                "resources": {
                    "profile": "constrained",
                    "limits": { "max_svg_bytes": 1048576 }
                }
            }"#,
        )
        .expect("a multi-operation engine must project each limit to its owning operation");

        let ascii = String::from_utf8(
            engine
                .render_ascii(b"flowchart TD\nA --> B")
                .expect("an unrelated render-only limit must not disable ASCII"),
        )
        .expect("ASCII output is UTF-8");
        assert!(!ascii.is_empty());
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
        assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
        assert!(err.message().contains("analysis feature"));

        let err = engine
            .analyze_document_json(b"flowchart TD\nA", b"file:///tmp/example.mmd")
            .unwrap_err();
        assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
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
                    if cfg!(feature = "svg") {
                        let svg = String::from_utf8(svg.unwrap()).unwrap();
                        assert!(svg.contains("<svg"));
                    } else {
                        let err = svg.unwrap_err();
                        assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
