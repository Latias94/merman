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
    presentation_profile_id: Option<String>,
    presentation_aspects: Vec<SvgPlanPresentationAspect>,
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
    pub fn presentation_profile_id(&self) -> Option<&str> {
        self.presentation_profile_id.as_deref()
    }

    #[must_use]
    pub fn presentation_aspects(&self) -> &[SvgPlanPresentationAspect] {
        &self.presentation_aspects
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SvgPlanPresentationAspect {
    id: String,
    state: String,
    required_capability_id: Option<String>,
}

impl SvgPlanPresentationAspect {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }

    #[must_use]
    pub fn required_capability_id(&self) -> Option<&str> {
        self.required_capability_id.as_deref()
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
        let presentation_aspects = plan
            .presentation_aspects()
            .iter()
            .copied()
            .map(|aspect| SvgPlanPresentationAspect {
                id: aspect.id().to_string(),
                state: aspect.state().as_str().to_string(),
                required_capability_id: aspect.required_capability_id().map(str::to_string),
            })
            .collect::<Vec<_>>();
        if let Some(aspect) = presentation_aspects.iter().find(|aspect| {
            aspect.state == "blocked"
                && aspect
                    .required_capability_id
                    .as_ref()
                    .is_none_or(|capability_id| {
                        missing_capability_ids.binary_search(capability_id).is_err()
                    })
        }) {
            return Err(BindingError::new(
                BindingStatus::InternalError,
                format!(
                    "SVG capability plan reported blocked presentation aspect `{}` without a missing required capability",
                    aspect.id
                ),
            ));
        }

        Ok(Self {
            schema_version: SVG_PLAN_SCHEMA_VERSION,
            planned_operation_id: "svg".to_string(),
            diagram_type: plan.diagram_type().to_string(),
            presentation_profile_id: plan.presentation_profile_id().map(str::to_string),
            presentation_aspects,
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
    crate::execute_once_data("svg-plan-json", source, None, options_json)
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
                "presentation_profile_id": null,
                "presentation_aspects": [],
                "required_capability_ids": [],
                "missing_capability_ids": [],
                "ready": true,
            })
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn presentation_plan_reports_family_and_effective_renderer_states() {
        let sequence = plan(
            "sequenceDiagram\nA->>B: Hello",
            br#"{"presentation":{"profile":"merman-modern"}}"#,
        );
        assert_eq!(sequence["presentation_profile_id"], "merman-modern");
        assert_eq!(
            sequence["presentation_aspects"],
            serde_json::json!([
                {
                    "id": "global-defaults",
                    "state": "active",
                    "required_capability_id": null,
                },
                {
                    "id": "flowchart-svg",
                    "state": "inactive",
                    "required_capability_id": null,
                },
                {
                    "id": "flowchart-elk-default",
                    "state": "inactive",
                    "required_capability_id": "layout-elk",
                },
            ])
        );
        assert_eq!(sequence["ready"], true);

        let dagre = plan(
            "flowchart TD\nA --> B",
            br#"{
                "presentation":{"profile":"merman-modern"},
                "site_config":{"flowchart":{"defaultRenderer":"dagre-wrapper"}}
            }"#,
        );
        assert_eq!(dagre["presentation_aspects"][1]["state"], "active");
        assert_eq!(dagre["presentation_aspects"][2]["state"], "inactive");
        assert_eq!(dagre["ready"], true);

        let default_flowchart = plan(
            "flowchart TD\nA --> B",
            br#"{"presentation":{"profile":"merman-modern"}}"#,
        );
        let expected = if cfg!(feature = "layout-elk") {
            "active"
        } else {
            "blocked"
        };
        assert_eq!(
            default_flowchart["presentation_aspects"][2]["state"],
            expected
        );
        assert_eq!(default_flowchart["ready"], cfg!(feature = "layout-elk"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn elk_flowchart_plan_follows_the_artifact_owner_feature() {
        let value = plan(
            "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B",
            b"",
        );
        let expected_missing = if cfg!(feature = "layout-elk") {
            serde_json::json!([])
        } else {
            serde_json::json!(["layout-elk"])
        };

        assert_eq!(
            value["required_capability_ids"],
            serde_json::json!(["layout-elk"])
        );
        assert_eq!(value["missing_capability_ids"], expected_missing);
        assert_eq!(value["ready"], cfg!(feature = "layout-elk"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn capability_ids_are_sorted_unique_and_missing_is_a_required_subset() {
        let value = plan(
            "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA[\"$$x^2$$\"] --> B",
            b"",
        );
        let expected_missing = [
            (!cfg!(feature = "layout-elk")).then_some("layout-elk"),
            (!cfg!(feature = "math")).then_some("math"),
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
