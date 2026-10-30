//! Retained canonical parsing for render-capable transports.

use super::BindingEngine;
use crate::operation::{BindingOperationOutput, operation_result};
use crate::render::RenderOperationConfig;
use crate::{
    BindingEngineServices, BindingError, BindingOperationKind, BindingOperationResult,
    OperationControl, OperationPhase, common,
};
use std::sync::Arc;

/// An immutable parsed source and its output defaults, bound to the preparing artifact.
///
/// Output options can change resources, measurement, layout and SVG emission. Parse configuration,
/// runtime selection and the compiled theme are frozen; prepare again to change those facts.
pub struct BindingPreparedSource {
    source: merman::PreparedSource,
    artifact: Arc<crate::ValidatedArtifactContract>,
    runtime_policy_id: &'static str,
    runtime_policy: merman::model::runtime::RuntimePolicy,
    base_options: common::BaseBindingOptions,
    defaults: RenderOperationConfig,
    default_options: common::BindingOptions,
}

impl BindingEngine {
    /// Parses source once without publishing a handle for rejected or empty input.
    pub fn prepare_source(
        &self,
        source: &[u8],
        control: OperationControl,
    ) -> Result<BindingPreparedSource, BindingError> {
        self.admit_operation(BindingOperationKind::from_id("svg")?)?;
        let control = control.child();
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let render = self.render_engine();
        let source = render.prepare_source(source, control.clone())?;
        let (options, base_options) = self
            .base_options
            .prepared_output_defaults(&self.artifact_contract)?;
        let runtime_policy = render.runtime_policy();
        let defaults = RenderOperationConfig::compile(
            &options,
            runtime_policy.clone(),
            self.artifact_contract.render_capability_policy(),
            None,
            base_options.theme_resource_policy().clone(),
        )?;
        control
            .checkpoint_at(OperationPhase::Postprocess)
            .map_err(BindingError::cancelled)?;
        Ok(BindingPreparedSource {
            source,
            artifact: Arc::clone(&self.artifact_contract),
            runtime_policy_id: self.runtime_policy_id,
            runtime_policy,
            base_options,
            defaults,
            default_options: options,
        })
    }
}

impl BindingPreparedSource {
    pub fn render_svg(
        &self,
        options: &[u8],
        control: OperationControl,
        services: &BindingEngineServices,
    ) -> Result<Vec<u8>, BindingError> {
        self.output("svg", options, control, services, false)
            .map(|output| output.into_data())
    }

    pub fn render_svg_result(
        &self,
        options: &[u8],
        control: OperationControl,
        services: &BindingEngineServices,
    ) -> Result<BindingOperationResult, BindingError> {
        operation_result(
            BindingOperationKind::from_id("svg")?,
            self.runtime_policy_id,
            self.output("svg", options, control, services, true)?,
        )
    }

    pub fn layout_json(
        &self,
        options: &[u8],
        control: OperationControl,
        services: &BindingEngineServices,
    ) -> Result<Vec<u8>, BindingError> {
        self.output("layout-json", options, control, services, false)
            .map(|output| output.into_data())
    }

    pub fn edge_geometry_json(
        &self,
        options: &[u8],
        control: OperationControl,
        services: &BindingEngineServices,
    ) -> Result<Vec<u8>, BindingError> {
        self.output("edge-geometry-json", options, control, services, false)
            .map(|output| output.into_data())
    }

    fn output(
        &self,
        id: &str,
        options: &[u8],
        control: OperationControl,
        services: &BindingEngineServices,
        metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        let operation = BindingOperationKind::from_id(id)?;
        self.artifact.admit_operation(operation)?;
        let control = control.child();
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        self.artifact.validate_engine_services(services)?;
        let overlay = common::parse_request_overlay_for_artifact(
            options,
            operation.resource_scope(),
            &self.artifact,
            self.base_options.theme_resource_policy(),
        )?;
        overlay.validate_prepared_output()?;
        let config = match overlay {
            common::BindingRequestOverlay::Unchanged => {
                // Validate the selector against each request's services without recompiling defaults.
                services.validate_options(&self.default_options)?;
                self.defaults.clone()
            }
            overlay => {
                let options = self.base_options.apply_overlay(overlay, &self.artifact)?;
                services.validate_options(&options)?;
                RenderOperationConfig::compile(
                    &options,
                    self.runtime_policy.clone(),
                    self.artifact.render_capability_policy(),
                    None,
                    self.base_options.theme_resource_policy().clone(),
                )?
            }
        };
        let output = config.materialize(services).render_prepared_output(
            &self.source,
            operation.key(),
            control.clone(),
            metadata,
        )?;
        control
            .checkpoint_at(OperationPhase::Postprocess)
            .map_err(BindingError::cancelled)?;
        Ok(output)
    }
}
