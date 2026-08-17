use std::collections::HashMap;
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::pie::PieDiagramRenderModel;

use crate::config::config_string;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
const MERMAID_PIE_PALETTE_SIZE: usize = 12;
const MERMAID_PIE_SLOT_KEYS: [&str; MERMAID_PIE_PALETTE_SIZE] = [
    "pie1", "pie2", "pie3", "pie4", "pie5", "pie6", "pie7", "pie8", "pie9", "pie10", "pie11",
    "pie12",
];
const MERMAID_PIE_SLOT_PATHS: [&str; MERMAID_PIE_PALETTE_SIZE] = [
    "themeVariables.pie1",
    "themeVariables.pie2",
    "themeVariables.pie3",
    "themeVariables.pie4",
    "themeVariables.pie5",
    "themeVariables.pie6",
    "themeVariables.pie7",
    "themeVariables.pie8",
    "themeVariables.pie9",
    "themeVariables.pie10",
    "themeVariables.pie11",
    "themeVariables.pie12",
];

#[derive(Debug)]
struct PieSlicePaint {
    fill: String,
    typed_owner: bool,
}

/// Resolves Pie slice paint once for layout, SVG emission, and terminal evidence.
#[derive(Debug)]
pub(crate) struct PieSlicePaintPlan {
    label_indices: HashMap<String, usize>,
    paints: Vec<PieSlicePaint>,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_complete: OnceLock<()>,
}

impl PieSlicePaintPlan {
    pub(crate) fn baseline(
        model: &PieDiagramRenderModel,
        effective_config: &serde_json::Value,
    ) -> Self {
        let mut plan = Self {
            label_indices: HashMap::new(),
            paints: Vec::new(),
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_complete: OnceLock::new(),
        };
        for section in &model.sections {
            plan.insert_mermaid_paint(&section.label, effective_config);
        }
        plan
    }

    pub(crate) fn resolve(
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> Self {
        let mut plan = Self::baseline(model, effective_config.as_value());
        let Some(theme) = theme else {
            return plan;
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::PieSlice) else {
            return plan;
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::PieSlice,
        };
        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                plan.palette_key = Some(key.clone());
                let mut missing_typed_color = false;
                for (index, paint) in plan.paints.iter_mut().enumerate() {
                    if merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        MERMAID_PIE_SLOT_PATHS[index % MERMAID_PIE_PALETTE_SIZE],
                    ) {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::PieSlice, index + 1) else {
                        missing_typed_color = true;
                        continue;
                    };
                    paint.fill = color.as_css();
                    paint.typed_owner = true;
                }
                if missing_typed_color {
                    plan.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    plan.palette_key = None;
                }
            }
            FamilyThemeDisposition::Unsupported => {
                plan.evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
            FamilyThemeDisposition::LegacyCompatibility => {}
        }
        plan
    }

    fn insert_mermaid_paint(&mut self, label: &str, effective_config: &serde_json::Value) {
        if self.label_indices.contains_key(label) {
            return;
        }
        let index = self.paints.len();
        let slot = index % MERMAID_PIE_PALETTE_SIZE;
        let fill = config_string(
            effective_config,
            &["themeVariables", MERMAID_PIE_SLOT_KEYS[slot]],
        )
        .unwrap_or_else(|| default_pie_palette()[slot].to_string());
        self.label_indices.insert(label.to_string(), index);
        self.paints.push(PieSlicePaint {
            fill,
            typed_owner: false,
        });
    }

    pub(crate) fn fill_for(&self, label: &str) -> Option<&str> {
        let index = *self.label_indices.get(label)?;
        self.paints.get(index).map(|paint| paint.fill.as_str())
    }

    pub(crate) fn record_terminal_complete(&self) -> bool {
        self.palette_key.is_none()
            || self.terminal_complete.get().is_some()
            || self.terminal_complete.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        if !self.paints.iter().any(|paint| paint.typed_owner) {
            evidence.mark_not_applicable(key);
        } else if self.terminal_complete.get().is_some() {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::SolidPaint]);
        } else {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
        }
        evidence
    }
}

fn default_pie_palette() -> [&'static str; MERMAID_PIE_PALETTE_SIZE] {
    [
        "#ECECFF",
        "#ffffde",
        "hsl(80, 100%, 56.2745098039%)",
        "hsl(240, 100%, 86.2745098039%)",
        "hsl(60, 100%, 63.5294117647%)",
        "hsl(80, 100%, 76.2745098039%)",
        "hsl(300, 100%, 76.2745098039%)",
        "hsl(180, 100%, 56.2745098039%)",
        "hsl(0, 100%, 56.2745098039%)",
        "hsl(300, 100%, 56.2745098039%)",
        "hsl(150, 100%, 56.2745098039%)",
        "hsl(0, 100%, 66.2745098039%)",
    ]
}
