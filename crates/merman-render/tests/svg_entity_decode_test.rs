use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixtures_root() -> PathBuf {
    workspace_root().join("fixtures")
}

fn render_fixture_svg(rel_fixture_path: impl AsRef<Path>) -> String {
    let mmd_path = fixtures_root().join(rel_fixture_path);
    let text = std::fs::read_to_string(&mmd_path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", mmd_path.display()));

    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::default();
    let layouted = layout_parsed(&parsed, &layout_opts).expect("layout ok");

    render_layouted_svg(
        &layouted,
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render ok")
}

fn contains_malformed_xml_entity_reference(s: &str) -> bool {
    fn is_valid_entity(entity: &str) -> bool {
        if entity.is_empty() {
            return false;
        }
        if let Some(hex) = entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
        {
            return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
        }
        if let Some(dec) = entity.strip_prefix('#') {
            return !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit());
        }
        let mut it = entity.chars();
        let Some(first) = it.next() else {
            return false;
        };
        if !first.is_ascii_alphabetic() {
            return false;
        }
        it.all(|c| c.is_ascii_alphanumeric())
    }

    let mut i = 0usize;
    while let Some(rel) = s[i..].find('&') {
        let amp = i + rel;
        let tail = &s[amp + 1..];
        let Some(semi_rel) = tail.find(';') else {
            return true;
        };
        let semi = amp + 1 + semi_rel;
        let entity = &s[amp + 1..semi];
        if !is_valid_entity(entity) {
            return true;
        }
        i = semi + 1;
    }
    false
}

#[test]
fn rendered_svgs_do_not_contain_mermaid_entity_placeholders() {
    // Mermaid preprocesses input with `encodeEntities(...)`, which introduces placeholder sequences
    // like `ﬂ°...¶ß` (to avoid grammar conflicts with `#...;`). Rendered SVG output should not leak
    // these internal placeholders.
    let fixtures = [
        "mindmap/stress_mindmap_markdown_vs_verbatim_030.mmd",
        "mindmap/stress_mindmap_shapes_with_ids_and_labels_028.mmd",
        "quadrantchart/stress_quadrantchart_batch1_unicode_quotes_punct_009.mmd",
        "sequence/stress_html_entities_and_escaping_038.mmd",
        "state/upstream_cypress_statediagram_v2_spec_v2_states_can_have_a_class_applied_032.mmd",
        "timeline/timeline_stress_common_section_br_and_entities.mmd",
        "timeline/timeline_stress_events_with_entities_and_ampersands.mmd",
        "timeline/timeline_stress_period_labels_with_colons_entities.mmd",
        "timeline/timeline_stress_section_titles_with_hashes_colons_semicolons.mmd",
    ];

    for fixture in fixtures {
        let svg = render_fixture_svg(fixture);
        assert!(
            !svg.contains("ﬂ°") && !svg.contains("¶ß"),
            "rendered SVG leaked encodeEntities placeholders for fixture {fixture}"
        );
    }
}

#[test]
fn rendered_svgs_do_not_contain_malformed_xml_entity_references() {
    let fixtures = [
        "mindmap/stress_mindmap_markdown_vs_verbatim_030.mmd",
        "mindmap/stress_mindmap_shapes_with_ids_and_labels_028.mmd",
        "quadrantchart/stress_quadrantchart_batch1_unicode_quotes_punct_009.mmd",
        "sequence/stress_html_entities_and_escaping_038.mmd",
        "state/upstream_cypress_statediagram_v2_spec_v2_states_can_have_a_class_applied_032.mmd",
        "timeline/timeline_stress_common_section_br_and_entities.mmd",
        "timeline/timeline_stress_events_with_entities_and_ampersands.mmd",
        "timeline/timeline_stress_period_labels_with_colons_entities.mmd",
        "timeline/timeline_stress_section_titles_with_hashes_colons_semicolons.mmd",
    ];

    for fixture in fixtures {
        let svg = render_fixture_svg(fixture);
        assert!(
            !contains_malformed_xml_entity_reference(&svg),
            "rendered SVG contained a malformed XML entity reference for fixture {fixture}"
        );
    }
}
