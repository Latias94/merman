use std::collections::BTreeSet;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget, ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{PacketThemeOutcome, PacketThemeSelection, packet_config_path_owns};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PacketTextRole {
    ByteStart,
    ByteEnd,
    Label,
    Title,
}

impl PacketTextRole {
    const BYTE_LABELS: [Self; 2] = [Self::ByteStart, Self::ByteEnd];
    const FIELD_LABEL: [Self; 1] = [Self::Label];
    const TITLE: [Self; 1] = [Self::Title];

    pub(super) const fn target(self) -> ThemeTarget {
        match self {
            Self::ByteStart | Self::ByteEnd => ThemeTarget::PacketByteLabel,
            Self::Label => ThemeTarget::PacketFieldLabel,
            Self::Title => ThemeTarget::Title,
        }
    }

    pub(crate) const fn class_attribute(self) -> &'static str {
        match self {
            Self::ByteStart => "packetByte start",
            Self::ByteEnd => "packetByte end",
            Self::Label => "packetLabel",
            Self::Title => "packetTitle",
        }
    }

    pub(crate) const fn css_selector(self) -> &'static str {
        match self {
            Self::ByteStart => ".packetByte.start",
            Self::ByteEnd => ".packetByte.end",
            Self::Label => ".packetLabel",
            Self::Title => ".packetTitle",
        }
    }

    fn for_target(target: ThemeTarget) -> &'static [Self] {
        match target {
            ThemeTarget::PacketByteLabel => &Self::BYTE_LABELS,
            ThemeTarget::Text | ThemeTarget::PacketFieldLabel => &Self::FIELD_LABEL,
            ThemeTarget::Title => &Self::TITLE,
            _ => &[],
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct PacketTextColorOwnership {
    byte_start: bool,
    byte_end: bool,
    label: bool,
    title: bool,
}

impl PacketTextColorOwnership {
    pub(super) fn from_config(config: &MermaidConfig) -> Self {
        Self {
            byte_start: packet_config_path_owns(config, "packet.startByteColor"),
            byte_end: packet_config_path_owns(config, "packet.endByteColor"),
            label: packet_config_path_owns(config, "packet.labelColor"),
            title: packet_config_path_owns(config, "packet.titleColor"),
        }
    }

    pub(super) const fn owns(self, role: PacketTextRole) -> bool {
        match role {
            PacketTextRole::ByteStart => self.byte_start,
            PacketTextRole::ByteEnd => self.byte_end,
            PacketTextRole::Label => self.label,
            PacketTextRole::Title => self.title,
        }
    }
}

#[derive(Debug)]
struct PacketTargetWinners {
    static_winners: Box<[(usize, ResolvedStyleProperty)]>,
    has_ordinal_rules: bool,
}

#[derive(Debug)]
pub(super) struct PacketTextWinners {
    theme: ResolvedDiagramTheme,
    targets: [PacketTargetWinners; 3],
}

impl PacketTextWinners {
    pub(super) fn new(
        theme: &ResolvedDiagramTheme,
        byte_label_style: &crate::diagram_theme::ResolvedThemeStyle,
        field_label_style: &crate::diagram_theme::ResolvedThemeStyle,
        title_style: &crate::diagram_theme::ResolvedThemeStyle,
    ) -> Self {
        let mut has_ordinal_rules = [false; 3];
        for (_, rule) in theme.family_rules() {
            if rule.ordinal().is_some()
                && matches!(rule.variant(), None | Some(ThemeVariant::Default))
                && let Some(index) = target_index(rule.target())
            {
                has_ordinal_rules[index] = true;
            }
        }
        let styles = [byte_label_style, field_label_style, title_style];
        Self {
            theme: theme.clone(),
            targets: std::array::from_fn(|index| PacketTargetWinners {
                static_winners: styles[index]
                    .winner_rule_properties()
                    .map(|(property, origin)| (origin.rule_index(), property))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                has_ordinal_rules: has_ordinal_rules[index],
            }),
        }
    }
}

const fn target_index(target: ThemeTarget) -> Option<usize> {
    match target {
        ThemeTarget::PacketByteLabel => Some(0),
        ThemeTarget::Text | ThemeTarget::PacketFieldLabel => Some(1),
        ThemeTarget::Title => Some(2),
        _ => None,
    }
}

/// Actual visible text decisions accumulated by the requested-theme SVG writer.
/// Ordinal rules resolve immediately in writer order; no terminal inventory is retained.
pub(crate) struct PacketTextOutput<'a> {
    selection: &'a PacketThemeSelection,
    occurrences: [usize; 3],
    visible_roles: u8,
    winners: BTreeSet<(usize, ResolvedStyleProperty)>,
}

impl<'a> PacketTextOutput<'a> {
    pub(super) fn new(selection: &'a PacketThemeSelection) -> Self {
        Self {
            selection,
            occurrences: [0; 3],
            visible_roles: 0,
            winners: BTreeSet::new(),
        }
    }

    pub(crate) fn record_non_empty_text(
        &mut self,
        role: PacketTextRole,
        text: &str,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !text.trim().is_empty() {
            self.record_text(role, work_meter)?;
        }
        Ok(())
    }

    pub(crate) fn record_text(
        &mut self,
        role: PacketTextRole,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let target = role.target();
        let index = target_index(target).expect("Packet text role has a text target");
        self.occurrences[index] = self.occurrences[index].saturating_add(1);
        let first_role = self.visible_roles & (1 << role as u8) == 0;
        self.visible_roles |= 1 << role as u8;
        let fill_owned = self.selection.text_color_ownership.owns(role);
        let plan = &self.selection.text_winners.targets[index];
        if plan.has_ordinal_rules {
            let theme = &self.selection.text_winners.theme;
            let style = if target == ThemeTarget::PacketFieldLabel {
                theme.text_style_with_work_meter(
                    target,
                    ThemeVariant::Default,
                    Some(self.occurrences[index]),
                    work_meter,
                )?
            } else {
                theme.style_with_work_meter(
                    target,
                    ThemeVariant::Default,
                    Some(self.occurrences[index]),
                    work_meter,
                )?
            };
            self.winners.extend(
                style
                    .winner_rule_properties()
                    .filter_map(|(property, origin)| {
                        (property != ResolvedStyleProperty::Fill || !fill_owned)
                            .then_some((origin.rule_index(), property))
                    }),
            );
        } else if first_role {
            self.winners.extend(
                plan.static_winners.iter().copied().filter(|(_, property)| {
                    *property != ResolvedStyleProperty::Fill || !fill_owned
                }),
            );
        }
        Ok(())
    }

    pub(super) fn occurrence_count(&self, target: ThemeTarget) -> usize {
        target_index(target).map_or(0, |index| self.occurrences[index])
    }

    pub(super) fn has_visible_text(&self) -> bool {
        self.visible_roles != 0
    }

    pub(super) fn has_typed_fill(&self, target: ThemeTarget) -> bool {
        PacketTextRole::for_target(target).iter().any(|role| {
            self.visible_roles & (1 << *role as u8) != 0
                && !self.selection.text_color_ownership.owns(*role)
        })
    }

    pub(super) fn route_won(&self, rule_index: usize, property: ResolvedStyleProperty) -> bool {
        self.winners.contains(&(rule_index, property))
    }

    pub(crate) fn finish(self) -> PacketThemeOutcome {
        self.selection.finish(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };

    #[test]
    fn packet_terminal_winners_preserve_actual_roles_and_ordinal_writer_order() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::PacketFieldLabel,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Packet field fill"),
                            ),
                        ))
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::PacketByteLabel,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#abcdef")
                                        .expect("valid Packet ordinal byte fill"),
                                ),
                            )
                            .with_ordinal(OrdinalSelector::exact(2).expect("valid byte ordinal")),
                        ),
                ),
            )
            .expect("compile mixed Packet terminal theme")
            .resolve(DiagramFamilyId::PACKET);
        let complete_roles = &[
            PacketTextRole::ByteEnd,
            PacketTextRole::Label,
            PacketTextRole::ByteStart,
        ][..];
        for (roles, config, static_won, ordinal_won) in [
            (complete_roles, serde_json::json!({}), true, true),
            (
                complete_roles,
                serde_json::json!({"packet": {"startByteColor": "#654321"}}),
                true,
                false,
            ),
            (
                complete_roles,
                serde_json::json!({"packet": {"endByteColor": "#654321"}}),
                true,
                true,
            ),
            (&complete_roles[..2], serde_json::json!({}), true, false),
            (&complete_roles[..0], serde_json::json!({}), false, false),
        ] {
            let work_meter = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let metadata = merman_core::Engine::new()
                .with_site_config(MermaidConfig::from_value(config))
                .parse_metadata_sync("packet\n0-7: Header")
                .expect("parse Packet terminal ownership");
            let plan = super::super::PacketTypographyThemePlan::resolve(
                Some(&theme),
                &metadata.effective_config,
                &work_meter,
            )
            .expect("resolve Packet text output");
            let mut output = plan
                .begin_text_output()
                .expect("requested Packet text theme");
            for role in roles {
                output
                    .record_text(*role, &work_meter)
                    .expect("resolve actual text winner");
            }
            assert_eq!(output.route_won(0, ResolvedStyleProperty::Fill), static_won);
            assert_eq!(
                output.route_won(1, ResolvedStyleProperty::Fill),
                ordinal_won
            );
        }
    }
}
