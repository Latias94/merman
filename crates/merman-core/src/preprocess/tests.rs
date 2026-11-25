use super::config_scan::{
    MAX_CONFIG_NESTING_DEPTH, config_nesting_exceeds_limit,
    config_nesting_exceeds_limit_controlled, max_yaml_indent_depth_controlled,
};
use super::directives::{parse_directive_like_upstream, sanitize_directive};
use super::frontmatter::FrontmatterCapture;
use super::text::{
    encode_mermaid_entities_like_upstream, normalize_crlf,
    normalize_html_tag_attributes_like_upstream,
};
use super::*;
use merman_core_preprocess::frontmatter::dedent_frontmatter_body_controlled;
use merman_core_preprocess::scan::controlled::{
    CONTROLLED_SCAN_CHECKPOINT_BYTES, ControlledScanCheckpoints, trim_whitespace_bounds_controlled,
};
use serde_json::{Map, Value, json};

#[test]
fn frontmatter_collection_text_preserves_source_order() {
    let mut source = String::from("---\nanchor: &ordered {z: 1, a: \"café\\n\"}\n");
    source.push_str("commented: # first comment\n");
    source.push_str(&"# buffered comment\n".repeat(128));
    source.push_str(
        "title: [*ordered, {y: false, b: null}]\ndisplayMode: *ordered\n---\nflowchart TD\nA-->B",
    );

    let assert_fields = |capture: FrontmatterCapture| {
        let processed = capture.result.expect("frontmatter parses");
        assert_eq!(
            processed.title.as_deref(),
            Some(r#"[{"z":1,"a":"café\n"},{"y":false,"b":null}]"#),
        );
        assert_eq!(
            processed.config.get_str("gantt.displayMode"),
            Some(r#"{"z":1,"a":"café\n"}"#),
        );
    };
    let control = OperationControl::new();
    assert_fields(
        process_frontmatter_controlled(
            &source,
            #[cfg(feature = "editor-facts")]
            SourceConfigCaptureMode::Omit,
            &control,
        )
        .unwrap(),
    );
    #[cfg(feature = "editor-facts")]
    {
        let captured =
            process_frontmatter_controlled(&source, SourceConfigCaptureMode::Collect, &control)
                .unwrap();
        assert!(captured.keys.is_empty());
        assert_fields(captured);
    }
}

#[test]
fn directive_json5_decodes_literal_keys_and_preserves_numeric_and_duplicate_semantics() {
    let directive = parse_directive_like_upstream(
        r#"init: {
                '$serde_json::private::RawValue': 'true',
                values: [/* JSON5 */ {
                    '\u0024serde_json::private::Number': '123',
                    hex: +0x10, fraction: .5, large: 18446744073709551615, nan: NaN,
                }],
                theme: 'dark',
                '$serde_json::private::RawValue': 'false',
            }"#,
    )
    .expect("valid directive")
    .expect("init directive");
    let args = directive.args.expect("decoded argument");
    let args = args.as_object().unwrap();
    assert_eq!(args.len(), 3);
    assert_eq!(args["$serde_json::private::RawValue"], "false");
    assert_eq!(args["theme"], "dark");
    let values = args["values"].as_array().unwrap();
    assert_eq!(values.len(), 1);
    let value = values[0].as_object().unwrap();
    assert_eq!(value.len(), 5);
    assert_eq!(value["$serde_json::private::Number"], "123");
    assert_eq!(value["hex"], 16);
    assert_eq!(value["fraction"].as_f64(), Some(0.5));
    assert_eq!(value["large"].as_u64(), Some(u64::MAX));
    assert!(value["nan"].is_null());

    for raw in ["init: {theme: 'dark'} {}", "init: {theme: 'dark'} /*"] {
        let directive = parse_directive_like_upstream(raw).unwrap().unwrap();
        assert!(directive.args.is_none(), "{raw}");
    }
}

#[cfg(feature = "presentation")]
#[test]
fn init_literal_key_preserves_theme_selection_and_color_error() {
    let engine = crate::MermaidEngine::new();
    let literal = r#""$serde_json::private::RawValue":"blue""#;
    let color = r#""primaryColor":"COLOR""#;
    for keys in [[literal, color], [color, literal]] {
        let source = format!(
            "%%{{init: {{theme: 'base', themeVariables: {{ {} }}}}}}%%\nflowchart TD\nA-->B",
            keys.join(", ")
        );
        let metadata = engine
            .parse_metadata_sync(&source.replace("COLOR", "#204080"))
            .unwrap();
        assert_eq!(metadata.effective_config.get_str("theme"), Some("base"));
        assert_eq!(
            metadata
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#204080")
        );

        let error = engine
            .parse_metadata_sync(&source.replace("COLOR", "not-a-color"))
            .unwrap_err();
        assert!(
            matches!(
                error,
                Error::ThemeColor(crate::theme_color::ColorError::UnsupportedFormat { ref input })
                    if input == "not-a-color"
            ),
            "{error}"
        );
    }
}

#[test]
fn comments_consume_preceding_blank_lines_but_preserve_following_blank_lines() {
    for (input, expected) in [
        ("first\n%% one\n\n%% two\nlast", "first\nlast"),
        ("first\n\n \t\n%% one\nlast", "first\nlast"),
        ("first\n%% one\n\nlast", "first\n\nlast"),
        ("first\n\nlast", "first\n\nlast"),
        ("first\n%% one\n \t\n%% two\n\nlast", "first\n\nlast"),
        ("first\n\n%% one", "first\n"),
        ("first %% inline\n\nlast", "first %% inline\n\nlast"),
        ("first\n\n%%\nlast", "first\n\n%%\nlast"),
        ("first\n\n%%", "first\n\n%%"),
        ("first\n\n%% \nlast", "first\nlast"),
        (
            "first\n\n%%{init: {}}%%\nlast",
            "first\n\n%%{init: {}}%%\nlast",
        ),
    ] {
        let mut source = PreprocessedSource::new(input);
        remove_mermaid_comments_controlled(
            &mut source,
            #[cfg(feature = "editor-facts")]
            false,
            &OperationControl::new(),
        )
        .expect("active operation");
        trim_source_start_controlled(&mut source, &OperationControl::new())
            .expect("active operation");
        assert_eq!(source.text(), expected, "{input:?}");
    }
}

#[test]
fn comments_follow_ecmascript_whitespace_and_multiline_anchors() {
    for (input, expected) in [
        ("\u{feff}\n \t%% comment\nfirst", "first"),
        ("\u{feff}first", "first"),
        ("\u{0085}first", "\u{0085}first"),
        (
            "first\n\u{0085}%% comment\nlast",
            "first\n\u{0085}%% comment\nlast",
        ),
        ("first\n\u{feff} \n\u{3000}%% comment\nlast", "first\nlast"),
        ("first\u{2028}%% comment\nlast", "first\u{2028}last"),
        ("first\u{2029}%% comment\nlast", "first\u{2029}last"),
        ("first\n%% comment\u{2028}last\nend", "first\nend"),
    ] {
        let mut source = PreprocessedSource::new(input);
        remove_mermaid_comments_controlled(
            &mut source,
            #[cfg(feature = "editor-facts")]
            false,
            &OperationControl::new(),
        )
        .expect("active operation");
        trim_source_start_controlled(&mut source, &OperationControl::new())
            .expect("active operation");
        assert_eq!(source.text(), expected, "{input:?}");
    }
}

#[test]
fn preprocessing_keeps_unicode_spans_across_scan_chunks() {
    let prefix = " \u{3000}\nflowchart TD\nA[\"";
    let padding = "x".repeat(CONTROLLED_SCAN_CHECKPOINT_BYTES - prefix.len() - 1);
    for (suffix, expected_suffix) in [
        ("雪\"];\n", "雪\"];\n"),
        (
            "雪<br title=\"call\">#alpha;\"]\r\n%% trailing comment\n",
            "雪<br title='call'>ﬂ°alpha¶ß\"]\n",
        ),
    ] {
        let input = format!("{prefix}{padding}{suffix}");
        let result = preprocess_builtin_semantic_controlled(
            &input,
            Some("flowchart-v2"),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            result.code(),
            format!("flowchart TD\nA[\"{padding}{expected_suffix}")
        );
        let start = result.code().find('雪').unwrap();
        let original = input.find('雪').unwrap();
        let len = '雪'.len_utf8();
        assert_eq!(
            result
                .source
                .try_map_span(crate::SourceSpan::new(start, start + len)),
            Some(crate::SourceSpan::new(original, original + len)),
        );
    }

    let source = "x".repeat(CONTROLLED_SCAN_CHECKPOINT_BYTES * 4);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    assert!(PreprocessMarkers::scan(&source, &control).is_err());
}

#[test]
fn comment_runs_preserve_original_utf8_spans_after_source_cleanup() {
    let input = concat!(
        "\u{feff}---\r\ntitle: Comments\r\n---\r\n",
        "%%{init: {flowchart: {htmlLabels: false}}}%%\r\n",
        "flowchart TD\r\nA[\"`first\r\n%% comment\r\n\r\n%% comment\r\nlast 雪`\"]\r\n",
    );
    for known_type in [None, Some("flowchart-v2")] {
        let result =
            preprocess_builtin_semantic_controlled(input, known_type, &OperationControl::new())
                .expect("active operation")
                .expect("source preprocessing");
        assert_eq!(result.title.as_deref(), Some("Comments"));
        assert_eq!(result.config.as_value()["flowchart"]["htmlLabels"], false);
        assert_eq!(result.code(), "flowchart TD\nA[\"`first\nlast 雪`\"]\n");
        for value in ["first", "last 雪"] {
            let start = result.code().find(value).expect("retained label text");
            let original_start = input.find(value).expect("original label text");
            assert_eq!(
                result
                    .source
                    .try_map_span(crate::SourceSpan::new(start, start + value.len())),
                Some(crate::SourceSpan::new(
                    original_start,
                    original_start + value.len(),
                )),
            );
        }
        let first = result.code().find("first").unwrap();
        let last = result.code().find("last 雪").unwrap() + "last 雪".len();
        assert_eq!(
            result
                .source
                .try_map_span(crate::SourceSpan::new(first, last)),
            None,
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn comment_evidence_excludes_consumed_blank_lines() {
    let input = "first\n \t\n  %% 雪\n\n\u{feff}%% last\nlast";
    let mut source = PreprocessedSource::new(input);
    remove_mermaid_comments_controlled(&mut source, true, &OperationControl::new())
        .expect("active operation");
    assert_eq!(source.text(), "first\nlast");
    let comments = source
        .global_expected_syntax()
        .iter()
        .map(|expected| &input[expected.span.start..expected.span.end])
        .collect::<Vec<_>>();
    assert_eq!(comments, ["  %% 雪", "\u{feff}%% last"]);
}

#[test]
fn preprocess_keeps_entity_encoding_and_source_maps_on_both_parser_buffers() {
    for (header, diagram_type) in [
        ("agentflow-beta", "agentflow"),
        ("eventmodeling", "eventmodeling"),
    ] {
        let input = format!(
            "---\r\ntitle: Encoding\r\n---\r\n%% comment #35;\r\n{header}\r\nA[\"<span title=\"😀\">#35;</span>\"]\r\n"
        );
        for known_type in [None, Some(diagram_type)] {
            let control = OperationControl::new();
            let result = preprocess_builtin_semantic_controlled(&input, known_type, &control)
                .expect("active operation")
                .expect("built-in preprocessing");
            assert_eq!(result.title.as_deref(), Some("Encoding"));
            assert!(!result.code().contains("%% comment"));
            assert!(result.code().contains("title='😀'>ﬂ°°35¶ß</span>"));
            let with_comments = result.with_comments.as_ref().expect("family parser input");
            assert!(with_comments.text().contains("%% comment #35;"));
            assert!(with_comments.text().contains("title='😀'>#35;</span>"));

            let selected = select_family_parser_source_controlled(
                result.source,
                result.with_comments,
                diagram_type,
                &MermaidConfig::empty_object(),
                &control,
            )
            .expect("active operation");
            assert!(selected.text().contains("%% comment ﬂ°°35¶ß"));
            assert!(selected.text().contains("title='😀'>ﬂ°°35¶ß</span>"));
            let emoji = selected.text().find('😀').expect("attribute value");
            let mapped = selected
                .try_map_span(crate::SourceSpan::new(emoji, emoji + '😀'.len_utf8()))
                .expect("original attribute span");
            assert_eq!(&input[mapped.start..mapped.end], "😀");
        }
    }
}

#[test]
fn builtin_preprocess_preserves_family_comments_without_a_full_registry() {
    for (header, diagram_type) in [
        ("agentflow-beta", "agentflow"),
        ("eventmodeling", "eventmodeling"),
    ] {
        let input = format!("{header}\n%% family comment\nNode");
        for known_type in [None, Some(diagram_type)] {
            let result = preprocess_builtin_semantic_controlled(
                &input,
                known_type,
                &OperationControl::new(),
            )
            .expect("active operation")
            .expect("built-in preprocessing");
            assert!(!result.code().contains("%% family comment"));
            assert!(
                result
                    .with_comments
                    .as_ref()
                    .expect("family parser input")
                    .text()
                    .contains("%% family comment")
            );
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn captured_detection_panic_does_not_swallow_cancellation() {
    let control = OperationControl::new();
    let detector = |_text: &str, control: &OperationControl| {
        control.cancel();
        panic!("detector cancelled before panicking");
    };
    let result = preprocess_source_capture_controlled(
        "%%{init: {config: {htmlLabels: false}}}%%\nflowchart TD\nA-->B",
        &detector,
        None,
        DirectiveRecoveryMode::Strict,
        SourceConfigCaptureMode::Collect,
        &control,
    );
    assert!(matches!(result, Err(crate::OperationCancelled { .. })));
}

fn transformed(input: &str, transform: fn(&mut PreprocessedSource)) -> String {
    let mut source = PreprocessedSource::new(input);
    transform(&mut source);
    source.into_text()
}

#[test]
fn frontmatter_depth_ignores_brackets_in_yaml_comments_and_block_scalars() {
    let brackets = "[".repeat(MAX_CONFIG_NESTING_DEPTH + 1);
    for (body, expected_css) in [
        (format!("# {brackets}\nconfig:\n  theme: dark\n"), None),
        (
            format!("config:\n  theme: dark\n  themeCSS: |-\n    {brackets}\n"),
            Some(brackets.as_str()),
        ),
    ] {
        let input = format!("---\n{body}---\nerror\n");
        let result =
            preprocess_builtin_semantic_controlled(&input, Some("error"), &OperationControl::new())
                .expect("active operation")
                .expect("brackets in comments and scalars do not add YAML collections");

        assert_eq!(result.config.get_str("theme"), Some("dark"));
        assert_eq!(result.config.get_str("themeCSS"), expected_css);
        assert_eq!(result.code(), "error\n");
    }
}

#[test]
fn frontmatter_depth_rejects_actual_flow_and_block_collections() {
    let depth = MAX_CONFIG_NESTING_DEPTH + 1;
    for body in [
        format!(
            "config:\n  values: {}0{}\n",
            "[".repeat(depth),
            "]".repeat(depth)
        ),
        format!("config:\n  values:\n    {}0\n", "- ".repeat(depth)),
    ] {
        let input = format!("---\n{body}---\nerror\n");
        let error =
            preprocess_builtin_semantic_controlled(&input, Some("error"), &OperationControl::new())
                .expect("active operation")
                .expect_err("real collection depth remains bounded by the YAML parser");
        let Error::InvalidFrontMatterYaml { message } = error else {
            panic!("unexpected frontmatter error: {error}");
        };
        assert!(
            message.contains("recursion limit exceeded")
                || message.contains("config nesting exceeds"),
            "unexpected YAML error: {message}"
        );
    }
}

#[test]
fn controlled_frontmatter_apis_preserve_existing_block_semantics() {
    for (source, expected_block) in [
        ("---\ntitle: Demo\n---\nflowchart TD\n", true),
        (" \t--- \r\n \ttitle: Demo\r\n \t---\r\nflowchart TD", true),
        (
            "\u{2003}---\n\u{2003}title: Demo\n\u{2003}---\nflowchart TD",
            true,
        ),
        ("---\r\r\ntitle: Demo\r\n---\r\r\nflowchart TD", true),
        ("---\rtitle: Demo\r---\rflowchart TD", true),
        ("---\ntitle: Demo\n--- \u{2003}\nflowchart TD", true),
        ("---\ntitle: Demo\n---", false),
        ("--- trailing\ntitle: Demo\n---\nflowchart TD", false),
        ("---\ntitle: Demo\n \t---\nflowchart TD", false),
        ("---", false),
    ] {
        let control = OperationControl::new();
        let controlled = split_frontmatter_block_controlled(source, &control)
            .expect("an active parse control must not cancel");
        let wrapped = split_frontmatter_block(source);

        assert_eq!(controlled.is_some(), expected_block, "source: {source:?}");
        assert_eq!(
            controlled.is_some(),
            wrapped.is_some(),
            "source: {source:?}"
        );
        if let (Some(controlled), Some(wrapped)) = (controlled, wrapped) {
            assert_eq!(controlled.full, wrapped.full, "source: {source:?}");
            assert_eq!(controlled.body, wrapped.body, "source: {source:?}");
            assert_eq!(controlled.indent, wrapped.indent, "source: {source:?}");
            assert_eq!(
                controlled.dedented_body, wrapped.dedented_body,
                "source: {source:?}"
            );
            assert_eq!(controlled.stripped, wrapped.stripped, "source: {source:?}");

            let location = locate_frontmatter_block_controlled(source, &OperationControl::new())
                .expect("an active parse control must not cancel")
                .expect("a split block must have a location");
            assert_eq!(location.full, controlled.full, "source: {source:?}");
            assert_eq!(location.body, controlled.body, "source: {source:?}");
            assert_eq!(location.indent, controlled.indent, "source: {source:?}");
            assert_eq!(location.stripped, controlled.stripped, "source: {source:?}");
            assert_eq!(&source[location.full.end..], controlled.stripped);
        }
    }
}

#[test]
fn frontmatter_matches_upstream_greedy_whitespace_and_lazy_fences() {
    for (source, indent, body, stripped) in [
        ("---\n\n---\n\nMORE\n---\n", "", "---\n\nMORE", ""),
        ("---\n\n\n---\ngraph TD\n", "", "", "graph TD\n"),
        (
            "---\ntitle: x\n---\n \t\n  graph TD\n",
            "",
            "title: x",
            "  graph TD\n",
        ),
        (
            "---\r\r\ntitle: Demo\r\n---\r\r\nflowchart TD",
            "",
            "title: Demo\r",
            "flowchart TD",
        ),
        (
            "\u{feff}---\n\u{feff}title: x\n\u{feff}---\ngraph TD",
            "\u{feff}",
            "\u{feff}title: x",
            "graph TD",
        ),
        (
            "---\ntitle: |\n  ---\n  still the body\n---\ngraph TD",
            "",
            "title: |\n  ---\n  still the body",
            "graph TD",
        ),
    ] {
        let block = locate_frontmatter_block_controlled(source, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(block.indent, indent, "{source:?}");
        assert_eq!(
            &source[block.body.start..block.body.end],
            body,
            "{source:?}"
        );
        assert_eq!(block.stripped, stripped, "{source:?}");
        assert_eq!(block.full.end, source.len() - stripped.len());
    }
    for source in [
        "---\n---\n",
        "---\ntitle: x\n---",
        "---\ntitle: x\n---   ",
        "\u{0085}---\ntitle: x\n---\ngraph TD",
        "----\ntitle: x\n----\ngraph TD",
    ] {
        assert!(
            locate_frontmatter_block_controlled(source, &OperationControl::new())
                .unwrap()
                .is_none(),
            "{source:?}"
        );
    }
}

#[test]
fn frontmatter_whitespace_only_input_is_scanned_and_cancellable() {
    let source = format!("---\n{}", " \n".repeat(16_000));
    assert!(
        locate_frontmatter_block_controlled(&source, &OperationControl::new())
            .unwrap()
            .is_none()
    );
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    assert!(locate_frontmatter_block_controlled(&source, &control).is_err());
}

#[test]
fn controlled_frontmatter_split_preserves_indented_body_bytes() {
    let source = "  ---\r\n  title: Demo\r\n  config:\r\n    theme: dark\r\n  ---\r\nflowchart TD";
    let block = split_frontmatter_block_controlled(source, &OperationControl::new())
        .expect("an active parse control must not cancel")
        .expect("frontmatter block");

    assert_eq!(block.indent, "  ");
    assert_eq!(
        &source[block.body.start..block.body.end],
        "  title: Demo\r\n  config:\r\n    theme: dark\r"
    );
    assert_eq!(
        block.dedented_body,
        "title: Demo\r\nconfig:\r\n  theme: dark\r"
    );
    assert_eq!(block.stripped, "flowchart TD");
}

#[test]
fn controlled_frontmatter_split_preserves_bare_cr_body_bytes() {
    let source = "  ---\r  title: Demo\r  config:\r    theme: dark\r  ---\rflowchart TD";
    let block = split_frontmatter_block_controlled(source, &OperationControl::new())
        .expect("an active parse control must not cancel")
        .expect("frontmatter block");

    assert_eq!(block.indent, "  ");
    assert_eq!(
        &source[block.body.start..block.body.end],
        "  title: Demo\r  config:\r    theme: dark"
    );
    assert_eq!(block.dedented_body, "title: Demo\rconfig:\r  theme: dark");
    assert_eq!(block.stripped, "flowchart TD");
}

#[test]
fn controlled_frontmatter_split_preserves_long_opening_body_and_indent_semantics() {
    let indent = " ".repeat(8 * 1024);
    let value = "x".repeat(8 * 1024);
    let indented_body = format!("{indent}title: {value}");
    let source = format!("{indent}---\n{indented_body}\n{indent}---\nflowchart TD");
    let block = split_frontmatter_block_controlled(&source, &OperationControl::new())
        .expect("an active parse control must not cancel")
        .expect("frontmatter block");

    let dedented_body = format!("title: {value}");
    assert_eq!(block.indent, indent.as_str());
    assert_eq!(
        &source[block.body.start..block.body.end],
        indented_body.as_str()
    );
    assert_eq!(block.dedented_body.as_ref(), dedented_body.as_str());
    assert_eq!(block.stripped, "flowchart TD");
}

#[test]
fn frontmatter_location_cancels_deterministically_on_a_long_opening_line() {
    let indent = " ".repeat(16 * 1024);
    let source = format!("{indent}---\n{indent}---\nflowchart TD");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(locate_frontmatter_block_controlled(&source, &control).is_err());
}

#[test]
fn frontmatter_location_cancels_deterministically_on_a_long_body() {
    let source = format!("---\n{}\n---\nflowchart TD", "x".repeat(16 * 1024));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(locate_frontmatter_block_controlled(&source, &control).is_err());
}

#[test]
fn frontmatter_location_cancels_deterministically_on_a_long_closing_line() {
    let source = format!(
        "---\ntitle: Demo\n---{}\nflowchart TD",
        " ".repeat(16 * 1024)
    );
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(locate_frontmatter_block_controlled(&source, &control).is_err());
}

#[test]
fn frontmatter_dedent_cancels_deterministically_on_a_long_indented_body() {
    let body = format!("  {}", "x".repeat(16 * 1024));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(dedent_frontmatter_body_controlled(&body, "  ", &control).is_err());
}

#[test]
fn controlled_frontmatter_yaml_fields_match_the_legacy_wrapper() {
    let yaml = "title: Demo\nconfig:\n  theme: dark\n";
    let controlled = parse_frontmatter_yaml_fields_controlled(yaml, &OperationControl::new())
        .expect("an active parse control must not cancel")
        .expect("valid frontmatter YAML");

    assert_eq!(controlled, parse_frontmatter_yaml_fields(yaml).unwrap());

    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert!(parse_frontmatter_yaml_fields_controlled(yaml, &cancelled).is_err());
}

#[test]
fn bounded_frontmatter_yaml_fields_honor_caller_materialization_limits() {
    let result = parse_frontmatter_yaml_fields_bounded_controlled(
        "values: [one, two, three, four]\n",
        1024,
        16,
        8,
        &OperationControl::new(),
    )
    .expect("an active parse control must not cancel");

    let error = result.expect_err("the caller budget must reject materialization");
    assert!(error.contains("safe materialization budget"));
}

#[cfg(feature = "editor-facts")]
fn capture_source_config(input: &str) -> PreprocessCaptureOutcome {
    let detector = |text: &str, control: &OperationControl| {
        Ok(crate::detect::detect_builtin_controlled(text, control)?.map(|fact| fact.id))
    };
    preprocess_source_capture_controlled(
        input,
        &detector,
        Some("flowchart-v2"),
        DirectiveRecoveryMode::RecoverLine,
        SourceConfigCaptureMode::Collect,
        &OperationControl::new(),
    )
    .expect("an active parse control must not cancel")
}

#[cfg(feature = "editor-facts")]
#[test]
fn source_config_evidence_maps_crlf_unicode_and_indentation_to_original_source() {
    let source = concat!(
        "\u{feff}  ---\r\n",
        "  title: 图\r\n",
        "  config:\r\n",
        "    flowchart:\r\n",
        "      \"htmlLabels\": false\r\n",
        "  ---\r\n",
        "%%{ initialize: { flowchart: { 'htmlLabels': false } } }%%\r\n",
        "flowchart TD\r\n",
        "A-->B\r\n",
    );
    let captured = capture_source_config(source);
    assert!(matches!(
        &captured.outcome,
        PreprocessCaptureResult::Ready(_)
    ));
    let evidence = captured.source_config;

    let frontmatter = evidence.frontmatter().expect("frontmatter evidence");
    assert_eq!(frontmatter.indent(), "  ");
    assert!(
        source[frontmatter.full_span().start..frontmatter.full_span().end].starts_with("  ---\r\n")
    );
    let directives = evidence.directives();
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].order(), 0);
    assert_eq!(directives[0].keyword(), "initialize");
    let keyword = directives[0].keyword_span();
    assert_eq!(&source[keyword.start..keyword.end], "initialize");

    let frontmatter_key = evidence
        .keys()
        .iter()
        .find(|key| {
            key.origin() == SourceConfigOrigin::Frontmatter
                && key.matches_path(&["config", "flowchart", "htmlLabels"])
        })
        .expect("frontmatter htmlLabels evidence");
    assert_eq!(
        frontmatter_key.path_segments().collect::<Vec<_>>(),
        ["config", "flowchart", "htmlLabels"]
    );
    assert_eq!(
        &source[frontmatter_key.span().start..frontmatter_key.span().end],
        "htmlLabels"
    );
    let directive_key = evidence
        .keys()
        .iter()
        .find(|key| {
            key.origin() == SourceConfigOrigin::Directive { directive_index: 0 }
                && key.matches_path(&["flowchart", "htmlLabels"])
        })
        .expect("directive htmlLabels evidence");
    assert_eq!(
        &source[directive_key.span().start..directive_key.span().end],
        "htmlLabels"
    );
    assert!(frontmatter_key.order() < directive_key.order());
    assert!(evidence.rewrite_safe());
}

#[cfg(feature = "editor-facts")]
#[test]
fn json5_evidence_omits_escaped_and_array_nested_keys_without_disabling_rewrite() {
    let source = concat!(
        r#"%%{init: { flowchart: { "html\u004cabels": false }, values: [{ htmlLabels: false }] }}%%"#,
        "\nflowchart TD\nA-->B\n",
    );
    let captured = capture_source_config(source);
    assert!(matches!(
        &captured.outcome,
        PreprocessCaptureResult::Ready(_)
    ));
    let evidence = captured.source_config;

    assert!(evidence.rewrite_safe());
    assert!(
        evidence
            .keys()
            .iter()
            .any(|key| key.matches_path(&["flowchart"]))
    );
    assert!(!evidence.keys().iter().any(|key| {
        key.matches_path(&["flowchart", "htmlLabels"])
            || key.matches_path(&["values", "htmlLabels"])
    }));
}

#[cfg(feature = "editor-facts")]
#[test]
fn source_config_evidence_retains_first_yaml_error_and_later_directive_facts() {
    let source = concat!(
        "---\n",
        "config:\n",
        "  flowchart:\n",
        "    htmlLabels: false\n",
        "  broken: [\n",
        "---\n",
        "%%{ initialize: { lazyLoadedDiagrams: true } }%%\n",
        "flowchart TD\nA-->B\n",
    );
    let captured = capture_source_config(source);
    assert!(matches!(
        &captured.outcome,
        PreprocessCaptureResult::Failed(Error::InvalidFrontMatterYaml { .. })
    ));
    assert!(!captured.source_config.rewrite_safe());
    assert!(captured.source_config.keys().iter().any(|key| {
        key.origin() == SourceConfigOrigin::Frontmatter
            && key.matches_path(&["config", "flowchart", "htmlLabels"])
    }));
    let directive = captured
        .source_config
        .directives()
        .iter()
        .find(|directive| directive.keyword() == "initialize")
        .expect("later directive evidence survives the YAML error");
    let span = directive.keyword_span();
    assert_eq!(&source[span.start..span.end], "initialize");
    assert!(captured.source_config.keys().iter().any(|key| {
        matches!(key.origin(), SourceConfigOrigin::Directive { .. })
            && key.matches_path(&["lazyLoadedDiagrams"])
    }));
}

#[cfg(feature = "editor-facts")]
#[test]
fn source_config_evidence_retains_later_directives_after_a_directive_budget_error() {
    let nested = format!(
        "{}true{}",
        "{".repeat(MAX_CONFIG_NESTING_DEPTH + 1),
        "}".repeat(MAX_CONFIG_NESTING_DEPTH + 1)
    );
    let source = format!(
        "%%{{init: {nested}}}%%\n%%{{initialize: {{ theme: 'dark' }} }}%%\nflowchart TD\nA-->B\n"
    );
    let captured = capture_source_config(&source);
    assert!(matches!(
        &captured.outcome,
        PreprocessCaptureResult::Failed(Error::InvalidDirectiveJson { .. })
    ));
    assert_eq!(captured.source_config.directives().len(), 2);
    assert_eq!(captured.source_config.directives()[0].keyword(), "init");
    assert_eq!(
        captured.source_config.directives()[1].keyword(),
        "initialize"
    );
    assert!(!captured.source_config.rewrite_safe());
}

#[cfg(feature = "editor-facts")]
#[test]
fn incomplete_directive_capture_stops_after_the_keyword() {
    let nested = format!(
        "{}true{}",
        "{".repeat(MAX_CONFIG_NESTING_DEPTH + 1),
        "}".repeat(MAX_CONFIG_NESTING_DEPTH + 1)
    );
    let source = format!("%%{{ initialize: {nested}\nflowchart TD\nA-->B\n");
    let captured = capture_source_config(&source);

    assert!(matches!(
        &captured.outcome,
        PreprocessCaptureResult::Ready(_)
    ));
    let directive = &captured.source_config.directives()[0];
    assert_eq!(directive.keyword(), "initialize");
    assert!(!directive.complete());
    assert!(captured.source_config.keys().is_empty());
    assert!(!captured.source_config.rewrite_safe());
}

#[cfg(feature = "editor-facts")]
#[test]
fn source_config_evidence_weight_does_not_scale_with_unrelated_diagram_body() {
    let prefix = "%%{init: { theme: 'dark' }}%%\nflowchart TD\n";
    let small = capture_source_config(&format!("{prefix}A-->B\n")).source_config;
    let large_source = format!("{prefix}{}", "A-->B\n".repeat(16_384));
    let large = capture_source_config(&large_source).source_config;

    assert_eq!(small, large);
    assert_eq!(
        small.estimated_owned_heap_bytes(),
        large.estimated_owned_heap_bytes()
    );
    assert!(large.estimated_owned_heap_bytes() < large_source.len() / 100);
}

#[cfg(feature = "editor-facts")]
#[test]
fn source_config_evidence_paths_remain_linear_for_deep_wide_json5() {
    let mut config = String::new();
    for depth in 0..64 {
        config.push_str(&format!("level{depth}:{{"));
    }
    config.push_str(&"repeated:0,".repeat(2_048));
    config.push_str("tail:1");
    config.push_str(&"}".repeat(64));
    let source = format!("%%{{init: {{{config}}}}}%%\nflowchart TD\nA-->B\n");

    let captured = capture_source_config(&source);
    assert!(matches!(
        &captured.outcome,
        PreprocessCaptureResult::Ready(_)
    ));
    assert!(captured.source_config.keys().len() > 2_048);
    assert!(
        captured.source_config.estimated_owned_heap_bytes() < source.len().saturating_mul(64),
        "shared path prefixes must keep retained evidence proportional to source bytes"
    );
}

#[cfg(feature = "editor-facts")]
#[test]
fn source_config_evidence_drops_unneeded_frontmatter_field_values() {
    let small =
        capture_source_config("---\nnotes: small\n---\nflowchart TD\nA-->B\n").source_config;
    let large = capture_source_config(&format!(
        "---\nnotes: \"{}\"\n---\nflowchart TD\nA-->B\n",
        "x".repeat(256 * 1024)
    ))
    .source_config;

    assert!(small.frontmatter().expect("frontmatter").fields().is_none());
    assert!(large.frontmatter().expect("frontmatter").fields().is_none());
    assert_eq!(
        small.estimated_owned_heap_bytes(),
        large.estimated_owned_heap_bytes()
    );
}

#[cfg(feature = "editor-facts")]
#[test]
fn frontmatter_rewrite_safety_distinguishes_insertion_from_materialization() {
    let commented_without_config = capture_source_config(concat!(
        "---\n",
        "# keep this comment\n",
        "title: Demo\n",
        "---\n",
        "%%{init: { theme: 'dark' }}%%\n",
        "flowchart TD\nA-->B\n",
    ))
    .source_config;
    assert!(commented_without_config.rewrite_safe());
    assert!(
        !commented_without_config
            .frontmatter()
            .expect("frontmatter")
            .rewrite_safe()
    );

    let commented_with_config = capture_source_config(concat!(
        "---\n",
        "# keep this comment\n",
        "config:\n",
        "  theme: default\n",
        "---\n",
        "%%{init: { theme: 'dark' }}%%\n",
        "flowchart TD\nA-->B\n",
    ))
    .source_config;
    assert!(!commented_with_config.rewrite_safe());
    assert!(
        commented_with_config
            .frontmatter()
            .expect("frontmatter")
            .fields()
            .is_none()
    );
}

#[test]
fn normalize_crlf_matches_mermaid_line_ending_cleanup() {
    assert_eq!(
        transformed("flowchart TD\r\nA-->B\rC-->D\n", normalize_crlf),
        "flowchart TD\nA-->B\nC-->D\n"
    );
    assert_eq!(transformed("\r\r\n\n", normalize_crlf), "\n\n\n");
}

#[test]
fn normalize_html_tag_attributes_matches_mermaid_cleanup_shape() {
    assert_eq!(
        transformed(
            r#"<span title="A" data-empty="">Label</span><br disabled="yes">"#,
            normalize_html_tag_attributes_like_upstream,
        ),
        r#"<span title='A' data-empty=''>Label</span><br disabled='yes'>"#
    );
    assert_eq!(
        transformed(
            r#"<é title="A"><_x value="B"><1 n="C">"#,
            normalize_html_tag_attributes_like_upstream,
        ),
        r#"<é title="A"><_x value='B'><1 n='C'>"#
    );
    assert_eq!(
        transformed(
            r#"<span a="x" title="A>B">"#,
            normalize_html_tag_attributes_like_upstream,
        ),
        r#"<span a='x' title="A>B">"#
    );
    assert_eq!(
        transformed(
            r#"<<span title="A">"#,
            normalize_html_tag_attributes_like_upstream,
        ),
        r#"<<span title='A'>"#
    );
}

#[test]
fn normalize_html_attribute_quotes_keep_exact_unicode_source_spans() {
    let original = r#"flowchart TD
A["<span title="😀">Label</span>"]
"#;
    let mut source = PreprocessedSource::new(original);
    normalize_html_tag_attributes_like_upstream(&mut source);

    assert!(source.text().contains("title='😀'"));
    let emoji = source.text().find('😀').unwrap();
    let mapped = source
        .try_map_span(crate::SourceSpan::new(emoji, emoji + '😀'.len_utf8()))
        .expect("normalized attribute value span");
    assert_eq!(&original[mapped.start..mapped.end], "😀");
}

#[test]
fn encode_entity_placeholders_matches_mermaid_ascii_word_shape() {
    assert_eq!(
        transformed(
            "Hello #there; #andHere;#77653;",
            encode_mermaid_entities_like_upstream,
        ),
        "Hello ﬂ°there¶ß ﬂ°andHere¶ßﬂ°°77653¶ß"
    );
    assert_eq!(
        transformed(
            "style this; is ; everything :something#not-nothing; and this too;",
            encode_mermaid_entities_like_upstream,
        ),
        "style this; is ; everything :something#not-nothing; and this too"
    );
    assert_eq!(
        transformed(
            "classDef this; is ; everything :something#not-nothing; and this too;",
            encode_mermaid_entities_like_upstream,
        ),
        "classDef this; is ; everything :something#not-nothing; and this too"
    );
    assert_eq!(
        transformed(
            "style a fill:#fff; style b fill:#000;",
            encode_mermaid_entities_like_upstream,
        ),
        "style a fill:ﬂ°fff¶ß style b fill:#000"
    );
    assert_eq!(
        transformed("style a fill: #fff;", encode_mermaid_entities_like_upstream,),
        "style a fill: ﬂ°fff¶ß"
    );
    assert_eq!(
        transformed(
            "#é; #+123; #has-dash;",
            encode_mermaid_entities_like_upstream,
        ),
        "#é; #+123; #has-dash;"
    );
}

#[test]
fn sanitize_directive_handles_deep_values_with_small_stack() {
    const DEPTH: usize = 2_048;
    let mut value = deep_directive_value(DEPTH, Value::String("<blocked>".to_string()));

    let handle = std::thread::Builder::new()
        .name("preprocess-deep-directive-sanitize".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            sanitize_directive(&mut value);
            let leaf = deep_directive_leaf(&value, DEPTH)
                .and_then(Value::as_str)
                .map(str::to_owned);
            crate::json_value::drop_value_nonrecursive(value);
            assert_eq!(leaf.as_deref(), Some("<blocked>"));
        })
        .expect("spawn deep directive sanitizer test");
    handle
        .join()
        .expect("deep directive sanitizer should finish without stack overflow");
}

#[test]
fn sanitize_directive_replaces_unbalanced_css_like_mermaid() {
    let mut value = json!({
        "themeCSS": "} * { background: red }",
        "flowchart": {
            "fontFamily": "valid { nested: value; }",
            "altFontFamily": "missing { close"
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(
        value["themeCSS"],
        Value::String("{ /* ERROR: Unbalanced CSS */ }".to_string())
    );
    assert_eq!(value["flowchart"]["fontFamily"], "valid { nested: value; }");
    assert!(value["flowchart"].get("altFontFamily").is_none());
}

#[test]
fn sanitize_directive_leaves_theme_values_for_shared_source_admission() {
    let mut value = json!({
        "gitGraph": {"mainBranchName": "host<br>override"},
        "themeVariables": {
            "primaryColor": "#123456",
            "secondaryColor": "rgb(1, 2, 3)",
            "tertiaryColor": "url(javascript:alert(1))",
            "noteBkgColor": "hsl(120, 50%, 25.5%)",
            "noteTextColor": "red-blue"
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(value["gitGraph"]["mainBranchName"], "host<br>override");
    assert_eq!(value["themeVariables"]["primaryColor"], "#123456");
    assert_eq!(value["themeVariables"]["secondaryColor"], "rgb(1, 2, 3)");
    assert_eq!(
        value["themeVariables"]["tertiaryColor"],
        "url(javascript:alert(1))"
    );
    assert_eq!(
        value["themeVariables"]["noteBkgColor"],
        "hsl(120, 50%, 25.5%)"
    );
    assert_eq!(value["themeVariables"]["noteTextColor"], "red-blue");
}

#[test]
fn sanitize_directive_uses_generated_config_shape_for_all_value_kinds() {
    let mut value = json!({
        "notAConfigKey": "removed",
        "theme": null,
        "prototype": "removed",
        "constructor": "removed",
        "deterministicIDSeed": "accepted undefined key",
        "sequence": {
            "messageFont": "accepted function key",
            "unknownNestedKey": true
        },
        "secure": ["theme"],
        "flowchart": {
            "secure": ["htmlLabels"],
            "htmlLabels": false
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(
        value,
        json!({
            "deterministicIDSeed": "accepted undefined key",
            "sequence": {
                "messageFont": "accepted function key"
            },
            "flowchart": {
                "htmlLabels": false
            }
        })
    );
}

#[test]
fn sanitize_directive_preserves_valid_dictionary_entries() {
    let mut value = json!({
        "sankey": {
            "nodeColors": {
                "shortHex": "#abc",
                "alphaHex": "#12345678",
                "rgb": "rgb(0, 10%, 255)",
                "hsl": "hsl(120, 50%, 25.5%)",
                "named": "rebeccapurple"
            }
        },
        "treeView": {
            "filenameIcons": {
                "Makefile": "cmake",
                "README.md": "fa:bell"
            },
            "extensionIcons": {
                ".ts": "logos:typescript-icon",
                ".txt": "none"
            }
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(value["sankey"]["nodeColors"]["shortHex"], "#abc");
    assert_eq!(value["sankey"]["nodeColors"]["alphaHex"], "#12345678");
    assert_eq!(value["sankey"]["nodeColors"]["rgb"], "rgb(0, 10%, 255)");
    assert_eq!(value["sankey"]["nodeColors"]["hsl"], "hsl(120, 50%, 25.5%)");
    assert_eq!(value["sankey"]["nodeColors"]["named"], "rebeccapurple");
    assert_eq!(value["treeView"]["filenameIcons"]["README.md"], "fa:bell");
    assert_eq!(
        value["treeView"]["extensionIcons"][".ts"],
        "logos:typescript-icon"
    );
}

#[test]
fn sanitize_directive_removes_invalid_dictionary_values() {
    let mut value = json!({
        "sankey": {
            "nodeColors": {
                "valid": "#ff0000",
                "short": "#12",
                "function": "url(javascript:alert(1))",
                "wrongType": 42
            }
        },
        "treeView": {
            "filenameIcons": {
                "valid": "docker",
                "markup": "<script>alert(1)</script>",
                "wrongType": false
            },
            "extensionIcons": {
                ".ts": "logos:typescript-icon",
                ".css": "not a valid name",
                ".json": "one:two:three"
            }
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(value["sankey"]["nodeColors"], json!({ "valid": "#ff0000" }));
    assert_eq!(
        value["treeView"]["filenameIcons"],
        json!({ "valid": "docker" })
    );
    assert_eq!(
        value["treeView"]["extensionIcons"],
        json!({ ".ts": "logos:typescript-icon" })
    );
}

#[test]
fn sanitize_directive_removes_suspicious_dictionary_keys() {
    let mut value = json!({
        "sankey": {
            "nodeColors": {
                "__proto__hack": "red",
                "prototype": "green",
                "constructor.js": "blue",
                "safe": "black"
            }
        },
        "treeView": {
            "filenameIcons": {
                "__proto__hack": "docker",
                "prototype.ts": "docker",
                "constructor.js": "docker",
                "a.ts": "docker"
            }
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(value["sankey"]["nodeColors"], json!({ "safe": "black" }));
    assert_eq!(
        value["treeView"]["filenameIcons"],
        json!({ "a.ts": "docker" })
    );
}

#[test]
fn sanitize_directive_validates_dictionary_arrays_like_javascript_objects() {
    let mut value = json!({
        "sankey": {
            "nodeColors": ["#ff0000", "url(javascript:alert(1))", 42]
        },
        "treeView": {
            "extensionIcons": ["logos:typescript-icon", "not a valid name", false]
        }
    });

    sanitize_directive(&mut value);

    assert_eq!(
        value["sankey"]["nodeColors"],
        json!(["#ff0000", null, null])
    );
    assert_eq!(
        value["treeView"]["extensionIcons"],
        json!(["logos:typescript-icon", null, null])
    );
}

#[test]
fn config_nesting_counts_inline_yaml_sequence_indicators() {
    let yaml = format!(
        "config:\n  {}\"leaf\"",
        "- ".repeat(MAX_CONFIG_NESTING_DEPTH + 1)
    );
    assert!(config_nesting_exceeds_limit(&yaml));
}

#[test]
fn controlled_config_nesting_preserves_existing_depth_semantics() {
    let deep_flow = "[".repeat(MAX_CONFIG_NESTING_DEPTH + 1);
    for (yaml, expected) in [
        ("title: '[not, nesting]'\n", false),
        ("config:\r\n  theme: dark\r\n", false),
        ("# [ignored]\nconfig: { theme: dark }\n", false),
        (deep_flow.as_str(), true),
    ] {
        let controlled = config_nesting_exceeds_limit_controlled(yaml, &OperationControl::new())
            .expect("an active parse control must not cancel");

        assert_eq!(controlled, expected, "yaml: {yaml:?}");
        assert_eq!(controlled, config_nesting_exceeds_limit(yaml));
    }
}

#[test]
fn config_nesting_flow_scan_cancels_deterministically_on_a_long_line() {
    let yaml = format!("title: \"{}\"", "x".repeat(16 * 1024));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(config_nesting_exceeds_limit_controlled(&yaml, &control).is_err());
}

#[test]
fn config_nesting_indent_scan_cancels_deterministically_on_a_long_line() {
    let yaml = format!("{}key: value", " ".repeat(16 * 1024));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    let mut checkpoints = ControlledScanCheckpoints::new(&control).unwrap();

    assert!(max_yaml_indent_depth_controlled(&yaml, &mut checkpoints).is_err());
}

#[test]
fn config_nesting_trim_cancels_deterministically_on_long_whitespace() {
    let line = format!("{}value{}", " ".repeat(16 * 1024), " ".repeat(16 * 1024));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    let mut checkpoints = ControlledScanCheckpoints::new(&control).unwrap();

    assert!(trim_whitespace_bounds_controlled(&line, &mut checkpoints).is_err());
}

fn deep_directive_value(depth: usize, leaf: Value) -> Value {
    let mut value = leaf;
    for _ in 0..depth {
        let mut map = Map::new();
        map.insert("flowchart".to_string(), value);
        value = Value::Object(map);
    }
    value
}

fn deep_directive_leaf(mut value: &Value, depth: usize) -> Option<&Value> {
    for _ in 0..depth {
        value = value.as_object()?.get("flowchart")?;
    }
    Some(value)
}
