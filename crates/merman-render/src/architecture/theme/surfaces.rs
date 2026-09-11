use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeTarget};
use crate::family::FamilyThemeEvidence;
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod receipt;

pub(crate) use receipt::{
    ArchitectureArrowSide, ArchitectureEdgeTerminal, ArchitectureEdgeTerminalEmission,
    ArchitectureGroupTerminal, ArchitectureGroupTerminalEmission,
    ArchitecturePaintTerminalEmission, ArchitectureServiceTerminal,
    ArchitectureServiceTerminalEmission, ArchitectureSurfaceThemeReceipt,
    ArchitectureTextTerminalEmission,
};
use receipt::{
    ArchitectureSurfaceOwnership, is_architecture_terminal_surface_target, mechanism_target,
};

const TEXT_COLOR_PATH: &str = "themeVariables.textColor";
const ARCH_EDGE_ARROW_COLOR_PATH: &str = "themeVariables.archEdgeArrowColor";
const ARCH_EDGE_COLOR_PATH: &str = "themeVariables.archEdgeColor";
const LINE_COLOR_PATH: &str = "themeVariables.lineColor";

/// Family-local plan for real Architecture service, SVG-text, and arrow terminals.
#[derive(Debug)]
pub(super) struct ArchitectureSurfaceThemePlan {
    theme: Option<ResolvedDiagramTheme>,
    title_keys: Box<[FamilyThemeMechanismKey]>,
    service_stroke_source_owned: bool,
    text_fill_source_owned: bool,
    arrow_fill_source_owned: bool,
    terminal_receipt: OnceLock<ArchitectureSurfaceThemeReceipt>,
}

impl ArchitectureSurfaceThemePlan {
    pub(super) fn baseline() -> Self {
        Self {
            theme: None,
            title_keys: Box::new([]),
            service_stroke_source_owned: false,
            text_fill_source_owned: false,
            arrow_fill_source_owned: false,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let title_keys = theme
            .map(|theme| {
                theme
                    .family_mechanism_routes()
                    .iter()
                    .copied()
                    .filter(|route| mechanism_target(route.mechanism()) == Some(ThemeTarget::Title))
                    .map(|route| theme.family_mechanism_key(route))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_boxed_slice();
        Self {
            theme: theme.cloned(),
            title_keys,
            service_stroke_source_owned: super::mermaid_owns_group_stroke(effective_config),
            text_fill_source_owned: merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                TEXT_COLOR_PATH,
            ),
            arrow_fill_source_owned: mermaid_owns_arrow_fill(effective_config),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(super) fn begin_terminal_receipt<'a>(
        &self,
        services: impl IntoIterator<Item = ArchitectureServiceTerminal<'a>>,
        groups: impl IntoIterator<Item = ArchitectureGroupTerminal<'a>>,
        edges: impl IntoIterator<Item = ArchitectureEdgeTerminal<'a>>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Option<ArchitectureSurfaceThemeReceipt>, OperationWorkError> {
        let Some(theme) = self.theme.as_ref() else {
            return Ok(None);
        };
        if !theme.family_mechanism_routes().iter().any(|route| {
            mechanism_target(route.mechanism()).is_some_and(is_architecture_terminal_surface_target)
        }) {
            return Ok(None);
        }

        ArchitectureSurfaceThemeReceipt::build(
            theme,
            ArchitectureSurfaceOwnership {
                service_stroke: self.service_stroke_source_owned,
                text_fill: self.text_fill_source_owned,
                arrow_fill: self.arrow_fill_source_owned,
            },
            services,
            groups,
            edges,
            work_meter,
        )
        .map(Some)
    }

    pub(super) fn record_terminal(&self, receipt: ArchitectureSurfaceThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(super) fn finish_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        for key in &self.title_keys {
            evidence.mark_not_applicable(key.clone());
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            return;
        };
        receipt.apply_to_evidence(evidence);
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(super) fn architecture_text_cutover_receipt(
        &self,
    ) -> Option<crate::__private::ArchitectureTextCutoverReceipt> {
        self.terminal_receipt
            .get()?
            .architecture_text_cutover_receipt()
    }
}

fn mermaid_owns_arrow_fill(config: &MermaidConfig) -> bool {
    if merman_core::__private::config_path_overrides_typed_default(
        config,
        ARCH_EDGE_ARROW_COLOR_PATH,
    ) {
        return true;
    }
    if merman_core::__private::config_path_overrides_typed_default(config, ARCH_EDGE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_ARROW_COLOR_PATH) == config.get_str(ARCH_EDGE_COLOR_PATH)
    {
        return true;
    }
    merman_core::__private::config_path_overrides_typed_default(config, LINE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_COLOR_PATH) == config.get_str(LINE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_ARROW_COLOR_PATH) == config.get_str(ARCH_EDGE_COLOR_PATH)
}
