use merman_core::{OperationControl, resources::InputResourcePolicy};
use merman_render::{
    RenderCapabilityPolicy,
    diagram_theme::{DiagramTheme, ThemeResourcePolicy},
    environment::{RenderEnvironment as BackendRenderEnvironment, TextMeasurementPolicy},
    resources::RenderResourcePolicy,
};

use super::RenderError;

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
pub(super) enum NativeFontMeasurementState {
    Default,
    Explicit,
}

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
pub(super) fn resolve_native_font_context(
    measuring: Option<&merman_export::NativeFontContext>,
    painting: Option<&merman_export::NativeFontContext>,
    measurement: NativeFontMeasurementState,
) -> Result<Option<merman_export::NativeFontContext>, RenderError> {
    let context = match (measuring, painting) {
        (Some(measuring), Some(painting)) if measuring != painting => {
            return Err(merman_export::NativeFontError::Measurement(
                "native font contexts for measurement and painting conflict; share the same context"
                    .to_owned(),
            )
            .into());
        }
        (Some(measuring), _) => Some(measuring),
        (None, Some(painting)) => match measurement {
            NativeFontMeasurementState::Default => Some(painting),
            NativeFontMeasurementState::Explicit => {
                return Err(merman_export::NativeFontError::Measurement(
                    "native painting font context conflicts with an explicit text measurement policy"
                        .to_owned(),
                )
                .into());
            }
        },
        (None, None) => None,
    };
    Ok(context.cloned())
}

/// SVG-only host services and rendering limits.
///
/// Operation time, timezone, randomness, cancellation, and deadlines deliberately do not live
/// here. [`Renderer`](super::Renderer) owns those operation-wide concerns and injects the already
/// captured context into the private SVG session. This keeps `SvgRequest` from becoming a second
/// operation owner.
#[derive(Debug, Clone)]
pub struct SvgEnvironment {
    backend: BackendRenderEnvironment,
    #[cfg(feature = "native-fonts")]
    // The caller-selected context is retained for measurement/painting identity checks. The
    // active context is forked per render operation so counters and sticky failures do not leak.
    pub(super) native_fonts: Option<merman_export::NativeFontContext>,
    #[cfg(feature = "native-fonts")]
    // Keep the original Arc identity for later measurement/painting compatibility checks.
    pub(super) selected_native_fonts: Option<merman_export::NativeFontContext>,
    #[cfg(all(
        feature = "native-fonts",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    pub(super) text_measurement_is_default: bool,
    text_measurement_routes: [merman_render::environment::TextMeasurementRoute; 4],
    // Retain source admission without opening a backend session.
    pub(super) input_resources: InputResourcePolicy,
}

impl SvgEnvironment {
    /// Creates the deterministic default SVG service set.
    pub fn deterministic() -> Self {
        let text_measurement = TextMeasurementPolicy::deterministic();
        let resources = RenderResourcePolicy::default();
        Self {
            #[cfg(feature = "native-fonts")]
            native_fonts: None,
            #[cfg(feature = "native-fonts")]
            selected_native_fonts: None,
            #[cfg(all(
                feature = "native-fonts",
                any(feature = "png", feature = "jpeg", feature = "pdf")
            ))]
            text_measurement_is_default: true,
            backend: BackendRenderEnvironment::deterministic()
                .with_text_measurement_policy(text_measurement.clone())
                .with_resource_policy(resources),
            text_measurement_routes: text_measurement.routes(),
            input_resources: *resources.input_policy(),
        }
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        #[cfg(feature = "native-fonts")]
        {
            self.native_fonts = None;
            self.selected_native_fonts = None;
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            {
                self.text_measurement_is_default = false;
            }
        }
        self.text_measurement_routes = policy.routes();
        self.backend = self.backend.with_text_measurement_policy(policy);
        self
    }

    /// Selects reusable font assets for measurement and painting in subsequent operations.
    #[cfg(feature = "native-fonts")]
    pub fn with_native_font_context(
        self,
        context: merman_export::NativeFontContext,
    ) -> Result<Self, merman_export::NativeFontError> {
        use merman_render::environment::{
            MeasurementProfileId, TextMeasurementPhase, TextMeasurementProfileIdentity,
        };
        let id = MeasurementProfileId::new("native-fonts")
            .map_err(|error| merman_export::NativeFontError::Measurement(error.to_string()))?;
        let identity = TextMeasurementProfileIdentity::new(id, context.identity())
            .map_err(|error| merman_export::NativeFontError::Measurement(error.to_string()))?;
        let policy = TextMeasurementPolicy::host_display(
            identity,
            std::sync::Arc::new(context.clone()),
            TextMeasurementPhase::ALL,
        );
        let mut environment = self.with_text_measurement_policy(policy);
        environment.native_fonts = Some(context.clone());
        environment.selected_native_fonts = Some(context);
        Ok(environment)
    }

    #[cfg(feature = "native-fonts")]
    pub(super) fn with_active_native_font_context(
        mut self,
        context: merman_export::NativeFontContext,
    ) -> Self {
        use merman_render::environment::{
            MeasurementProfileId, TextMeasurementPhase, TextMeasurementProfileIdentity,
        };
        let id = MeasurementProfileId::new("native-fonts")
            .expect("native font profile id is fixed and valid");
        let identity = TextMeasurementProfileIdentity::new(id, context.identity())
            .expect("native font context identity is valid");
        let policy = TextMeasurementPolicy::host_display(
            identity,
            std::sync::Arc::new(context.clone()),
            TextMeasurementPhase::ALL,
        );
        let selected = self.selected_native_fonts.clone();
        self = self.with_text_measurement_policy(policy);
        self.native_fonts = Some(context);
        self.selected_native_fonts = selected;
        self
    }

    #[cfg(feature = "native-fonts")]
    pub(super) fn selected_native_font_context(&self) -> Option<&merman_export::NativeFontContext> {
        self.selected_native_fonts.as_ref()
    }

    #[cfg(feature = "native-fonts")]
    pub(super) fn check_native_fonts(&self) -> Result<(), RenderError> {
        if let Some(context) = &self.native_fonts {
            context.check()?;
        }
        Ok(())
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
        self.input_resources = *policy.input_policy();
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
