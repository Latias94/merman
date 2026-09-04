use merman_core::OperationControl;
use merman_render::{
    RenderCapabilityPolicy,
    diagram_theme::{DiagramTheme, ThemeResourcePolicy},
    environment::{RenderEnvironment as BackendRenderEnvironment, TextMeasurementPolicy},
    resources::RenderResourcePolicy,
};

use super::RenderError;

/// SVG-only host services and rendering limits.
///
/// Operation time, timezone, randomness, cancellation, and deadlines deliberately do not live
/// here. [`Renderer`](super::Renderer) owns those operation-wide concerns and injects the already
/// captured context into the private SVG session. This keeps `SvgRequest` from becoming a second
/// operation owner.
#[derive(Debug, Clone)]
pub struct SvgEnvironment {
    backend: BackendRenderEnvironment,
    text_measurement_routes: [merman_render::environment::TextMeasurementRoute; 4],
}

impl SvgEnvironment {
    /// Creates the deterministic default SVG service set.
    pub fn deterministic() -> Self {
        let text_measurement = TextMeasurementPolicy::deterministic();
        Self {
            backend: BackendRenderEnvironment::deterministic()
                .with_text_measurement_policy(text_measurement.clone()),
            text_measurement_routes: text_measurement.routes(),
        }
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.text_measurement_routes = policy.routes();
        self.backend = self.backend.with_text_measurement_policy(policy);
        self
    }

    pub fn with_capability_policy(mut self, policy: RenderCapabilityPolicy) -> Self {
        self.backend = self.backend.with_capability_policy(policy);
        self
    }

    pub fn with_compiled_math_renderer(mut self) -> Self {
        self.backend = self.backend.with_compiled_math_renderer();
        self
    }

    pub fn without_math_renderer(mut self) -> Self {
        self.backend = self.backend.without_math_renderer();
        self
    }

    pub fn with_icon_registry(mut self, registry: merman_render::svg::IconRegistry) -> Self {
        self.backend = self.backend.with_icon_registry(registry);
        self
    }

    pub fn with_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.backend = self.backend.with_resource_policy(policy);
        self
    }

    /// Sets the host-owned ceiling for resources used while materializing a compiled theme.
    ///
    /// Session creation intersects this ceiling with the restriction captured by the theme
    /// compiler. Neither side can widen the other.
    pub fn with_theme_resource_ceiling(mut self, ceiling: ThemeResourcePolicy) -> Self {
        self.backend = self.backend.with_theme_resource_ceiling(ceiling);
        self
    }

    pub fn with_theme_admission_policy(
        mut self,
        policy: merman_render::diagram_theme::ThemeAdmissionPolicy,
    ) -> Self {
        self.backend = self.backend.with_theme_admission_policy(policy);
        self
    }

    pub fn with_theme_portability_requirement(
        mut self,
        requirement: merman_render::diagram_theme::ThemePortabilityRequirement,
    ) -> Self {
        self.backend = self.backend.with_theme_portability_requirement(requirement);
        self
    }

    /// Returns the portability requirement that will be captured by the next session.
    pub fn theme_portability_requirement(
        &self,
    ) -> merman_render::diagram_theme::ThemePortabilityRequirement {
        self.backend.theme_portability_requirement()
    }

    /// Returns the configured text-measurement routes without creating an operation session.
    pub fn text_measurement_routes(&self) -> [merman_render::environment::TextMeasurementRoute; 4] {
        self.text_measurement_routes.clone()
    }

    pub(super) fn begin_session_in_context(
        &self,
        theme: Option<&DiagramTheme>,
        context: merman_core::runtime::OperationContext,
        control: OperationControl,
    ) -> Result<merman_render::environment::RenderSession, RenderError> {
        match theme {
            Some(theme) => self
                .backend
                .begin_session_with_theme_in_context(theme, context, control)
                .map_err(RenderError::from),
            None => self
                .backend
                .begin_session_in_context(context, control)
                .map_err(RenderError::from),
        }
    }
}

impl Default for SvgEnvironment {
    fn default() -> Self {
        Self::deterministic()
    }
}
