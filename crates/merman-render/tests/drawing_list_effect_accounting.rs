use merman_core::{Engine, OperationControl, ParseOptions};
use merman_display_list::{
    DrawingCommand, DrawingListDocument, DrawingListPolicy, DrawingResource,
};
use merman_render::environment::RenderEnvironment;
use merman_render::{LayoutOptions, family};
use serde::Deserialize;
use std::collections::BTreeSet;

const EFFECT_COVERAGE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/drawing-list/v1/effect-coverage.json"
));

#[derive(Debug, Deserialize)]
struct EffectCoverageMatrix {
    schema_version: u32,
    effects: Vec<EffectCoverageRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectDisposition {
    Vector,
    Raster,
    Error,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectAssertion {
    PathFillStroke,
    HostText,
    LinearGradient,
    ClipPath,
    SemanticLink,
    Opacity,
    BlendMode,
    Transform,
    ExpandedMarker,
    ErrorMessage,
}

impl EffectAssertion {
    fn protocol_kind(&self) -> &'static str {
        match self {
            Self::PathFillStroke => "path",
            Self::HostText => "host_text",
            Self::LinearGradient => "linear_gradient",
            Self::ClipPath => "clip_path",
            Self::SemanticLink => "semantic_link",
            Self::Opacity => "opacity",
            Self::BlendMode => "blend_mode",
            Self::Transform => "transform",
            Self::ExpandedMarker => "expanded_marker",
            Self::ErrorMessage => "structured_error",
        }
    }
}

#[derive(Debug, Deserialize)]
struct EffectCoverageRow {
    id: String,
    disposition: EffectDisposition,
    family: String,
    source: String,
    assertion: EffectAssertion,
    expected_error: Option<String>,
    evidence: String,
}

#[test]
fn exercised_effects_have_an_explicit_vector_raster_or_error_disposition() {
    let matrix: EffectCoverageMatrix = serde_json::from_str(EFFECT_COVERAGE_JSON)
        .expect("effect coverage fixture must be valid JSON");
    assert_eq!(matrix.schema_version, 1);
    assert!(!matrix.effects.is_empty());
    let known_families = merman_render::family::RenderFamilyKind::ALL
        .into_iter()
        .map(merman_render::family::RenderFamilyKind::as_str)
        .collect::<BTreeSet<_>>();
    let mut effect_ids = BTreeSet::new();
    let declared_effects = matrix
        .effects
        .iter()
        .map(|row| row.assertion.protocol_kind())
        .collect::<BTreeSet<_>>();
    let mut observed_effects = BTreeSet::new();

    for row in matrix.effects {
        assert!(!row.id.trim().is_empty(), "effect ids must not be empty");
        assert!(
            effect_ids.insert(row.id.clone()),
            "duplicate effect id {}",
            row.id
        );
        assert!(!row.family.trim().is_empty(), "{} has no family", row.id);
        assert!(
            known_families.contains(row.family.as_str()),
            "{} references unknown family {}",
            row.id,
            row.family
        );
        assert!(!row.source.trim().is_empty(), "{} has no source", row.id);
        assert!(
            !row.evidence.trim().is_empty(),
            "{} has no evidence",
            row.id
        );

        let (family, rendered) = render_drawing_list(&row.source);
        assert_eq!(family, row.family, "{} family fixture drifted", row.id);
        match row.disposition {
            EffectDisposition::Vector => {
                assert!(
                    row.expected_error.is_none(),
                    "vector effect {} must not carry an error expectation",
                    row.id
                );
                let document = rendered.unwrap_or_else(|error| {
                    panic!(
                        "{} ({}) is classified as vector but failed: {error}",
                        row.id, row.family
                    )
                });
                observed_effects.extend(observed_protocol_effects(&document));
                assert_effect_construct(&row.id, row.assertion, &document);
            }
            EffectDisposition::Raster => {
                assert!(
                    row.expected_error.is_none(),
                    "raster effect {} must not carry an error expectation",
                    row.id
                );
                let document = rendered.unwrap_or_else(|error| {
                    panic!(
                        "{} ({}) is classified as raster but failed: {error}",
                        row.id, row.family
                    )
                });
                observed_effects.extend(observed_protocol_effects(&document));
                assert_effect_construct(&row.id, row.assertion, &document);
                assert!(
                    document
                        .commands
                        .iter()
                        .any(|command| matches!(command, DrawingCommand::DrawRasterSubtree { .. })),
                    "{} must expose a raster subtree command",
                    row.id
                );
            }
            EffectDisposition::Error => {
                assert!(
                    row.expected_error.is_some(),
                    "error effect {} must carry an error expectation",
                    row.id
                );
                let error = rendered.expect_err(&format!(
                    "{} ({}) is classified as an explicit error",
                    row.id, row.family
                ));
                assert!(matches!(row.assertion, EffectAssertion::ErrorMessage));
                let expected = row
                    .expected_error
                    .as_deref()
                    .unwrap_or_else(|| panic!("{} has no expected error text", row.id));
                assert!(
                    error.contains(expected),
                    "{} error should contain {expected:?}, got {error:?}",
                    row.id
                );
                assert!(
                    error.contains("DrawingList"),
                    "{} must remain a structured DrawingList capability error: {error}",
                    row.id
                );
            }
        }
    }

    for effect in observed_effects {
        assert!(
            declared_effects.contains(effect),
            "observed DrawingList effect {effect:?} has no explicit coverage assertion"
        );
    }
}

fn render_drawing_list(source: &str) -> (String, Result<DrawingListDocument, String>) {
    let parsed =
        match Engine::new().parse_diagram_for_render_model_sync(source, ParseOptions::lenient()) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => {
                return (
                    String::new(),
                    Err("source did not produce a diagram".into()),
                );
            }
            Err(error) => return (String::new(), Err(error.to_string())),
        };
    let session = match RenderEnvironment::deterministic()
        .begin_session_with_control(OperationControl::new())
    {
        Ok(session) => session,
        Err(error) => return (String::new(), Err(error.to_string())),
    };
    let artifact = match family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session) {
        Ok(artifact) => artifact,
        Err(error) => return (String::new(), Err(error.to_string())),
    };
    let family = artifact.family_kind().as_str().to_string();
    let rendered = artifact
        .render_drawing_list(DrawingListPolicy::AllowRasterSubtree, Default::default())
        .map(|output| output.document().clone())
        .map_err(|error| error.to_string());
    (family, rendered)
}

fn assert_effect_construct(id: &str, assertion: EffectAssertion, document: &DrawingListDocument) {
    match assertion {
        EffectAssertion::PathFillStroke => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawPath { style, .. }
                    if style.fill.is_some() && style.stroke.is_some()
            )),
            "{id} must retain both fill and stroke"
        ),
        EffectAssertion::HostText => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawText { run }
                    if matches!(run.obligation, merman_display_list::TextObligation::HostText { .. })
            )),
            "{id} must retain a host-text obligation"
        ),
        EffectAssertion::LinearGradient => assert!(
            document
                .resources
                .iter()
                .any(|resource| matches!(resource, DrawingResource::LinearGradient(_))),
            "{id} must retain a linear-gradient resource"
        ),
        EffectAssertion::ClipPath => assert!(
            document
                .commands
                .iter()
                .any(|command| matches!(command, DrawingCommand::ClipPath { .. })),
            "{id} must retain a clip-path command"
        ),
        EffectAssertion::SemanticLink => assert!(
            document
                .semantics
                .iter()
                .any(|semantic| semantic.link.is_some()),
            "{id} must retain semantic navigation metadata"
        ),
        EffectAssertion::Opacity => assert!(
            document.commands.iter().any(|command| {
                matches!(command, DrawingCommand::SetOpacity { opacity } if *opacity < 1.0)
            }),
            "{id} must retain a non-default opacity command"
        ),
        EffectAssertion::BlendMode => assert!(
            document.commands.iter().any(|command| {
                matches!(
                    command,
                    DrawingCommand::SetBlendMode { blend_mode }
                        if *blend_mode != merman_display_list::BlendMode::Normal
                )
            }),
            "{id} must retain a non-default blend-mode command"
        ),
        EffectAssertion::Transform => assert!(
            document.commands.iter().any(|command| {
                matches!(
                    command,
                    DrawingCommand::ConcatTransform { transform }
                        if *transform != merman_display_list::Transform::IDENTITY
                )
            }),
            "{id} must retain a non-identity transform command"
        ),
        EffectAssertion::ExpandedMarker => assert!(
            has_expanded_marker(document),
            "{id} must retain expanded marker geometry"
        ),
        EffectAssertion::ErrorMessage => {
            panic!("{id} is an error assertion but was rendered as a document")
        }
    }
}

fn observed_protocol_effects(document: &DrawingListDocument) -> BTreeSet<&'static str> {
    let mut effects = BTreeSet::new();
    if document
        .resources
        .iter()
        .any(|resource| matches!(resource, DrawingResource::Path(_)))
    {
        effects.insert("path");
    }
    if document
        .resources
        .iter()
        .any(|resource| matches!(resource, DrawingResource::LinearGradient(_)))
    {
        effects.insert("linear_gradient");
    }
    if document
        .commands
        .iter()
        .any(|command| matches!(command, DrawingCommand::ClipPath { .. }))
    {
        effects.insert("clip_path");
    }
    if document
        .commands
        .iter()
        .any(|command| matches!(command, DrawingCommand::SetOpacity { opacity } if *opacity < 1.0))
    {
        effects.insert("opacity");
    }
    if document.commands.iter().any(|command| {
        matches!(
            command,
            DrawingCommand::SetBlendMode { blend_mode }
                if *blend_mode != merman_display_list::BlendMode::Normal
        )
    }) {
        effects.insert("blend_mode");
    }
    if document.commands.iter().any(|command| {
        matches!(
            command,
            DrawingCommand::ConcatTransform { transform }
                if *transform != merman_display_list::Transform::IDENTITY
        )
    }) {
        effects.insert("transform");
    }
    if document.commands.iter().any(|command| {
        matches!(
            command,
            DrawingCommand::DrawText { run }
                if matches!(run.obligation, merman_display_list::TextObligation::HostText { .. })
        )
    }) {
        effects.insert("host_text");
    }
    if document
        .semantics
        .iter()
        .any(|semantic| semantic.link.is_some())
    {
        effects.insert("semantic_link");
    }
    if has_expanded_marker(document) {
        effects.insert("expanded_marker");
    }
    effects
}

fn has_expanded_marker(document: &DrawingListDocument) -> bool {
    document.resources.iter().any(|resource| {
        matches!(
            resource,
            DrawingResource::Path(path)
                if path.id.as_str().contains("marker")
                    || (path.id.as_str().starts_with("wardley.link.")
                        && (path.id.as_str().ends_with(".start")
                            || path.id.as_str().ends_with(".end")))
                    || (path.id.as_str().starts_with("wardley.trend.")
                        && path.id.as_str().ends_with(".end"))
        )
    })
}
