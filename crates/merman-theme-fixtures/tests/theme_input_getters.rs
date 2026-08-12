use merman_theme_fixtures::{
    ReferenceCanvasLayer, ReferenceDiagramFamily, ReferenceGradientRepetition,
    ReferenceSemanticRule, ReferenceSemanticTarget, ReferenceTextTransform, ThemeFixtureCatalog,
};
use std::path::{Path, PathBuf};

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

fn source_catalog() -> ThemeFixtureCatalog {
    ThemeFixtureCatalog::load(themes_root()).expect("load committed theme source catalog")
}

#[test]
fn token_typography_and_node_style_values_are_readable_without_mutation() {
    let catalog = source_catalog();
    let input = catalog
        .fixture("fixture-token-baseline")
        .and_then(|fixture| fixture.theme_input())
        .expect("token baseline input");

    let tokens = input.tokens().expect("theme tokens");
    assert_eq!(tokens.background(), "#ffffff");
    assert_eq!(tokens.surface(), "#f8fafc");
    assert_eq!(tokens.primary(), "#2563eb");
    assert_eq!(tokens.text(), "#0f172a");

    let typography = input.typography().expect("theme typography");
    let font_stack = typography.font_stack().expect("font stack");
    assert_eq!(
        font_stack.families(),
        &["Inter", "Noto Sans SC", "sans-serif"]
    );
    assert_eq!(typography.letter_spacing_milli_em(), None);
    assert_eq!(typography.text_transform(), None);

    let node_style = input.node_style().expect("node style");
    let border = node_style.border().expect("node border");
    assert_eq!(border.color(), "#2563eb");
    assert_eq!(border.width_px(), 1);
    assert!(node_style.dash_pattern().is_empty());
    assert_eq!(node_style.corner_radius_px(), Some(8));

    let shadow = node_style.shadow().expect("node shadow");
    assert_eq!(shadow.offset_x_px(), 0);
    assert_eq!(shadow.offset_y_px(), 2);
    assert_eq!(shadow.blur_px(), 8);
    assert_eq!(shadow.spread_px(), 0);
    assert_eq!(shadow.color(), "#0f172a33");
}

#[test]
fn canvas_layers_and_gradient_stop_values_are_readable() {
    let catalog = source_catalog();
    let input = catalog
        .fixture("fixture-layered-canvas")
        .and_then(|fixture| fixture.theme_input())
        .expect("layered canvas input");
    let layers = input.canvas();

    assert_eq!(layers.len(), 4);
    assert!(matches!(
        &layers[0],
        ReferenceCanvasLayer::Solid { color } if color == "#020617"
    ));

    let ReferenceCanvasLayer::LinearGradient {
        angle_degrees,
        repetition,
        tile_width_px,
        tile_height_px,
        stops,
    } = &layers[1]
    else {
        panic!("second layer should be a tiled linear gradient");
    };
    assert_eq!(*angle_degrees, 90);
    assert_eq!(*repetition, ReferenceGradientRepetition::Tiled);
    assert_eq!(*tile_width_px, Some(24));
    assert_eq!(*tile_height_px, Some(24));
    assert_eq!(stops[0].offset_percent(), 0);
    assert_eq!(stops[0].color(), "#22d3ee33");
    assert_eq!(stops[3].offset_percent(), 100);
    assert_eq!(stops[3].color(), "#22d3ee00");
}

#[test]
fn semantic_rule_patch_values_are_readable() {
    let catalog = source_catalog();
    let input = catalog
        .fixture("fixture-semantic-style-capabilities")
        .and_then(|fixture| fixture.theme_input())
        .expect("semantic style input");

    let typography = input.typography().expect("semantic typography");
    assert!(typography.font_stack().is_none());
    assert_eq!(typography.letter_spacing_milli_em(), Some(40));
    assert_eq!(
        typography.text_transform(),
        Some(ReferenceTextTransform::Uppercase)
    );

    let rules = input.semantic_rules();
    assert_eq!(rules.len(), 2);
    let ReferenceSemanticRule::HasDescendant {
        family,
        target,
        descendant,
        apply,
    } = &rules[0]
    else {
        panic!("first rule should be a descendant selector");
    };
    assert_eq!(*family, ReferenceDiagramFamily::StateDiagram);
    assert_eq!(*target, ReferenceSemanticTarget::Node);
    assert_eq!(*descendant, ReferenceSemanticTarget::Label);
    let shadow = apply.shadow().expect("semantic shadow");
    assert_eq!(shadow.offset_x_px(), 0);
    assert_eq!(shadow.offset_y_px(), 4);
    assert_eq!(shadow.blur_px(), 10);
    assert_eq!(shadow.spread_px(), 0);
    assert_eq!(shadow.color(), "#0f172a33");

    let ReferenceSemanticRule::NotClass {
        family,
        target,
        class_name,
        apply,
    } = &rules[1]
    else {
        panic!("second rule should be a negated class selector");
    };
    assert_eq!(*family, ReferenceDiagramFamily::StateDiagram);
    assert_eq!(*target, ReferenceSemanticTarget::Node);
    assert_eq!(class_name, "disabled");
    assert_eq!(apply.fill(), Some("#e0f2fe"));
    let border = apply.border().expect("semantic border");
    assert_eq!(border.color(), "#0284c7");
    assert_eq!(border.width_px(), 1);
    assert_eq!(apply.corner_radius_px(), Some(6));
    assert!(apply.shadow().is_none());
    assert_eq!(apply.text_color(), Some("#0c4a6e"));
    assert_eq!(apply.font_weight(), Some(600));
}
