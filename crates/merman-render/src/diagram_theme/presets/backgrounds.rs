//! Excluded calendar periods have an independent surface, separate from tasks and canvas.

use merman_theme_contract::ThemeDesignV1;

use super::catalog::PresetPalette;
use super::design::{color_token, rule, style};
use crate::DiagramFamilyId;
use crate::diagram_theme::ThemeTarget;

pub(super) fn apply_gantt_backgrounds(design: &mut ThemeDesignV1, palette: PresetPalette) {
    color_token(
        design,
        "gantt-excluded-period",
        "Excluded period",
        "Gantt",
        "Calendar exclusion surface beneath section overlays, independent of task and canvas fills.",
        palette.gantt_excluded_period,
    );
    design.rules.push(rule(
        Some(DiagramFamilyId::GANTT),
        ThemeTarget::ExcludedPeriod,
        None,
        style(Some("gantt-excluded-period"), None),
    ));
}
