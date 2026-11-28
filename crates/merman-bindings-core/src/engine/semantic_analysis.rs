use super::BindingEngine;
#[cfg(feature = "analysis")]
use crate::artifact_contract::ValidatedArtifactContract;
use crate::{BindingError, OperationKey, common};
use merman::{OperationControl, OperationPhase};
#[cfg(feature = "analysis")]
use merman_analysis::Analyzer;
#[cfg(feature = "analysis")]
use std::sync::Arc;

#[cfg(test)]
thread_local! {
    pub(super) static SEMANTIC_ENGINE_MATERIALIZATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl BindingEngine {
    pub fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("semantic-json", source))
    }

    pub(crate) fn parse_json_data_controlled(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        self.semantic
            .get_or_init(|| self.semantic_config.clone().materialize())
            .parse_json_controlled(source, control)
    }

    pub(super) fn execute_analysis_projection(
        &self,
        operation: OperationKey,
        #[cfg(feature = "analysis")] options: merman_analysis::AnalysisOptions,
        source: &[u8],
        uri: Option<&[u8]>,
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            let analyzer = Analyzer::with_options(analysis_projection_options(
                options,
                operation != OperationKey::ValidationJson,
                &self.artifact_contract,
            ));
            let data = match operation {
                OperationKey::AnalysisJson => analyze_json_with(&analyzer, source, control),
                OperationKey::AnalysisFactsJson => {
                    analyze_facts_json_with(&analyzer, source, control)
                }
                OperationKey::ValidationJson => validate_json_with(&analyzer, source, control),
                OperationKey::DocumentAnalysisJson => analyze_document_json_with(
                    &analyzer,
                    source,
                    uri.expect("validated document URI presence"),
                    control,
                ),
                OperationKey::DocumentAnalysisFactsJson => analyze_document_facts_json_with(
                    &analyzer,
                    source,
                    uri.expect("validated document URI presence"),
                    control,
                ),
                _ => unreachable!("analysis projection requires an analysis operation"),
            }?;
            control
                .checkpoint_at(OperationPhase::Postprocess)
                .map_err(BindingError::cancelled)?;
            Ok(data)
        }
        #[cfg(not(feature = "analysis"))]
        {
            let _ = (source, uri, control);
            match operation {
                OperationKey::AnalysisJson => {
                    Err(common::feature_required_error("analysis", "analysis"))
                }
                OperationKey::AnalysisFactsJson => {
                    Err(common::feature_required_error("analysis facts", "analysis"))
                }
                OperationKey::ValidationJson => {
                    Err(common::feature_required_error("validation", "analysis"))
                }
                OperationKey::DocumentAnalysisJson => Err(common::feature_required_error(
                    "document analysis",
                    "analysis",
                )),
                OperationKey::DocumentAnalysisFactsJson => Err(common::feature_required_error(
                    "document analysis facts",
                    "analysis",
                )),
                _ => unreachable!("analysis projection requires an analysis operation"),
            }
        }
    }

    pub fn analyze_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        self.execute_data(crate::BindingOperationRequest::new("analysis-json", source))
    }

    #[cfg(feature = "analysis")]
    fn full_analysis_analyzer(&self) -> Analyzer {
        let options = analysis_projection_options(
            self.analyzer.options().clone(),
            true,
            &self.artifact_contract,
        );
        self.analyzer
            .with_diagnostic_policy(options.diagnostic_policy().clone())
    }

    pub(crate) fn analyze_json_data(
        &self,
        source: &[u8],
        control: &OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "analysis")]
        {
            analyze_json_with(&self.full_analysis_analyzer(), source, control)
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
            analyze_facts_json_with(&self.full_analysis_analyzer(), source, control)
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
            analyze_document_json_with(&self.full_analysis_analyzer(), source, uri, control)
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
            analyze_document_facts_json_with(&self.full_analysis_analyzer(), source, uri, control)
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

/// Legacy analysis/facts DTOs include fixes only for a host that explicitly exposes them.
/// Validation remains diagnostic-only even in a fully compiled product.
#[cfg(feature = "analysis")]
pub(super) fn analysis_projection_options(
    options: merman_analysis::AnalysisOptions,
    full_dto: bool,
    artifact: &ValidatedArtifactContract,
) -> merman_analysis::AnalysisOptions {
    #[cfg(feature = "fixes")]
    if full_dto && artifact.exposes_capability(crate::CapabilityKey::Fixes) {
        return options.with_fix_generation(merman_analysis::FixGeneration::Generate);
    }
    let _ = (full_dto, artifact);
    options.with_fix_generation(merman_analysis::FixGeneration::Omit)
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

#[derive(Clone)]
pub(super) struct SemanticOperationConfig {
    runtime_policy: merman::model::runtime::RuntimePolicy,
    site_config: Option<merman::MermaidConfig>,
    parse_options: merman::ParseOptions,
    resources: merman::model::resources::InputResourcePolicy,
}

impl SemanticOperationConfig {
    pub(super) fn compile(
        options: &common::BindingOptions,
        runtime_policy: merman::model::runtime::RuntimePolicy,
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

    pub(super) fn materialize(self) -> SemanticOperationEngine {
        #[cfg(test)]
        SEMANTIC_ENGINE_MATERIALIZATIONS.with(|count| count.set(count.get() + 1));
        let mut engine = merman::MermaidEngine::new().with_runtime_policy(self.runtime_policy);
        if let Some(site_config) = self.site_config {
            engine =
                engine.with_site_config(merman::SourcePresentationConfig::from_config(site_config));
        }
        SemanticOperationEngine {
            engine,
            parse_options: self.parse_options,
            resources: self.resources,
        }
    }
}

#[derive(Clone)]
pub(super) struct SemanticOperationEngine {
    engine: merman::MermaidEngine,
    parse_options: merman::ParseOptions,
    resources: merman::model::resources::InputResourcePolicy,
}

impl SemanticOperationEngine {
    pub(super) fn parse_json_controlled(
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
            Ok(result) => result.map_err(common::core_error)?,
        }
        .ok_or_else(common::no_diagram_error)?;

        control
            .checkpoint_at(OperationPhase::Semantic)
            .map_err(BindingError::cancelled)?;

        self.resources
            .check_model_complexity(
                merman::model::resources::model_complexity_from_render_model(parsed.model()),
            )
            .map_err(common::input_resource_limit_error)?;

        let model = parsed
            .compatibility_json_controlled(control)
            .map_err(BindingError::cancelled)?
            .map_err(common::core_error)?;
        control
            .checkpoint_at(OperationPhase::Postprocess)
            .map_err(BindingError::cancelled)?;
        let mut output = Vec::new();
        model
            .write_json_controlled(&mut output, &control.for_phase(OperationPhase::Postprocess))
            .map_err(BindingError::cancelled)?
            .map_err(common::internal_json_error)?;
        Ok(output)
    }
}
