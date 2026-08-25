use crate::{
    BINDING_OPERATION_SCHEMA_VERSION, BindingError, BindingErrorKind, BindingOperationKind,
    BindingStatus,
};
use serde::Serialize;
use std::sync::OnceLock;

/// Schema version for the machine-readable operation-metadata contract itself.
pub const BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// One JSON field in the stable operation-metadata contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingJsonFieldContract {
    name: &'static str,
    json_type: &'static str,
    required: bool,
    integer_width_bits: Option<u8>,
    open_value: bool,
}

impl BindingJsonFieldContract {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn json_type(&self) -> &'static str {
        self.json_type
    }

    #[must_use]
    pub const fn required(&self) -> bool {
        self.required
    }

    #[must_use]
    pub const fn integer_width_bits(&self) -> Option<u8> {
        self.integer_width_bits
    }

    #[must_use]
    pub const fn open_value(&self) -> bool {
        self.open_value
    }
}

/// One known output-plan discriminant in schema 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingOutputPlanContract {
    kind: &'static str,
    fields: &'static [BindingJsonFieldContract],
}

impl BindingOutputPlanContract {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    #[must_use]
    pub const fn fields(&self) -> &'static [BindingJsonFieldContract] {
        self.fields
    }
}

/// Stable generator input for operation metadata and output-plan decoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingOperationMetadataContract {
    contract_schema_version: u32,
    metadata_schema_version: u32,
    fields: &'static [BindingJsonFieldContract],
    output_plans: &'static [BindingOutputPlanContract],
    additional_fields_policy: &'static str,
    unknown_output_plan_policy: &'static str,
    original_json_policy: &'static str,
}

impl BindingOperationMetadataContract {
    #[must_use]
    pub const fn contract_schema_version(&self) -> u32 {
        self.contract_schema_version
    }

    #[must_use]
    pub const fn metadata_schema_version(&self) -> u32 {
        self.metadata_schema_version
    }

    #[must_use]
    pub const fn fields(&self) -> &'static [BindingJsonFieldContract] {
        self.fields
    }

    #[must_use]
    pub const fn output_plans(&self) -> &'static [BindingOutputPlanContract] {
        self.output_plans
    }

    #[must_use]
    pub const fn additional_fields_policy(&self) -> &'static str {
        self.additional_fields_policy
    }

    #[must_use]
    pub const fn unknown_output_plan_policy(&self) -> &'static str {
        self.unknown_output_plan_policy
    }

    #[must_use]
    pub const fn original_json_policy(&self) -> &'static str {
        self.original_json_policy
    }
}

const TOP_LEVEL_FIELDS: &[BindingJsonFieldContract] = &[
    field("version", "unsigned-integer", true, Some(32), false),
    field("operation_id", "string", true, None, true),
    field("media_type", "string", true, None, true),
    field("runtime_policy", "string", true, None, true),
    field("byte_length", "unsigned-integer", true, Some(64), false),
    field("output_plan", "object", false, None, true),
];

const RASTER_FIELDS: &[BindingJsonFieldContract] = &[
    field("kind", "string", true, None, false),
    field("requested_width_px", "number", true, None, false),
    field("requested_height_px", "number", true, None, false),
    field("width_px", "unsigned-integer", true, Some(32), false),
    field("height_px", "unsigned-integer", true, Some(32), false),
    field("requested_scale", "number", true, None, false),
    field("effective_scale", "number", true, None, false),
    field("limited", "boolean", true, None, false),
];

const PDF_FILTER_IMAGE_FIELDS: &[BindingJsonFieldContract] = &[
    field("kind", "string", true, None, false),
    field("filtered_groups", "unsigned-integer", true, Some(64), false),
    field("requested_scale", "number", true, None, false),
    field("effective_scale", "number", true, None, false),
    field(
        "requested_image_pixels",
        "unsigned-integer",
        true,
        Some(64),
        false,
    ),
    field(
        "effective_image_pixels",
        "unsigned-integer",
        true,
        Some(64),
        false,
    ),
    field("limited", "boolean", true, None, false),
];

const ASCII_FIELDS: &[BindingJsonFieldContract] = &[
    field("kind", "string", true, None, false),
    field("schema_version", "unsigned-integer", true, Some(16), false),
    field("family", "string", true, None, true),
    field("projection", "string", true, None, false),
    field("primary_width", "unsigned-integer", true, Some(64), false),
    field("primary_height", "unsigned-integer", true, Some(64), false),
    field("emitted_width", "unsigned-integer", true, Some(64), false),
    field("emitted_height", "unsigned-integer", true, Some(64), false),
    field("width_profile", "string", true, None, false),
    field("layout_profile", "string", true, None, false),
    field(
        "requested_max_width",
        "unsigned-integer",
        false,
        Some(64),
        false,
    ),
    field("overflowed", "boolean", true, None, false),
    field("outcome", "string", true, None, false),
    field("fallback_capability", "string", true, None, false),
    field("fallback_attempted", "boolean", true, None, false),
    field("fallback_reason", "string", false, None, false),
    field("trimmed", "boolean", true, None, false),
    field("lossiness", "string", true, None, false),
];

const OUTPUT_PLANS: &[BindingOutputPlanContract] = &[
    BindingOutputPlanContract {
        kind: "ascii",
        fields: ASCII_FIELDS,
    },
    BindingOutputPlanContract {
        kind: "raster",
        fields: RASTER_FIELDS,
    },
    BindingOutputPlanContract {
        kind: "pdf-filter-images",
        fields: PDF_FILTER_IMAGE_FIELDS,
    },
];

const OPERATION_METADATA_CONTRACT: BindingOperationMetadataContract =
    BindingOperationMetadataContract {
        contract_schema_version: BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION,
        metadata_schema_version: BINDING_OPERATION_SCHEMA_VERSION,
        fields: TOP_LEVEL_FIELDS,
        output_plans: OUTPUT_PLANS,
        additional_fields_policy: "preserve",
        unknown_output_plan_policy: "preserve",
        original_json_policy: "preserve-exact-bytes",
    };

const fn field(
    name: &'static str,
    json_type: &'static str,
    required: bool,
    integer_width_bits: Option<u8>,
    open_value: bool,
) -> BindingJsonFieldContract {
    BindingJsonFieldContract {
        name,
        json_type,
        required,
        integer_width_bits,
        open_value,
    }
}

#[must_use]
pub const fn operation_metadata_contract() -> &'static BindingOperationMetadataContract {
    &OPERATION_METADATA_CONTRACT
}

pub fn operation_metadata_contract_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(operation_metadata_contract()).map_err(|error| {
        BindingError::internal(format!(
            "failed to serialize the operation metadata contract: {error}"
        ))
    })
}

/// Fixed caller-visible failure expected when an operation is not compiled into an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingUnavailableOperationExpectation {
    status_code: i32,
    status_name: &'static str,
    error_kind: &'static str,
    capability_id: &'static str,
}

impl BindingUnavailableOperationExpectation {
    #[must_use]
    pub const fn status_code(&self) -> i32 {
        self.status_code
    }

    #[must_use]
    pub const fn status_name(&self) -> &'static str {
        self.status_name
    }

    #[must_use]
    pub const fn error_kind(&self) -> &'static str {
        self.error_kind
    }

    #[must_use]
    pub const fn capability_id(&self) -> &'static str {
        self.capability_id
    }
}

/// One descriptor-derived row in the shared 13-operation expectation matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingOperationExpectation {
    operation_id: &'static str,
    output_id: Option<&'static str>,
    media_type: &'static str,
    metadata_schema_version: u32,
    requires_uri: bool,
    availability_capability_id: Option<&'static str>,
    compiled_prerequisite_ids: Box<[&'static str]>,
    compiled: bool,
    unavailable: Option<BindingUnavailableOperationExpectation>,
}

impl BindingOperationExpectation {
    #[must_use]
    pub const fn operation_id(&self) -> &'static str {
        self.operation_id
    }

    #[must_use]
    pub const fn output_id(&self) -> Option<&'static str> {
        self.output_id
    }

    #[must_use]
    pub const fn media_type(&self) -> &'static str {
        self.media_type
    }

    #[must_use]
    pub const fn metadata_schema_version(&self) -> u32 {
        self.metadata_schema_version
    }

    #[must_use]
    pub const fn requires_uri(&self) -> bool {
        self.requires_uri
    }

    #[must_use]
    pub const fn availability_capability_id(&self) -> Option<&'static str> {
        self.availability_capability_id
    }

    #[must_use]
    pub fn compiled_prerequisite_ids(&self) -> &[&'static str] {
        &self.compiled_prerequisite_ids
    }

    #[must_use]
    pub const fn compiled(&self) -> bool {
        self.compiled
    }

    #[must_use]
    pub const fn unavailable(&self) -> Option<BindingUnavailableOperationExpectation> {
        self.unavailable
    }
}

static OPERATION_EXPECTATIONS: OnceLock<Box<[BindingOperationExpectation]>> = OnceLock::new();

#[must_use]
pub fn binding_operation_expectations() -> &'static [BindingOperationExpectation] {
    OPERATION_EXPECTATIONS.get_or_init(|| {
        BindingOperationKind::all()
            .map(|operation| {
                let compiled = operation.is_compiled();
                let unavailable = operation.availability_capability_id().map(|capability_id| {
                    BindingUnavailableOperationExpectation {
                        status_code: BindingStatus::UnsupportedOperation.code(),
                        status_name: BindingStatus::UnsupportedOperation.code_name(),
                        error_kind: BindingErrorKind::MissingCapability.id(),
                        capability_id,
                    }
                });
                BindingOperationExpectation {
                    operation_id: operation.operation_id(),
                    output_id: operation.key().spec().output.map(crate::OutputKey::id),
                    media_type: operation.media_type(),
                    metadata_schema_version: BINDING_OPERATION_SCHEMA_VERSION,
                    requires_uri: operation.requires_uri(),
                    availability_capability_id: operation.availability_capability_id(),
                    compiled_prerequisite_ids: operation
                        .key()
                        .spec()
                        .compiled_prerequisites
                        .iter()
                        .copied()
                        .map(crate::CapabilityKey::id)
                        .collect(),
                    compiled,
                    unavailable,
                }
            })
            .collect()
    })
}

pub fn binding_operation_expectations_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(binding_operation_expectations()).map_err(|error| {
        BindingError::internal(format!(
            "failed to serialize the binding operation expectation matrix: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_metadata_contract_is_open_and_width_explicit() {
        let contract = operation_metadata_contract();
        assert_eq!(contract.contract_schema_version(), 1);
        assert_eq!(contract.metadata_schema_version(), 1);
        assert_eq!(contract.additional_fields_policy(), "preserve");
        assert_eq!(contract.unknown_output_plan_policy(), "preserve");
        assert_eq!(contract.original_json_policy(), "preserve-exact-bytes");

        let byte_length = contract
            .fields()
            .iter()
            .find(|field| field.name() == "byte_length")
            .unwrap();
        assert_eq!(byte_length.integer_width_bits(), Some(64));
        assert_eq!(contract.output_plans().len(), 3);
    }

    #[test]
    fn shared_expectation_matrix_covers_every_descriptor_operation() {
        let rows = binding_operation_expectations();
        assert_eq!(rows.len(), 13);
        for (row, operation) in rows.iter().zip(BindingOperationKind::all()) {
            assert_eq!(row.operation_id(), operation.operation_id());
            assert_eq!(
                row.output_id(),
                operation.key().spec().output.map(crate::OutputKey::id)
            );
            assert_eq!(row.media_type(), operation.media_type());
            assert_eq!(row.requires_uri(), operation.requires_uri());
            assert_eq!(
                row.availability_capability_id(),
                operation.availability_capability_id()
            );
            assert_eq!(
                row.compiled_prerequisite_ids(),
                operation
                    .key()
                    .spec()
                    .compiled_prerequisites
                    .iter()
                    .copied()
                    .map(crate::CapabilityKey::id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(row.compiled(), operation.is_compiled());
            match row.unavailable() {
                Some(error) => {
                    assert_eq!(
                        error.status_code(),
                        BindingStatus::UnsupportedOperation.code()
                    );
                    assert_eq!(error.error_kind(), BindingErrorKind::MissingCapability.id());
                    assert_eq!(
                        error.capability_id(),
                        row.availability_capability_id().unwrap()
                    );
                }
                None => assert_eq!(row.operation_id(), "semantic-json"),
            }
        }
    }

    #[test]
    fn generator_projections_are_stable_json() {
        let contract: serde_json::Value =
            serde_json::from_slice(&operation_metadata_contract_json().unwrap()).unwrap();
        assert_eq!(contract["metadata_schema_version"], 1);
        assert_eq!(contract["unknown_output_plan_policy"], "preserve");

        let matrix: serde_json::Value =
            serde_json::from_slice(&binding_operation_expectations_json().unwrap()).unwrap();
        assert_eq!(matrix.as_array().unwrap().len(), 13);
        assert_eq!(matrix[0]["operation_id"], "analysis-facts-json");
        let png = matrix
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["operation_id"] == "png")
            .unwrap();
        assert_eq!(png["output_id"], "png");
        assert_eq!(png["availability_capability_id"], "png");
        assert_eq!(png["compiled_prerequisite_ids"], serde_json::json!(["svg"]));
        assert_eq!(matrix[12]["operation_id"], "validation-json");
    }
}
