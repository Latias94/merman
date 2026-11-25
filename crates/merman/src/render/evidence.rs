/// Immutable native font measurement diagnostics from one completed SVG operation.
///
/// This snapshot does not retain font services or describe later painting operations.
#[cfg(feature = "native-fonts")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFontDiagnostics {
    identity: String,
    selected_faces: Vec<String>,
}

#[cfg(feature = "native-fonts")]
impl NativeFontDiagnostics {
    /// Context policy and initially admitted bytes, not a cross-process font fingerprint.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Faces used by successful native shaping probes, reported as `PostScriptName#N`.
    /// `N` is the zero-based face index in the font bytes used for measurement. A face
    /// extracted from a system collection into a standalone font has index `0`.
    /// Compare indices across layout and painting only when both use the same font bytes.
    pub fn selected_faces(&self) -> &[String] {
        &self.selected_faces
    }
}

/// Immutable evidence captured by a completed SVG operation.
///
/// The evidence is created only by the renderer after SVG emission/postprocessing succeeds. It
/// is intentionally a narrow projection: callers can inspect measurement provenance and runtime
/// identity without gaining access to SVG session services or family layout internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEvidence {
    execution_path: super::OperationExecutionPath,
    session: merman_render::environment::RenderSessionReport,
    family_id: merman_model::family::DiagramFamilyId,
    output_mutated: bool,
    required_capabilities: Box<[merman_render::RenderCapability]>,
    #[cfg(feature = "native-fonts")]
    native_font_diagnostics: Option<NativeFontDiagnostics>,
}

impl RenderEvidence {
    pub(super) fn from_family(
        family: merman_render::family::FamilyRenderReport,
        required_capabilities: Vec<merman_render::RenderCapability>,
    ) -> Self {
        let family_id = family.family_id();
        let output_mutated = family.output_mutated();
        Self {
            execution_path: super::OperationExecutionPath::Renderer,
            session: family.into_session_report(),
            family_id,
            output_mutated,
            required_capabilities: required_capabilities.into_boxed_slice(),
            #[cfg(feature = "native-fonts")]
            native_font_diagnostics: None,
        }
    }

    #[cfg(feature = "native-fonts")]
    pub(super) fn with_native_font_diagnostics(
        mut self,
        context: Option<&merman_export::NativeFontContext>,
    ) -> Self {
        self.native_font_diagnostics = context.map(|context| NativeFontDiagnostics {
            identity: context.identity().to_owned(),
            selected_faces: context.selected_faces(),
        });
        self
    }

    /// Native measurement diagnostics frozen at SVG completion, when native fonts were selected.
    #[cfg(feature = "native-fonts")]
    pub fn native_font_diagnostics(&self) -> Option<&NativeFontDiagnostics> {
        self.native_font_diagnostics.as_ref()
    }

    pub const fn execution_path(&self) -> super::OperationExecutionPath {
        self.execution_path
    }

    pub(super) const fn session(&self) -> &merman_render::environment::RenderSessionReport {
        &self.session
    }

    pub(super) fn portability_requirement(
        &self,
    ) -> merman_render::theme::ThemePortabilityRequirement {
        self.session().portability_requirement()
    }

    pub fn measurement_routes(&self) -> &[merman_render::environment::TextMeasurementRoute; 4] {
        self.session().measurement_routes()
    }

    pub fn measurement(&self) -> &merman_render::environment::TextMeasurementReport {
        self.session().measurement()
    }

    pub fn operation_context(&self) -> &merman_model::runtime::OperationContext {
        self.session().operation_context()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.session().unix_millis()
    }

    pub const fn local_date(&self) -> merman_model::time::CivilDate {
        self.session().local_date()
    }

    pub fn local_time_zone(&self) -> &merman_model::time::LocalTimeZoneProvenance {
        self.session().local_time_zone()
    }

    pub fn render_seed(&self) -> std::num::NonZeroU64 {
        self.session().render_seed()
    }

    pub const fn layout_work_units(&self) -> usize {
        self.session().layout_work_units()
    }

    pub const fn family_id(&self) -> merman_model::family::DiagramFamilyId {
        self.family_id
    }

    pub const fn font_catalog_fingerprint(&self) -> merman_render::text::FontCatalogFingerprint {
        self.session().font_catalog_fingerprint()
    }

    /// Reports whether document CSS or a custom postprocessor changed the generated SVG.
    pub const fn output_mutated(&self) -> bool {
        self.output_mutated
    }

    /// Returns the optional render capabilities selected during family preparation.
    pub fn required_capabilities(&self) -> &[merman_render::RenderCapability] {
        &self.required_capabilities
    }
}
