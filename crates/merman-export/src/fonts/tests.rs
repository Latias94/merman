use super::*;
use merman_render::environment::TextMeasurementPhase;
use merman_render::text::{TextStyle, WrapMode};

#[test]
fn acquisition_requires_an_explicit_nonempty_font_set() {
    assert!(matches!(
        NativeFontOptions::default().load(&OperationControl::new()),
        Err(NativeFontError::EmptyFontSet)
    ));
}

#[test]
fn one_shape_preserves_two_anchor_rounding_and_bounds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(root.join("NotoSans-Latin-subset.ttf"))
        .with_font_file(root.join("Cousine-Regular.ttf"))
        .with_font_file(root.join("NotoSansSC-subset.ttf"))
        .load(&control)
        .unwrap()
        .with_strict_glyph_coverage(true);
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    for (family, text, preserve, tspan) in [
        ("Noto Sans", "AVA fi", false, false),
        ("Noto Sans", "  AV\u{a0}  ", true, false),
        ("Noto Sans", " \u{a0} ", true, true),
        ("Cousine", "אבגד", false, false),
        ("Cousine", "A中V", false, false),
    ] {
        let style = TextStyle {
            font_family: Some(family.to_owned()),
            font_size: 31.125,
            ..Default::default()
        };
        let actual = context
            .shape_probe(text, &style, &control, preserve, tspan, None)
            .unwrap();
        let whitespace = text.chars().all(char::is_whitespace);
        let space = if preserve && !tspan {
            "preserve"
        } else {
            "default"
        };
        let decoration = if whitespace { "underline" } else { "none" };
        let payload = if tspan {
            format!(r#"<tspan xml:space="preserve">{text}</tspan>"#)
        } else {
            text.to_owned()
        };
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><g font-family="{family}" font-size="31.125" font-weight="400" font-style="normal" xml:space="{space}" text-decoration="{decoration}"><text id="start" x="0" y="0" text-anchor="start">{payload}</text><text id="end" x="0" y="0" text-anchor="end">{payload}</text></g></svg>"#,
        );
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let (Some(usvg::Node::Text(start)), Some(usvg::Node::Text(end))) =
            (tree.node_by_id("start"), tree.node_by_id("end"))
        else {
            panic!("expected both reference anchors for {text:?}");
        };
        let metric = start.bounding_box();
        let ink = if whitespace {
            metric
        } else {
            start.flattened().bounding_box().unwrap()
        };
        assert_eq!(
            [
                actual.advance,
                actual.width,
                actual.height,
                actual.x,
                actual.ink_x,
                actual.ink_right,
                actual.raw_y
            ]
            .map(f64::to_bits),
            [
                (metric.x() - end.bounding_box().x()).abs(),
                metric.width(),
                metric.height(),
                metric.x(),
                ink.x(),
                ink.right(),
                metric.y(),
            ]
            .map(|value| f64::from(value).to_bits()),
            "{text:?}"
        );
    }
}

#[test]
fn partial_fallback_with_different_ligatures_resolves_matching_clusters() {
    let base = include_bytes!("../../tests/fonts/MermanProbeBase.ttf");
    let partial = include_bytes!("../../tests/fonts/MermanProbePartial.ttf");
    let full = include_bytes!("../../tests/fonts/MermanProbeFull.ttf");
    for (text, expected) in [
        (
            "fi✓\u{e000}",
            ["MermanProbeBase", "MermanProbePartial", "MermanProbeBase"],
        ),
        (
            "fi✓中",
            ["MermanProbeBase", "MermanProbePartial", "MermanProbeFull"],
        ),
    ] {
        let control = OperationControl::new();
        let context = NativeFontContext::from_font_bytes(
            [base.to_vec(), partial.to_vec(), full.to_vec()],
            NativeFontLimits::default(),
            &control,
        )
        .unwrap()
        .with_strict_glyph_coverage(true);
        let style = TextStyle {
            font_family: Some("MermanProbeBase".to_owned()),
            font_size: 28.0,
            ..Default::default()
        };
        let measured = context.measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::CanvasMeasureTextWidth,
            phase: TextMeasurementPhase::ComputedLength,
            text,
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::HtmlLike,
        });
        let Some(HostTextMeasurement::Length(width)) = measured.unwrap() else {
            panic!("expected complete native measurement");
        };
        assert_eq!(width, 42.0);

        let mut options = usvg::Options::default();
        let observer = context.apply_to_usvg_options_for_export(&mut options, &control);
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50"><text id="probe" x="0" y="40" font-family="MermanProbeBase" font-size="28">{text}</text></svg>"#
        );
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
            panic!("expected a laid-out text node");
        };
        let faces: Vec<_> = text
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .map(|glyph| {
                assert_ne!(glyph.id.0, 0);
                tree.fontdb()
                    .face(glyph.font)
                    .unwrap()
                    .post_script_name
                    .as_str()
            })
            .collect();
        assert_eq!(faces, expected);
        assert!(observer.glyph_fallback_used());
        context.check().unwrap();
    }
}

#[test]
fn native_export_glyph_fallback_filters_symbols_and_records_the_outcome() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
    for fixture in ["MermanSymbolicTest.ttf", "MermanPictorialTest.ttf"] {
        for include_text_fallback in [false, true] {
            let mut fonts = NativeFontOptions::default()
                .with_font_file(root.join("NotoSans-Latin-subset.ttf"))
                .with_font_file(root.join(fixture));
            if include_text_fallback {
                fonts = fonts.with_font_file(root.join("Cousine-Regular.ttf"));
            }
            let control = OperationControl::new();
            let context = fonts.load(&control).unwrap();
            let mut options = usvg::Options::default();
            let observer = context.apply_to_usvg_options_for_export(&mut options, &control);
            let tree = usvg::Tree::from_str(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40" font-family="Noto Sans">Aam</text></svg>"#,
                &options,
            ).unwrap();
            let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
                panic!("partial text must still be painted");
            };
            let mut expected = BTreeSet::from(["NotoSans-Regular"]);
            if include_text_fallback {
                expected.insert("Cousine-Regular");
            }
            let glyphs: Vec<_> = text
                .layouted()
                .iter()
                .flat_map(|span| &span.positioned_glyphs)
                .collect();
            let selected: BTreeSet<_> = glyphs
                .iter()
                .map(|glyph| {
                    tree.fontdb()
                        .face(glyph.font)
                        .unwrap()
                        .post_script_name
                        .as_str()
                })
                .collect();
            assert_eq!(selected, expected);
            assert_eq!(observer.glyph_fallback_used(), include_text_fallback);
            context.check().unwrap();
        }
    }
}

#[test]
#[cfg(not(all(feature = "system-fonts", not(target_family = "wasm"))))]
fn system_discovery_is_explicitly_unavailable() {
    assert!(matches!(
        NativeFontOptions::default()
            .with_system_fonts(true)
            .load(&OperationControl::new()),
        Err(NativeFontError::SystemDiscoveryUnavailable)
    ));
}

#[test]
fn acquisition_replays_cancellation_before_io() {
    let control = OperationControl::new();
    control.cancel();
    assert!(matches!(
        NativeFontOptions::default()
            .with_font_file("missing.ttf")
            .load(&control),
        Err(NativeFontError::Operation(_))
    ));
}

#[test]
fn directory_filter_is_case_insensitive_and_excludes_web_fonts() {
    assert!(is_font_path(Path::new("font.TTC")));
    assert!(!is_font_path(Path::new("font.woff2")));
}

#[test]
fn measurement_budget_failure_is_sticky_at_the_fallible_boundary() {
    let limits = NativeFontLimits {
        max_label_bytes: 1,
        ..Default::default()
    };
    let context = NativeFontContext(Arc::new(FontContextInner {
        database: Arc::new(usvg::fontdb::Database::new()),
        selection: Arc::new(FontSelectionPolicy::default()),
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        discovery: None,
        #[cfg(all(feature = "system-fonts", not(target_family = "wasm")))]
        sources: registry::FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0),
        default_family: "sans-serif".to_owned(),
        language: None,
        identity: "test".to_owned(),
        limits,
        control: OperationControl::new(),
        state: Mutex::new(MeasurementState::default()),
        fatal: OnceLock::new(),
    }));
    let style = TextStyle::default();
    let request = HostTextMeasurementRequest {
        operation: TextMeasurementOperation::TitleBBoxX,
        phase: TextMeasurementPhase::SvgBBox,
        text: "long",
        style: &style,
        max_width: None,
        wrap_mode: WrapMode::SvgLike,
    };
    for _ in 0..2 {
        let control = OperationControl::new();
        assert!(context.measure_with_control(request, &control).is_err());
        assert!(matches!(
            control.terminal_checkpoint_at(OperationPhase::Layout),
            Err(OperationLedgerError::ResourceLimitExceeded(event))
                if event.id == "native_font_label_bytes"
        ));
    }
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Limit {
            resource: "label-bytes",
            ..
        })
    ));
    assert!(matches!(
        context.for_export(&OperationControl::new()),
        Err(NativeFontError::Limit {
            resource: "label-bytes",
            ..
        })
    ));
}

#[test]
fn cancellation_at_the_label_limit_is_the_sticky_font_failure() {
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .with_limits(NativeFontLimits {
            max_label_bytes: 0,
            ..Default::default()
        })
        .load(&OperationControl::new())
        .unwrap();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let result = context.measure_with_control(
        HostTextMeasurementRequest {
            operation: TextMeasurementOperation::TitleBBoxX,
            phase: TextMeasurementPhase::SvgBBox,
            text: "label",
            style: &TextStyle::default(),
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        },
        &control,
    );
    let terminal = control
        .terminal_checkpoint_at(OperationPhase::Layout)
        .unwrap_err();
    assert!(matches!(terminal, OperationLedgerError::Cancelled(_)));
    assert!(matches!(context.check(), Err(NativeFontError::Operation(error)) if error == terminal));
    assert!(matches!(
        context.for_export(&OperationControl::new()),
        Err(NativeFontError::Operation(error)) if error == terminal
    ));
    assert_eq!(result.unwrap_err().to_string(), terminal.to_string());
}

#[test]
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn measurement_and_export_replay_the_active_operation_terminal() {
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&OperationControl::new())
        .unwrap();
    let style = TextStyle::default();
    let request = HostTextMeasurementRequest {
        operation: TextMeasurementOperation::TitleBBoxX,
        phase: TextMeasurementPhase::SvgBBox,
        text: "label",
        style: &style,
        max_width: None,
        wrap_mode: WrapMode::SvgLike,
    };

    let cancelled = OperationControl::new();
    cancelled.cancel();
    let source_error = cancelled
        .terminal_checkpoint_at(OperationPhase::Layout)
        .unwrap_err();
    assert_eq!(
        context
            .measure_with_control(request, &cancelled)
            .unwrap_err()
            .to_string(),
        source_error.to_string()
    );

    let expired = OperationControl::new().with_deadline(std::time::Duration::ZERO);
    let current_error = expired
        .terminal_checkpoint_at(OperationPhase::Parse)
        .unwrap_err();
    assert_ne!(current_error, source_error);
    assert_eq!(
        context
            .measure_with_control(request, &expired)
            .unwrap_err()
            .to_string(),
        current_error.to_string()
    );
    assert!(
        matches!(context.for_export(&expired), Err(NativeFontError::Operation(error)) if error == current_error)
    );
    assert!(
        matches!(context.check(), Err(NativeFontError::Operation(error)) if error == source_error)
    );
}

#[test]
fn whitespace_advance_is_not_mistaken_for_missing_glyph_coverage() {
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&OperationControl::new())
        .unwrap();
    let style = TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 16.0,
        ..TextStyle::default()
    };
    let result = context
        .measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::CanvasMeasureTextWidth,
            phase: TextMeasurementPhase::ComputedLength,
            text: "   ",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        })
        .unwrap();
    assert!(matches!(result, Some(HostTextMeasurement::Length(width)) if width > 0.0));
}

#[test]
fn repeated_missing_glyph_probes_share_the_bounded_measurement_cache() {
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .with_limits(NativeFontLimits {
            max_measurements: 1,
            ..Default::default()
        })
        .load(&control)
        .unwrap();
    let style = TextStyle {
        font_family: Some("Cousine".to_owned()),
        ..TextStyle::default()
    };
    let request = HostTextMeasurementRequest {
        operation: TextMeasurementOperation::TitleBBoxX,
        phase: TextMeasurementPhase::SvgBBox,
        text: "\u{10ffff}",
        style: &style,
        max_width: None,
        wrap_mode: WrapMode::SvgLike,
    };
    for _ in 0..3 {
        assert!(context.measure(request).unwrap().is_none());
    }
    let state = context.0.state.lock().unwrap();
    assert_eq!(state.measurements, 1);
    assert_eq!(state.cache.len(), 1);
    assert!(state.cache.values().all(Option::is_none));
    assert!(state.cache_bytes > request.text.len());
    assert!(state.cache_bytes <= context.0.limits.max_cache_bytes);
    drop(state);
    context.check().unwrap();

    let fresh = context.for_operation(&OperationControl::new());
    assert!(fresh.0.state.lock().unwrap().cache.is_empty());
    assert!(fresh.measure(request).unwrap().is_none());
    assert_eq!(fresh.0.state.lock().unwrap().measurements, 1);

    control.cancel();
    assert!(context.measure(request).is_err());
    assert_eq!(context.0.state.lock().unwrap().measurements, 1);
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Operation(_))
    ));
    fresh.check().unwrap();
}

#[test]
fn missing_glyph_cache_respects_entry_and_byte_limits() {
    for (max_cache_entries, max_cache_bytes, warm_success) in
        [(0, 4096, false), (4, 0, false), (1, 4096, true)]
    {
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .with_limits(NativeFontLimits {
                max_cache_entries,
                max_cache_bytes,
                max_measurements: 1 + usize::from(warm_success),
                ..Default::default()
            })
            .load(&control)
            .unwrap();
        let style = TextStyle {
            font_family: Some("Cousine".to_owned()),
            ..TextStyle::default()
        };
        let request = HostTextMeasurementRequest {
            operation: TextMeasurementOperation::TitleBBoxX,
            phase: TextMeasurementPhase::SvgBBox,
            text: "\u{10ffff}",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        if warm_success {
            assert!(
                context
                    .measure(HostTextMeasurementRequest {
                        text: "A",
                        ..request
                    })
                    .unwrap()
                    .is_some()
            );
        }
        assert!(context.measure(request).unwrap().is_none());
        assert!(context.measure(request).is_err());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "measurements",
                ..
            })
        ));
        let state = context.0.state.lock().unwrap();
        assert_eq!(state.cache.len(), usize::from(warm_success));
        assert!(state.cache_bytes <= max_cache_bytes);
    }
}

#[test]
fn missing_glyph_cache_keys_keep_probe_text_and_style_distinct() {
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&control)
        .unwrap();
    let style = TextStyle::default();
    let larger = TextStyle {
        font_size: style.font_size + 1.0,
        ..style.clone()
    };
    for (text, style) in [
        ("\u{10ffff}", &style),
        ("A\u{10ffff}", &style),
        ("\u{10ffff}", &larger),
    ] {
        let request = HostTextMeasurementRequest {
            operation: TextMeasurementOperation::TitleBBoxX,
            phase: TextMeasurementPhase::SvgBBox,
            text,
            style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        for _ in 0..2 {
            assert!(context.measure(request).unwrap().is_none());
        }
    }
    let state = context.0.state.lock().unwrap();
    assert_eq!(state.measurements, 3);
    assert_eq!(state.cache.len(), 3);
}

#[test]
fn svg_wrap_height_uses_native_bbox_and_em_row_offsets() {
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&control)
        .unwrap();
    let style = TextStyle {
        font_size: 20.0,
        ..TextStyle::default()
    };
    let row = context.shape_line("text", &style, &control).unwrap();
    for mode in [WrapMode::SvgLike, WrapMode::SvgLikeSingleRun] {
        let result = context
            .measure(HostTextMeasurementRequest {
                operation: TextMeasurementOperation::Wrapped,
                phase: TextMeasurementPhase::Wrap,
                text: "text<br>text<br>text",
                style: &style,
                max_width: None,
                wrap_mode: mode,
            })
            .unwrap()
            .unwrap();
        let HostTextMeasurement::Metrics(metrics) = result else {
            panic!("expected metrics")
        };
        assert_eq!(metrics.line_count, 3);
        assert!((metrics.height - (row.height + 2.0 * 20.0 * 1.1)).abs() < 1e-6);
    }
}

#[test]
fn uncached_svg_final_rows_supply_width_and_height_from_one_probe_each() {
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .with_limits(NativeFontLimits {
            max_cache_entries: 0,
            max_cache_bytes: 0,
            max_measurements: 2,
            ..Default::default()
        })
        .load(&OperationControl::new())
        .unwrap();
    let style = TextStyle {
        font_size: 20.0,
        ..Default::default()
    };
    let result = context
        .measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::Wrapped,
            phase: TextMeasurementPhase::Wrap,
            text: "first<br>second",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        })
        .unwrap()
        .unwrap();
    let HostTextMeasurement::Metrics(metrics) = result else {
        panic!("expected metrics")
    };
    assert_eq!(metrics.line_count, 2);
    assert!(metrics.width > 0.0 && metrics.height > 20.0);
    let state = context.0.state.lock().unwrap();
    assert_eq!(state.measurements, 2);
    assert!(state.cache.is_empty());
    drop(state);
    context.check().unwrap();
}

#[test]
fn default_svg_whitespace_matches_independent_text_probes() {
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&control)
        .unwrap();
    let style = TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 20.0,
        ..TextStyle::default()
    };
    for label in ["  A  B  ", "A\tB", "A\nB", "A\r\nB", "\u{a0}A\u{a0}B\u{a0}"] {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="100"><text id="independent" x="0" y="0" font-family="Cousine" font-size="20">{}</text></svg>"#,
            escape_xml(label)
        );
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let Some(usvg::Node::Text(text)) = tree.node_by_id("independent") else {
            panic!("probe text missing");
        };
        let expected = f64::from(text.bounding_box().width());
        for operation in [
            TextMeasurementOperation::MermaidCalculateTextDimensions,
            TextMeasurementOperation::SimpleBBoxWidth,
            TextMeasurementOperation::RawBBoxWidth,
            TextMeasurementOperation::TspanBBoxWidth,
        ] {
            let result = context
                .measure(HostTextMeasurementRequest {
                    operation,
                    phase: TextMeasurementPhase::SvgBBox,
                    text: label,
                    style: &style,
                    max_width: None,
                    wrap_mode: WrapMode::SvgLike,
                })
                .unwrap()
                .unwrap();
            let width = match result {
                HostTextMeasurement::Length(width) => width,
                HostTextMeasurement::Metrics(metrics) => metrics.width,
                _ => panic!("unexpected measurement kind"),
            };
            assert!(
                (width - expected).abs() < 1e-6,
                "{operation:?} {label:?}: {width} != {expected}"
            );
        }
    }
}

#[test]
fn preserved_tspan_requests_use_distinct_native_cache_entries() {
    let context = NativeFontOptions::default()
        .with_font_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&OperationControl::new())
        .unwrap();
    let style = TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 20.0,
        ..TextStyle::default()
    };
    let label = "  A  B  ";
    let width = |mode| {
        let result = context
            .measure(HostTextMeasurementRequest {
                operation: TextMeasurementOperation::TspanBBoxWidth,
                phase: TextMeasurementPhase::SvgBBox,
                text: label,
                style: &style,
                max_width: None,
                wrap_mode: mode,
            })
            .unwrap()
            .unwrap();
        let HostTextMeasurement::Length(width) = result else {
            panic!("expected width")
        };
        width
    };
    let collapsed = width(WrapMode::SvgLike);
    let preserved = width(WrapMode::HtmlLike);
    assert!(preserved > collapsed);
    for mode in [WrapMode::SvgLike, WrapMode::HtmlLike, WrapMode::SvgLike] {
        let space = if mode == WrapMode::HtmlLike {
            "preserve"
        } else {
            "default"
        };
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="100"><text id="probe" x="0" y="0" font-family="Cousine" font-size="20"><tspan xml:space="{space}">{label}</tspan></text></svg>"#
        );
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
            panic!("missing probe")
        };
        assert!(
            (width(mode) - f64::from(text.bounding_box().width())).abs() < 1e-6,
            "{mode:?}: native={}, probe={}",
            width(mode),
            text.bounding_box().width()
        );
    }
}

#[test]
fn baseline_probes_are_lazy_and_respect_the_shared_cache_limit() {
    for max_cache_entries in [0, 16] {
        let context = NativeFontOptions::default()
            .with_font_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
            )
            .with_limits(NativeFontLimits {
                max_cache_entries,
                ..Default::default()
            })
            .load(&OperationControl::new())
            .unwrap();
        let style = TextStyle {
            font_size: 20.0,
            ..TextStyle::default()
        };
        let request = |operation| HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::SvgBBox,
            text: "baseline",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        context
            .measure(request(TextMeasurementOperation::RawBBoxWidth))
            .unwrap();
        assert_eq!(context.0.state.lock().unwrap().measurements, 1);
        let first = context
            .measure(request(TextMeasurementOperation::CreateTextBBoxYOffset))
            .unwrap();
        let second = context
            .measure(request(TextMeasurementOperation::CreateTextBBoxYOffset))
            .unwrap();
        assert!(matches!((first, second),
            (Some(HostTextMeasurement::Length(a)), Some(HostTextMeasurement::Length(b)))
            if a == b));
        let state = context.0.state.lock().unwrap();
        if max_cache_entries == 0 {
            assert_eq!(state.measurements, 3);
            assert_eq!(state.cache.len(), 0);
            assert_eq!(state.cache_bytes, 0);
        } else {
            assert_eq!(state.measurements, 2);
            assert_eq!(state.cache.len(), 2);
        }
    }
}
