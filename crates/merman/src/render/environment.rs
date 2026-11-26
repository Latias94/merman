use merman_model::{OperationControl, resources::InputResourcePolicy};
use merman_render::{
    RenderCapabilityPolicy,
    environment::{RenderEnvironment as BackendRenderEnvironment, TextMeasurementPolicy},
    resources::RenderResourcePolicy,
    theme::ThemePortabilityRequirement,
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
    pub(super) native_theme: Option<crate::theme::ThemeSelection>,
    pub(super) background: Option<crate::theme::Color>,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    export_backdrop: Option<crate::theme::Color>,
    #[cfg(feature = "native-fonts")]
    pub(super) native_render_config: Option<merman_render::theme::NativeRenderConfig>,
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
            native_theme: None,
            background: None,
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            export_backdrop: None,
            #[cfg(feature = "native-fonts")]
            native_render_config: None,
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
        .with_capability_policy(RenderCapabilityPolicy::unrestricted())
    }

    /// Supplies the destination paint for labels rendered before a separate export step.
    #[doc(hidden)]
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub fn set_export_backdrop(
        &mut self,
        paint: Option<&str>,
        default: Option<merman_export::ExportRgbaColor>,
    ) {
        if self.native_theme.is_some() {
            let color = match paint {
                Some(paint) => merman_export::ExportRgbaColor::parse(paint),
                None => default,
            };
            self.export_backdrop = color
                .filter(|color| color.is_opaque())
                .map(|color| crate::theme::Color::rgb(color.red(), color.green(), color.blue()));
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
        // Another consumer can compile Shoal into the shared renderer independently of this facade.
        let policy = if cfg!(feature = "layout-shoal") {
            policy
        } else {
            policy.with_denied(merman_render::RenderCapability::LayoutShoal)
        };
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

    /// Reuses bounded row storage across independent, flat, natural-width Shoal layouts.
    /// Keep this environment alive and clone it into each request that should reuse storage.
    #[cfg(feature = "layout-shoal")]
    pub fn with_shoal_scratch_reuse(mut self) -> Self {
        self.backend = self.backend.with_shoal_scratch_reuse();
        self
    }

    /// Allows document-provided Mermaid `themeCSS` from a trusted source. Defaults to false.
    pub fn with_trusted_theme_css(mut self, allow: bool) -> Self {
        self.backend = self.backend.with_trusted_theme_css(allow);
        self
    }

    pub fn with_theme_portability_requirement(
        mut self,
        requirement: ThemePortabilityRequirement,
    ) -> Self {
        self.backend = self.backend.with_theme_portability_requirement(requirement);
        self
    }

    /// Returns the portability requirement that will be captured by the next session.
    pub fn theme_portability_requirement(&self) -> ThemePortabilityRequirement {
        self.backend.theme_portability_requirement()
    }

    /// Returns the configured text-measurement routes without creating an operation session.
    pub fn text_measurement_routes(&self) -> [merman_render::environment::TextMeasurementRoute; 4] {
        self.text_measurement_routes.clone()
    }

    pub(super) fn begin_session_in_context(
        &self,
        metadata: &merman_model::presentation::PresentationMetadata,
        context: merman_model::runtime::OperationContext,
        control: OperationControl,
    ) -> Result<merman_render::environment::RenderSession, RenderError> {
        let session = if let Some(selection) = &self.native_theme {
            let native = selection.resolve_svg(metadata)?;
            match native {
                Some(native) => {
                    #[cfg(feature = "native-fonts")]
                    let native = native.with_render_config(self.native_render_config.clone());
                    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
                    let native = match self.export_backdrop {
                        Some(backdrop) => native.with_export_backdrop(backdrop),
                        None => native,
                    };
                    self.backend
                        .begin_session_with_native_theme_in_context(native, context, control)
                }
                None => self.backend.begin_session_in_context(context, control),
            }
        } else {
            self.backend.begin_session_in_context(context, control)
        }?;
        Ok(match self.background {
            Some(color) => session.with_canvas_background(color),
            None => session,
        })
    }
}

impl Default for SvgEnvironment {
    fn default() -> Self {
        Self::deterministic()
    }
}
