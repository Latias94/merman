//! Filled categories own their foreground separately from canvas text and chart strokes.

use merman_theme_contract::*;

use super::catalog::PresetPalette;
use super::design::{color_token, literal, reference, rule, style, token};
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

pub(super) fn append_categorical_adaptations(design: &mut ThemeDesignV1, palette: PresetPalette) {
    // These start with the authored opaque Pie pairs, but remain independently editable.
    // In particular, editing chart-line colors or Pie opacity must not recolor labeled nodes.
    for (index, color) in palette.pie_series.iter().enumerate() {
        color_token(
            design,
            &format!("category-fill.{index}"),
            &format!("Category fill {}", index + 1),
            "Filled categories",
            "Opaque categorical surface paired with category-text.",
            color,
        );
    }
    token(
        design,
        "category-fills",
        "Category fills",
        "Filled categories",
        "Filled-category colors, independent of chart strokes and Pie sectors.",
        ThemeDesignTokenValueV1::Palette(literal(
            (0..palette.pie_series.len())
                .map(|index| reference(&format!("category-fill.{index}")))
                .collect(),
        )),
    );
    color_token(
        design,
        "category-text",
        "Category text",
        "Filled categories",
        "Foreground for labels on the opaque category-fills palette.",
        palette.pie_text,
    );
    for target in [
        ThemeTarget::Node,
        ThemeTarget::JourneyTask,
        ThemeTarget::TimelineEvent,
    ] {
        design
            .palettes
            .insert(target.id().to_owned(), reference("category-fills"));
    }
    for (family, target) in [
        (DiagramFamilyId::MINDMAP, ThemeTarget::NodeLabel),
        (DiagramFamilyId::JOURNEY, ThemeTarget::TaskLabel),
        (DiagramFamilyId::TIMELINE, ThemeTarget::NodeLabel),
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::ClusterLabel),
    ] {
        design.rules.push(rule(
            Some(family),
            target,
            None,
            style(Some("category-text"), None),
        ));
    }
    // Treemap sections are translucent even when the category color is opaque. Some dark
    // appearances need a darker foreground than the one used on opaque Mindmap/Journey nodes.
    token(
        design,
        "treemap-text",
        "Treemap category text",
        "Filled categories",
        "Foreground over translucent Treemap sections; linked to category-text unless independently authored.",
        ThemeDesignTokenValueV1::Color(
            palette
                .treemap_text
                .map(|color| literal(color.to_owned()))
                .unwrap_or_else(|| reference("category-text")),
        ),
    );
    design.rules.push(rule(
        Some(DiagramFamilyId::TREEMAP),
        ThemeTarget::NodeLabel,
        None,
        style(Some("treemap-text"), None),
    ));
    // The Mindmap root has no branch ordinal. Its primary body and label are a separate pair.
    for (id, label, description, color) in [
        (
            "mindmap-root-fill",
            "Mindmap root fill",
            "Primary root surface outside the eleven-slot branch palette.",
            palette.surface_alt,
        ),
        (
            "mindmap-root-text",
            "Mindmap root text",
            "Foreground paired with the primary Mindmap root surface.",
            palette.text,
        ),
    ] {
        color_token(design, id, label, "Filled categories", description, color);
    }
    for (target, fill) in [
        (ThemeTarget::Node, "mindmap-root-fill"),
        (ThemeTarget::NodeLabel, "mindmap-root-text"),
    ] {
        design.rules.push(rule(
            Some(DiagramFamilyId::MINDMAP),
            target,
            Some(ThemeVariant::Primary),
            style(Some(fill), None),
        ));
    }
}
