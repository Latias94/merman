use crate::BindingError;
#[cfg(feature = "svg")]
use crate::BindingStatus;
use serde::Serialize;

/// Stable schema version for the SVG capability-plan JSON payload.
pub const SVG_PLAN_SCHEMA_VERSION: u32 = 1;

/// Capabilities required by one parsed SVG render operation.
///
/// This transport-neutral payload is derived from the renderer owner's typed plan. Capability
/// IDs are sorted and unique; every missing capability is also present in
/// `required_capability_ids`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SvgPlanPayload {
    schema_version: u32,
    planned_operation_id: String,
    diagram_type: String,
    required_capability_ids: Vec<String>,
    missing_capability_ids: Vec<String>,
    ready: bool,
}

impl SvgPlanPayload {
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub fn planned_operation_id(&self) -> &str {
        &self.planned_operation_id
    }

    #[must_use]
    pub fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    #[must_use]
    pub fn required_capability_ids(&self) -> &[String] {
        &self.required_capability_ids
    }

    #[must_use]
    pub fn missing_capability_ids(&self) -> &[String] {
        &self.missing_capability_ids
    }

    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.ready
    }
}

#[cfg(feature = "svg")]
impl SvgPlanPayload {
    pub(crate) fn from_render_plan(
        plan: &merman::svg::RenderCapabilityPlan,
    ) -> Result<Self, BindingError> {
        let mut required_capability_ids = plan
            .required_capability_ids()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        required_capability_ids.sort_unstable();
        required_capability_ids.dedup();

        let mut missing_capability_ids = plan
            .missing_capability_ids()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        missing_capability_ids.sort_unstable();
        missing_capability_ids.dedup();

        if let Some(capability_id) = missing_capability_ids.iter().find(|capability_id| {
            required_capability_ids
                .binary_search(capability_id)
                .is_err()
        }) {
            return Err(BindingError::new(
                BindingStatus::InternalError,
                format!(
                    "SVG capability plan reported missing capability `{capability_id}` without requiring it"
                ),
            ));
        }

        Ok(Self {
            schema_version: SVG_PLAN_SCHEMA_VERSION,
            planned_operation_id: "svg".to_string(),
            diagram_type: plan.diagram_type().to_string(),
            ready: missing_capability_ids.is_empty(),
            required_capability_ids,
            missing_capability_ids,
        })
    }

    pub(crate) fn to_json_bytes(&self) -> Result<Vec<u8>, BindingError> {
        serde_json::to_vec(self).map_err(crate::common::internal_json_error)
    }
}

/// Plans the capabilities needed to render one Mermaid source as SVG.
///
/// Builds without the `svg` feature preserve the API shape and return a typed
/// `missing-capability(svg)` error.
pub fn svg_plan_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    crate::execute_once(crate::BindingOperationRequest {
        operation_id: "svg-plan-json",
        source,
        uri: None,
        options_json,
    })
    .map(|result| result.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "svg")]
    fn plan(source: &str, options_json: &[u8]) -> serde_json::Value {
        serde_json::from_slice(&svg_plan_json(source.as_bytes(), options_json).unwrap()).unwrap()
    }

    #[cfg(not(feature = "svg"))]
    #[test]
    fn unavailable_svg_plan_is_a_typed_capability_error() {
        let error = svg_plan_json(b"flowchart TD\nA --> B", b"").unwrap_err();

        assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("svg"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn basic_flowchart_plan_is_ready_without_optional_backends() {
        assert_eq!(
            plan("flowchart TD\nA --> B", b""),
            serde_json::json!({
                "schema_version": SVG_PLAN_SCHEMA_VERSION,
                "planned_operation_id": "svg",
                "diagram_type": "flowchart-v2",
                "required_capability_ids": [],
                "missing_capability_ids": [],
                "ready": true,
            })
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn elk_flowchart_plan_follows_the_resolved_dependency_feature_set() {
        let value = plan(
            "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B",
            b"",
        );
        let expected_missing = if merman::svg::layout_elk_available() {
            serde_json::json!([])
        } else {
            serde_json::json!(["layout-elk"])
        };

        assert_eq!(
            value["required_capability_ids"],
            serde_json::json!(["layout-elk"])
        );
        assert_eq!(value["missing_capability_ids"], expected_missing);
        assert_eq!(value["ready"], merman::svg::layout_elk_available());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn capability_ids_are_sorted_unique_and_missing_is_a_required_subset() {
        let value = plan(
            "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA[\"$$x^2$$\"] --> B",
            b"",
        );
        let expected_missing = [
            (!merman::svg::layout_elk_available()).then_some("layout-elk"),
            (!merman::svg::math_available()).then_some("math"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        assert_eq!(
            value["required_capability_ids"],
            serde_json::json!(["layout-elk", "math"])
        );
        assert_eq!(
            value["missing_capability_ids"],
            serde_json::json!(expected_missing)
        );
        assert_eq!(value["ready"], expected_missing.is_empty());
    }

    #[cfg(all(
        feature = "svg",
        feature = "layout-cytoscape",
        feature = "layout-elk",
        feature = "math"
    ))]
    #[test]
    fn complete_svg_backends_make_their_operations_ready() {
        let cases = [
            "architecture-beta\n  service api(server)[API]",
            "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B",
            "flowchart TD\nA[\"$$x^2$$\"] --> B",
        ];

        for source in cases {
            let value = plan(source, b"");
            assert_eq!(value["ready"], true, "plan was not ready: {value}");
            assert_eq!(value["missing_capability_ids"], serde_json::json!([]));
        }
    }

    #[cfg(all(feature = "svg", feature = "math"))]
    #[test]
    fn request_environment_policy_can_make_compiled_math_unavailable() {
        let value = plan(
            "flowchart TD\nA[\"$$x^2$$\"] --> B",
            br#"{"environment":{"math_renderer":"none"}}"#,
        );

        assert_eq!(
            value["required_capability_ids"],
            serde_json::json!(["math"])
        );
        assert_eq!(value["missing_capability_ids"], serde_json::json!(["math"]));
        assert_eq!(value["ready"], false);
    }
}
