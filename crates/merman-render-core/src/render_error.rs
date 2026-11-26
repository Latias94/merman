use crate::resources::{OperationWorkError, ResourceLimitExceeded};
use merman_model::family::DiagramFamilyId;

/// Optional renderer capabilities that a typed diagram operation may require.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderCapability {
    LayoutCytoscape,
    LayoutElk,
    Math,
    LayoutShoal,
    LayoutDagre,
}

impl RenderCapability {
    const fn bit(self) -> u8 {
        match self {
            Self::LayoutCytoscape => 1 << 0,
            Self::LayoutElk => 1 << 1,
            Self::Math => 1 << 2,
            Self::LayoutShoal => 1 << 3,
            Self::LayoutDagre => 1 << 4,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::LayoutCytoscape => "layout-cytoscape",
            Self::LayoutElk => "layout-elk",
            Self::Math => "math",
            Self::LayoutShoal => "layout-shoal",
            Self::LayoutDagre => "layout-dagre",
        }
    }
}

const ALL_RENDER_CAPABILITY_BITS: u8 = RenderCapability::LayoutCytoscape.bit()
    | RenderCapability::LayoutElk.bit()
    | RenderCapability::Math.bit()
    | RenderCapability::LayoutShoal.bit()
    | RenderCapability::LayoutDagre.bit();

/// Operation-level permission for optional renderer capabilities.
///
/// The policy does not claim that a backend is compiled or that a service is installed. Effective
/// availability requires both this permission and the concrete backend/service. The unrestricted
/// default preserves the direct Rust renderer API, while artifact facades can project their exact
/// owner-selected capability contract without depending on binding-specific types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCapabilityPolicy {
    allowed_bits: u8,
}

impl RenderCapabilityPolicy {
    /// Allows every optional renderer capability that is otherwise available.
    pub const fn unrestricted() -> Self {
        Self {
            allowed_bits: ALL_RENDER_CAPABILITY_BITS,
        }
    }

    /// Denies every optional renderer capability until explicitly allowed.
    pub const fn deny_all() -> Self {
        Self { allowed_bits: 0 }
    }

    /// Allows one optional renderer capability.
    #[must_use]
    pub const fn with_allowed(mut self, capability: RenderCapability) -> Self {
        self.allowed_bits |= capability.bit();
        self
    }

    /// Denies one optional capability while preserving the other permissions.
    #[must_use]
    pub const fn with_denied(mut self, capability: RenderCapability) -> Self {
        self.allowed_bits &= !capability.bit();
        self
    }

    /// Reports whether this policy permits the capability.
    pub const fn allows(self, capability: RenderCapability) -> bool {
        self.allowed_bits & capability.bit() != 0
    }
}

impl Default for RenderCapabilityPolicy {
    fn default() -> Self {
        Self::unrestricted()
    }
}

impl std::fmt::Display for RenderCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Cancelled(#[from] merman_model::OperationCancelled),
    #[error("unsupported diagram type for layout: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },
    #[error("layout backend `{backend}` is unsupported for diagram `{diagram_type}`")]
    UnsupportedLayoutBackend {
        backend: &'static str,
        diagram_type: String,
    },
    #[error("layout backend `{backend}` does not support {construct}")]
    UnsupportedLayoutConstruct {
        backend: &'static str,
        construct: &'static str,
    },
    #[error("layout backend `{backend}` could not allocate layout storage")]
    LayoutAllocationFailed { backend: &'static str },
    #[error("layout adaptation `{adaptation}` is unsupported for diagram `{diagram_type}`")]
    UnsupportedLayoutAdaptation {
        adaptation: &'static str,
        diagram_type: String,
    },
    #[error("fit-width layout.container_width must be finite and positive")]
    InvalidAdaptationWidth,
    #[error("render session lacks capability `{capability}` required by diagram `{diagram_type}`")]
    MissingCapability {
        capability: RenderCapability,
        diagram_type: String,
    },
    #[error(
        "portable native theme rendering for `{family_id}` cannot apply a requested feature. {message}"
    )]
    UnsupportedNativeThemeFeature {
        family_id: DiagramFamilyId,
        message: &'static str,
    },
    #[error("SVG output profile `{profile}` is unsupported: {reason}")]
    UnsupportedSvgOutputProfile {
        profile: crate::svg_output::SvgOutputProfile,
        reason: &'static str,
    },
    #[error(transparent)]
    SemanticProjection(merman_model::presentation::compat::CompatProjectionError),
    #[error(transparent)]
    PresentationConfig(#[from] merman_model::presentation::PresentationConfigError),
    #[error("invalid semantic model: {message}")]
    InvalidModel { message: String },
    #[error(
        "render target `{target}` does not support RequirePortable because it has no target portability admission"
    )]
    PortabilityUnavailableForTarget { target: &'static str },
    #[error("trusted theme lane `raw-theme-css` is disabled by the host")]
    UntrustedThemeCss,
    #[error(
        "custom JSON model `{model_name}` from {provenance:?} cannot render diagram type `{diagram_type}`"
    )]
    NonRenderableCustomModel {
        diagram_type: String,
        model_name: String,
        provenance: merman_model::presentation::CustomJsonProvenance,
    },
    #[error("SVG postprocessor `{pass}` failed: {message}")]
    SvgPostprocess { pass: String, message: String },
    #[error("external icon output is invalid: {message}")]
    InvalidIconOutput { message: String },
    #[error("icon rendering failed internally: {message}")]
    IconProcessing { message: String },
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[error(transparent)]
    OperationResourceTerminal(merman_model::OperationLedgerError),
    #[error(transparent)]
    Color(#[from] merman_markup::theme_color::ColorError),
    #[error("semantic model JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    OperationTimingUnavailable(#[from] merman_model::runtime::OperationTimingUnavailable),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "layout-dagre")]
impl From<dugong::LayoutError> for Error {
    fn from(error: dugong::LayoutError) -> Self {
        Self::InvalidModel {
            message: error.to_string(),
        }
    }
}

impl From<OperationWorkError> for Error {
    fn from(error: OperationWorkError) -> Self {
        match error {
            OperationWorkError::Cancelled(error) => Self::Cancelled(error),
            OperationWorkError::ResourceLimitExceeded(error) => Self::ResourceLimitExceeded(error),
            OperationWorkError::ForeignResourceTerminal(error) => {
                Self::OperationResourceTerminal(error)
            }
        }
    }
}

impl Error {
    pub fn svg_postprocess(pass: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SvgPostprocess {
            pass: pass.into(),
            message: message.into(),
        }
    }

    #[doc(hidden)]
    pub fn invalid_icon_output(message: impl Into<String>) -> Self {
        Self::InvalidIconOutput {
            message: message.into(),
        }
    }

    #[doc(hidden)]
    pub fn icon_processing(message: impl Into<String>) -> Self {
        Self::IconProcessing {
            message: message.into(),
        }
    }

    pub const fn missing_capability(&self) -> Option<RenderCapability> {
        match self {
            Self::MissingCapability { capability, .. } => Some(*capability),
            _ => None,
        }
    }
}
