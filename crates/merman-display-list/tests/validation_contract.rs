mod common;

use common::sample_document;
use merman_display_list::{
    DrawingCommand, DrawingListLimits, Paint, PathSegment, Point, ResourceId,
};

#[test]
fn validator_rejects_unbalanced_state_and_wrong_resource_kind() {
    let mut document = sample_document();
    document.commands = vec![DrawingCommand::Restore];
    assert!(document.validate().is_err());

    let mut document = sample_document();
    let path = document
        .resources
        .iter_mut()
        .find_map(|resource| resource.path_mut())
        .expect("fixture has a path resource");
    path.segments[0] = PathSegment::LineTo {
        to: Point::new(1.0, 1.0),
    };
    assert!(document.validate().is_err());

    let mut document = sample_document();
    let path_id = ResourceId::new("path.node-a");
    document.commands[3] = DrawingCommand::DrawPath {
        path: path_id,
        style: merman_display_list::PathStyle {
            fill: Some(Paint::resource(ResourceId::new("path.node-a"))),
            stroke: None,
        },
    };
    assert!(document.validate().is_err());
}

#[test]
fn validator_enforces_exact_command_and_numeric_boundaries() {
    let document = sample_document();
    let exact = DrawingListLimits {
        max_commands: document.commands.len(),
        ..DrawingListLimits::default()
    };
    document
        .validate_with_limits(&exact)
        .expect("exact command limit is accepted");

    let too_small = DrawingListLimits {
        max_commands: document.commands.len() - 1,
        ..exact
    };
    assert!(document.validate_with_limits(&too_small).is_err());

    let mut non_finite = document;
    non_finite.viewport.bounds.width = f64::NAN;
    assert!(non_finite.validate().is_err());

    let document = sample_document();
    let encoded = document.canonical_json_bytes().unwrap();
    let too_small = DrawingListLimits {
        max_serialized_bytes: encoded.len() - 1,
        ..DrawingListLimits::default()
    };
    assert!(
        document
            .canonical_json_bytes_with_limits(&too_small)
            .is_err()
    );
    assert!(
        merman_display_list::DrawingListDocument::from_json_bytes_with_limits(&encoded, &too_small)
            .is_err()
    );
}
