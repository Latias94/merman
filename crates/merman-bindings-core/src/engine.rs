use crate::artifact_contract::{ValidatedArtifactContract, default_artifact_contract};
use crate::operation::{AdmittedArtifactOperation, BindingOperationOutput};
use crate::{
    BindingEngineServices, BindingError, BindingRuntimePolicy, RuntimePolicyExposure, common,
};
use merman::{OperationControl, OperationPhase};
#[cfg(feature = "analysis")]
use merman_analysis::Analyzer;
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub struct BindingEngine {
    artifact_contract: Arc<ValidatedArtifactContract>,
    runtime_policy_id: &'static str,
    runtime_policy: merman::runtime::RuntimePolicy,
    base_options: common::BaseBindingOptions,
    unchanged_request_validation: OnceLock<Result<(), BindingError>>,
    semantic: SemanticOperationEngine,
    #[cfg(feature = "analysis")]
    analyzer: Analyzer,
    #[cfg(feature = "svg")]
    render: crate::render::CachedRenderEngine,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::CachedAsciiEngine,
    services: BindingEngineServices,
}

pub(crate) enum PreparedRequestOverlay {
    Unchanged,
    Override(Box<BindingOperationConfigs>),
}

impl ValidatedArtifactContract {
    /// Creates a reusable engine bound to this exact transport contract.
    pub fn create_engine(&self, options_json: &[u8]) -> Result<BindingEngine, BindingError> {
        self.create_engine_with_services(options_json, BindingEngineServices::new())
    }

    /// Creates a reusable engine with immutable services bound to this exact transport contract.
    pub fn create_engine_with_services(
        &self,
        options_json: &[u8],
        services: BindingEngineServices,
    ) -> Result<BindingEngine, BindingError> {
        BindingEngine::from_options_and_services_for_contract(
            Arc::new(*self),
            options_json,
            services,
        )
    }
}

impl BindingEngine {
    /// Creates a deterministic engine that never consults ambient host state.
    pub fn new(options_json: &[u8]) -> Result<Self, BindingError> {
        let artifact_contract = default_artifact_contract();
        let (options, base_options) =
            common::parse_base_options_for_artifact(options_json, &artifact_contract)?;
        ensure_selected_runtime_policy(&options, BindingRuntimePolicy::Deterministic)?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            &options,
            merman::runtime::RuntimePolicy::deterministic(),
            BindingRuntimePolicy::Deterministic.id(),
            base_options,
            BindingEngineServices::new(),
        )
    }

    /// Creates an engine from the explicit `runtime_policy` binding option.
    ///
    /// An omitted policy selects deterministic behavior even when system adapters are compiled.
    pub fn from_options(options_json: &[u8]) -> Result<Self, BindingError> {
        Self::from_options_and_services(options_json, BindingEngineServices::new())
    }

    /// Creates an engine from options and one immutable constructor-owned service bundle.
    pub fn from_options_and_services(
        options_json: &[u8],
        services: BindingEngineServices,
    ) -> Result<Self, BindingError> {
        Self::from_options_and_services_for_contract(
            default_artifact_contract(),
            options_json,
            services,
        )
    }

    pub(crate) fn from_options_and_services_for_contract(
        artifact_contract: Arc<ValidatedArtifactContract>,
        options_json: &[u8],
        services: BindingEngineServices,
    ) -> Result<Self, BindingError> {
        let (options, base_options) =
            common::parse_base_options_for_artifact(options_json, &artifact_contract)?;
        artifact_contract.validate_engine_services(&services)?;
        services.validate_options(&options)?;
        validate_runtime_policy_exposure(&artifact_contract, &options)?;
        let (selection, runtime_policy) = common::selected_runtime_policy(&options)?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            &options,
            runtime_policy,
            selection.id(),
            base_options,
            services,
        )
    }

    /// Creates an engine that explicitly selects the compiled native runtime adapters.
    pub fn try_native(options_json: &[u8]) -> Result<Self, BindingError> {
        let artifact_contract = default_artifact_contract();
        let (options, base_options) =
            common::parse_base_options_for_artifact(options_json, &artifact_contract)?;
        ensure_selected_runtime_policy(&options, BindingRuntimePolicy::Native)?;
        artifact_contract.validate_native_runtime_policy()?;
        let runtime_policy =
            merman::runtime::RuntimePolicy::try_native().map_err(common::runtime_policy_error)?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            &options,
            runtime_policy,
            BindingRuntimePolicy::Native.id(),
            base_options,
            BindingEngineServices::new(),
        )
    }

    pub fn with_operation_context(
        options_json: &[u8],
        context: merman::runtime::OperationContext,
    ) -> Result<Self, BindingError> {
        let artifact_contract = default_artifact_contract();
        let (options, base_options) =
            common::parse_base_options_for_artifact(options_json, &artifact_contract)?;
        common::reject_selected_runtime_policy(&options, "operation-context")?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            &options,
            merman::runtime::RuntimePolicy::from_operation_context(context),
            "operation-context",
            base_options,
            BindingEngineServices::new(),
        )
    }

    pub fn with_runtime_policy(
        options_json: &[u8],
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let artifact_contract = default_artifact_contract();
        let (options, base_options) =
            common::parse_base_options_for_artifact(options_json, &artifact_contract)?;
        common::reject_selected_runtime_policy(&options, "custom")?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            &options,
            runtime_policy,
            "custom",
            base_options,
            BindingEngineServices::new(),
        )
    }

    fn with_parsed_options_and_services(
        artifact_contract: Arc<ValidatedArtifactContract>,
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        runtime_policy_id: &'static str,
        base_options: common::BaseBindingOptions,
        services: BindingEngineServices,
    ) -> Result<Self, BindingError> {
        let configs =
            BindingOperationConfigs::compile(options, runtime_policy.clone(), &artifact_contract)?;
        #[cfg(feature = "svg")]
        let render = configs.render.materialize(&services);
        Ok(Self {
            artifact_contract,
            runtime_policy_id,
            runtime_policy,
            base_options,
            unchanged_request_validation: OnceLock::new(),
            semantic: configs.semantic.materialize(),
            #[cfg(feature = "analysis")]
            analyzer: Analyzer::with_options(configs.analysis),
            #[cfg(feature = "svg")]
            render,
            #[cfg(feature = "ascii")]
            ascii: configs.ascii.materialize(),
            services,
        })
    }

    pub(crate) fn prepare_request_overlay(
        &self,
        operation: crate::BindingOperationKind,
        options_json: &[u8],
    ) -> Result<PreparedRequestOverlay, BindingError> {
        let overlay = common::parse_request_overlay_for_artifact(
            options_json,
            operation.resource_scope(),
            &self.artifact_contract,
        )?;
        match overlay {
            common::BindingRequestOverlay::Unchanged => {
                self.unchanged_request_validation
                    .get_or_init(|| {
                        self.base_options
                            .validate_unchanged_request(&self.artifact_contract)
                    })
                    .clone()?;
                Ok(PreparedRequestOverlay::Unchanged)
            }
            overlay @ common::BindingRequestOverlay::Override { .. } => {
                let options = self
                    .base_options
                    .apply_overlay(overlay, &self.artifact_contract)?;
                self.services.validate_options(&options)?;
                let configs = BindingOperationConfigs::compile(
                    &options,
                    self.runtime_policy.clone(),
                    &self.artifact_contract,
                )?;
                Ok(PreparedRequestOverlay::Override(Box::new(configs)))
            }
        }
    }

    pub(crate) fn execute_request_projection(
        &self,
        admitted: AdmittedArtifactOperation,
        configs: BindingOperationConfigs,
        source: &[u8],
        _uri: Option<&[u8]>,
        control: OperationControl,
    ) -> Result<BindingOperationOutput, BindingError> {
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let operation = admitted.operation();
        match operation.key() {
            crate::OperationKey::SemanticJson => configs
                .semantic
                .materialize()
                .parse_json_controlled(source, &control)
                .map(BindingOperationOutput::plain),
            crate::OperationKey::AnalysisJson
            | crate::OperationKey::AnalysisFactsJson
            | crate::OperationKey::ValidationJson
            | crate::OperationKey::DocumentAnalysisJson
            | crate::OperationKey::DocumentAnalysisFactsJson => {
                #[cfg(feature = "analysis")]
                {
                    let analyzer = Analyzer::with_options(configs.analysis);
                    let data = match operation.key() {
                        crate::OperationKey::AnalysisJson => {
                            analyze_json_with(&analyzer, source, &control)
                        }
                        crate::OperationKey::AnalysisFactsJson => {
                            analyze_facts_json_with(&analyzer, source, &control)
                        }
                        crate::OperationKey::ValidationJson => {
                            validate_json_with(&analyzer, source, &control)
                        }
                        crate::OperationKey::DocumentAnalysisJson => analyze_document_json_with(
                            &analyzer,
                            source,
                            _uri.expect("validated document URI presence"),
                            &control,
                        ),
                        crate::OperationKey::DocumentAnalysisFactsJson => {
                            analyze_document_facts_json_with(
                                &analyzer,
                                source,
                                _uri.expect("validated document URI presence"),
                                &control,
                            )
                        }
                        _ => unreachable!("analysis projection requires an analysis operation"),
                    }?;
                    control
                        .checkpoint_at(OperationPhase::Postprocess)
                        .map_err(BindingError::cancelled)?;
                    Ok(BindingOperationOutput::plain(data))
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
                    control
                        .checkpoint_at(OperationPhase::Layout)
                        .map_err(BindingError::cancelled)?;
                    configs
                        .ascii
                        .materialize()
                        .render_ascii(source, control.clone())
                        .map(BindingOperationOutput::plain)
                        .and_then(|output| {
                            control
                                .checkpoint_at(OperationPhase::Emit)
                                .map_err(BindingError::cancelled)?;
                            Ok(output)
                        })
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
                    control
                        .checkpoint_at(OperationPhase::Layout)
                        .map_err(BindingError::cancelled)?;
                    let render = configs.render.materialize(&self.services);
                    let output = match operation.key() {
                        crate::OperationKey::Svg => render
                            .render_svg(source, control.clone())
                            .map(BindingOperationOutput::plain),
                        crate::OperationKey::SvgPlanJson => render
                            .svg_plan_json(source, control.clone())
                            .map(BindingOperationOutput::plain),
                        crate::OperationKey::LayoutJson => render
                            .layout_json(source, control.clone())
                            .map(BindingOperationOutput::plain),
                        crate::OperationKey::Png => {
                            #[cfg(feature = "png")]
                            {
                                render.render_png_output(source, control.clone())
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
                                render.render_jpeg_output(source, control.clone())
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
                                render.render_pdf_output(source, control.clone())
                            }
                            #[cfg(not(feature = "pdf"))]
                            {
                                let _ = render;
                                Err(common::feature_required_error("PDF rendering", "pdf"))
                            }
                        }
                        _ => unreachable!("render projection requires a render operation"),
                    }?;
                    control
                        .checkpoint_at(OperationPhase::Postprocess)
                        .map_err(BindingError::cancelled)?;
                    Ok(output)
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

    pub(crate) fn admit_operation(
        &self,
        operation: crate::BindingOperationKind,
    ) -> Result<AdmittedArtifactOperation, BindingError> {
        self.artifact_contract.admit_operation(operation)
    }

    pub fn render_svg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("svg", source))
    }

    pub(crate) fn render_svg_data(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render.render_svg(source, control)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("SVG rendering", "svg"))
        }
    }

    pub fn render_png(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("png", source))
    }

    pub fn render_png_result(
        &self,
        source: &[u8],
    ) -> Result<crate::BindingOperationResult, BindingError> {
        self.execute(crate::BindingOperationRequest::new("png", source))
    }

    pub(crate) fn render_png_output(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "png")]
        {
            self.render.render_png_output(source, control)
        }

        #[cfg(not(feature = "png"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("PNG rendering", "png"))
        }
    }

    pub fn render_jpeg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("jpeg", source))
    }

    pub fn render_jpeg_result(
        &self,
        source: &[u8],
    ) -> Result<crate::BindingOperationResult, BindingError> {
        self.execute(crate::BindingOperationRequest::new("jpeg", source))
    }

    pub(crate) fn render_jpeg_output(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "jpeg")]
        {
            self.render.render_jpeg_output(source, control)
        }

        #[cfg(not(feature = "jpeg"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("JPEG rendering", "jpeg"))
        }
    }

    pub fn render_pdf(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("pdf", source))
    }

    pub fn render_pdf_result(
        &self,
        source: &[u8],
    ) -> Result<crate::BindingOperationResult, BindingError> {
        self.execute(crate::BindingOperationRequest::new("pdf", source))
    }

    pub(crate) fn render_pdf_output(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "pdf")]
        {
            self.render.render_pdf_output(source, control)
        }

        #[cfg(not(feature = "pdf"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("PDF rendering", "pdf"))
        }
    }

    pub fn render_ascii(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("ascii", source))
    }

    pub(crate) fn render_ascii_data(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "ascii")]
        {
            self.ascii.render_ascii(source, control)
        }

        #[cfg(not(feature = "ascii"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("ASCII rendering", "ascii"))
        }
    }

    pub fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("semantic-json", source))
    }

    pub(crate) fn parse_json_data_controlled(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        self.semantic.parse_json_controlled(source, control)
    }

    pub fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("layout-json", source))
    }

    pub(crate) fn layout_json_data(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render.layout_json(source, control)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("layout_json", "svg"))
        }
    }

    pub fn svg_plan_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("svg-plan-json", source))
    }

    pub(crate) fn svg_plan_json_data(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render.svg_plan_json(source, control)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error(
                "SVG capability planning",
                "svg",
            ))
        }
    }

    pub fn analyze_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("analysis-json", source))
    }

    pub(crate) fn analyze_json_data(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            analyze_json_with(&self.analyzer, source, control)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("analysis", "analysis"))
        }
    }

    pub fn analysis_facts_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new(
            "analysis-facts-json",
            source,
        ))
    }

    pub(crate) fn analysis_facts_json_data(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            analyze_facts_json_with(&self.analyzer, source, control)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("analysis facts", "analysis"))
        }
    }

    pub fn analyze_document_json(
        &self,
        source: &[u8],
        uri: &[u8],
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_data(
            crate::BindingOperationRequest::new("document-analysis-json", source).with_uri(uri),
        )
    }

    pub(crate) fn analyze_document_json_data(
        &self,
        source: &[u8],
        uri: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            analyze_document_json_with(&self.analyzer, source, uri, control)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, uri, control);
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
        self.execute_data(
            crate::BindingOperationRequest::new("document-analysis-facts-json", source)
                .with_uri(uri),
        )
    }

    pub(crate) fn analyze_document_facts_json_data(
        &self,
        source: &[u8],
        uri: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            analyze_document_facts_json_with(&self.analyzer, source, uri, control)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, uri, control);
            Err(common::feature_required_error(
                "document analysis facts",
                "analysis",
            ))
        }
    }

    pub fn validate_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new(
            "validation-json",
            source,
        ))
    }

    pub(crate) fn validate_json_data(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            validate_json_with(&self.analyzer, source, control)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("validation", "analysis"))
        }
    }
}

#[cfg(feature = "analysis")]
fn analyze_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    control: &OperationControl,
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let cancellation = merman_analysis::AnalysisCancellationToken::from_operation_control(control);
    analyzer
        .analyze_cancellable(Arc::from(source), &cancellation)
        .map_err(|error| analysis_cancelled(error, &cancellation))?
        .to_json_bytes()
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn analyze_facts_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    control: &OperationControl,
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let cancellation = merman_analysis::AnalysisCancellationToken::from_operation_control(control);
    analyzer
        .analyze_facts_cancellable(Arc::from(source), &cancellation)
        .map_err(|error| analysis_cancelled(error, &cancellation))?
        .to_json_bytes()
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn analyze_document_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    uri: &[u8],
    control: &OperationControl,
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let uri = common::source_text_utf8(uri)?;
    let descriptor = common::source_descriptor_for_uri(uri);
    let cancellation = merman_analysis::AnalysisCancellationToken::from_operation_control(control);
    analyzer
        .analyze_source_cancellable(Arc::from(source), descriptor, &cancellation)
        .map_err(|error| analysis_cancelled(error, &cancellation))?
        .to_json_bytes()
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn analyze_document_facts_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    uri: &[u8],
    control: &OperationControl,
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let uri = common::source_text_utf8(uri)?;
    let descriptor = common::source_descriptor_for_uri(uri);
    let cancellation = merman_analysis::AnalysisCancellationToken::from_operation_control(control);
    analyzer
        .analyze_source_facts_cancellable(Arc::from(source), descriptor, &cancellation)
        .map_err(|error| analysis_cancelled(error, &cancellation))?
        .to_json_bytes()
        .map_err(common::internal_json_error)
}

#[cfg(feature = "analysis")]
fn validate_json_with(
    analyzer: &Analyzer,
    source: &[u8],
    control: &OperationControl,
) -> Result<Vec<u8>, BindingError> {
    let source = common::source_text_utf8(source)?;
    let cancellation = merman_analysis::AnalysisCancellationToken::from_operation_control(control);
    let payload = analyzer
        .analyze_cancellable(Arc::from(source), &cancellation)
        .map_err(|error| analysis_cancelled(error, &cancellation))?;
    cancellation
        .checkpoint()
        .map_err(|error| analysis_cancelled(error, &cancellation))?;
    common::validation_payload_json_from_analysis(&payload)
}

#[cfg(feature = "analysis")]
fn analysis_cancelled(
    _: merman_analysis::AnalysisCancelled,
    cancellation: &merman_analysis::AnalysisCancellationToken,
) -> BindingError {
    BindingError::cancelled(cancellation.operation_cancellation().unwrap_or(
        merman::OperationCancelled {
            phase: OperationPhase::Analysis,
            reason: merman::CancelReason::Requested,
        },
    ))
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

fn validate_runtime_policy_exposure(
    artifact_contract: &ValidatedArtifactContract,
    options: &common::BindingOptions,
) -> Result<(), BindingError> {
    if options.runtime_policy != Some(BindingRuntimePolicy::Native) {
        return Ok(());
    }
    if artifact_contract.runtime_policy_exposure() == RuntimePolicyExposure::DeterministicOnly {
        return Err(BindingError::invalid_options_json(format!(
            "runtime_policy `native` is not exposed by target `{}`",
            artifact_contract.target().id()
        )));
    }
    artifact_contract.validate_native_runtime_policy()
}

pub(crate) struct BindingOperationConfigs {
    semantic: SemanticOperationConfig,
    #[cfg(feature = "analysis")]
    analysis: merman_analysis::AnalysisOptions,
    #[cfg(feature = "svg")]
    render: crate::render::RenderOperationConfig,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::AsciiOperationConfig,
}

impl BindingOperationConfigs {
    fn compile(
        options: &common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        artifact_contract: &ValidatedArtifactContract,
    ) -> Result<Self, BindingError> {
        let runtime_policy = common::binding_runtime_policy_from(options, runtime_policy)?;
        let semantic = SemanticOperationConfig::compile(options, runtime_policy.clone())?;
        #[cfg(feature = "analysis")]
        let analysis =
            common::artifact_analysis_options(options)?.with_runtime_policy(runtime_policy.clone());
        #[cfg(feature = "svg")]
        let render = crate::render::RenderOperationConfig::compile(
            options,
            runtime_policy.clone(),
            artifact_contract.render_capability_policy(),
        )?;
        #[cfg(not(feature = "svg"))]
        let _ = artifact_contract;
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
    fn parse_json_controlled(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        control
            .checkpoint_at(OperationPhase::Parse)
            .map_err(BindingError::cancelled)?;
        let source = common::source_text(source)?;
        self.resources
            .check_source_bytes(source)
            .map_err(common::input_resource_limit_error)?;
        let parsed = match self.engine.parse_diagram_for_render_model_controlled_sync(
            source,
            self.parse_options,
            control,
        ) {
            Err(error) => return Err(BindingError::cancelled(error)),
            Ok(result) => result.map_err(classify_semantic_error)?,
        }
        .ok_or_else(common::no_diagram_error)?;

        control
            .checkpoint_at(OperationPhase::Semantic)
            .map_err(BindingError::cancelled)?;

        self.resources
            .check_render_model(parsed.model())
            .map_err(common::input_resource_limit_error)?;

        let model = parsed
            .model()
            .compatibility_json_controlled(parsed.metadata(), control)
            .map_err(BindingError::cancelled)?
            .map_err(classify_semantic_error)?;
        control
            .checkpoint_at(OperationPhase::Postprocess)
            .map_err(BindingError::cancelled)?;
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
    #[cfg(feature = "svg")]
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[cfg(feature = "svg")]
    struct CountingHostTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "svg")]
    impl CountingHostTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[cfg(feature = "svg")]
    impl crate::HostTextMeasurer for CountingHostTextMeasurer {
        fn measure(
            &self,
            _request: crate::HostTextMeasurementRequest<'_>,
        ) -> crate::HostMeasurementResult {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
    }

    #[cfg(feature = "svg")]
    struct PanicHostTextMeasurer;

    #[cfg(feature = "svg")]
    impl crate::HostTextMeasurer for PanicHostTextMeasurer {
        fn measure(
            &self,
            _request: crate::HostTextMeasurementRequest<'_>,
        ) -> crate::HostMeasurementResult {
            panic!("engine construction must not invoke host text measurement")
        }
    }

    #[test]
    fn try_native_uses_the_exact_artifact_adapter_selection() {
        let contract = crate::artifact_contract::default_artifact_contract();
        let missing_adapter = contract
            .validate_native_runtime_policy()
            .err()
            .and_then(|error| error.capability_id());

        match (missing_adapter, BindingEngine::try_native(b"")) {
            (Some(expected), Err(error)) => {
                assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
                assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
                assert_eq!(error.capability_id(), Some(expected));
            }
            (None, Ok(_engine)) => {}
            (Some(expected), Ok(_)) => {
                panic!("try_native accepted an artifact missing `{expected}`")
            }
            (None, Err(error)) => panic!("complete native artifact was rejected: {error:?}"),
        }
    }

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

    #[cfg(feature = "svg")]
    #[test]
    fn constructor_owned_text_service_distinguishes_omitted_and_explicit_selectors() {
        for options in [b"".as_slice(), br#"{"environment":{}}"#.as_slice()] {
            BindingEngine::from_options_and_services(
                options,
                BindingEngineServices::new()
                    .with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
            )
            .expect("omitted selectors do not conflict or invoke the callback");
        }

        for selector in ["vendored", "parity", "deterministic"] {
            let options = format!(r#"{{"environment":{{"text_measurement":"{selector}"}}}}"#);
            let error = BindingEngine::from_options_and_services(
                options.as_bytes(),
                BindingEngineServices::new()
                    .with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
            )
            .err()
            .expect("an explicit selector must conflict with the constructor service");
            assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
            assert_eq!(
                error.message(),
                "constructor service `host-text-measurement` conflicts with explicit option `environment.text_measurement`"
            );
        }

        let null_error = BindingEngine::from_options_and_services(
            br#"{"environment":{"text_measurement":null}}"#,
            BindingEngineServices::new().with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
        )
        .err()
        .expect("an explicitly supplied null selector remains explicit provenance");
        assert_eq!(null_error.status(), crate::BindingStatus::InvalidArgument);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn request_overlay_text_selector_conflicts_before_execution_work() {
        let counter = CountingHostTextMeasurer::new();
        let services = BindingEngineServices::new().with_host_text_measurer(counter.clone());
        let engine = BindingEngine::from_options_and_services(b"", services).unwrap();
        assert_eq!(counter.calls(), 0, "construction must not call the host");

        for options in [
            br#"{"environment":{"text_measurement":"vendored"}}"#.as_slice(),
            br#"{"environment":{"text_measurement":"deterministic"}}"#.as_slice(),
            br#"{"environment":{"text_measurement":null}}"#.as_slice(),
        ] {
            let error = engine
                .execute(
                    crate::BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                        .with_options_json(options),
                )
                .expect_err("request-local selector must conflict before operation work");
            assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
            assert_eq!(counter.calls(), 0);
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn cloned_services_share_one_callback_across_immutable_engines() {
        let counter = CountingHostTextMeasurer::new();
        let services = BindingEngineServices::new().with_host_text_measurer(counter.clone());
        let first = BindingEngine::from_options_and_services(b"", services.clone()).unwrap();
        let second = BindingEngine::from_options_and_services(
            br#"{"environment":{"math_renderer":"none"}}"#,
            services,
        )
        .unwrap();
        assert_eq!(counter.calls(), 0, "construction must not call the host");

        first
            .execute(
                crate::BindingOperationRequest::new("svg", b"flowchart TD\nA[First]")
                    .with_options_json(br#"{"svg":{"diagram_id":"first"}}"#),
            )
            .unwrap();
        second.render_svg(b"flowchart TD\nB[Second]").unwrap();
        assert!(counter.calls() > 0);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn cloned_services_share_one_immutable_icon_registry_across_engines() {
        let pack = br#"{
            "prefix":"test",
            "icons":{
                "rocket":{
                    "body":"<path data-icon=\"binding-registry\" d=\"M0 0H16V16H0z\"/>"
                }
            }
        }"#;
        let registry = crate::build_icon_registry([crate::IconPack::new(pack)])
            .expect("valid Iconify pack through the binding admission seam");
        let services = BindingEngineServices::new().with_icon_registry(registry.clone());
        let first = BindingEngine::from_options_and_services(b"", services.clone()).unwrap();
        let second = BindingEngine::from_options_and_services(b"", services).unwrap();

        for engine in [&first, &second] {
            let svg = String::from_utf8(
                engine
                    .render_svg(b"flowchart TD\nA@{ icon: \"test:rocket\", label: \"A\" }")
                    .unwrap(),
            )
            .unwrap();
            assert!(svg.contains(r#"data-icon="binding-registry""#), "{svg}");
        }
        assert_eq!(registry.len(), 1);
    }

    #[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
    #[test]
    fn empty_icon_registry_matches_no_service_svg_output() {
        let registry = crate::build_icon_registry(std::iter::empty::<crate::IconPack<'_>>())
            .expect("an empty registry is a valid immutable value");
        let with_empty_registry = BindingEngine::from_options_and_services(
            b"",
            BindingEngineServices::new().with_icon_registry(registry),
        )
        .unwrap();
        let without_services = BindingEngine::new(b"").unwrap();
        let source = b"architecture-beta\n  service api(server)[API]";

        assert_eq!(
            with_empty_registry.render_svg(source).unwrap(),
            without_services.render_svg(source).unwrap()
        );
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
            let details = error
                .resource_details()
                .expect("structured resource details");
            assert_eq!(details.limit_id, limit);
            assert_eq!(details.phase, "layout_model");
            assert_eq!(details.max, (exact - 1) as u64);
            assert_eq!(details.profile, "constrained");
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

    #[cfg(feature = "svg")]
    #[test]
    fn engine_reuses_options_for_rendering() {
        let engine = BindingEngine::new(
            br#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "cached engine", "pipeline": "readable" }
            }"#,
        )
        .unwrap();

        let svg = String::from_utf8(engine.render_svg(b"flowchart TD\nA[Hello]").unwrap()).unwrap();
        assert!(svg.contains("id=\"cached-engine\""));
        assert!(svg.contains("data-merman-foreignobject"));
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

    #[cfg(feature = "analysis")]
    #[test]
    fn document_analysis_enforces_the_profile_diagram_limit() {
        let engine = BindingEngine::new(br#"{"resources":{"profile":"constrained"}}"#).unwrap();
        let source = "```mermaid\nflowchart TD\nA\n```\n".repeat(129);

        let payload: Value = serde_json::from_slice(
            &engine
                .analyze_document_json(source.as_bytes(), b"file:///tmp/many.md")
                .expect("analysis rejection is represented as a diagnostics payload"),
        )
        .unwrap();

        assert_eq!(payload["valid"], false);
        assert_eq!(
            payload["diagnostics"][0]["id"],
            "merman.resource.document_diagrams_exceeded"
        );
        assert!(
            payload["diagnostics"][0]["message"]
                .as_str()
                .is_some_and(|message| message.contains("128"))
        );
    }

    #[cfg(not(feature = "analysis"))]
    #[test]
    fn engine_reports_missing_analysis_feature() {
        let engine = BindingEngine::new(b"").unwrap();

        let err = engine.validate_json(b"flowchart TD\nA").unwrap_err();
        assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
        assert_eq!(err.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(err.capability_id(), Some("analysis"));

        let err = engine
            .analyze_document_json(b"flowchart TD\nA", b"file:///tmp/example.mmd")
            .unwrap_err();
        assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
        assert_eq!(err.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(err.capability_id(), Some("analysis"));
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
