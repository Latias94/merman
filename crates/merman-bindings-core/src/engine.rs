#[cfg(feature = "svg")]
mod prepared_source;
mod semantic_analysis;
#[cfg(feature = "svg")]
pub use prepared_source::BindingPreparedSource;
#[cfg(all(test, feature = "svg", feature = "diagram-flowchart"))]
use semantic_analysis::SEMANTIC_ENGINE_MATERIALIZATIONS;
#[cfg(all(test, feature = "fixes", feature = "diagram-flowchart"))]
use semantic_analysis::analysis_projection_options;
use semantic_analysis::{SemanticOperationConfig, SemanticOperationEngine};

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
    runtime_policy: merman::model::runtime::RuntimePolicy,
    base_options: common::BaseBindingOptions,
    unchanged_request_validation: OnceLock<Result<(), BindingError>>,
    // Keep option validation eager; full engines are needed only by their output consumers.
    semantic_config: SemanticOperationConfig,
    semantic: OnceLock<SemanticOperationEngine>,
    #[cfg(feature = "analysis")]
    analyzer: Analyzer,
    #[cfg(feature = "svg")]
    render_config: crate::render::RenderOperationConfig,
    #[cfg(feature = "svg")]
    render: OnceLock<crate::render::CachedRenderEngine>,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    base_theme: Option<merman::theme::ThemeSelection>,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::CachedAsciiEngine,
    services: BindingEngineServices,
}

pub(crate) enum PreparedRequestOverlay {
    Unchanged,
    #[cfg(feature = "svg")]
    Svg(common::LightSvgOverride),
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
            merman::model::runtime::RuntimePolicy::deterministic(),
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
        Self::from_parsed_options_for_contract(artifact_contract, &options, base_options, services)
    }

    pub(crate) fn from_parsed_options_for_contract(
        artifact_contract: Arc<ValidatedArtifactContract>,
        options: &common::BindingOptions,
        base_options: common::BaseBindingOptions,
        services: BindingEngineServices,
    ) -> Result<Self, BindingError> {
        artifact_contract.validate_engine_services(&services)?;
        services.validate_options(options)?;
        validate_runtime_policy_exposure(&artifact_contract, options)?;
        let (selection, runtime_policy) = common::selected_runtime_policy(options)?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            options,
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
        let runtime_policy = merman::model::runtime::RuntimePolicy::try_native()
            .map_err(common::runtime_policy_error)?;
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
        context: merman::model::runtime::OperationContext,
    ) -> Result<Self, BindingError> {
        let artifact_contract = default_artifact_contract();
        let (options, base_options) =
            common::parse_base_options_for_artifact(options_json, &artifact_contract)?;
        common::reject_selected_runtime_policy(&options, "operation-context")?;
        Self::with_parsed_options_and_services(
            artifact_contract,
            &options,
            merman::model::runtime::RuntimePolicy::from_operation_context(context),
            "operation-context",
            base_options,
            BindingEngineServices::new(),
        )
    }

    pub fn with_runtime_policy(
        options_json: &[u8],
        runtime_policy: merman::model::runtime::RuntimePolicy,
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
        runtime_policy: merman::model::runtime::RuntimePolicy,
        runtime_policy_id: &'static str,
        base_options: common::BaseBindingOptions,
        services: BindingEngineServices,
    ) -> Result<Self, BindingError> {
        let configs =
            BindingOperationConfigs::compile(options, runtime_policy.clone(), &artifact_contract)?;
        Ok(Self {
            artifact_contract,
            runtime_policy_id,
            runtime_policy,
            base_options,
            unchanged_request_validation: OnceLock::new(),
            semantic_config: configs.semantic,
            semantic: OnceLock::new(),
            #[cfg(feature = "analysis")]
            analyzer: Analyzer::with_options(configs.analysis),
            #[cfg(feature = "svg")]
            render_config: configs.render,
            #[cfg(feature = "svg")]
            render: OnceLock::new(),
            #[cfg(any(feature = "svg", feature = "ascii"))]
            base_theme: options.theme.clone(),
            #[cfg(feature = "ascii")]
            ascii: configs.ascii.materialize(),
            services,
        })
    }

    #[cfg(feature = "svg")]
    fn render_engine(&self) -> &crate::render::CachedRenderEngine {
        self.render
            .get_or_init(|| self.render_config.clone().materialize(&self.services))
    }

    pub(crate) fn prepare_request_overlay(
        &self,
        operation: crate::BindingOperationKind,
        options: common::BindingRequestOptions<'_>,
    ) -> Result<PreparedRequestOverlay, BindingError> {
        let overlay = common::parse_request_overlay_for_artifact(
            options,
            operation.resource_scope(),
            &self.artifact_contract,
        )?;
        #[cfg(feature = "svg")]
        let mut overlay = overlay;
        #[cfg(feature = "svg")]
        if operation.key() == crate::OperationKey::Svg
            && !self.base_options.has_analysis_wrapper()
            && let Some(overlay) = overlay.take_light_svg_override()
        {
            return Ok(PreparedRequestOverlay::Svg(overlay));
        }
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
                #[cfg(any(feature = "svg", feature = "ascii"))]
                let theme = overlay.select_theme(self.base_theme.as_ref());
                let options = self
                    .base_options
                    .apply_overlay(overlay, &self.artifact_contract)?;
                #[cfg(any(feature = "svg", feature = "ascii"))]
                let options = common::BindingOptions { theme, ..options };
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

    #[cfg(feature = "svg")]
    pub(crate) fn prepare_svg_diagram_id(
        &self,
        operation: crate::BindingOperationKind,
        diagram_id: &str,
    ) -> Result<PreparedRequestOverlay, BindingError> {
        const OPTIONS_FRAME_BYTES: usize = br#"{"svg":{"diagram_id":""}}"#.len();
        let max_json_bytes = OPTIONS_FRAME_BYTES.saturating_add(diagram_id.len().saturating_mul(6));
        if max_json_bytes <= crate::BINDING_OPTIONS_JSON_MAX_BYTES
            && !self.base_options.has_analysis_wrapper()
            && self
                .artifact_contract
                .exposes_option_group(crate::BindingOptionGroupKey::Svg)
        {
            return Ok(PreparedRequestOverlay::Svg(common::LightSvgOverride {
                diagram_id: Some(diagram_id.to_owned()),
            }));
        }

        #[derive(serde::Serialize)]
        struct Options<'a> {
            svg: Svg<'a>,
        }
        #[derive(serde::Serialize)]
        struct Svg<'a> {
            diagram_id: &'a str,
        }
        let options = Options {
            svg: Svg { diagram_id },
        };
        common::enforce_compact_options_budget(&options)?;
        let options = serde_json::to_vec(&options).map_err(common::internal_json_error)?;
        self.prepare_request_overlay(operation, common::BindingRequestOptions::Raw(&options))
    }

    pub(crate) fn execute_request_projection(
        &self,
        admitted: AdmittedArtifactOperation,
        configs: BindingOperationConfigs,
        source: &[u8],
        uri: Option<&[u8]>,
        control: OperationControl,
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(not(any(feature = "ascii", feature = "svg")))]
        let _ = include_metadata;
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let operation = admitted.operation();
        match operation.key() {
            crate::OperationKey::ExportThemePreset | crate::OperationKey::ResolveTheme => {
                unreachable!("theme operations own their request options")
            }
            crate::OperationKey::SemanticJson => configs
                .semantic
                .materialize()
                .parse_json_controlled(source, &control)
                .map(BindingOperationOutput::plain),
            crate::OperationKey::AnalysisJson
            | crate::OperationKey::AnalysisFactsJson
            | crate::OperationKey::ValidationJson
            | crate::OperationKey::DocumentAnalysisJson
            | crate::OperationKey::DocumentAnalysisFactsJson => self
                .execute_analysis_projection(
                    operation.key(),
                    #[cfg(feature = "analysis")]
                    configs.analysis,
                    source,
                    uri,
                    &control,
                )
                .map(BindingOperationOutput::plain),
            crate::OperationKey::Ascii => {
                #[cfg(feature = "ascii")]
                {
                    control
                        .checkpoint_at(OperationPhase::Layout)
                        .map_err(BindingError::cancelled)?;
                    configs
                        .ascii
                        .materialize()
                        .render_ascii_output(source, control.clone())
                        .map(|output| BindingOperationOutput::ascii(output, include_metadata))
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
            | crate::OperationKey::LayoutJson
            | crate::OperationKey::EdgeGeometryJson => {
                #[cfg(feature = "svg")]
                {
                    control
                        .checkpoint_at(OperationPhase::Layout)
                        .map_err(BindingError::cancelled)?;
                    let render = configs.render.materialize(&self.services);
                    let output = match operation.key() {
                        crate::OperationKey::Svg => render.render_svg_output(
                            source,
                            control.clone(),
                            include_metadata,
                            None,
                        ),
                        crate::OperationKey::SvgPlanJson => render
                            .svg_plan_json(source, control.clone())
                            .map(BindingOperationOutput::plain),
                        crate::OperationKey::LayoutJson => render
                            .layout_json(source, control.clone())
                            .map(BindingOperationOutput::plain),
                        crate::OperationKey::EdgeGeometryJson => render
                            .edge_geometry_json(source, control.clone())
                            .map(BindingOperationOutput::plain),
                        crate::OperationKey::Png => {
                            #[cfg(feature = "png")]
                            {
                                render.render_png_output(source, control.clone(), include_metadata)
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
                                render.render_jpeg_output(source, control.clone(), include_metadata)
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
                                render.render_pdf_output(source, control.clone(), include_metadata)
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
                        crate::OperationKey::EdgeGeometryJson => {
                            Err(common::feature_required_error("edge_geometry_json", "svg"))
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

    /// Returns metadata exposed by this engine's artifact.
    pub fn metadata_json(&self, id: &str) -> Result<Vec<u8>, BindingError> {
        self.artifact_contract.metadata_json(id)
    }

    /// Exports the editable v2 document for a built-in preset.
    pub fn export_theme_preset(&self, preset_id: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new(
            "export-theme-preset",
            preset_id,
        ))
    }

    /// Resolves the v2 document in a `resolve-theme` request envelope.
    pub fn resolve_theme(&self, request_json: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new(
            "resolve-theme",
            request_json,
        ))
    }

    pub(crate) fn render_svg_output(
        &self,
        source: &[u8],
        control: OperationControl,
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render_engine()
                .render_svg_output(source, control, include_metadata, None)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = (source, control, include_metadata);
            Err(common::feature_required_error("SVG rendering", "svg"))
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn execute_svg_override(
        &self,
        overlay: common::LightSvgOverride,
        source: &[u8],
        control: OperationControl,
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        for phase in [OperationPhase::Admission, OperationPhase::Layout] {
            control
                .checkpoint_at(phase)
                .map_err(BindingError::cancelled)?;
        }
        let output = self.render_engine().render_svg_output(
            source,
            control.clone(),
            include_metadata,
            Some(overlay),
        )?;
        control
            .checkpoint_at(OperationPhase::Postprocess)
            .map_err(BindingError::cancelled)?;
        Ok(output)
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
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "png")]
        {
            self.render_engine()
                .render_png_output(source, control, include_metadata)
        }

        #[cfg(not(feature = "png"))]
        {
            let _ = (source, control, include_metadata);
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
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "jpeg")]
        {
            self.render_engine()
                .render_jpeg_output(source, control, include_metadata)
        }

        #[cfg(not(feature = "jpeg"))]
        {
            let _ = (source, control, include_metadata);
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
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        #[cfg(feature = "pdf")]
        {
            self.render_engine()
                .render_pdf_output(source, control, include_metadata)
        }

        #[cfg(not(feature = "pdf"))]
        {
            let _ = (source, control, include_metadata);
            Err(common::feature_required_error("PDF rendering", "pdf"))
        }
    }

    pub fn render_ascii(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("ascii", source))
    }

    #[cfg(feature = "ascii")]
    pub fn render_ascii_result(
        &self,
        source: &[u8],
    ) -> Result<crate::BindingOperationResult, BindingError> {
        self.execute(crate::BindingOperationRequest::new("ascii", source))
    }

    #[cfg(not(feature = "ascii"))]
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

    #[cfg(feature = "ascii")]
    pub(crate) fn render_ascii_output(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<merman::ascii::AsciiOutput, BindingError> {
        self.ascii.render_ascii_output(source, control)
    }

    pub fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("layout-json", source))
    }

    pub fn edge_geometry_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new(
            "edge-geometry-json",
            source,
        ))
    }

    pub(crate) fn layout_json_data(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render_engine().layout_json(source, control)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("layout_json", "svg"))
        }
    }

    pub(crate) fn edge_geometry_json_data(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "svg")]
        {
            self.render_engine().edge_geometry_json(source, control)
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = (source, control);
            Err(common::feature_required_error("edge_geometry_json", "svg"))
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
            self.render_engine().svg_plan_json(source, control)
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
        runtime_policy: merman::model::runtime::RuntimePolicy,
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
            options.theme.clone(),
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

#[cfg(test)]
mod tests;
