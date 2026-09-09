use std::collections::{BTreeMap, BTreeSet};

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget, ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::packet_config_path_owns;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PacketTextRole {
    ByteStart,
    ByteEnd,
    Label,
    Title,
}

impl PacketTextRole {
    const ALL: [Self; 4] = [Self::ByteStart, Self::ByteEnd, Self::Label, Self::Title];
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

    const fn for_target(target: ThemeTarget) -> &'static [Self] {
        match target {
            ThemeTarget::PacketByteLabel => &Self::BYTE_LABELS,
            ThemeTarget::Text | ThemeTarget::PacketFieldLabel => &Self::FIELD_LABEL,
            ThemeTarget::Title => &Self::TITLE,
            _ => &[],
        }
    }
}

#[derive(Debug)]
struct PacketCssEmissionReceipt {
    font_family_css: Box<str>,
    role_fills: BTreeMap<PacketTextRole, Box<str>>,
    schema_valid: bool,
}

impl PacketCssEmissionReceipt {
    fn from_successful_write(
        font_family_css: &str,
        role_fills: [(PacketTextRole, &str); 4],
    ) -> Self {
        let mut receipt = Self {
            font_family_css: font_family_css.into(),
            role_fills: BTreeMap::new(),
            schema_valid: css_value_is_valid(font_family_css),
        };
        for (role, fill) in role_fills {
            receipt.schema_valid &= css_value_is_valid(fill);
            receipt.schema_valid &= receipt.role_fills.insert(role, fill.into()).is_none();
        }
        receipt.schema_valid &= receipt.role_fills.len() == PacketTextRole::ALL.len();
        receipt
    }

    fn verifies_font_family(&self, expected_font_family_css: &str) -> bool {
        self.schema_valid && self.font_family_css.as_ref() == expected_font_family_css
    }

    fn fill_for(&self, role: PacketTextRole) -> Option<&str> {
        if !self.schema_valid {
            return None;
        }
        self.role_fills.get(&role).map(|fill| fill.as_ref())
    }
}

fn css_value_is_valid(value: &str) -> bool {
    !value.trim().is_empty()
        && !value
            .chars()
            .any(|character| matches!(character, ';' | '{' | '}'))
}

#[derive(Debug)]
struct PacketRoleReceipt {
    expected_count: usize,
    dom_valid: bool,
    occurrence_count: usize,
}

impl Default for PacketRoleReceipt {
    fn default() -> Self {
        Self {
            expected_count: 0,
            dom_valid: true,
            occurrence_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PacketFillProof {
    NotApplicable,
    Verified,
    Unverified,
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

/// Writer-owned receipt for the independently styled Packet text surfaces in terminal SVG.
#[derive(Debug)]
pub(crate) struct PacketSurfaceReceipt {
    expected_font_family_css: Box<str>,
    visible_text_count: usize,
    byte_label_count: usize,
    field_label_count: usize,
    title_count: usize,
    text_roles: BTreeMap<PacketTextRole, PacketRoleReceipt>,
    text_occurrences: Vec<PacketTextRole>,
    role_schema_valid: bool,
    css_emission: Option<PacketCssEmissionReceipt>,
    css_emission_unique: bool,
}

impl PacketSurfaceReceipt {
    pub(super) fn new(
        expected_counts: [(PacketTextRole, usize); 4],
        expected_font_family_css: &str,
    ) -> Self {
        let mut receipt = Self {
            expected_font_family_css: expected_font_family_css.into(),
            visible_text_count: 0,
            byte_label_count: 0,
            field_label_count: 0,
            title_count: 0,
            text_roles: BTreeMap::new(),
            text_occurrences: Vec::new(),
            role_schema_valid: true,
            css_emission: None,
            css_emission_unique: true,
        };
        for (role, expected_count) in expected_counts {
            let previous = receipt.text_roles.insert(
                role,
                PacketRoleReceipt {
                    expected_count,
                    ..PacketRoleReceipt::default()
                },
            );
            receipt.role_schema_valid &= previous.is_none();
            receipt.visible_text_count = receipt.visible_text_count.saturating_add(expected_count);
            match role.target() {
                ThemeTarget::PacketByteLabel => {
                    receipt.byte_label_count =
                        receipt.byte_label_count.saturating_add(expected_count);
                }
                ThemeTarget::PacketFieldLabel => {
                    receipt.field_label_count =
                        receipt.field_label_count.saturating_add(expected_count);
                }
                ThemeTarget::Title => {
                    receipt.title_count = receipt.title_count.saturating_add(expected_count);
                }
                _ => unreachable!("Packet text role returned a foreign semantic target"),
            }
        }
        receipt
    }

    pub(crate) fn record_successful_css_emission(
        &mut self,
        font_family_css: &str,
        role_fills: [(PacketTextRole, &str); 4],
    ) {
        if self.css_emission.is_some() {
            self.css_emission_unique = false;
            return;
        }
        self.css_emission = Some(PacketCssEmissionReceipt::from_successful_write(
            font_family_css,
            role_fills,
        ));
    }

    pub(crate) fn record_non_empty_text(
        &mut self,
        role: PacketTextRole,
        emitted_class: &str,
        text: &str,
    ) {
        if !text.trim().is_empty() {
            self.record_text_occurrence(role, emitted_class);
        }
    }

    pub(crate) fn record_text_occurrence(&mut self, role: PacketTextRole, emitted_class: &str) {
        let role_receipt = self.text_roles.entry(role).or_default();
        role_receipt.dom_valid &= emitted_class == role.class_attribute();
        role_receipt.occurrence_count = role_receipt.occurrence_count.saturating_add(1);
        self.text_occurrences.push(role);
    }

    pub(super) fn occurrence_count(&self, target: ThemeTarget) -> usize {
        match target {
            ThemeTarget::PacketByteLabel => self.byte_label_count,
            ThemeTarget::Text | ThemeTarget::PacketFieldLabel => self.field_label_count,
            ThemeTarget::Title => self.title_count,
            _ => 0,
        }
    }

    pub(super) fn has_visible_text(&self) -> bool {
        self.visible_text_count != 0
    }

    pub(super) fn terminal_dom_verified(&self) -> bool {
        self.role_schema_valid
            && self
                .text_roles
                .values()
                .all(|role| role.dom_valid && role.occurrence_count == role.expected_count)
    }

    pub(crate) fn typography_stylesheet_verified(&self) -> bool {
        self.css_emission_unique
            && self.css_emission.as_ref().is_some_and(|emission| {
                emission.verifies_font_family(&self.expected_font_family_css)
            })
    }

    pub(super) fn prove_typed_fill(
        &self,
        target: ThemeTarget,
        expected_fill: &str,
        ownership: PacketTextColorOwnership,
    ) -> PacketFillProof {
        let Some(emission) = self
            .css_emission
            .as_ref()
            .filter(|_| self.css_emission_unique)
        else {
            return PacketFillProof::Unverified;
        };
        let mut applicable = false;
        for role in PacketTextRole::for_target(target) {
            let Some(role_receipt) = self.text_roles.get(role) else {
                continue;
            };
            if role_receipt.expected_count == 0 || ownership.owns(*role) {
                continue;
            }
            applicable = true;
            if !role_receipt.dom_valid
                || role_receipt.occurrence_count != role_receipt.expected_count
                || emission.fill_for(*role) != Some(expected_fill)
            {
                return PacketFillProof::Unverified;
            }
        }
        if applicable {
            PacketFillProof::Verified
        } else {
            PacketFillProof::NotApplicable
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct PacketTerminalWinnerLedger {
    winners: BTreeSet<(usize, ResolvedStyleProperty)>,
}

impl PacketTerminalWinnerLedger {
    fn record_style(
        &mut self,
        style: &crate::diagram_theme::ResolvedThemeStyle,
        fill_owned_by_config: bool,
    ) {
        self.record_winners(
            style
                .winner_rule_properties()
                .map(|(property, origin)| (origin.rule_index(), property)),
            fill_owned_by_config,
        );
    }

    fn record_winners(
        &mut self,
        winners: impl IntoIterator<Item = (usize, ResolvedStyleProperty)>,
        fill_owned_by_config: bool,
    ) {
        self.winners
            .extend(winners.into_iter().filter(|(_, property)| {
                *property != ResolvedStyleProperty::Fill || !fill_owned_by_config
            }));
    }

    pub(super) fn route_won(&self, rule_index: usize, property: ResolvedStyleProperty) -> bool {
        self.winners.contains(&(rule_index, property))
    }
}

#[derive(Debug)]
struct PacketTargetWinnerPlan {
    static_winners: Box<[(usize, ResolvedStyleProperty)]>,
    has_ordinal_rules: bool,
}

#[derive(Debug)]
pub(super) struct PacketTerminalWinnerResolver {
    theme: ResolvedDiagramTheme,
    targets: BTreeMap<ThemeTarget, PacketTargetWinnerPlan>,
}

impl PacketTerminalWinnerResolver {
    pub(super) fn new(
        theme: &ResolvedDiagramTheme,
        byte_label_style: &crate::diagram_theme::ResolvedThemeStyle,
        field_label_style: &crate::diagram_theme::ResolvedThemeStyle,
        title_style: &crate::diagram_theme::ResolvedThemeStyle,
    ) -> Self {
        let ordinal_rule_targets = theme
            .family_rules()
            .filter_map(|(_, rule)| {
                (rule.ordinal().is_some()
                    && matches!(rule.variant(), None | Some(ThemeVariant::Default)))
                .then_some(rule.target())
            })
            .collect::<BTreeSet<_>>();
        let mut targets = BTreeMap::new();
        targets.insert(
            ThemeTarget::PacketByteLabel,
            PacketTargetWinnerPlan {
                static_winners: packet_style_winners(byte_label_style),
                has_ordinal_rules: ordinal_rule_targets.contains(&ThemeTarget::PacketByteLabel),
            },
        );
        targets.insert(
            ThemeTarget::PacketFieldLabel,
            PacketTargetWinnerPlan {
                static_winners: packet_style_winners(field_label_style),
                has_ordinal_rules: ordinal_rule_targets.contains(&ThemeTarget::Text)
                    || ordinal_rule_targets.contains(&ThemeTarget::PacketFieldLabel),
            },
        );
        targets.insert(
            ThemeTarget::Title,
            PacketTargetWinnerPlan {
                static_winners: packet_style_winners(title_style),
                has_ordinal_rules: ordinal_rule_targets.contains(&ThemeTarget::Title),
            },
        );
        Self {
            theme: theme.clone(),
            targets,
        }
    }

    pub(super) fn resolve(
        &self,
        receipt: &PacketSurfaceReceipt,
        ownership: PacketTextColorOwnership,
        work_meter: &OperationWorkMeter,
    ) -> Result<PacketTerminalWinnerLedger, OperationWorkError> {
        // The writer supplies the actual role order so ordinal resolution uses the same
        // one-based occurrence domain as the terminal SVG. Static winners are reused; only
        // targets with ordinal candidates perform metered per-occurrence resolution.
        let mut ledger = PacketTerminalWinnerLedger::default();
        let mut target_ordinals = BTreeMap::<ThemeTarget, usize>::new();
        for role in receipt.text_occurrences.iter().copied() {
            let target = role.target();
            let ordinal = target_ordinals.entry(target).or_default();
            *ordinal = ordinal.saturating_add(1);
            let Some(plan) = self.targets.get(&target) else {
                continue;
            };
            let fill_owned_by_config = ownership.owns(role);
            if !plan.has_ordinal_rules {
                ledger.record_winners(plan.static_winners.iter().copied(), fill_owned_by_config);
                continue;
            }

            let style = if target == ThemeTarget::PacketFieldLabel {
                self.theme.text_style_with_work_meter(
                    target,
                    ThemeVariant::Default,
                    Some(*ordinal),
                    work_meter,
                )?
            } else {
                self.theme.style_with_work_meter(
                    target,
                    ThemeVariant::Default,
                    Some(*ordinal),
                    work_meter,
                )?
            };
            ledger.record_style(&style, fill_owned_by_config);
        }
        Ok(ledger)
    }
}

fn packet_style_winners(
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Box<[(usize, ResolvedStyleProperty)]> {
    style
        .winner_rule_properties()
        .map(|(property, origin)| (origin.rule_index(), property))
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[derive(Debug)]
pub(super) struct PacketTerminalEvidence {
    pub(super) receipt: PacketSurfaceReceipt,
    pub(super) winners: PacketTerminalWinnerLedger,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt_for_byte_labels() -> PacketSurfaceReceipt {
        PacketSurfaceReceipt::new(
            [
                (PacketTextRole::ByteStart, 1),
                (PacketTextRole::ByteEnd, 1),
                (PacketTextRole::Label, 0),
                (PacketTextRole::Title, 0),
            ],
            "monospace",
        )
    }

    fn complete_css_emission(
        receipt: &mut PacketSurfaceReceipt,
        role_fills: [(PacketTextRole, &str); 4],
    ) {
        receipt.record_successful_css_emission("monospace", role_fills);
    }

    #[test]
    fn packet_css_emission_requires_the_expected_font_and_unique_role_schema() {
        let mut wrong_font = receipt_for_byte_labels();
        wrong_font.record_successful_css_emission(
            "serif",
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        assert!(!wrong_font.typography_stylesheet_verified());

        let mut duplicate_role = receipt_for_byte_labels();
        duplicate_role.record_successful_css_emission(
            "monospace",
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        assert!(!duplicate_role.typography_stylesheet_verified());
    }

    #[test]
    fn packet_css_evidence_ignores_semantically_equivalent_role_rule_order() {
        let mut canonical = receipt_for_byte_labels();
        complete_css_emission(
            &mut canonical,
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        let mut reordered = receipt_for_byte_labels();
        complete_css_emission(
            &mut reordered,
            [
                (PacketTextRole::Title, "black"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::ByteStart, "#e5e7eb"),
            ],
        );
        for receipt in [&mut canonical, &mut reordered] {
            for role in PacketTextRole::BYTE_LABELS {
                receipt.record_text_occurrence(role, role.class_attribute());
            }
        }

        assert!(canonical.typography_stylesheet_verified());
        assert!(reordered.typography_stylesheet_verified());
        let canonical_proof = canonical.prove_typed_fill(
            ThemeTarget::PacketByteLabel,
            "#e5e7eb",
            PacketTextColorOwnership::default(),
        );
        assert_eq!(canonical_proof, PacketFillProof::Verified);
        assert_eq!(
            canonical_proof,
            reordered.prove_typed_fill(
                ThemeTarget::PacketByteLabel,
                "#e5e7eb",
                PacketTextColorOwnership::default(),
            )
        );
    }

    #[test]
    fn packet_byte_label_fill_receipt_requires_every_visible_unowned_byte_role() {
        let ownership = PacketTextColorOwnership::default();
        let mut complete = receipt_for_byte_labels();
        complete_css_emission(
            &mut complete,
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        for role in PacketTextRole::BYTE_LABELS {
            complete.record_text_occurrence(role, role.class_attribute());
        }
        assert_eq!(
            complete.prove_typed_fill(ThemeTarget::PacketByteLabel, "#e5e7eb", ownership),
            PacketFillProof::Verified
        );

        let mut mismatch = receipt_for_byte_labels();
        complete_css_emission(
            &mut mismatch,
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "black"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        for role in PacketTextRole::BYTE_LABELS {
            mismatch.record_text_occurrence(role, role.class_attribute());
        }
        assert_eq!(
            mismatch.prove_typed_fill(ThemeTarget::PacketByteLabel, "#e5e7eb", ownership),
            PacketFillProof::Unverified
        );

        let mut missing_end = receipt_for_byte_labels();
        complete_css_emission(
            &mut missing_end,
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        missing_end.record_text_occurrence(
            PacketTextRole::ByteStart,
            PacketTextRole::ByteStart.class_attribute(),
        );
        assert_eq!(
            missing_end.prove_typed_fill(ThemeTarget::PacketByteLabel, "#e5e7eb", ownership),
            PacketFillProof::Unverified
        );

        let mut wrong_dom_role = PacketSurfaceReceipt::new(
            [
                (PacketTextRole::ByteStart, 1),
                (PacketTextRole::ByteEnd, 0),
                (PacketTextRole::Label, 0),
                (PacketTextRole::Title, 0),
            ],
            "monospace",
        );
        complete_css_emission(
            &mut wrong_dom_role,
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        wrong_dom_role.record_text_occurrence(PacketTextRole::ByteStart, "packetLabel");
        assert_eq!(
            wrong_dom_role.prove_typed_fill(ThemeTarget::PacketByteLabel, "#e5e7eb", ownership,),
            PacketFillProof::Unverified
        );
    }

    #[test]
    fn packet_title_fill_receipt_respects_explicit_config_ownership() {
        let mut receipt = PacketSurfaceReceipt::new(
            [
                (PacketTextRole::ByteStart, 0),
                (PacketTextRole::ByteEnd, 0),
                (PacketTextRole::Label, 0),
                (PacketTextRole::Title, 1),
            ],
            "monospace",
        );
        complete_css_emission(
            &mut receipt,
            [
                (PacketTextRole::ByteStart, "black"),
                (PacketTextRole::ByteEnd, "black"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "#333333"),
            ],
        );
        receipt.record_text_occurrence(
            PacketTextRole::Title,
            PacketTextRole::Title.class_attribute(),
        );

        assert_eq!(
            receipt.prove_typed_fill(
                ThemeTarget::Title,
                "#fef3c7",
                PacketTextColorOwnership {
                    title: true,
                    ..PacketTextColorOwnership::default()
                },
            ),
            PacketFillProof::NotApplicable
        );
    }
}
