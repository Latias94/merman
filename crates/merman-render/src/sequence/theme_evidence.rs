mod observation;
mod receipts;
mod recorder;

pub(crate) use receipts::{
    SequenceActorThemeReceipt, SequenceLifelineThemeEmission, SequenceLifelineThemeReceipt,
    SequenceLoopThemeEmission, SequenceLoopThemeReceipt, SequenceMessageThemeEmission,
    SequenceMessageThemeReceipt, SequenceNumberLabelThemeEmission, SequenceNumberLabelThemeReceipt,
    SequenceStaticRectThemeEmission, SequenceStaticRectThemeReceipt,
    SequenceTypographyThemeReceipt,
};
pub(crate) use recorder::SequenceThemeEvidenceRecorder;

#[cfg(test)]
use crate::diagram_theme::{ResolvedStyleProperty, ThemeCapability, ThemeTarget, ThemeVariant};
#[cfg(test)]
use crate::family::FamilyThemeResidualReason;
#[cfg(test)]
use receipts::{SequenceRoleTypographyReceipt, record_style_winners};
#[cfg(test)]
use std::collections::BTreeSet;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, Specified, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };

    #[test]
    fn actor_rule_is_not_applicable_when_terminal_model_has_no_actors() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        let evidence = SequenceThemeEvidenceRecorder::default().finish(Some(&resolved));

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            [crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn note_winner_without_complete_rect_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Note,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Note theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (note_count, emitted_rects) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceStaticRectThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Note,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..emitted_rects {
                receipt.record_rect_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_note_emission(SequenceStaticRectThemeEmission::from_terminal_writer(
                note_count,
                Some("#ef4444"),
                false,
                None,
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn message_winner_without_complete_line_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Message,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Message theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (line_candidates, emitted_lines) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceMessageThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Message,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..line_candidates {
                receipt.record_line_candidate();
            }
            for _ in 0..emitted_lines {
                receipt.record_line_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_message_emission(SequenceMessageThemeEmission::from_terminal_writer(
                Some("#2563eb"),
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn sequence_role_fill_requires_both_stylesheet_and_native_text_receipts() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::MessageLabel,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456")
                                    .expect("valid Sequence message-label fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence message-label theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);

        for (record_native_emission, expected_applied) in [(false, false), (true, true)] {
            let mut message = SequenceRoleTypographyReceipt::default();
            record_style_winners(
                &mut message.static_winners,
                &resolved.style(ThemeTarget::MessageLabel, ThemeVariant::Default, None),
            );
            message.record_stylesheet_emission(
                crate::sequence::SequenceTextSurface::MessageLabel,
                "#123456",
                Some("#123456"),
            );
            message.record_candidate(crate::sequence::SequenceTextSurface::MessageLabel);
            if record_native_emission {
                message.record_emission(crate::sequence::SequenceTextSurface::MessageLabel);
            }

            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_typography_emission(SequenceTypographyThemeReceipt {
                actor: SequenceRoleTypographyReceipt::default(),
                message,
                note: SequenceRoleTypographyReceipt::default(),
                loop_label: SequenceRoleTypographyReceipt::default(),
            });

            let evidence = recorder.finish(Some(&resolved));
            assert_eq!(evidence.applied().len(), usize::from(expected_applied));
            assert_eq!(
                evidence.applied_capabilities(),
                if expected_applied {
                    BTreeSet::from([ThemeCapability::SolidPaint])
                } else {
                    BTreeSet::new()
                }
            );
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn sequence_role_unsupported_ordinal_is_applicable_only_inside_the_terminal_domain() {
        for (ordinal, expected_not_applicable, expected_residual) in [(1, 0, 1), (2, 1, 0)] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::MessageLabel,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#123456")
                                        .expect("valid Sequence ordinal message-label fill"),
                                ),
                            )
                            .with_ordinal(
                                OrdinalSelector::exact(ordinal)
                                    .expect("valid Sequence message-label ordinal"),
                            )
                            .for_family(DiagramFamilyId::SEQUENCE),
                        ),
                    ),
                )
                .expect("compile Sequence ordinal message-label theme");
            let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
            let mut message = SequenceRoleTypographyReceipt::default();
            message.seed_complete_surface(
                crate::sequence::SequenceTextSurface::MessageLabel,
                "#123456",
                None,
            );
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_typography_emission(SequenceTypographyThemeReceipt {
                actor: SequenceRoleTypographyReceipt::default(),
                message,
                note: SequenceRoleTypographyReceipt::default(),
                loop_label: SequenceRoleTypographyReceipt::default(),
            });

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms().len(),
                expected_not_applicable,
                "ordinal {ordinal}"
            );
            assert_eq!(
                evidence.residuals().len(),
                expected_residual,
                "ordinal {ordinal}"
            );
        }
    }

    #[test]
    fn sequence_loop_paints_require_stylesheet_and_actual_label_box_receipts() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Loop,
                            ThemeStylePatch::default()
                                .with_fill(
                                    CanvasPaint::solid("#123456")
                                        .expect("valid Sequence Loop fill"),
                                )
                                .with_stroke(CanvasPaint::Transparent),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Loop paint theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);

        for (record_surface_emission, expected_applied) in [(false, false), (true, true)] {
            let mut receipt = SequenceLoopThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Loop,
                ThemeVariant::Default,
                None,
            ));
            receipt.record_stylesheet_emission(
                "#123456",
                Some("#123456"),
                "transparent",
                Some("transparent"),
            );
            receipt.record_surface_candidate();
            if record_surface_emission {
                receipt.record_surface_emission();
            }

            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_loop_emission(SequenceLoopThemeEmission::from_terminal_writer(
                Some("#123456"),
                false,
                Some("transparent"),
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert_eq!(evidence.applied().len(), usize::from(expected_applied));
            assert_eq!(
                evidence.applied_capabilities(),
                if expected_applied {
                    BTreeSet::from([
                        ThemeCapability::SolidPaint,
                        ThemeCapability::TransparentPaint,
                    ])
                } else {
                    BTreeSet::new()
                }
            );
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn sequence_number_label_fill_requires_a_complete_terminal_color_receipt() {
        for (paint, css, capability) in [
            (
                CanvasPaint::solid("#123456").expect("valid Sequence number fill"),
                "#123456",
                ThemeCapability::SolidPaint,
            ),
            (
                CanvasPaint::Transparent,
                "transparent",
                ThemeCapability::TransparentPaint,
            ),
        ] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::SequenceNumberLabel,
                                ThemeStylePatch::default().with_fill(paint),
                            )
                            .for_family(DiagramFamilyId::SEQUENCE),
                        ),
                    ),
                )
                .expect("compile Sequence number label theme");
            let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
            let mut receipt = SequenceNumberLabelThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::SequenceNumberLabel,
                ThemeVariant::Default,
                None,
            ));
            receipt.record_stylesheet_emission(css, Some(css));
            for _ in 0..2 {
                receipt.record_text_candidate();
                receipt.record_text_emission(Some(css));
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_sequence_number_emission(
                SequenceNumberLabelThemeEmission::from_terminal_writer(Some(css), false, receipt),
            );

            let evidence = recorder.finish(Some(&resolved));
            assert_eq!(evidence.applied().len(), 1);
            assert_eq!(
                evidence.applied_capabilities(),
                BTreeSet::from([capability])
            );
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn sequence_number_label_is_not_applicable_without_numbers_or_under_source_ownership() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::SequenceNumberLabel,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Sequence number fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence number label theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);

        for (has_number, overridden, final_fill, typed_fill) in [
            (false, false, "#123456", Some("#123456")),
            (true, true, "#fedcba", None),
        ] {
            let mut receipt = SequenceNumberLabelThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::SequenceNumberLabel,
                ThemeVariant::Default,
                None,
            ));
            receipt.record_stylesheet_emission(final_fill, typed_fill);
            if has_number {
                receipt.record_text_candidate();
                receipt.record_text_emission(Some(final_fill));
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_sequence_number_emission(
                SequenceNumberLabelThemeEmission::from_terminal_writer(
                    typed_fill, overridden, receipt,
                ),
            );

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn sequence_number_label_color_mismatch_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::SequenceNumberLabel,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Sequence number fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence number label theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        let mut receipt = SequenceNumberLabelThemeReceipt::default();
        receipt.record_static_style(&resolved.style(
            ThemeTarget::SequenceNumberLabel,
            ThemeVariant::Default,
            None,
        ));
        receipt.record_stylesheet_emission("#123456", Some("#123456"));
        receipt.record_text_candidate();
        receipt.record_text_emission(Some("#654321"));
        let recorder = SequenceThemeEvidenceRecorder::default();
        recorder.record_sequence_number_emission(
            SequenceNumberLabelThemeEmission::from_terminal_writer(Some("#123456"), false, receipt),
        );

        let evidence = recorder.finish(Some(&resolved));
        assert!(evidence.applied().is_empty());
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn lifeline_winner_without_complete_actor_line_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Lifeline,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Lifeline theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (line_candidates, emitted_lines) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceLifelineThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Lifeline,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..line_candidates {
                receipt.record_line_candidate();
            }
            for _ in 0..emitted_lines {
                receipt.record_line_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_lifeline_emission(SequenceLifelineThemeEmission::from_terminal_writer(
                Some("#2563eb"),
                None,
                false,
                Some(ResolvedStyleProperty::Stroke),
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn lifeline_stroke_width_requires_complete_actor_line_receipt() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Lifeline,
                            ThemeStylePatch::default()
                                .with_stroke_width(2.0)
                                .expect("valid Lifeline stroke width"),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Lifeline width theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);

        for (line_candidates, emitted_lines, paint_overridden, expected_applied) in [
            (1, 0, false, false),
            (2, 1, false, false),
            (2, 2, true, true),
        ] {
            let mut receipt = SequenceLifelineThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Lifeline,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..line_candidates {
                receipt.record_line_candidate();
            }
            for _ in 0..emitted_lines {
                receipt.record_line_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_lifeline_emission(SequenceLifelineThemeEmission::from_terminal_writer(
                None,
                Some(2.0),
                true,
                None,
                paint_overridden,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert_eq!(evidence.applied().len(), usize::from(expected_applied));
            assert_eq!(
                evidence.applied_capabilities(),
                if expected_applied {
                    BTreeSet::from([ThemeCapability::BorderStyling])
                } else {
                    BTreeSet::new()
                }
            );
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn lifeline_mixed_typed_paint_and_unsupported_sibling_facets_remains_residual() {
        let mut style = ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Lifeline stroke"));
        style.geometry.radius = Specified::Value(6.0);
        style.spacing.padding = Specified::Value(crate::diagram_theme::InsetsPx::all(4.0));
        style.paint.opacity = Specified::Value(0.75);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(ThemeTarget::Lifeline, style)
                            .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile mixed Sequence Lifeline theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        let mut receipt = SequenceLifelineThemeReceipt::default();
        receipt.record_static_style(&resolved.style(
            ThemeTarget::Lifeline,
            ThemeVariant::Default,
            None,
        ));
        receipt.record_line_candidate();
        receipt.record_line_emission();
        let recorder = SequenceThemeEvidenceRecorder::default();
        recorder.record_lifeline_emission(SequenceLifelineThemeEmission::from_terminal_writer(
            Some("#2563eb"),
            None,
            false,
            Some(ResolvedStyleProperty::Stroke),
            false,
            receipt,
        ));

        let evidence = recorder.finish(Some(&resolved));
        assert!(evidence.applied().is_empty());
        let [residual] = evidence.residuals() else {
            panic!("mixed Lifeline rule must retain exactly one residual")
        };
        assert_eq!(
            residual.key(),
            &crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Lifeline,
            }
        );
        assert_eq!(
            residual.reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn activation_winner_without_complete_rect_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Activation,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Activation theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (activation_count, emitted_rects) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceStaticRectThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Activation,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..emitted_rects {
                receipt.record_rect_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_activation_emission(
                SequenceStaticRectThemeEmission::from_terminal_writer(
                    activation_count,
                    Some("#ef4444"),
                    false,
                    None,
                    false,
                    receipt,
                ),
            );

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }
}
