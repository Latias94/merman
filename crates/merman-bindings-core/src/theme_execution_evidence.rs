use crate::BindingError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Schema version for renderer-owned theme execution evidence and explanatory diagnostics.
pub const BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// Maximum UTF-8 byte length accepted for one identifier in known evidence.
pub const BINDING_THEME_EXECUTION_EVIDENCE_MAX_ID_UTF8_BYTES: usize = 128;

/// Maximum number of target-admission reasons accepted in known evidence.
pub const BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS: usize = 32;

/// Versioned theme execution evidence attached to successful render metadata.
///
/// Version one projects renderer-owned render evidence and target admission as coarse states.
/// Diagnostics explain residuals without exposing internal rule IDs or conferring certification.
/// Unknown versions remain opaque so bindings can forward metadata produced by newer revisions.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BindingThemeExecutionEvidence {
    V1(BindingThemeExecutionEvidenceV1),
    Unknown(BindingUnknownThemeExecutionEvidence),
}

impl BindingThemeExecutionEvidence {
    #[must_use]
    pub const fn version(&self) -> u32 {
        match self {
            Self::V1(_) => BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION,
            Self::Unknown(evidence) => evidence.version(),
        }
    }

    #[must_use]
    pub const fn as_v1(&self) -> Option<&BindingThemeExecutionEvidenceV1> {
        match self {
            Self::V1(evidence) => Some(evidence),
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
        BindingThemeExecutionEvidenceV1::new(
            render_evidence.family_id().as_str().to_owned(),
            theme_status_id.to_owned(),
            theme.output_mutated(),
            admission.artifact_kind().id().to_owned(),
            admission.status().id().to_owned(),
            admission
                .reasons()
                .iter()
                .map(|reason| reason.id().to_owned())
                .collect(),
            admission.font_source().id().to_owned(),
        )
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
            Self::V1(evidence)
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
            Self::V1(evidence) => evidence.serialize(serializer),
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

/// Known version-one projection of successful renderer theme and target evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingThemeExecutionEvidenceV1 {
    family_id: String,
    theme_status_id: String,
    output_mutated: bool,
    target_kind_id: String,
    target_status_id: String,
    target_reason_ids: Box<[String]>,
    font_source_id: String,
    diagnostics: Option<Box<[BindingThemeDiagnostic]>>,
}

impl BindingThemeExecutionEvidenceV1 {
    fn new(
        family_id: String,
        theme_status_id: String,
        output_mutated: bool,
        target_kind_id: String,
        target_status_id: String,
        target_reason_ids: Vec<String>,
        font_source_id: String,
    ) -> Result<Self, String> {
        for (field, id) in [
            ("family_id", family_id.as_str()),
            ("theme_status", theme_status_id.as_str()),
            ("target_kind", target_kind_id.as_str()),
            ("target_status", target_status_id.as_str()),
            ("font_source", font_source_id.as_str()),
        ] {
            validate_id(field, id)?;
        }
        if target_reason_ids.len() > BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS {
            return Err(format!(
                "`target_reason_ids` must contain at most {BINDING_THEME_EXECUTION_EVIDENCE_MAX_TARGET_REASON_IDS} entries"
            ));
        }
        for (index, reason) in target_reason_ids.iter().enumerate() {
            validate_id(&format!("target_reason_ids[{index}]"), reason)?;
            if target_reason_ids[..index].contains(reason) {
                return Err("`target_reason_ids` entries must be unique".to_owned());
            }
        }

        Ok(Self {
            family_id,
            theme_status_id,
            output_mutated,
            target_kind_id,
            target_status_id,
            target_reason_ids: target_reason_ids.into_boxed_slice(),
            font_source_id,
            diagnostics: None,
        })
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

    /// `None` means the producer did not supply diagnostics. An empty slice means it supplied
    /// no explanations; consumers must still inspect the theme and target admission states.
    #[must_use]
    pub fn diagnostics(&self) -> Option<&[BindingThemeDiagnostic]> {
        self.diagnostics.as_deref()
    }
}

impl Serialize for BindingThemeExecutionEvidenceV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        BindingThemeExecutionEvidenceV1Wire {
            version: BINDING_THEME_EXECUTION_EVIDENCE_SCHEMA_VERSION,
            family_id: self.family_id(),
            theme_status: self.theme_status_id(),
            output_mutated: self.output_mutated(),
            target_kind: self.target_kind_id(),
            target_status: self.target_status_id(),
            target_reason_ids: self.target_reason_ids(),
            font_source: self.font_source_id(),
            diagnostics: self.diagnostics(),
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
struct BindingThemeExecutionEvidenceV1Wire<'a> {
    version: u32,
    family_id: &'a str,
    theme_status: &'a str,
    output_mutated: bool,
    target_kind: &'a str,
    target_status: &'a str,
    target_reason_ids: &'a [String],
    font_source: &'a str,
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

    BindingThemeExecutionEvidenceV1::new(
        required_bounded_id(object, "family_id")?,
        required_bounded_id(object, "theme_status")?,
        required_bool(object, "output_mutated")?,
        required_bounded_id(object, "target_kind")?,
        required_bounded_id(object, "target_status")?,
        target_reason_ids,
        required_bounded_id(object, "font_source")?,
    )
    .map(|mut evidence| {
        evidence.diagnostics = diagnostics;
        BindingThemeExecutionEvidence::V1(evidence)
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
