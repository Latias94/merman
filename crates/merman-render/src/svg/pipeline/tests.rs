use super::*;
use merman_model::{CancelReason, OperationControl, OperationPhase};

fn render_session() -> RenderSession {
    crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap()
}

#[test]
fn scoped_css_detection_reads_wrapped_passes_without_trusting_them() {
    for css in ["", "   ", ".node { font-family: Fixture; }"] {
        let pipeline = SvgPipeline::parity().with_postprocessor(ScopedCssPostprocessor::new(css));
        assert_eq!(pipeline.has_scoped_css(), !css.trim().is_empty());
        assert!(pipeline.has_custom_postprocessors(Some(crate::DiagramFamilyId::FLOWCHART)));
    }
}

#[test]
fn finalization_report_moves_terminal_owned_allocations() {
    let session = render_session();
    let svg =
        r##"<svg><defs><path id="shape"/></defs><use href="#shape"/><text>label</text></svg>"##;
    let terminal = final_validation::validate_resvg_compatible_svg_with_execution(
        svg,
        SvgPostprocessExecution::new(&session),
    )
    .unwrap();
    let occurrences = terminal.reference_plan.raw_element_occurrences().as_ptr();
    let available_ids = terminal.resource_closure.available_fragment_ids().as_ptr();
    let referenced_ids = terminal.resource_closure.referenced_fragment_ids().as_ptr();
    let available_id = terminal.resource_closure.available_fragment_ids()[0].as_ptr();
    let referenced_id = terminal.resource_closure.referenced_fragment_ids()[0].as_ptr();
    let report = SvgFinalizationReport::from_pipeline(&SvgPipeline::resvg_safe(), terminal);

    assert_eq!(
        report.reference_plan().raw_element_occurrences().as_ptr(),
        occurrences
    );
    assert_eq!(
        report.resource_closure().available_fragment_ids().as_ptr(),
        available_ids
    );
    assert_eq!(
        report.resource_closure().referenced_fragment_ids().as_ptr(),
        referenced_ids
    );
    assert_eq!(
        report.resource_closure().available_fragment_ids()[0].as_ptr(),
        available_id
    );
    assert_eq!(
        report.resource_closure().referenced_fragment_ids()[0].as_ptr(),
        referenced_id
    );
    assert_eq!(report.text_element_count(), 1);
    assert!(!report.font_seal().is_complete());
}

#[test]
fn resvg_seal_reuses_supplied_terminal_validation() {
    let session = render_session();
    let pipeline = SvgPipeline::resvg_safe();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text>label</text></svg>"#.to_owned();
    let metadata =
        SvgPostprocessMetadata::from_svg(&svg).with_family_id(crate::DiagramFamilyId::STATE);

    let (sealed, work) = final_validation::measure_terminal_validation_work(|| {
        let result = pipeline
            .run(
                Cow::Owned(svg),
                &metadata,
                &session,
                None,
                PipelineTerminalValidation::Immediate,
            )
            .unwrap();
        ResvgCompatibleSvg::seal(
            result.svg.into_owned(),
            result.terminal,
            PreparedMathEvidenceLease::default(),
            true,
            &pipeline,
            &session,
        )
        .unwrap()
    });

    assert_eq!(sealed.as_str().len(), work.svg_bytes);
    assert_eq!(
        sealed.reference_plan(),
        sealed.finalization_report().reference_plan()
    );
    assert_eq!(
        sealed.resource_closure(),
        sealed.finalization_report().resource_closure()
    );
    assert_eq!(work.calls, 1);
}

#[test]
fn fallback_css_diagnostics_are_sealed_without_granting_resource_admission() {
    let session = render_session();
    for (marker, expected) in [("true", true), ("false", false)] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g data-merman-fallback-css-residual="{marker}"><path d="M0 0h1v1z"/></g></svg>"#
        );
        let (sealed, work) = final_validation::measure_terminal_validation_work(|| {
            SvgPipeline::resvg_safe()
                .process_resvg_compatible(&svg, &session)
                .unwrap()
        });
        assert_eq!(
            sealed.finalization_report().fallback_css_residual(),
            expected
        );
        assert_eq!(work.calls, 1);
    }
    let invalid = r##"<svg xmlns="http://www.w3.org/2000/svg"><use data-merman-fallback-css-residual="true" href="#missing"/></svg>"##;
    assert!(
        SvgPipeline::resvg_safe()
            .process_resvg_compatible(invalid, &session)
            .is_err()
    );
}

#[test]
fn blocked_resource_links_do_not_activate_lower_priority_fragments() {
    let svg = r##"<svg xmlns:q="http://www.w3.org/1999/xlink" xmlns:other="urn:other"><defs><filter id="filter"><feImage q:href="#hidden" href="blocked.png"/></filter><linearGradient href="blocked.svg" q:href="#hidden"/></defs><use q:href="#hidden" href="blocked.svg"/><text><tref href="#hidden" q:href="blocked.svg"/><tref other:href="blocked.svg" href="#hidden"/></text></svg>"##;
    let session = render_session();
    let sealed = finalize_resvg_svg(svg, &session).unwrap();
    assert!(!sealed.as_str().contains("blocked."));
    assert!(sealed.as_str().contains(r#"other:href="""#));
    assert!(
        sealed
            .resource_closure()
            .referenced_fragment_ids()
            .is_empty()
    );
    assert_eq!(sealed.finalization_report().text_element_count(), 0);
    let again = finalize_resvg_svg(sealed.as_str(), &session).unwrap();
    assert_eq!(again.as_str(), sealed.as_str());
}

#[test]
fn whitespace_inherited_by_use_does_not_bypass_expansion_limits() {
    let svg = r##"<svg><defs><g id="spaces">     </g><g id="label"><text><tref href="#spaces"/></text></g></defs><use href="#label" xml:space="preserve"/><use href="#label" xml:space="preserve"/></svg>"##;
    let session = |max_elements| {
        crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::trusted_native()
                    .with_limit(
                        crate::resources::ResourceLimitId::MaxSvgElements,
                        max_elements,
                    )
                    .unwrap(),
            )
            .begin_session()
            .unwrap()
    };
    let error = finalize_resvg_svg(svg, &session(13)).unwrap_err();
    assert!(error.to_string().contains("max_svg_elements"), "{error}");
    let sealed = finalize_resvg_svg(svg, &session(14)).unwrap();
    assert_eq!(sealed.reference_plan().expanded_elements(), 14);
    assert_eq!(sealed.finalization_report().text_element_count(), 1);
}

#[test]
fn resvg_seal_moves_owned_terminal_reference_and_resource_inventories() {
    let session = render_session();
    let pipeline = SvgPipeline::resvg_safe();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="paint"><stop stop-color="#123456"/></linearGradient></defs><path fill="url(#paint)" d="M0 0h1"/></svg>"##.to_owned();
    let terminal = final_validation::validate_resvg_compatible_svg_with_execution(
        &svg,
        SvgPostprocessExecution::new(&session),
    )
    .unwrap();
    let reference_allocation = terminal.reference_plan.raw_element_occurrences().as_ptr();
    let available_allocation = terminal.resource_closure.available_fragment_ids().as_ptr();
    let referenced_allocation = terminal.resource_closure.referenced_fragment_ids().as_ptr();
    assert!(!terminal.reference_plan.raw_element_occurrences().is_empty());
    assert!(
        !terminal
            .resource_closure
            .available_fragment_ids()
            .is_empty()
    );
    assert!(
        !terminal
            .resource_closure
            .referenced_fragment_ids()
            .is_empty()
    );
    let expected_plan = terminal.reference_plan.clone();
    let expected_closure = terminal.resource_closure.clone();
    let expected_font_seal = terminal.font_seal.clone();
    let expected_text_elements = terminal.text_elements;

    let sealed = ResvgCompatibleSvg::seal(
        svg,
        Some(terminal),
        PreparedMathEvidenceLease::default(),
        true,
        &pipeline,
        &session,
    )
    .unwrap();
    let report = sealed.finalization_report();

    assert_eq!(report.reference_plan(), &expected_plan);
    assert_eq!(report.resource_closure(), &expected_closure);
    assert_eq!(report.font_seal(), &expected_font_seal);
    assert_eq!(report.text_element_count(), expected_text_elements);
    assert_eq!(
        report.reference_plan().raw_element_occurrences().as_ptr(),
        reference_allocation
    );
    assert_eq!(
        report.resource_closure().available_fragment_ids().as_ptr(),
        available_allocation
    );
    assert_eq!(
        report.resource_closure().referenced_fragment_ids().as_ptr(),
        referenced_allocation
    );
}

#[test]
fn strict_parity_artifact_is_compatible_when_its_bytes_satisfy_the_terminal_contract() {
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(
            crate::theme::ThemePortabilityRequirement::RequirePortable,
        )
        .begin_session()
        .unwrap();
    let artifact = StandaloneSvgArtifact::finalize_exact(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path d="M0 0h1v1z"/></svg>"#.to_owned(),
            &SvgPipeline::parity(),
            &session,
        )
        .unwrap();

    assert_eq!(
        artifact.terminal_status(),
        StandaloneSvgTerminalStatus::Compatible
    );
    assert_eq!(artifact.selected_pipeline(), SvgPipelinePreset::Parity);
    assert!(artifact.finalization_report().is_some());
    assert!(artifact.text_fonts_are_self_contained());
}

#[test]
fn parity_pipeline_preserves_svg_exactly() {
    let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><rect width="10"/></svg>"#;
    let session = render_session();
    let out = SvgPipeline::parity().process(svg, &session).unwrap();
    assert!(matches!(out, Cow::Borrowed(_)));
    assert_eq!(out, svg);
}

#[test]
fn browser_inline_contract_preserves_html_labels_and_rebases_references() {
    let svg = r##"<svg id="root" xmlns="http://www.w3.org/2000/svg" aria-labelledby="title"><title id="title">Diagram</title><defs><marker id="arrow"/></defs><style>@keyframes dash{to{stroke-dashoffset:0}}#root .edge{animation:dash 20s linear infinite}#root #label{color:red}</style><path class="edge" marker-end="url(#arrow)"/><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml" id="label" style="transform:rotate(5deg)"><strong>Safe label</strong></div></foreignObject></svg>"##;
    let output = SvgPipeline::parity()
        .with_browser_inline_contract("embed")
        .process_to_string(svg, &render_session())
        .unwrap();

    assert!(output.contains("<foreignObject"), "{output}");
    assert!(output.contains("<strong>Safe label</strong>"), "{output}");
    assert!(output.contains("transform:rotate(5deg)"), "{output}");
    assert!(output.contains("animation:dash 20s"), "{output}");
    assert!(output.contains(r#"id="embed-root""#), "{output}");
    assert!(
        output.contains(r#"aria-labelledby="embed-title""#),
        "{output}"
    );
    assert!(output.contains("url(#embed-arrow)"), "{output}");
    assert!(output.contains("#embed-root #embed-label"), "{output}");
}

#[test]
fn browser_inline_contract_accepts_renderer_owned_family_styles() {
    for (name, source) in [
        ("error", "error"),
        #[cfg(feature = "diagram-event-modeling")]
        ("eventmodeling", "eventmodeling\ntf 01 event Start"),
        #[cfg(feature = "diagram-ishikawa")]
        (
            "ishikawa",
            "ishikawa-beta\n    Problem\n        Cause\n            Detail",
        ),
        #[cfg(feature = "diagram-tree-view")]
        ("treeView", "treeView-beta\n    root/\n        file.txt"),
        #[cfg(feature = "diagram-zenuml")]
        ("zenuml", "zenuml\nAlice->Bob: Hello"),
        #[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
        ("mindmap", "mindmap\n  root\n    a\n    b"),
        #[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
        (
            "architecture",
            "architecture-beta\nservice worker \"<a href='https://example.test'>Docs</a>\" [Worker]",
        ),
    ] {
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .unwrap()
            .into_presentation();
        let artifact =
            crate::family::prepare(&parsed, &crate::LayoutOptions::default(), render_session())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
        let rendered = artifact
            .render_svg(
                &crate::svg::SvgRenderOptions {
                    diagram_id: Some(name.to_string()),
                    ..Default::default()
                },
                &crate::svg::SvgDebugOptions::default(),
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .apply_pipeline(&SvgPipeline::parity().with_browser_inline_contract("embed"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(rendered.svg().contains(&format!("id=\"embed-{name}\"")));
    }
}

#[test]
fn browser_inline_contract_rejects_external_html_and_escaped_css_resources() {
    for (svg, expected) in [
        (
            r#"<svg><foreignObject><img xmlns="http://www.w3.org/1999/xhtml" src="https://tracker.test/image.png"/></foreignObject></svg>"#,
            "forbidden <img>",
        ),
        (
            r#"<svg><foreignObject><div xmlns="http://www.w3.org/1999/xhtml" style="background:u\72l(https://tracker.test/image.png)">Label</div></foreignObject></svg>"#,
            "non-local CSS URL",
        ),
        (
            r#"<svg id="root"><style>#root .label{background:u\72l('https://tracker.test/image.png')}</style></svg>"#,
            "non-local CSS URL",
        ),
    ] {
        let error = SvgPipeline::parity()
            .with_browser_inline_contract("embed")
            .process_to_string(svg, &render_session())
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{svg}: {error}");
    }
}

#[test]
fn inline_contract_admits_before_any_lossy_postprocessor() {
    struct MustNotRun;
    impl SvgPostprocessor for MustNotRun {
        fn name(&self) -> &'static str {
            "must-not-run"
        }

        fn process<'a>(
            &self,
            _: Cow<'a, str>,
            _: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            panic!("unsafe SVG must be rejected before postprocessing")
        }
    }
    for pipeline in [
        SvgPipeline::parity()
            .with_postprocessor(MustNotRun)
            .with_browser_inline_contract("embed"),
        SvgPipeline::parity()
            .with_postprocessor(MustNotRun)
            .with_static_inline_contract("embed"),
    ] {
        let error = pipeline
            .process_to_string("<svg><script/></svg>", &render_session())
            .unwrap_err();
        assert!(error.to_string().contains("forbidden <script>"), "{error}");
    }
}

#[test]
fn inline_contract_validates_after_all_custom_postprocessors() {
    struct InjectResource;
    impl SvgPostprocessor for InjectResource {
        fn name(&self) -> &'static str {
            "inject-resource"
        }

        fn process<'a>(
            &self,
            _: Cow<'a, str>,
            _: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(Cow::Borrowed(
                r#"<svg><image href="https://tracker.test/image.png"/></svg>"#,
            ))
        }
    }
    for pipeline in [
        SvgPipeline::parity().with_browser_inline_contract("embed"),
        SvgPipeline::parity().with_static_inline_contract("embed"),
    ] {
        let error = pipeline
            .with_postprocessor(InjectResource)
            .process_to_string("<svg/>", &render_session())
            .unwrap_err();
        assert!(error.to_string().contains("non-local resource"), "{error}");
    }
}

#[test]
fn static_inline_contract_still_converts_html_and_removes_animation() {
    let svg = r#"<svg id="root"><style>@keyframes dash{to{opacity:0}}#root .edge{animation:dash 1s}</style><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml">Safe label</div></foreignObject></svg>"#;
    let output = SvgPipeline::parity()
        .with_static_inline_contract("embed")
        .process_to_string(svg, &render_session())
        .unwrap();

    assert!(!output.contains("<foreignObject"), "{output}");
    assert!(!output.contains("@keyframes"), "{output}");
    assert!(!output.contains("animation:"), "{output}");
    assert!(output.contains("Safe label"), "{output}");
    assert!(output.contains(r#"id="embed-root""#), "{output}");
}

#[test]
fn rebasing_without_inline_contract_preserves_external_resources() {
    let svg = r#"<svg id="root"><image href="https://example.test/image.png"/></svg>"#;
    let output = SvgPipeline::parity()
        .with_rebased_ids("embed")
        .process_to_string(svg, &render_session())
        .unwrap();

    assert!(output.contains(r#"id="embed-root""#), "{output}");
    assert!(
        output.contains("https://example.test/image.png"),
        "{output}"
    );
}

#[test]
fn parity_pipeline_returns_owned_svg_without_reallocating() {
    let svg = String::from(r#"<svg><rect width="10"/></svg>"#);
    let allocation = svg.as_ptr();
    let session = render_session();

    let out = SvgPipeline::parity()
        .process_owned_to_string(svg, &session)
        .unwrap();

    assert_eq!(out.as_ptr(), allocation);
}

#[test]
fn inline_pipelines_reuse_unchanged_math_structure_but_revalidate_opaque_output() {
    struct PreserveBytes;

    impl SvgPostprocessor for PreserveBytes {
        fn name(&self) -> &'static str {
            "preserve-bytes"
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            _: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(svg)
        }
    }

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>"#;
    let metadata = SvgPostprocessMetadata::from_svg(svg);
    let evidence = PreparedMathEvidenceLease::default();
    for contract in [InlineContract::Browser, InlineContract::Static] {
        for opaque in [false, true] {
            let mut pipeline = SvgPipeline {
                inline_contract: Some(contract),
                ..SvgPipeline::parity()
            };
            if opaque {
                pipeline.push_postprocessor(PreserveBytes);
            }
            for generated in [false, true] {
                let session = render_session();
                let (output, work) = final_validation::measure_xml_validation_work(|| {
                    if generated {
                        pipeline.process_generated_with_metadata_and_math_evidence(
                            svg.to_owned(),
                            &metadata,
                            &session,
                            Some(&evidence),
                        )
                    } else {
                        pipeline.process_to_string_with_metadata(svg, &metadata, &session)
                    }
                });

                assert_eq!(output.unwrap(), svg);
                // Initial XML, inline admission, final inline validation, and any opaque pass.
                let expected_calls = 3 + usize::from(opaque);
                assert_eq!(
                    work.calls, expected_calls,
                    "{contract:?}, opaque={opaque}, generated={generated}"
                );
                assert_eq!(work.svg_bytes, expected_calls * svg.len());
            }
        }
    }
}

#[test]
fn inline_pipelines_project_renderer_owned_math_before_admission() {
    let projection = concat!(
        r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
        r#"<path d="M0 0h1v1z"/></g>"#,
    );
    let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(
        crate::DiagramFamilyId::FLOWCHART,
        "node-label",
        0,
    );
    let native = Arc::new(crate::math::PreparedMathNativeProjection::new(
        projection.to_owned(),
        8.0,
        9.0,
    ));
    let evidence = PreparedMathEvidenceLease::with_native_projections(
        vec![(
            crate::math::PreparedMathExpectation::available(
                occurrence_id.clone(),
                native.fingerprint(),
                1,
            ),
            Some(native),
        )],
        Vec::new(),
    );
    let svg = format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<foreignObject width="10" height="11"><div xmlns="http://www.w3.org/1999/xhtml">"#,
            r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
            r#"data-merman-prepared-math-occurrence="{}">browser"#,
            r#"</span></div></foreignObject></svg>"#,
        ),
        occurrence_id.as_str(),
    );
    let session = render_session();
    let metadata =
        SvgPostprocessMetadata::from_svg(&svg).with_family_id(crate::DiagramFamilyId::FLOWCHART);

    for (pipeline, expected_calls) in [
        (
            SvgPipeline::parity().with_static_inline_contract("static-math"),
            7,
        ),
        (
            SvgPipeline::parity().with_browser_inline_contract("browser-math"),
            6,
        ),
    ] {
        let (result, work) = final_validation::measure_xml_validation_work(|| {
            pipeline.run(
                Cow::Borrowed(&svg),
                &metadata,
                &session,
                Some(&evidence),
                PipelineTerminalValidation::Immediate,
            )
        });
        let output = result.unwrap().svg.into_owned();

        assert_eq!(work.calls, expected_calls);
        assert!(!output.contains("<template"), "{output}");
        assert!(!output.contains("<foreignObject"), "{output}");
        assert!(output.contains(r#"class="merman-prepared-math-native""#));
        validate_static_inline_svg(&output, &session).unwrap();
    }
}

#[test]
fn resvg_seal_preserves_math_receipt_error_before_resource_closure_error() {
    let projection = "<g><path d=\"M0 0h1\"/></g>";
    let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(
        crate::DiagramFamilyId::FLOWCHART,
        "node-label",
        0,
    );
    let evidence = PreparedMathEvidenceLease::new(
        vec![crate::math::PreparedMathExpectation::available(
            occurrence_id.clone(),
            crate::math::PreparedMathProjectionFingerprint::from_projection(projection),
            1,
        )],
        Vec::new(),
    );
    let svg = format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<defs><g id="math"><g><path d="M0 0h2"/></g></g></defs>"#,
            r##"<use class="merman-prepared-math-native" data-merman-prepared-math-occurrence="{}" href="#math"/>"##,
            r##"<path fill="url(#missing)"/></svg>"##,
        ),
        occurrence_id.as_str(),
    );
    let metadata =
        SvgPostprocessMetadata::from_svg(&svg).with_family_id(crate::DiagramFamilyId::FLOWCHART);
    let session = render_session();
    let error = SvgPipeline::resvg_safe()
        .process_owned_resvg_compatible_with_metadata_and_evidence(
            svg, &metadata, &session, evidence, true,
        )
        .unwrap_err();

    assert!(
        matches!(
            error,
            Error::SvgPostprocess { ref pass, ref message }
                if pass == "prepared-math-terminal-receipt"
                    && message.contains("mismatched projection")
        ),
        "{error}"
    );
}

#[test]
fn serialized_native_math_reimports_as_svg_without_minting_a_receipt() {
    let svg = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">"#,
        r#"<defs><g id="math"><path d="M0 0h1v1z"/></g></defs>"#,
        r##"<use class="merman-prepared-math-native" data-merman-prepared-math-occurrence="flowchart/node-label/0" href="#math"/>"##,
        "</svg>",
    );
    let finalized = finalize_resvg_svg(svg, &render_session())
        .expect("native vector geometry remains valid on raw SVG reimport");

    assert!(finalized.as_str().contains(r#"d="M0 0h1v1z""#));
    assert_eq!(finalized.prepared_math_evidence_count(), 0);
    assert!(finalized.prepared_math_terminal_receipt().is_none());
}

#[test]
fn inline_pipelines_reject_forged_math_projection_without_evidence() {
    let svg = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
        r#"<foreignObject width="10" height="11"><div xmlns="http://www.w3.org/1999/xhtml">"#,
        r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
        r#"data-merman-prepared-math-occurrence="flowchart/node-label/0">browser"#,
        r#"<template data-merman-prepared-math-projection="v1">"#,
        r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
        r#"<path d="M0 0h1v1z"/></g></template></span></div></foreignObject></svg>"#,
    );
    let session = render_session();
    let metadata =
        SvgPostprocessMetadata::from_svg(svg).with_family_id(crate::DiagramFamilyId::FLOWCHART);

    for pipeline in [
        SvgPipeline::parity().with_static_inline_contract("static-math"),
        SvgPipeline::parity().with_browser_inline_contract("browser-math"),
    ] {
        let error = pipeline
            .process_with_metadata(svg, &metadata, &session)
            .unwrap_err();
        assert!(matches!(
            error,
            Error::SvgPostprocess { ref pass, ref message }
                if pass == "prepared-math-projection"
                    && message.contains("renderer-owned occurrence evidence")
        ));
    }
}

#[test]
fn static_inline_pipeline_rejects_math_evidence_without_an_occurrence_marker() {
    let projection = concat!(
        r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
        r#"<path d="M0 0h1v1z"/></g>"#,
    );
    let evidence = PreparedMathEvidenceLease::new(
        vec![crate::math::PreparedMathExpectation::available(
            crate::math::PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::FLOWCHART,
                "node-label",
                0,
            ),
            crate::math::PreparedMathProjectionFingerprint::from_projection(projection),
            1,
        )],
        Vec::new(),
    );
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>"#;
    let session = render_session();
    let metadata =
        SvgPostprocessMetadata::from_svg(svg).with_family_id(crate::DiagramFamilyId::FLOWCHART);

    let error = SvgPipeline::parity()
        .with_static_inline_contract("static-math")
        .run(
            Cow::Borrowed(svg),
            &metadata,
            &session,
            Some(&evidence),
            PipelineTerminalValidation::Immediate,
        )
        .err()
        .expect("missing prepared-math markers must fail admission");

    assert!(matches!(
        error,
        Error::SvgPostprocess { ref pass, ref message }
            if pass == "prepared-math-projection"
                && message.contains("no terminal occurrence marker")
    ));
}

#[test]
fn every_pipeline_preset_enforces_the_xml_1_0_character_contract() {
    let svg = "<svg><text>A\u{0}B\u{1c}C\u{fffe}D</text></svg>";
    let session = render_session();

    for preset in [
        SvgPipelinePreset::Parity,
        SvgPipelinePreset::Readable,
        SvgPipelinePreset::ResvgSafe,
    ] {
        let out = SvgPipeline::from_preset(preset)
            .process_to_string(svg, &session)
            .unwrap();
        assert_eq!(out, "<svg><text>ABCD</text></svg>", "{preset:?}");
        crate::xml_dom::Document::parse(&out).expect("pipeline output must remain XML 1.0");
    }
}

#[test]
fn every_pipeline_preset_rejects_unknown_xml_entities() {
    let session = render_session();

    for preset in [
        SvgPipelinePreset::Parity,
        SvgPipelinePreset::Readable,
        SvgPipelinePreset::ResvgSafe,
    ] {
        let error = SvgPipeline::from_preset(preset)
            .process_to_string("<svg><text>&unknown;</text></svg>", &session)
            .unwrap_err();
        assert!(
            error.to_string().contains("invalid XML"),
            "{preset:?}: {error}"
        );
    }
}

#[test]
fn readable_pipeline_matches_foreign_object_fallback() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(10,20)"><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Layer 7\nHTTP</p></div></foreignObject></g></svg>"#;
    let session = render_session();
    let measurer = session.text_measurer(crate::environment::TextMeasurementPhase::Wrap);

    let expected = super::builtin::foreign_object::foreign_object_fallback_svg(svg, &measurer);
    let out = SvgPipeline::readable()
        .process_to_string(svg, &session)
        .unwrap();

    assert_eq!(out, expected);
    assert!(out.contains(">Layer 7</text>"));
    assert!(out.contains(">HTTP</text>"));
}

#[test]
fn ordinary_postprocessors_do_not_inherit_native_backend_limits() {
    let depth = crate::resources::MAX_RESVG_TREE_DEPTH + 1;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="48"><g id="label">{}<foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Hello</p></div></foreignObject>{}</g></svg>"#,
        "<g>".repeat(depth),
        "</g>".repeat(depth),
    );
    for pipeline in [
        SvgPipeline::parity().with_rebased_ids("browser"),
        SvgPipeline::parity().with_browser_inline_contract("browser"),
    ] {
        let output = pipeline
            .process_to_string(&svg, &render_session())
            .expect("ordinary SVG consumers accept depth beyond the native backend ceiling");
        assert!(output.contains("Hello"));
    }
    assert!(
        final_validation::validate_resvg_compatible_svg_with_execution(
            &svg,
            SvgPostprocessExecution::new(&render_session()),
        )
        .is_err()
    );

    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1e308" height="48"><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Hello</p></div></foreignObject></svg>"#;
    let output = SvgPipeline::readable()
        .process_to_string(svg, &render_session())
        .expect("readable SVG has its own bounded work and a finite numeric domain");
    assert!(output.contains(">Hello</text>"));
    assert!(
        final_validation::validate_resvg_compatible_svg_with_execution(
            &output,
            SvgPostprocessExecution::new(&render_session()),
        )
        .is_err()
    );
}

#[test]
fn resvg_safe_pipeline_strips_generic_raster_hazards() {
    let svg = r#"<svg id="test" xmlns="http://www.w3.org/2000/svg"><style type="text/css">@keyframes bounce { 0% { transform: scale(1); } 100% { transform: scale(1.1); } } #test :root { --bg: white; } .node rect { animation: dash 1s linear; transform: rotate(45deg); fill: red; }</style><g transform="translate(undefined,NaN)"><foreignObject width="10" height="10"><div xmlns="http://www.w3.org/1999/xhtml"><p>Hello</p></div></foreignObject><rect width="10px" height="12px" stroke="" style="fill: ; stroke: #333; transform: rotate(45deg); animation: dash 1s;"/><rect width="10px" height="" fill="hsl(240, 100%, NaN%)"/></g></svg>"#;
    let session = render_session();

    let out = SvgPipeline::resvg_safe()
        .process_to_string(svg, &session)
        .unwrap();

    assert!(!out.contains("<foreignObject"));
    assert!(!out.contains("@keyframes"));
    assert!(!out.contains(":root"));
    assert!(!out.contains("animation"));
    assert!(!out.contains("deg"));
    assert!(!out.contains("NaN"));
    assert!(!out.contains("undefined"));
    assert!(!out.contains(r#"height="""#));
    assert!(!out.contains(r#"fill="hsl"#));
    assert!(!out.contains(r#"stroke="""#));
    assert!(out.contains(r#"width="10""#));
    assert!(out.contains(r#"height="12""#));
    assert!(out.contains("stroke:#333"));
    assert!(out.contains(">Hello</text>"));
}

#[test]
fn resvg_safe_pipeline_sanitizes_url_attributes_after_malformed_text() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
"#shape"/>
  <path stroke="url(javascript:alert(1))" style="filterBurl('file://юmp/xter:');svroke:#033"/>
</svg>"##;
    let session = render_session();

    let out = SvgPipeline::resvg_safe()
        .process_to_string(svg, &session)
        .expect("resvg-safe pipeline should sanitize the fuzz regression input");

    assert!(!out.to_ascii_lowercase().contains("javascript"), "{out}");
    assert!(!out.contains(r#"stroke="url("#), "{out}");
}

#[test]
fn resvg_safe_finalization_drops_unclosed_inline_css_blocks_idempotently() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="filter:5rl('file:///{animatiEtroke:#333"/></svg>"##;
    let session = render_session();

    let once = super::finalize_resvg_svg(svg, &session).unwrap();
    let twice = super::finalize_resvg_svg(once.as_str(), &session).unwrap();

    assert_eq!(twice, once);
    assert!(!once.as_str().contains("style="), "{}", once.as_str());
}

#[test]
fn resvg_safe_finalization_normalizes_stylesheet_line_endings_once() {
    let session = render_session();
    for line_ending in ["&#13;", "&#xD;&#10;", "\r", "\r\n"] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.outer{line_ending}.inner{{font-family:"A&amp;B";fill:red;}}</style></svg>"#
        );
        let once = super::finalize_resvg_svg(&svg, &session).unwrap();
        let twice = super::finalize_resvg_svg(once.as_str(), &session).unwrap();

        assert_eq!(twice.as_str(), once.as_str(), "line ending {line_ending:?}");
        assert!(!once.as_str().contains('\r'), "line ending {line_ending:?}");
        let document = crate::xml_dom::Document::parse(once.as_str()).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text());
        assert_eq!(css, Some(".outer\n.inner{font-family:\"A&B\";fill:red;}"));
    }
}

#[test]
fn resvg_safe_pipeline_bounds_css_nesting_on_a_small_thread_stack() {
    const CSS_NESTING_LIMIT: usize = 64;

    fn nested_function(depth: usize, leaf: &str) -> String {
        format!("{}{}{}", "f(".repeat(depth), leaf, ")".repeat(depth))
    }

    fn nested_media(depth: usize, rule: &str) -> String {
        format!(
            "{}{}{}",
            "@media all{".repeat(depth),
            rule,
            "}".repeat(depth)
        )
    }

    std::thread::Builder::new()
        .name("bounded-css-nesting".into())
        .stack_size(512 * 1024)
        .spawn(|| {
            let style_exact = nested_function(CSS_NESTING_LIMIT - 1, "red");
            let style_over = nested_function(CSS_NESTING_LIMIT, "red");
            let inline_exact = nested_function(CSS_NESTING_LIMIT, "red");
            let selector_exact = format!(
                "{}.selector-exact{}",
                ":is(".repeat(CSS_NESTING_LIMIT),
                ")".repeat(CSS_NESTING_LIMIT)
            );
            let selector_over = format!(
                "{}.selector-over{}",
                ":is(".repeat(CSS_NESTING_LIMIT + 1),
                ")".repeat(CSS_NESTING_LIMIT + 1)
            );
            let media_exact = nested_media(CSS_NESTING_LIMIT - 1, ".media-exact{fill:red}");
            let media_over = nested_media(CSS_NESTING_LIMIT, ".media-over{fill:red}");
            let very_deep = nested_function(4_096, "red");
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><style>
.style-exact{{fill:{style_exact}}}
.style-over{{fill:{style_over};stroke:blue}}
{selector_exact}{{fill:red}}
{selector_over}{{fill:red}}
{media_exact}
{media_over}
</style>
<path id="inline-exact" style="fill:{inline_exact};stroke:blue"/>
<path id="inline-over" style="fill:{very_deep};stroke:blue"/>
<path id="presentation-over" fill="{very_deep}" stroke="blue"/>
</svg>"#
            );

            let session = render_session();
            let out = SvgPipeline::resvg_safe()
                .process_to_string(&svg, &session)
                .expect("resvg-safe CSS sanitization must remain bounded");

            assert!(out.contains(".style-exact{fill:"), "{out}");
            assert!(out.contains(".style-over{stroke:blue}"), "{out}");
            assert!(out.contains(".selector-exact"), "{out}");
            assert!(out.contains("){fill:red}"), "{out}");
            assert!(!out.contains("selector-over"), "{out}");
            assert!(out.contains("media-exact"), "{out}");
            assert!(!out.contains("media-over"), "{out}");

            let document = crate::xml_dom::Document::parse(&out).expect("valid sanitized SVG");
            let inline_exact = document
                .descendants()
                .find(|node| node.attribute("id") == Some("inline-exact"))
                .unwrap();
            assert!(inline_exact.attribute("style").unwrap().contains("fill:"));
            let inline_over = document
                .descendants()
                .find(|node| node.attribute("id") == Some("inline-over"))
                .unwrap();
            assert_eq!(inline_over.attribute("style"), Some("stroke:blue"));
            let presentation_over = document
                .descendants()
                .find(|node| node.attribute("id") == Some("presentation-over"))
                .unwrap();
            assert_eq!(presentation_over.attribute("fill"), None);
            assert_eq!(presentation_over.attribute("stroke"), Some("blue"));
        })
        .expect("small-stack CSS regression thread must start")
        .join()
        .expect("bounded CSS traversal must not overflow the small thread stack");
}

struct AppendPass(&'static str);

impl SvgPostprocessor for AppendPass {
    fn name(&self) -> &'static str {
        self.0
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        Ok(Cow::Owned(format!(
            "{}<!--{}:{}:{:?}:{}:{}:{}-->",
            svg,
            ctx.pass_index(),
            ctx.pass_name(),
            ctx.preset(),
            ctx.diagram_type().unwrap_or("none"),
            ctx.diagram_title().unwrap_or("none"),
            ctx.svg_id().unwrap_or("none")
        )))
    }
}

struct CancelOperationPass(OperationControl);

impl SvgPostprocessor for CancelOperationPass {
    fn name(&self) -> &'static str {
        "cancel-operation"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        self.0.cancel();
        Ok(svg)
    }
}

#[test]
fn pipeline_checks_control_after_an_opaque_postprocessor() {
    let control = OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();

    let error = SvgPipeline::parity()
        .with_postprocessor(CancelOperationPass(control))
        .process_to_string("<svg/>", &session)
        .unwrap_err();

    let Error::Cancelled(error) = error else {
        panic!("expected structured cancellation");
    };
    assert_eq!(error.phase, OperationPhase::Postprocess);
    assert_eq!(error.reason, CancelReason::Requested);
}

#[test]
fn custom_postprocessors_run_before_builtin_finalizer_in_order() {
    let svg = r#"<svg><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject></svg>"#;
    let pipeline = SvgPipeline::readable()
        .with_postprocessor(AppendPass("first"))
        .with_postprocessor(AppendPass("second"));
    let session = render_session();

    let out = pipeline.process_to_string(svg, &session).unwrap();

    let first = out.find("<!--0:first:Readable").unwrap();
    let second = out.find("<!--1:second:Readable").unwrap();
    assert!(first < second);
    assert!(out.contains("data-merman-foreignobject"));
}

#[test]
fn custom_postprocessor_output_is_cleaned_by_resvg_finalizer() {
    struct InjectActiveContent;

    impl SvgPostprocessor for InjectActiveContent {
        fn name(&self) -> &'static str {
            "inject-active-content"
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(Cow::Owned(svg.replace(
                "</svg>",
                r#"<script>alert(1)</script><rect animation="spin 1s"/></svg>"#,
            )))
        }
    }

    let session = render_session();
    let output = SvgPipeline::resvg_safe()
        .with_postprocessor(InjectActiveContent)
        .process_resvg_compatible("<svg></svg>", &session)
        .unwrap();

    assert!(!output.as_str().contains("script"));
    assert!(!output.as_str().contains("animation"));
}

#[test]
fn expanded_draft_is_budgeted_before_terminal_xml_validation() {
    struct ExpandDraft;

    impl SvgPostprocessor for ExpandDraft {
        fn name(&self) -> &'static str {
            "expand-draft"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(Cow::Owned(format!("<svg>{}</svg>", "x".repeat(128))))
        }
    }

    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 64)
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    let error = SvgPipeline::resvg_safe()
        .with_postprocessor(ExpandDraft)
        .process_resvg_compatible("<svg/>", &session)
        .unwrap_err();

    assert!(error.to_string().contains("max_svg_bytes"), "{error}");
}

#[test]
fn provided_metadata_admits_raw_svg_bytes_before_xml_cleanup() {
    let svg = "<svg>\u{0}</svg>";
    let maximum = svg.len() - 1;
    let control = OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, maximum)
                .unwrap(),
        )
        .begin_session_with_control(control.clone())
        .unwrap();
    let metadata = SvgPostprocessMetadata::new();

    let error = SvgPipeline::parity()
        .process_with_metadata(svg, &metadata, &session)
        .expect_err("raw SVG bytes must be admitted before XML cleanup allocates");
    let Error::ResourceLimitExceeded(first) = error else {
        panic!("expected a resource rejection");
    };
    assert_eq!(first.limit, "max_svg_bytes");
    assert_eq!(first.actual, svg.len());
    assert_eq!(first.max, maximum);

    control.cancel();
    let replayed = SvgPipeline::parity()
        .process_with_metadata("<svg/>", &metadata, &session)
        .expect_err("the first resource terminal must remain sticky");
    let Error::ResourceLimitExceeded(replayed) = replayed else {
        panic!("expected the resource rejection to replay");
    };
    assert_eq!(replayed, first);
}

#[test]
fn structure_limit_rejects_before_custom_postprocessors_run() {
    struct MustNotRun;

    impl SvgPostprocessor for MustNotRun {
        fn name(&self) -> &'static str {
            "must-not-run"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            panic!("structure admission must precede custom postprocessing")
        }
    }

    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxSvgElements, 1)
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    let error = SvgPipeline::parity()
        .with_postprocessor(MustNotRun)
        .process_to_string("<svg><g/></svg>", &session)
        .unwrap_err();

    assert!(error.to_string().contains("max_svg_elements"), "{error}");
}

#[test]
fn fallback_generated_elements_are_rejected_before_the_overlay_is_completed() {
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxSvgElements, 4)
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    let svg = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
        r#"<foreignObject width="10" height="10">"#,
        r#"<div xmlns="http://www.w3.org/1999/xhtml">Hello</div>"#,
        "</foreignObject></svg>",
    );
    let error = SvgPipeline::readable()
        .process_to_string(svg, &session)
        .unwrap_err();

    assert!(error.to_string().contains("max_svg_elements"), "{error}");
}

#[test]
fn non_resvg_pipeline_cannot_construct_resvg_compatible_svg() {
    let session = render_session();
    let error = SvgPipeline::parity()
        .process_resvg_compatible("<svg/>", &session)
        .unwrap_err();

    assert!(error.to_string().contains("resvg-safe terminal preset"));
}

#[test]
fn custom_postprocessor_context_exposes_metadata() {
    let svg = r#"<svg id="host-diagram"><rect width="10"/></svg>"#;
    let metadata = SvgPostprocessMetadata::from_svg(svg)
        .with_diagram_type("flowchart-v2")
        .with_diagram_title("Host Diagram");
    let pipeline = SvgPipeline::parity().with_postprocessor(AppendPass("meta"));
    let session = render_session();

    let out = pipeline
        .process_to_string_with_metadata(svg, &metadata, &session)
        .unwrap();

    assert!(out.contains("<!--0:meta:Parity:flowchart-v2:Host Diagram:host-diagram-->"));
}

struct ErrorPass;

impl SvgPostprocessor for ErrorPass {
    fn name(&self) -> &'static str {
        "error-pass"
    }

    fn process<'a>(
        &self,
        _svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        Err(Error::InvalidModel {
            message: "boom".to_string(),
        })
    }
}

#[test]
fn custom_postprocessor_errors_surface_with_pass_name() {
    let session = render_session();
    let err = SvgPipeline::parity()
        .with_postprocessor(ErrorPass)
        .process_to_string("<svg/>", &session)
        .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("error-pass"));
    assert!(message.contains("boom"));
}

struct CancelAndErrorPass(OperationControl);

impl SvgPostprocessor for CancelAndErrorPass {
    fn name(&self) -> &'static str {
        "cancel-and-error"
    }

    fn process<'a>(
        &self,
        _svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        self.0.cancel();
        Err(Error::InvalidModel {
            message: "opaque failure".to_string(),
        })
    }
}

#[test]
fn cancellation_observed_after_a_failing_postprocessor_wins_over_wrapping() {
    let control = OperationControl::new();
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();

    let error = SvgPipeline::parity()
        .with_postprocessor(CancelAndErrorPass(control))
        .process_to_string("<svg/>", &session)
        .unwrap_err();

    let Error::Cancelled(error) = error else {
        panic!("expected structured cancellation");
    };
    assert_eq!(error.phase, OperationPhase::Postprocess);
    assert_eq!(error.reason, CancelReason::Requested);
}
