use crate::{BindingError, BindingRuntimePolicy, common};
#[cfg(feature = "analysis")]
use merman_analysis::Analyzer;
#[cfg(feature = "svg")]
use std::sync::Arc;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct BindingEngine {
    runtime_policy_id: &'static str,
    runtime_policy: merman::runtime::RuntimePolicy,
    base_options: common::BaseBindingOptions,
    unchanged_request_validation: OnceLock<Result<(), BindingError>>,
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
        let (options, base_options) = common::parse_base_options(options_json)?;
        ensure_selected_runtime_policy(&options, BindingRuntimePolicy::Deterministic)?;
        Self::with_parsed_options(
            &options,
            merman::runtime::RuntimePolicy::deterministic(),
            BindingRuntimePolicy::Deterministic.id(),
            base_options,
        )
    }

    /// Creates an engine from the explicit `runtime_policy` binding option.
    ///
    /// An omitted policy selects deterministic behavior even when system adapters are compiled.
    pub fn from_options(options_json: &[u8]) -> Result<Self, BindingError> {
        let (options, base_options) = common::parse_base_options(options_json)?;
        let (selection, runtime_policy) = common::selected_runtime_policy(&options)?;
        Self::with_parsed_options(&options, runtime_policy, selection.id(), base_options)
    }

    /// Creates an engine that explicitly selects the compiled native runtime adapters.
    pub fn try_native(options_json: &[u8]) -> Result<Self, BindingError> {
        let (options, base_options) = common::parse_base_options(options_json)?;
        ensure_selected_runtime_policy(&options, BindingRuntimePolicy::Native)?;
        let runtime_policy =
            merman::runtime::RuntimePolicy::try_native().map_err(common::runtime_policy_error)?;
        Self::with_parsed_options(
            &options,
            runtime_policy,
            BindingRuntimePolicy::Native.id(),
            base_options,
        )
    }

    pub fn with_operation_context(
        options_json: &[u8],
        context: merman::runtime::OperationContext,
    ) -> Result<Self, BindingError> {
        let (options, base_options) = common::parse_base_options(options_json)?;
        common::reject_selected_runtime_policy(&options, "operation-context")?;
        Self::with_parsed_options(
            &options,
            merman::runtime::RuntimePolicy::from_operation_context(context),
            "operation-context",
            base_options,
        )
    }

    pub fn with_runtime_policy(
        options_json: &[u8],
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let (options, base_options) = common::parse_base_options(options_json)?;
        common::reject_selected_runtime_policy(&options, "custom")?;
        Self::with_parsed_options(&options, runtime_policy, "custom", base_options)
    }

    fn with_parsed_options(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        runtime_policy_id: &'static str,
        base_options: common::BaseBindingOptions,
    ) -> Result<Self, BindingError> {
        let configs = BindingOperationConfigs::compile(options, runtime_policy.clone())?;
        Ok(Self {
            runtime_policy_id,
            runtime_policy,
            base_options,
            unchanged_request_validation: OnceLock::new(),
            #[cfg(feature = "svg")]
            host_text_measurer: None,
            semantic: configs.semantic.materialize(),
            #[cfg(feature = "analysis")]
            analyzer: configs.analysis.materialize(),
            #[cfg(feature = "svg")]
            render: configs.render.materialize(),
            #[cfg(feature = "ascii")]
            ascii: configs.ascii.materialize(),
        })
    }

    pub(crate) fn execute_request_overlay(
        &self,
        operation: crate::BindingOperationKind,
        source: &[u8],
        uri: Option<&[u8]>,
        options_json: &[u8],
    ) -> Result<Option<Vec<u8>>, BindingError> {
        let overlay = common::parse_request_overlay(options_json, operation.resource_scope())?;
        match overlay {
            common::BindingRequestOverlay::Unchanged => {
                self.unchanged_request_validation
                    .get_or_init(|| self.base_options.validate_unchanged_request())
                    .clone()?;
                Ok(None)
            }
            overlay @ common::BindingRequestOverlay::Override { .. } => {
                let options = self.base_options.apply_overlay(overlay)?;
                let configs =
                    BindingOperationConfigs::compile(&options, self.runtime_policy.clone())?;
                self.execute_request_projection(operation, configs, source, uri)
                    .map(Some)
            }
        }
    }

    fn execute_request_projection(
        &self,
        operation: crate::BindingOperationKind,
        configs: BindingOperationConfigs,
        source: &[u8],
        _uri: Option<&[u8]>,
    ) -> Result<Vec<u8>, BindingError> {
        match operation.key() {
            crate::OperationKey::SemanticJson => configs.semantic.materialize().parse_json(source),
            crate::OperationKey::AnalysisJson
            | crate::OperationKey::AnalysisFactsJson
            | crate::OperationKey::ValidationJson
            | crate::OperationKey::DocumentAnalysisJson
            | crate::OperationKey::DocumentAnalysisFactsJson => {
                #[cfg(feature = "analysis")]
                {
                    let analyzer = configs.analysis.materialize();
                    match operation.key() {
                        crate::OperationKey::AnalysisJson => analyze_json_with(&analyzer, source),
                        crate::OperationKey::AnalysisFactsJson => {
                            analyze_facts_json_with(&analyzer, source)
                        }
                        crate::OperationKey::ValidationJson => {
                            validate_json_with(&analyzer, source)
                        }
                        crate::OperationKey::DocumentAnalysisJson => analyze_document_json_with(
                            &analyzer,
                            source,
                            _uri.expect("validated document URI presence"),
                        ),
                        crate::OperationKey::DocumentAnalysisFactsJson => {
                            analyze_document_facts_json_with(
                                &analyzer,
                                source,
                                _uri.expect("validated document URI presence"),
                            )
                        }
                        _ => unreachable!("analysis projection requires an analysis operation"),
                    }
                }
                #[cfg(not(feature = "analysis"))]
                {
                    let _ = configs;
                    match operation.key() {
                        crate::OperationKey::AnalysisJson => {
                            Err(common::feature_required_error("analysis", "analysis"))
                        }
                        crate::OperationKey::AnalysisFactsJson => {
                            Err(common::feature_required_error("analysis facts", "analysis"))
                        }
                        crate::OperationKey::ValidationJson => {
                            Err(common::feature_required_error("validation", "analysis"))
                        }
                        crate::OperationKey::DocumentAnalysisJson => Err(
                            common::feature_required_error("document analysis", "analysis"),
                        ),
                        crate::OperationKey::DocumentAnalysisFactsJson => Err(
                            common::feature_required_error("document analysis facts", "analysis"),
                        ),
                        _ => unreachable!("analysis projection requires an analysis operation"),
                    }
                }
            }
            crate::OperationKey::Ascii => {
                #[cfg(feature = "ascii")]
                {
                    configs.ascii.materialize().render_ascii(source)
                }
                #[cfg(not(feature = "ascii"))]
                {
                    let _ = configs;
                    Err(common::feature_required_error("ASCII rendering", "ascii"))
                }
            }
            crate::OperationKey::Svg
            | crate::OperationKey::SvgPlanJson
            | crate::OperationKey::Png
            | crate::OperationKey::Jpeg
            | crate::OperationKey::Pdf
            | crate::OperationKey::LayoutJson => {
                #[cfg(feature = "svg")]
                {
                    let render = configs.render.materialize();
                    let render = match &self.host_text_measurer {
                        Some(measurer) => render.with_host_text_measurer(Arc::clone(measurer)),
                        None => render,
                    };
                    match operation.key() {
                        crate::OperationKey::Svg => render.render_svg(source),
                        crate::OperationKey::SvgPlanJson => render.svg_plan_json(source),
                        crate::OperationKey::LayoutJson => render.layout_json(source),
                        crate::OperationKey::Png => {
                            #[cfg(feature = "png")]
                            {
                                render.render_png(source)
                            }
                            #[cfg(not(feature = "png"))]
                            {
                                let _ = render;
                                Err(common::feature_required_error("PNG rendering", "png"))
                            }
                        }
                        crate::OperationKey::Jpeg => {
                            #[cfg(feature = "jpeg")]
                            {
                                render.render_jpeg(source)
                            }
                            #[cfg(not(feature = "jpeg"))]
                            {
                                let _ = render;
                                Err(common::feature_required_error("JPEG rendering", "jpeg"))
                            }
                        }
                        crate::OperationKey::Pdf => {
                            #[cfg(feature = "pdf")]
                            {
                                render.render_pdf(source)
                            }
                            #[cfg(not(feature = "pdf"))]
                            {
                                let _ = render;
                                Err(common::feature_required_error("PDF rendering", "pdf"))
                            }
                        }
                        _ => unreachable!("render projection requires a render operation"),
                    }
                }
                #[cfg(not(feature = "svg"))]
                {
                    let _ = configs;
                    match operation.key() {
                        crate::OperationKey::Svg => {
                            Err(common::feature_required_error("SVG rendering", "svg"))
                        }
                        crate::OperationKey::SvgPlanJson => Err(common::feature_required_error(
                            "SVG capability planning",
                            "svg",
                        )),
                        crate::OperationKey::LayoutJson => {
                            Err(common::feature_required_error("layout_json", "svg"))
                        }
                        crate::OperationKey::Png => {
                            Err(common::feature_required_error("PNG rendering", "png"))
                        }
                        crate::OperationKey::Jpeg => {
                            Err(common::feature_required_error("JPEG rendering", "jpeg"))
                        }
                        crate::OperationKey::Pdf => {
                            Err(common::feature_required_error("PDF rendering", "pdf"))
                        }
                        _ => unreachable!("render projection requires a render operation"),
                    }
                }
            }
        }
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
            base_options: self.base_options.clone(),
            unchanged_request_validation: self.unchanged_request_validation.clone(),
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

    pub fn svg_plan_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render.svg_plan_json(source)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = source;
            Err(common::feature_required_error(
                "SVG capability planning",
                "svg",
            ))
        }
    }

    pub fn analyze_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            analyze_json_with(&self.analyzer, source)
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
            analyze_facts_json_with(&self.analyzer, source)
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
            analyze_document_json_with(&self.analyzer, source, uri)
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
            analyze_document_facts_json_with(&self.analyzer, source, uri)
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
            validate_json_with(&self.analyzer, source)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = source;
            Err(common::feature_required_error("validation", "analysis"))
        }
    }
}

#[cfg(feature = "analysis")]
fn analyze_json_with(analyzer: &Analyzer, source: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    analyzer
        .analyze_json(source)
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn analyze_facts_json_with(analyzer: &Analyzer, source: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    analyzer
        .analyze_facts_json(source)
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn analyze_document_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    uri: &[u8],
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let uri = common::source_text_utf8(uri)?;
    let descriptor = common::source_descriptor_for_uri(uri);
    merman_analysis::analyze_document(source, analyzer, descriptor)
        .to_json_bytes()
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn analyze_document_facts_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    uri: &[u8],
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let uri = common::source_text_utf8(uri)?;
    let descriptor = common::source_descriptor_for_uri(uri);
    merman_analysis::analyze_document_facts(source, analyzer, descriptor)
        .to_json_bytes()
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn validate_json_with(analyzer: &Analyzer, source: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    common::validation_payload_json_from_analysis(&analyzer.analyze(source))
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

struct BindingOperationConfigs {
    semantic: SemanticOperationConfig,
    #[cfg(feature = "analysis")]
    analysis: AnalysisOperationConfig,
    #[cfg(feature = "svg")]
    render: crate::render::RenderOperationConfig,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::AsciiOperationConfig,
}

impl BindingOperationConfigs {
    fn compile(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let semantic = SemanticOperationConfig::compile(options, runtime_policy.clone())?;
        #[cfg(feature = "analysis")]
        let analysis = AnalysisOperationConfig::compile(options, runtime_policy.clone())?;
        #[cfg(feature = "svg")]
        let render =
            crate::render::RenderOperationConfig::compile(options, runtime_policy.clone())?;
        #[cfg(feature = "ascii")]
        let ascii = crate::ascii::AsciiOperationConfig::compile(options, runtime_policy)?;

        Ok(Self {
            semantic,
            #[cfg(feature = "analysis")]
            analysis,
            #[cfg(feature = "svg")]
            render,
            #[cfg(feature = "ascii")]
            ascii,
        })
    }
}

#[cfg(feature = "analysis")]
struct AnalysisOperationConfig {
    options: merman_analysis::AnalysisOptions,
}

#[cfg(feature = "analysis")]
impl AnalysisOperationConfig {
    fn compile(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let analysis = common::artifact_analysis_options(options)?;
        let runtime_policy = common::binding_runtime_policy_from(options, runtime_policy)?;
        Ok(Self {
            options: analysis.with_runtime_policy(runtime_policy),
        })
    }

    fn materialize(self) -> Analyzer {
        Analyzer::with_options(self.options)
    }
}

struct SemanticOperationConfig {
    runtime_policy: merman::runtime::RuntimePolicy,
    site_config: Option<merman::MermaidConfig>,
    parse_options: merman::ParseOptions,
    resources: merman::resources::InputResourcePolicy,
}

impl SemanticOperationConfig {
    fn compile(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let runtime_policy = common::binding_runtime_policy_from(options, runtime_policy)?;
        let site_config = common::binding_site_config(options)?;
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
        let resources = common::binding_input_resource_policy(options.analysis.resources.as_ref())?;
        Ok(Self {
            runtime_policy,
            site_config,
            parse_options,
            resources,
        })
    }

    fn materialize(self) -> SemanticOperationEngine {
        let mut engine = merman::Engine::new().with_runtime_policy(self.runtime_policy);
        if let Some(site_config) = self.site_config {
            engine = engine.with_site_config(site_config);
        }
        SemanticOperationEngine {
            engine,
            parse_options: self.parse_options,
            resources: self.resources,
        }
    }
}

#[derive(Clone)]
struct SemanticOperationEngine {
    engine: merman::Engine,
    parse_options: merman::ParseOptions,
    resources: merman::resources::InputResourcePolicy,
}

impl SemanticOperationEngine {
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
            .parse_diagram_for_render_model_sync(source, self.parse_options)
            .map_err(classify_semantic_error)?
            .ok_or_else(common::no_diagram_error)?;

        self.resources
            .check_render_model(parsed.model())
            .map_err(|error| {
                BindingError::new(
                    crate::BindingStatus::ResourceLimitExceeded,
                    error.to_string(),
                )
            })?;

        let model = parsed
            .model()
            .compatibility_json(parsed.metadata())
            .map_err(classify_semantic_error)?;
        serde_json::to_vec(&model).map_err(common::internal_json_error)
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
    fn semantic_parse_enforces_model_item_budget() {
        assert_semantic_model_limit("max_model_items", b"flowchart TD\nA --> B");
    }

    #[test]
    fn semantic_parse_accepts_exact_semantic_model_item_budget() {
        let engine = BindingEngine::new(
            br#"{
                "resources": {
                    "profile": "constrained",
                    "limits": { "max_model_items": 3 }
                }
            }"#,
        )
        .expect("semantic-json engine with exact model item budget");

        engine
            .parse_json(b"flowchart TD\nA --> B")
            .expect("two nodes and one edge consume three semantic model items");
    }

    #[test]
    fn semantic_parse_preserves_the_public_compatibility_json_projection() {
        let source = "flowchart TD\nsubgraph outer\nA[Start] --> B[Done]\nend";
        let expected = merman::Engine::new()
            .parse_diagram_sync(source, merman::ParseOptions::strict())
            .expect("compatibility JSON parse")
            .expect("flowchart diagram")
            .model;
        let actual: Value = serde_json::from_slice(
            &BindingEngine::new(b"")
                .expect("semantic bindings engine")
                .parse_json(source.as_bytes())
                .expect("binding semantic JSON"),
        )
        .expect("binding semantic JSON value");

        assert_eq!(actual, expected);
    }

    #[test]
    fn semantic_parse_resource_boundaries_match_the_typed_model() {
        let source = "flowchart TD\nsubgraph outer\nsubgraph inner\nA[Start] --> B[Done]\nend\nend";
        let parsed = merman::Engine::new()
            .parse_diagram_for_render_model_sync(source, merman::ParseOptions::strict())
            .expect("typed model parse")
            .expect("flowchart diagram");
        let complexity = merman::resources::ModelComplexity::from_render_model(parsed.model());

        for (limit, exact) in [
            ("max_model_items", complexity.items),
            ("max_model_text_bytes", complexity.text_bytes),
            ("max_model_nesting_depth", complexity.nesting_depth),
        ] {
            assert!(exact > 0, "fixture must exercise {limit}");
            let exact_options = format!(
                r#"{{"resources":{{"profile":"constrained","limits":{{"{limit}":{exact}}}}}}}"#
            );
            BindingEngine::new(exact_options.as_bytes())
                .expect("exact semantic model limit")
                .parse_json(source.as_bytes())
                .unwrap_or_else(|error| panic!("exact {limit} boundary failed: {error:?}"));

            let lower_options = format!(
                r#"{{"resources":{{"profile":"constrained","limits":{{"{limit}":{}}}}}}}"#,
                exact - 1
            );
            let error = BindingEngine::new(lower_options.as_bytes())
                .expect("lower semantic model limit")
                .parse_json(source.as_bytes())
                .expect_err("one-below semantic model boundary must fail");
            assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
            assert!(error.message().contains(limit), "{error:?}");
        }
    }

    #[test]
    fn semantic_parse_enforces_model_text_budget() {
        assert_semantic_model_limit("max_model_text_bytes", b"flowchart TD\nA[Long label]");
    }

    #[test]
    fn semantic_parse_enforces_model_nesting_budget() {
        assert_semantic_model_limit(
            "max_model_nesting_depth",
            b"flowchart TD\nsubgraph outer\nsubgraph inner\nA --> B\nend\nend",
        );
    }

    fn assert_semantic_model_limit(limit: &str, source: &[u8]) {
        let options =
            format!(r#"{{"resources":{{"profile":"constrained","limits":{{"{limit}":1}}}}}}"#);
        let engine = BindingEngine::new(options.as_bytes())
            .expect("semantic-json must accept every semantic model resource budget");

        let error = engine.parse_json(source).unwrap_err();

        assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains(limit), "{error:?}");
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
