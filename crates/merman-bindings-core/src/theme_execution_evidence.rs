use crate::BindingError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Schema version for renderer-owned theme execution evidence and explanatory diagnostics.
pub const BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION: u32 = 2;

/// Maximum UTF-8 byte length accepted for one identifier in known evidence.
pub const BINDING_THEME_EXECUTION_EVIDENCE_MAX_ID_UTF8_BYTES: usize = 128;

/// Maximum number of target-admission reasons accepted in known evidence.
pub const BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS: usize = 32;

/// Versioned theme execution evidence attached to successful render metadata.
///
/// Version two projects renderer-owned render evidence and target admission as coarse states.
/// Diagnostics explain residuals without exposing internal rule IDs or conferring certification.
/// Unknown versions remain opaque so bindings can forward metadata produced by newer revisions.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BindingThemeExecutionEvidence {
    V2(BindingThemeExecutionEvidenceV2),
    Unknown(BindingUnknownThemeExecutionEvidence),
}

impl BindingThemeExecutionEvidence {
    #[must_use]
    pub const fn version(&self) -> u32 {
        match self {
            Self::V2(_) => BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION,
            Self::Unknown(evidence) => evidence.version(),
        }
    }

    #[must_use]
    pub const fn as_v2(&self) -> Option<&BindingThemeExecutionEvidenceV2> {
        match self {
            Self::V2(evidence) => Some(evidence),
            Self::Unknown(_) => None,
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn project(
        render_evidence: &merman::RenderEvidence,
        admission: &merman::TargetAdmissionReceipt,
    ) -> Result<Self, BindingError> {
        let theme = render_evidence.theme_evidence();
        let theme_status_id = theme_status_id(theme.status()).ok_or_else(|| {
            BindingError::internal("renderer returned an unsupported theme evidence status")
        })?;
        BindingThemeExecutionEvidenceV2 {
            family_id: render_evidence.family_id().as_str().to_owned(),
            theme_status_id: theme_status_id.to_owned(),
            output_mutated: theme.output_mutated(),
            target_kind_id: admission.artifact_kind().id().to_owned(),
            target_status_id: admission.status().id().to_owned(),
            target_reason_ids: admission
                .reasons()
                .iter()
                .map(|reason| reason.id().to_owned())
                .collect(),
            font_source_id: admission.font_source().id().to_owned(),
            materialized_theme: render_evidence
                .materialized_theme()
                .map(|theme| theme.as_str().to_owned()),
            diagnostics: None,
        }
        .validate()
        .map(|mut evidence| {
            evidence.diagnostics = Some(
                render_evidence
                    .theme_diagnostics()
                    .iter()
                    .map(|diagnostic| BindingThemeDiagnostic {
                        code: diagnostic.code().to_owned(),
                        subject: diagnostic.subject().to_owned(),
                        target: diagnostic.target().map(|target| target.id().to_owned()),
                        property: diagnostic.property().map(str::to_owned),
                        source_document: diagnostic.source_document().map(str::to_owned),
                        source_paths: diagnostic.source_paths().to_vec(),
                        generated: diagnostic.generated(),
                    })
                    .collect(),
            );
            Self::V2(evidence)
        })
        .map_err(|message| {
            BindingError::internal(format!(
                "renderer produced invalid theme execution evidence: {message}"
            ))
        })
    }
}

impl Serialize for BindingThemeExecutionEvidence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::V2(evidence) => evidence.serialize(serializer),
            Self::Unknown(evidence) => evidence.serialize(serializer),
        }
    }
}

/// An explanation of an actual render residual, not a portable-target certificate.
///
/// All identifiers are open strings. A rule diagnostic can describe a partially consumed rule;
/// it does not identify a failing facet unless `property` explicitly names one. Source pointers
/// are RFC 6901 paths relative to the named input document payload. Generated defaults and typed
/// Rust input can have no source paths. Empty diagnostics do not establish portability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BindingThemeDiagnostic {
    code: String,
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    property: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_document: Option<String>,
    source_paths: Vec<String>,
    generated: bool,
}

impl BindingThemeDiagnostic {
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    #[must_use]
    pub fn property(&self) -> Option<&str> {
        self.property.as_deref()
    }

    #[must_use]
    pub fn source_document(&self) -> Option<&str> {
        self.source_document.as_deref()
    }

    #[must_use]
    pub fn source_paths(&self) -> &[String] {
        &self.source_paths
    }

    #[must_use]
    pub const fn generated(&self) -> bool {
        self.generated
    }
}

/// Known version-two projection of successful renderer theme and target evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingThemeExecutionEvidenceV2 {
    family_id: String,
    theme_status_id: String,
    output_mutated: bool,
    target_kind_id: String,
    target_status_id: String,
    target_reason_ids: Box<[String]>,
    font_source_id: String,
    diagnostics: Option<Box<[BindingThemeDiagnostic]>>,
    materialized_theme: Option<String>,
}

impl BindingThemeExecutionEvidenceV2 {
    fn validate(self) -> Result<Self, String> {
        for (field, id) in [
            ("family_id", self.family_id.as_str()),
            ("theme_status", self.theme_status_id.as_str()),
            ("target_kind", self.target_kind_id.as_str()),
            ("target_status", self.target_status_id.as_str()),
            ("font_source", self.font_source_id.as_str()),
        ] {
            validate_id(field, id)?;
        }
        if self.target_reason_ids.len() > BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS {
            return Err(format!(
                "`target_reason_ids` must contain at most {BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS} entries"
            ));
        }
        for (index, reason) in self.target_reason_ids.iter().enumerate() {
            validate_id(&format!("target_reason_ids[{index}]"), reason)?;
            if self.target_reason_ids[..index].contains(reason) {
                return Err("`target_reason_ids` entries must be unique".to_owned());
            }
        }
        if let Some(theme) = &self.materialized_theme
            && !merman::supported_themes().contains(&theme.as_str())
        {
            return Err(
                "`materialized_theme` must be a registered Mermaid theme or null".to_owned(),
            );
        }

        Ok(self)
    }

    #[must_use]
    pub fn family_id(&self) -> &str {
        &self.family_id
    }

    #[must_use]
    pub fn theme_status_id(&self) -> &str {
        &self.theme_status_id
    }

    #[must_use]
    pub const fn output_mutated(&self) -> bool {
        self.output_mutated
    }

    #[must_use]
    pub fn target_kind_id(&self) -> &str {
        &self.target_kind_id
    }

    #[must_use]
    pub fn target_status_id(&self) -> &str {
        &self.target_status_id
    }

    #[must_use]
    pub fn target_reason_ids(&self) -> &[String] {
        &self.target_reason_ids
    }

    #[must_use]
    pub fn font_source_id(&self) -> &str {
        &self.font_source_id
    }

    #[must_use]
    pub fn materialized_theme(&self) -> Option<&str> {
        self.materialized_theme.as_deref()
    }

    /// `None` means the producer did not supply diagnostics. An empty slice means it supplied
    /// no explanations; consumers must still inspect the theme and target admission states.
    #[must_use]
    pub fn diagnostics(&self) -> Option<&[BindingThemeDiagnostic]> {
        self.diagnostics.as_deref()
    }
}

impl Serialize for BindingThemeExecutionEvidenceV2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        BindingThemeExecutionEvidenceV2Wire {
            version: BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION,
            family_id: self.family_id(),
            theme_status: self.theme_status_id(),
            output_mutated: self.output_mutated(),
            target_kind: self.target_kind_id(),
            target_status: self.target_status_id(),
            target_reason_ids: self.target_reason_ids(),
            font_source: self.font_source_id(),
            diagnostics: self.diagnostics(),
            materialized_theme: self.materialized_theme(),
        }
        .serialize(serializer)
    }
}

/// Opaque evidence from a newer schema revision.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BindingUnknownThemeExecutionEvidence {
    version: u32,
    value: Value,
}

impl BindingUnknownThemeExecutionEvidence {
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

impl Serialize for BindingUnknownThemeExecutionEvidence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.value.serialize(serializer)
    }
}

#[derive(Serialize)]
struct BindingThemeExecutionEvidenceV2Wire<'a> {
    version: u32,
    family_id: &'a str,
    theme_status: &'a str,
    output_mutated: bool,
    target_kind: &'a str,
    target_status: &'a str,
    target_reason_ids: &'a [String],
    font_source: &'a str,
    materialized_theme: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<&'a [BindingThemeDiagnostic]>,
}

pub(crate) fn parse_theme_execution_evidence(
    value: &Value,
) -> Result<BindingThemeExecutionEvidence, BindingError> {
    let object = value.as_object().ok_or_else(|| {
        invalid_evidence("`theme_execution_evidence` must be an object".to_owned())
    })?;
    let version = required_u32(object, "version")?;
    if version != BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION {
        return Ok(BindingThemeExecutionEvidence::Unknown(
            BindingUnknownThemeExecutionEvidence {
                version,
                value: value.clone(),
            },
        ));
    }

    let target_reason_values = required_value(object, "target_reason_ids")?
        .as_array()
        .ok_or_else(|| {
            invalid_evidence("`theme_execution_evidence.target_reason_ids` must be an array")
        })?;
    if target_reason_values.len() > BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS {
        return Err(invalid_evidence(format!(
            "`theme_execution_evidence.target_reason_ids` must contain at most {BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS} entries"
        )));
    }
    let mut target_reason_ids = Vec::with_capacity(target_reason_values.len());
    for (index, value) in target_reason_values.iter().enumerate() {
        let reason = value.as_str().ok_or_else(|| {
            invalid_evidence(format!(
                "`theme_execution_evidence.target_reason_ids[{index}]` must be a string"
            ))
        })?;
        validate_id(&format!("target_reason_ids[{index}]"), reason).map_err(|message| {
            invalid_evidence(format!("`theme_execution_evidence`: {message}"))
        })?;
        if target_reason_ids
            .iter()
            .any(|known: &String| known.as_str() == reason)
        {
            return Err(invalid_evidence(
                "`theme_execution_evidence.target_reason_ids` entries must be unique",
            ));
        }
        target_reason_ids.push(reason.to_owned());
    }

    // Preserve the producer's complete list. Existing transport document limits own resource
    // admission; an independent diagnostic-count cap could reject a valid renderer result.
    let diagnostics = object
        .get("diagnostics")
        .map(|value| {
            Box::<[BindingThemeDiagnostic]>::deserialize(value).map_err(|error| {
                invalid_evidence(format!("`theme_execution_evidence.diagnostics`: {error}"))
            })
        })
        .transpose()?;

    let materialized_theme = match required_value(object, "materialized_theme")? {
        Value::Null => None,
        Value::String(theme) => Some(theme.clone()),
        _ => {
            return Err(invalid_evidence(
                "`theme_execution_evidence.materialized_theme` must be a string or null",
            ));
        }
    };

    BindingThemeExecutionEvidenceV2 {
        family_id: required_bounded_id(object, "family_id")?,
        theme_status_id: required_bounded_id(object, "theme_status")?,
        output_mutated: required_bool(object, "output_mutated")?,
        target_kind_id: required_bounded_id(object, "target_kind")?,
        target_status_id: required_bounded_id(object, "target_status")?,
        target_reason_ids: target_reason_ids.into_boxed_slice(),
        font_source_id: required_bounded_id(object, "font_source")?,
        materialized_theme,
        diagnostics: None,
    }
    .validate()
    .map(|mut evidence| {
        evidence.diagnostics = diagnostics;
        BindingThemeExecutionEvidence::V2(evidence)
    })
    .map_err(|message| invalid_evidence(format!("`theme_execution_evidence`: {message}")))
}

fn required_bounded_id(object: &Map<String, Value>, field: &str) -> Result<String, BindingError> {
    let id = required_string(object, field)?;
    validate_id(field, id)
        .map_err(|message| invalid_evidence(format!("`theme_execution_evidence`: {message}")))?;
    Ok(id.to_owned())
}

fn validate_id(field: &str, id: &str) -> Result<(), String> {
    if id.len() > BINDING_THEME_EXECUTION_EVIDENCE_MAX_ID_UTF8_BYTES {
        return Err(format!(
            "`{field}` exceeds {BINDING_THEME_EXECUTION_EVIDENCE_MAX_ID_UTF8_BYTES} UTF-8 bytes"
        ));
    }
    Ok(())
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Value, BindingError> {
    object.get(field).ok_or_else(|| {
        invalid_evidence(format!(
            "missing required field `theme_execution_evidence.{field}`"
        ))
    })
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, BindingError> {
    required_value(object, field)?.as_str().ok_or_else(|| {
        invalid_evidence(format!(
            "`theme_execution_evidence.{field}` must be a string"
        ))
    })
}

fn required_bool(object: &Map<String, Value>, field: &str) -> Result<bool, BindingError> {
    required_value(object, field)?.as_bool().ok_or_else(|| {
        invalid_evidence(format!(
            "`theme_execution_evidence.{field}` must be a boolean"
        ))
    })
}

fn required_u32(object: &Map<String, Value>, field: &str) -> Result<u32, BindingError> {
    let value = required_value(object, field)?.as_u64().ok_or_else(|| {
        invalid_evidence(format!(
            "`theme_execution_evidence.{field}` must be an unsigned 32-bit integer"
        ))
    })?;
    u32::try_from(value).map_err(|_| {
        invalid_evidence(format!(
            "`theme_execution_evidence.{field}` exceeds unsigned 32-bit range"
        ))
    })
}

fn invalid_evidence(message: impl Into<String>) -> BindingError {
    BindingError::invalid_argument(format!(
        "invalid operation metadata JSON: {}",
        message.into()
    ))
}

#[cfg(feature = "svg")]
const fn theme_status_id(status: merman::ThemeEvidenceStatus) -> Option<&'static str> {
    match status {
        merman::ThemeEvidenceStatus::NotApplicable => Some("not_applicable"),
        merman::ThemeEvidenceStatus::Verified => Some("verified"),
        merman::ThemeEvidenceStatus::Residual => Some("residual"),
        merman::ThemeEvidenceStatus::Incomplete => Some("incomplete"),
        _ => None,
    }
}

#[cfg(all(
    test,
    feature = "svg",
    feature = "diagram-info",
    any(feature = "png", feature = "pdf")
))]
mod native_quality_tests {
    use super::*;
    use std::borrow::Cow;

    struct ReplaceScene(&'static str);

    impl merman::svg::SvgPostprocessor for ReplaceScene {
        fn name(&self) -> &'static str {
            "test-binding-quality-scene"
        }
        fn process<'a>(
            &self,
            _: Cow<'a, str>,
            _: &merman::svg::SvgPostprocessContext<'_>,
        ) -> merman::svg::RenderResult<Cow<'a, str>> {
            Ok(Cow::Owned(self.0.to_owned()))
        }
    }

    #[test]
    fn successful_best_effort_native_quality_residuals_remain_visible_in_binding_metadata() {
        let scenes = [
            (
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(60 20) rotate(90)" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##,
                "native_directional_filter_transform_residual",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g data-merman-fallback-css-residual="true"><rect width="20" height="10" fill="red"/></g></svg>"#,
                "fallback_css_residual",
            ),
            #[cfg(feature = "pdf")]
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><rect x="-10" y="10" width="20" height="20" fill="red"/></svg>"#,
                "pdf_implicit_viewport_clipping_residual",
            ),
        ];
        for (source, reason) in scenes {
            let result = merman::Renderer::new()
                .render(merman::RenderRequest::document(
                    "info",
                    merman::OperationControl::new(),
                    merman::SvgRequest {
                        pipeline: Some(
                            merman::svg::SvgPipeline::resvg_safe()
                                .with_postprocessor(ReplaceScene(source)),
                        ),
                        ..Default::default()
                    },
                ))
                .expect("default document preserves export availability");
            let merman::RenderOutput::Document(Some(document)) = result else {
                panic!("expected document");
            };
            let mut projections = Vec::new();
            #[cfg(feature = "png")]
            if reason != "pdf_implicit_viewport_clipping_residual" {
                let output = document
                    .export_png(&Default::default(), merman::OperationControl::new())
                    .expect("best-effort PNG remains available");
                projections.push(
                    BindingThemeExecutionEvidence::project(output.evidence(), output.admission())
                        .unwrap(),
                );
            }
            #[cfg(feature = "pdf")]
            {
                let output = document
                    .export_pdf(&Default::default(), merman::OperationControl::new())
                    .expect("best-effort PDF remains available");
                projections.push(
                    BindingThemeExecutionEvidence::project(output.evidence(), output.admission())
                        .unwrap(),
                );
            }
            for evidence in projections {
                let projection = evidence.as_v2().unwrap();
                assert_eq!(
                    projection.target_status_id(),
                    "rejected",
                    "nonportable quality must not be advertised as portable"
                );
                assert!(projection.target_reason_ids().iter().any(|id| id == reason));
                let encoded = serde_json::to_value(&evidence)
                    .expect("existing schema-one binding projection");
                assert!(
                    encoded["target_reason_ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|id| id == reason)
                );
            }
        }
    }
}
