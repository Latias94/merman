use super::*;
#[cfg(any(feature = "resources-flowchart", feature = "resources-zenuml"))]
use merman_core::{MermaidEngine, ParseOptions};
#[cfg(any(
    feature = "resources-flowchart",
    feature = "resources-swimlane",
    feature = "resources-agentflow"
))]
use merman_model::families::flowchart::{
    FlowEdge, FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNode, FlowSubgraph,
};
#[cfg(feature = "resources-zenuml")]
use merman_model::presentation::RenderSemanticModel;
use merman_model::{
    OperationControl, OperationPhase, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance,
};
use std::collections::HashSet;

#[test]
fn layout_work_defaults_preserve_bounded_general_and_native_profiles() {
    for (profile, expected) in [
        (RenderResourceProfile::Constrained, Some(125_000)),
        (RenderResourceProfile::Interactive, Some(14_100_000)),
        (RenderResourceProfile::TrustedNative, Some(15_000_000)),
        (RenderResourceProfile::UnboundedForTrustedInput, None),
    ] {
        let policy = RenderResourcePolicy::for_profile(profile);
        assert_eq!(policy.value(ResourceLimitId::MaxLayoutWorkUnits), expected);
        assert!(policy.explicit_overrides().next().is_none());
    }
    assert_eq!(
        RenderResourcePolicy::default(),
        RenderResourcePolicy::interactive()
    );
}

#[test]
fn resource_contract_is_complete_unique_and_drives_every_profile() {
    assert_eq!(
        RESOURCE_PROFILE_DESCRIPTORS.len(),
        RenderResourceProfile::ALL.len()
    );
    assert_eq!(RESOURCE_LIMIT_DESCRIPTORS.len(), RESOURCE_LIMIT_COUNT);

    let profile_ids = RESOURCE_PROFILE_DESCRIPTORS
        .iter()
        .map(|descriptor| descriptor.id)
        .collect::<HashSet<_>>();
    assert_eq!(profile_ids.len(), RESOURCE_PROFILE_DESCRIPTORS.len());
    assert_eq!(
        RESOURCE_PROFILE_DESCRIPTORS
            .iter()
            .filter(|descriptor| descriptor.recommended_binding_default)
            .map(|descriptor| descriptor.profile)
            .collect::<Vec<_>>(),
        vec![GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE]
    );
    for profile in RenderResourceProfile::ALL {
        let descriptor = profile.descriptor();
        assert_eq!(RenderResourceProfile::from_id(descriptor.id), Some(profile));
        let policy = RenderResourcePolicy::for_profile(profile);
        for limit in RESOURCE_LIMIT_DESCRIPTORS {
            assert_eq!(policy.profile(), profile);
            if limit.hard_cap {
                assert!(!limit.overridable);
                assert!(policy.value(limit.id).is_some());
            }
        }
    }

    let limit_ids = RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .map(|descriptor| descriptor.stable_id)
        .collect::<HashSet<_>>();
    assert_eq!(limit_ids.len(), RESOURCE_LIMIT_DESCRIPTORS.len());
    for descriptor in RESOURCE_LIMIT_DESCRIPTORS {
        assert_eq!(
            ResourceLimitId::from_stable_id(descriptor.stable_id),
            Some(descriptor.id)
        );
        assert_eq!(descriptor.id.descriptor(), descriptor);
    }
}

#[test]
fn resource_overrides_fail_closed_for_unknown_and_internal_ids() {
    let mut limits = RenderResourcePolicy::interactive();
    assert!(matches!(
        limits.apply_override("future_limit", 1),
        Err(ResourceLimitOverrideError::UnknownLimit(_))
    ));
    for hard_cap in [
        SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
        SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
    ] {
        assert_eq!(
            limits.apply_override(hard_cap, 1),
            Err(ResourceLimitOverrideError::HardCap(hard_cap))
        );
    }
    assert_eq!(
        limits.apply_override("max_svg_elements", 0),
        Err(ResourceLimitOverrideError::NonPositive("max_svg_elements"))
    );
    limits.apply_override("max_svg_elements", 7).unwrap();
    assert_eq!(limits.value(ResourceLimitId::MaxSvgElements), Some(7));
}

#[test]
fn resource_policy_meet_and_restriction_are_pointwise_monotonic_for_every_public_limit() {
    for descriptor in RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .copied()
        .filter(|descriptor| descriptor.overridable)
    {
        let host = RenderResourcePolicy::interactive();
        let ceiling = host
            .value(descriptor.id)
            .expect("interactive profile must bound every overridable limit");
        let stricter_value = descriptor.minimum_value.max(ceiling / 2);
        let stricter = host.with_limit(descriptor.id, stricter_value).unwrap();

        let met = host.meet(&stricter);
        assert_eq!(met.profile(), host.profile());
        assert_eq!(met.value(descriptor.id), Some(stricter_value));
        let restricted = host.restrict_with(&stricter).unwrap();
        assert_eq!(restricted.value(descriptor.id), Some(stricter_value));

        let looser_value = ceiling.checked_add(1).unwrap();
        let looser = host.with_limit(descriptor.id, looser_value).unwrap();
        assert_eq!(
            host.restrict_with(&looser),
            Err(ResourcePolicyRestrictionError {
                id: descriptor.id,
                requested: Some(looser_value),
                ceiling: Some(ceiling),
            })
        );
    }

    let host = RenderResourcePolicy::unbounded_for_trusted_input();
    let restricted = host
        .restrict_with(&RenderResourcePolicy::constrained())
        .unwrap();
    assert_eq!(restricted.profile(), host.profile());
    for descriptor in RESOURCE_LIMIT_DESCRIPTORS {
        assert_eq!(
            restricted.value(descriptor.id),
            minimum_ceiling(
                host.value(descriptor.id),
                RenderResourcePolicy::constrained().value(descriptor.id)
            )
        );
    }
}

#[test]
fn resolved_svg_backend_hard_caps_remain_active_for_unbounded_policy() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    for (elements, tree_depth, limit, actual, max) in [
        (
            MAX_RESVG_TREE_NODES + 1,
            0,
            SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
            MAX_RESVG_TREE_NODES + 1,
            MAX_RESVG_TREE_NODES,
        ),
        (
            0,
            MAX_RESVG_TREE_DEPTH + 1,
            SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
            MAX_RESVG_TREE_DEPTH + 1,
            MAX_RESVG_TREE_DEPTH,
        ),
    ] {
        let error = policy
            .check_svg_structure(elements, tree_depth)
            .unwrap_err();

        assert_eq!(error.phase, ResourceLimitPhase::SvgPostprocess);
        assert_eq!(error.limit, limit);
        assert_eq!(error.actual, actual);
        assert_eq!(error.max, max);
    }
}

#[test]
fn source_limit_reports_structured_error() {
    let err = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSourceBytes, 4)
        .unwrap()
        .with_limit(ResourceLimitId::MaxSvgBytes, 123)
        .unwrap()
        .check_source_bytes("12345")
        .unwrap_err();

    assert_eq!(err.phase, ResourceLimitPhase::Source);
    assert_eq!(err.limit, "max_source_bytes");
    assert_eq!(err.actual, 5);
    assert_eq!(err.max, 4);
    assert_eq!(
        err.explicit_overrides,
        vec![
            ResourceLimitOverride {
                id: ResourceLimitId::MaxSourceBytes,
                value: 4,
            },
            ResourceLimitOverride {
                id: ResourceLimitId::MaxSvgBytes,
                value: 123,
            },
        ]
    );
}

#[test]
fn derived_layout_work_limits_report_the_owned_phase_and_metric() {
    let limits = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 7)
        .unwrap();

    let error = limits.check_layout_work_units(8).unwrap_err();
    assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(error.limit, "max_layout_work_units");
    assert_eq!(error.actual, 8);
    assert_eq!(error.max, 7);
}

#[test]
fn prepared_text_retained_limits_report_the_owned_phase_and_metric() {
    let limits = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 7)
        .unwrap();

    let error = limits.check_prepared_text_retained_bytes(8).unwrap_err();
    assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(error.limit, "max_prepared_text_retained_bytes");
    assert_eq!(error.actual, 8);
    assert_eq!(error.max, 7);
}

#[test]
fn operation_work_meter_preflight_does_not_consume_budget() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 7)
        .unwrap();
    let meter = OperationWorkMeter::new(policy);

    meter.preflight(7).unwrap();
    meter.preflight(7).unwrap();
    assert_eq!(meter.used(), 0);
    meter.charge(7).unwrap();
    assert_eq!(meter.used(), 7);
}

#[test]
fn operation_work_meter_checks_shared_control_before_charging() {
    let control = OperationControl::new();
    control.cancel();
    let meter = OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap(),
        control,
    );

    let first = meter.charge(2).unwrap_err();
    let OperationWorkError::Cancelled(error) = &first else {
        panic!("expected cancellation to win before resource accounting");
    };
    assert_eq!(error.phase, OperationPhase::Layout);
    assert_eq!(meter.used(), 0);
    assert_eq!(meter.charge(2).unwrap_err(), first);
}

#[test]
fn operation_work_meter_checks_deadline_before_charging() {
    let control = OperationControl::new().with_deadline(std::time::Duration::ZERO);
    let meter = OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control,
    );

    let OperationWorkError::Cancelled(error) = meter.charge(1).unwrap_err() else {
        panic!("expected deadline cancellation before resource accounting");
    };
    assert_eq!(error.phase, OperationPhase::Layout);
    assert_eq!(error.reason, merman_model::CancelReason::DeadlineExceeded);
    assert_eq!(meter.used(), 0);
}

#[cfg(feature = "resources-flowchart")]
#[test]
fn model_preflight_observes_cancellation_before_latching_its_limit() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .expect("the controlled fixture should parse")
        .expect("the controlled fixture should produce a render model")
        .into_presentation();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .expect("the model limit should be valid");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let meter = OperationWorkMeter::new_with_control(policy, control);

    let first = meter
        .preflight_parsed_render(&parsed, OperationPhase::Layout)
        .expect_err("cancellation after the scan must beat its pending resource rejection");
    assert!(matches!(
        first,
        OperationWorkError::Cancelled(error)
            if error.phase == OperationPhase::Layout
                && error.reason == merman_model::CancelReason::Requested
    ));
    assert_eq!(
        meter
            .preflight_parsed_render(&parsed, OperationPhase::Emit)
            .expect_err("the cancellation must remain the sticky terminal"),
        first
    );
}

#[test]
fn operation_svg_meter_checks_shared_control_before_reserving_bytes() {
    let control = OperationControl::new();
    control.cancel();
    let meter = OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control,
    );

    let OperationWorkError::Cancelled(error) = meter.charge_svg_bytes(1).unwrap_err() else {
        panic!("expected cancellation to win before SVG accounting");
    };
    assert_eq!(error.phase, OperationPhase::Emit);
    assert_eq!(meter.projected_svg_bytes(), 0);
}

#[test]
fn operation_work_meter_replays_first_resource_terminal_after_cancellation() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 10)
        .unwrap();
    let control = OperationControl::new();
    let meter = OperationWorkMeter::new_with_control(policy, control.clone());
    meter.charge(8).unwrap();

    let first = meter.charge(3).unwrap_err();
    let OperationWorkError::ResourceLimitExceeded(error) = &first else {
        panic!("expected a resource rejection");
    };
    assert_eq!(error.cause, ResourceLimitCause::Ceiling);
    assert_eq!(error.actual, 11);
    assert_eq!(error.max, 10);
    assert_eq!(meter.used(), 8);

    control.cancel();
    assert_eq!(meter.charge(2).unwrap_err(), first);
    assert_eq!(meter.used(), 8);
}

#[test]
fn operation_work_meter_replays_the_originating_policy_provenance() {
    let originating_policy = RenderResourcePolicy::constrained()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap()
        .with_limit(ResourceLimitId::MaxSvgBytes, 17)
        .unwrap();
    let observing_policy = RenderResourcePolicy::interactive()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 99)
        .unwrap();
    let control = OperationControl::new();
    let originating = OperationWorkMeter::new_with_control(originating_policy, control.clone());
    let observing = OperationWorkMeter::new_with_control(observing_policy, control);

    let OperationWorkError::ResourceLimitExceeded(first) = originating.charge(2).unwrap_err()
    else {
        panic!("expected the originating ceiling");
    };
    let OperationWorkError::ResourceLimitExceeded(replayed) =
        observing.checkpoint(OperationPhase::Emit).unwrap_err()
    else {
        panic!("expected the stored render terminal to retain its typed projection");
    };

    assert_eq!(replayed, first);
    assert_eq!(replayed.profile, RenderResourceProfile::Constrained);
    assert_eq!(
        replayed.explicit_overrides,
        vec![
            ResourceLimitOverride {
                id: ResourceLimitId::MaxLayoutWorkUnits,
                value: 1,
            },
            ResourceLimitOverride {
                id: ResourceLimitId::MaxSvgBytes,
                value: 17,
            },
        ]
    );
}

#[test]
fn operation_work_meter_preserves_a_foreign_ascii_domain() {
    let control = OperationControl::new();
    let terminal = control.terminate_resource_limit(OperationResourceLimitExceeded {
        id: "max_ascii_output_bytes",
        phase: OperationPhase::Emit,
        resource_phase: "ascii_output",
        limit: 7,
        consumed: 7,
        requested: 1,
        provenance: OperationResourceProvenance::new(
            OperationResourceDomain::Ascii,
            Some(RenderResourceProfile::Constrained),
            [OperationResourceOverride {
                id: "max_ascii_output_bytes",
                value: 7,
            }],
        ),
    });
    let observing =
        OperationWorkMeter::new_with_control(RenderResourcePolicy::interactive(), control);

    assert_eq!(
        observing
            .checkpoint(OperationPhase::Layout)
            .expect_err("render must not project an ASCII resource terminal"),
        OperationWorkError::ForeignResourceTerminal(terminal)
    );
}

#[test]
fn operation_work_meter_overflow_fails_under_unlimited_policy() {
    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    meter.charge(usize::MAX).unwrap();

    let preflight = meter.preflight(1).unwrap_err();
    let OperationWorkError::ResourceLimitExceeded(preflight_error) = &preflight else {
        panic!("expected a resource rejection");
    };
    assert_eq!(
        preflight_error.cause,
        ResourceLimitCause::ArithmeticOverflow
    );
    assert_eq!(preflight_error.limit, "max_layout_work_units");
    assert_eq!(preflight_error.actual, usize::MAX);
    assert_eq!(preflight_error.max, usize::MAX);
    assert_eq!(meter.used(), usize::MAX);

    assert_eq!(meter.charge(1).unwrap_err(), preflight);
    assert_eq!(meter.used(), usize::MAX);
}

#[test]
fn operation_prepared_text_reservation_tracks_peak_reconcile_and_release() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 10)
        .unwrap();
    let meter = std::sync::Arc::new(OperationWorkMeter::new(policy));

    let mut reservation = meter.reserve_prepared_text_retained_bytes(10).unwrap();
    assert_eq!(reservation.retained_bytes(), 10);
    assert_eq!(meter.prepared_text_retained_bytes(), 10);
    assert_eq!(meter.prepared_text_retained_bytes_peak(), 10);
    assert_eq!(meter.used(), 0);

    reservation.reconcile_downward(6);
    assert_eq!(reservation.retained_bytes(), 6);
    assert_eq!(meter.prepared_text_retained_bytes(), 6);
    assert_eq!(meter.prepared_text_retained_bytes_peak(), 10);

    drop(reservation);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
    assert_eq!(meter.prepared_text_retained_bytes_peak(), 10);
    assert_eq!(meter.used(), 0);
}

#[test]
fn operation_prepared_text_rejection_is_structured_and_does_not_advance() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 10)
        .unwrap();
    let meter = std::sync::Arc::new(OperationWorkMeter::new(policy));
    let reservation = meter.reserve_prepared_text_retained_bytes(8).unwrap();

    let error = meter.reserve_prepared_text_retained_bytes(3).unwrap_err();
    assert_eq!(error.cause, ResourceLimitCause::Ceiling);
    assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(error.limit, "max_prepared_text_retained_bytes");
    assert_eq!(error.actual, 11);
    assert_eq!(error.max, 10);
    assert_eq!(meter.prepared_text_retained_bytes(), 8);
    assert_eq!(meter.prepared_text_retained_bytes_peak(), 8);

    drop(reservation);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn operation_prepared_text_preflight_observes_existing_reservations_without_advancing() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 10)
        .unwrap();
    let meter = std::sync::Arc::new(OperationWorkMeter::new(policy));
    let reservation = meter.reserve_prepared_text_retained_bytes(6).unwrap();

    assert_eq!(meter.remaining_prepared_text_retained_bytes(), Some(4));
    meter.preflight_prepared_text_retained_bytes(4).unwrap();
    assert_eq!(meter.prepared_text_retained_bytes(), 6);

    let error = meter.preflight_prepared_text_retained_bytes(5).unwrap_err();
    let OperationWorkError::ResourceLimitExceeded(error) = error else {
        panic!("expected prepared-text resource limit")
    };
    assert_eq!(error.actual, 11);
    assert_eq!(error.max, 10);
    assert_eq!(meter.prepared_text_retained_bytes(), 6);

    drop(reservation);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn operation_prepared_text_reservation_overflow_fails_under_unlimited_policy() {
    let meter = std::sync::Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let reservation = meter
        .reserve_prepared_text_retained_bytes(usize::MAX)
        .unwrap();

    let error = meter.reserve_prepared_text_retained_bytes(1).unwrap_err();
    assert_eq!(error.cause, ResourceLimitCause::ArithmeticOverflow);
    assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(error.limit, "max_prepared_text_retained_bytes");
    assert_eq!(error.actual, usize::MAX);
    assert_eq!(error.max, usize::MAX);
    assert_eq!(meter.prepared_text_retained_bytes(), usize::MAX);

    drop(reservation);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn operation_svg_meter_accepts_exact_limit_and_rejects_one_more() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 10)
        .unwrap();
    let control = OperationControl::new();
    let meter = OperationWorkMeter::new_with_control(policy, control.clone());

    meter.charge_svg_bytes(10).unwrap();
    assert_eq!(meter.projected_svg_bytes(), 10);
    assert_eq!(meter.remaining_svg_bytes(), Some(0));

    let first = meter.charge_svg_bytes(1).unwrap_err();
    let OperationWorkError::ResourceLimitExceeded(error) = &first else {
        panic!("expected a resource rejection");
    };
    assert_eq!(error.actual, 11);
    assert_eq!(error.max, 10);
    assert_eq!(meter.projected_svg_bytes(), 10);

    control.cancel();
    assert_eq!(meter.charge_svg_bytes(1).unwrap_err(), first);
    assert_eq!(meter.projected_svg_bytes(), 10);
}

#[test]
fn operation_svg_append_check_accepts_exact_and_does_not_charge() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 10)
        .unwrap();
    let meter = OperationWorkMeter::new(policy);
    meter.charge_svg_bytes(2).unwrap();

    meter.check_svg_append(7, 3).unwrap();
    assert_eq!(meter.projected_svg_bytes(), 2);

    let OperationWorkError::ResourceLimitExceeded(error) =
        meter.check_svg_append(7, 4).unwrap_err()
    else {
        panic!("expected a resource rejection");
    };
    assert_eq!(error.cause, ResourceLimitCause::Ceiling);
    assert_eq!(error.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(error.limit, "max_svg_bytes");
    assert_eq!(error.actual, 11);
    assert_eq!(error.max, 10);
    assert_eq!(meter.projected_svg_bytes(), 2);
}

#[test]
fn operation_svg_append_overflow_is_structured_and_does_not_charge() {
    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    meter.charge_svg_bytes(2).unwrap();

    let OperationWorkError::ResourceLimitExceeded(error) =
        meter.check_svg_append(usize::MAX, 1).unwrap_err()
    else {
        panic!("expected a resource rejection");
    };
    assert_eq!(error.cause, ResourceLimitCause::ArithmeticOverflow);
    assert_eq!(error.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(error.limit, "max_svg_bytes");
    assert_eq!(error.actual, usize::MAX);
    assert_eq!(error.max, usize::MAX);
    assert_eq!(meter.projected_svg_bytes(), 2);
}

#[test]
fn operation_svg_meter_reconciles_conservative_reservations() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 12)
        .unwrap();
    let meter = OperationWorkMeter::new(policy);

    meter.charge_svg_bytes(8).unwrap();
    meter.reconcile_svg_bytes(8, 5).unwrap();
    assert_eq!(meter.projected_svg_bytes(), 5);
    assert_eq!(meter.remaining_svg_bytes(), Some(7));

    meter.reconcile_svg_bytes(5, 12).unwrap();
    assert_eq!(meter.projected_svg_bytes(), 12);
    assert_eq!(meter.remaining_svg_bytes(), Some(0));
}

#[test]
fn operation_svg_meter_atomically_reserves_available_growth() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 10)
        .unwrap();
    let meter = OperationWorkMeter::new(policy);

    meter.charge_svg_bytes(7).unwrap();
    let reservation = meter.reserve_svg_bytes_up_to(5).unwrap();
    assert_eq!(reservation.additional_bytes, 3);
    let error = reservation
        .limit_error
        .expect("a partial reservation carries the deterministic limit error");
    assert_eq!(error.actual, 11);
    assert_eq!(error.max, 10);
    assert_eq!(meter.projected_svg_bytes(), 10);

    meter.reconcile_svg_bytes(3, 1).unwrap();
    assert_eq!(meter.projected_svg_bytes(), 8);
    assert_eq!(meter.remaining_svg_bytes(), Some(2));

    meter.charge_svg_bytes(2).unwrap();
    assert_eq!(meter.projected_svg_bytes(), 10);
}

#[test]
fn partial_svg_reservation_preserves_arithmetic_overflow_classification() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, usize::MAX)
        .unwrap();
    let control = OperationControl::new();
    let meter = OperationWorkMeter::new_with_control(policy, control.clone());

    meter.charge_svg_bytes(usize::MAX).unwrap();
    let reservation = meter.reserve_svg_bytes_up_to(1).unwrap();
    assert_eq!(reservation.additional_bytes, 0);
    let error = reservation
        .limit_error
        .expect("an overflowing partial reservation must retain its error");
    assert_eq!(error.cause, ResourceLimitCause::ArithmeticOverflow);
    assert_eq!(error.actual, usize::MAX);
    assert_eq!(error.max, usize::MAX);
    assert_eq!(meter.projected_svg_bytes(), usize::MAX);

    let first = meter.terminate_absolute_resource_error(error, OperationPhase::Emit);
    control.cancel();
    assert_eq!(
        meter
            .charge(1)
            .expect_err("the finalized overflow must remain the sticky terminal"),
        first
    );
}

#[cfg(feature = "resources-zenuml")]
#[test]
fn zenuml_complexity_includes_inline_decorations() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "zenuml\nA->[rocket]B.call()\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Zenuml(model) = parsed.model() else {
        panic!("expected ZenUML model");
    };
    let complexity = ZenumlComplexity::from_model(model);

    assert_eq!(complexity.participants, 2);
    assert_eq!(complexity.statements, 1);
    let required = ["rocket", "call()"]
        .into_iter()
        .map(str::len)
        .sum::<usize>();
    assert!(complexity.label_bytes >= required);
}

#[cfg(feature = "resources-zenuml")]
#[test]
fn zenuml_uses_the_shared_model_budget() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "zenuml\nA.call() {\n  if(ok) {\n    if(inner) {\n      B.work()\n    }\n  }\n}\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Zenuml(model) = parsed.model() else {
        panic!("expected ZenUML model");
    };

    let limits = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .unwrap();
    let error = limits.check_zenuml_complexity(model).unwrap_err();
    assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(error.limit, "max_model_items");
}

#[cfg(any(
    feature = "resources-flowchart",
    feature = "resources-swimlane",
    feature = "resources-agentflow"
))]
#[test]
fn flowchart_complexity_counts_layout_nodes_and_labels() {
    let model = FlowchartModel {
        keyword: "graph".to_string(),
        acc_descr: None,
        acc_title: None,
        class_defs: Default::default(),
        direction: None,
        edge_defaults: None,
        vertex_calls: Vec::new(),
        nodes: vec![FlowNode {
            id: "A".to_string(),
            provenance: Default::default(),
            label: Some("Alpha".to_string()),
            label_type: None,
            layout_shape: None,
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }],
        edges: vec![FlowEdge {
            id: "L-A-B".to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: Some("edge".to_string()),
            label_type: None,
            edge_type: None,
            arrow: "-->".to_string(),
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::Point,
            is_user_defined_id: false,
            stroke: None,
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            step: None,
            length: 1,
        }],
        subgraphs: vec![FlowSubgraph {
            metadata: None,
            id: "cluster".to_string(),
            title: "Cluster".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string()],
        }],
        dropped_subgraph_members: Vec::new(),
        subgraph_vertex_styles: Default::default(),
        label_sources: Default::default(),
        tooltips: Default::default(),
        warning_facts: Vec::new(),
    };

    let complexity = FlowchartComplexity::from_model(&model);
    assert_eq!(complexity.nodes, 2);
    assert_eq!(complexity.edges, 1);
    assert_eq!(complexity.subgraphs, 1);
    assert!(complexity.label_bytes >= "AlphaedgeCluster".len());
}
