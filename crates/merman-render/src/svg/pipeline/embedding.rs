use super::{SvgPostprocessExecution, SvgPostprocessMetadata};
use crate::DiagramFamilyId;
use crate::Result;
use std::borrow::Cow;

/// Removes the redundant background ID emitted by Mermaid's default Mindmap node shape.
///
/// Raw parity retains the upstream duplicate. Only renderer-certified Mindmap output receives
/// this narrowly shaped repair before embedding admission; arbitrary duplicate IDs still fail.
pub(super) fn normalize_renderer_ids<'a>(
    svg: Cow<'a, str>,
    metadata: &SvgPostprocessMetadata,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    if metadata.family_id() != Some(DiagramFamilyId::MINDMAP) {
        return Ok(svg);
    }
    execution
        .validation()
        .normalize_mindmap_ids_for_embedding(svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::SvgPipeline;

    const MINDMAP: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram"><g class="node mindmap-node section-root" id="node"><path class="node-7 node-bkg" id="node"/></g><use href="#node"/></svg>"##;

    #[test]
    fn renderer_certified_mindmap_ids_are_isolated_for_every_embedding_mode() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let metadata =
            SvgPostprocessMetadata::from_svg(MINDMAP).with_family_id(DiagramFamilyId::MINDMAP);
        let raw = SvgPipeline::parity()
            .process_with_metadata(MINDMAP, &metadata, &session)
            .unwrap();
        assert_eq!(raw, MINDMAP);

        for pipeline in [
            SvgPipeline::parity().with_browser_inline_contract("embed"),
            SvgPipeline::parity().with_static_inline_contract("embed"),
            SvgPipeline::parity().with_rebased_ids("embed"),
        ] {
            let output = pipeline
                .process_with_metadata(MINDMAP, &metadata, &session)
                .unwrap();
            let document = crate::xml_dom::Document::parse(&output).unwrap();
            let target = document
                .descendants()
                .filter(|node| node.attribute("id") == Some("embed-node"))
                .collect::<Vec<_>>();
            assert_eq!(target.len(), 1);
            assert!(target[0].has_tag_name("g"));
            assert!(output.contains("href=\"#embed-node\""));
        }
    }

    #[test]
    fn inferred_family_and_unrelated_duplicates_still_fail_closed() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let inferred = SvgPostprocessMetadata::from_svg(MINDMAP).with_diagram_type("mindmap");
        let certified = inferred.clone().with_family_id(DiagramFamilyId::MINDMAP);
        for (source, metadata) in [
            (MINDMAP.to_string(), inferred),
            (
                MINDMAP.replace("node-7 node-bkg", "unrelated"),
                certified.clone(),
            ),
            (
                MINDMAP.replace("</svg>", "<rect id=\"node\"/></svg>"),
                certified,
            ),
        ] {
            let error = SvgPipeline::parity()
                .with_browser_inline_contract("embed")
                .process_with_metadata(&source, &metadata, &session)
                .unwrap_err();
            assert!(error.to_string().contains("duplicate id"), "{error}");
        }
    }

    #[test]
    fn identity_repair_does_not_hide_active_content_from_admission() {
        let source = MINDMAP.replace("<path class", "<path onclick=\"run()\" class");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let metadata =
            SvgPostprocessMetadata::from_svg(&source).with_family_id(DiagramFamilyId::MINDMAP);
        let error = SvgPipeline::parity()
            .with_browser_inline_contract("embed")
            .process_with_metadata(&source, &metadata, &session)
            .unwrap_err();
        assert!(error.to_string().contains("event attribute"), "{error}");
    }
}
