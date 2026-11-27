#[cfg(any(
    all(
        feature = "diagram-architecture",
        any(feature = "layout-cytoscape", feature = "layout-shoal")
    ),
    all(
        test,
        any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        )
    )
))]
pub(in crate::svg) use merman_render_core::icon_registry::IconIdScope;
#[cfg(any(
    all(
        feature = "diagram-architecture",
        any(feature = "layout-cytoscape", feature = "layout-shoal")
    ),
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(in crate::svg) use merman_render_core::icon_registry::IconIdScopePrefix;

pub use merman_render_core::icon_registry::{
    IconPack, IconRegistry, IconRegistryBuildError, IconRegistryBuildErrorKind,
    IconRegistryBuilder, IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId,
    icon_registry_resource_limit_descriptors,
};

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(in crate::svg) use merman_render_core::icon_registry::mermaid_unknown_icon_svg;
#[cfg(any(
    all(
        feature = "diagram-architecture",
        any(feature = "layout-cytoscape", feature = "layout-shoal")
    ),
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(in crate::svg) use merman_render_core::icon_registry::{IconRenderRequest, render_icon};

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(in crate::svg) fn bpmn_icon_svg(
    registry: Option<&IconRegistry>,
    request: IconRenderRequest<'_>,
) -> crate::Result<String> {
    let width = request.width_px;
    let height = request.height_px;
    request
        .work_meter
        .charge_emit_work(request.icon_name.len())?;
    let reference = merman_render_core::icon_registry::parse_icon_reference(
        request.icon_name,
        request.fallback_prefix,
    );
    let use_builtin = reference.is_some_and(|(prefix, _)| prefix == "bpmn")
        && registry.is_none_or(|registry| !registry.is_pack_registered("bpmn"));
    let rendered = if use_builtin {
        static BUILTIN: std::sync::OnceLock<IconRegistry> = std::sync::OnceLock::new();
        let registry = BUILTIN.get_or_init(|| {
            IconRegistry::from_packs([IconPack::new(include_bytes!("icon_registry/bpmn.json"))])
                .expect("source-owned BPMN glyphs are valid IconifyJSON")
        });
        render_icon(registry, request)?
    } else {
        registry
            .map(|registry| render_icon(registry, request))
            .transpose()?
            .flatten()
    };
    Ok(rendered.unwrap_or_else(|| {
        mermaid_unknown_icon_svg(
            crate::number_format::canonical_number(width),
            crate::number_format::canonical_number(height),
        )
    }))
}

/// Rewrites IDs in renderer-owned Architecture artwork.
#[cfg(all(
    feature = "diagram-architecture",
    any(feature = "layout-cytoscape", feature = "layout-shoal")
))]
pub(in crate::svg) fn rewrite_builtin_icon_ids(
    body: &'static str,
    scope: IconIdScope,
) -> crate::Result<merman_render_core::icon_registry::UntrustedIconXml> {
    use merman_render_core::icon_registry::{IconDefsPlacement, ParsedIconBody};

    let parsed = ParsedIconBody::parse_builtin_fragment(body)
        .map_err(|_| crate::Error::icon_processing("built-in icon fragment is invalid"))?;
    parsed
        .rewrite_ids(scope, IconDefsPlacement::Inline)
        .map_err(|_| crate::Error::icon_processing("built-in icon ID scoping failed"))
}

#[cfg(all(
    test,
    any(
        all(
            feature = "diagram-architecture",
            any(feature = "layout-cytoscape", feature = "layout-shoal")
        ),
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
pub(in crate::svg) fn icon_id_scope_for_test(value: &str) -> IconIdScope {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    IconIdScopePrefix::from_parts(&[value], &meter)
        .unwrap()
        .scope_parts(&[], &meter)
        .unwrap()
}

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    fn bpmn_icon(
        registry: Option<&IconRegistry>,
        name: &str,
        policy: RenderResourcePolicy,
    ) -> crate::Result<String> {
        let config =
            crate::test_support::presentation_config(serde_json::json!({"securityLevel":"strict"}));
        let meter = OperationWorkMeter::new(policy);
        bpmn_icon_svg(
            registry,
            IconRenderRequest {
                icon_name: name,
                width_px: 18.0,
                height_px: 18.0,
                fallback_prefix: Some("bpmn"),
                extra_class: None,
                id_scope: icon_id_scope_for_test("bpmn"),
                effective_config: &config,
                work_meter: &meter,
            },
        )
    }

    #[test]
    fn bpmn_glyphs_use_the_validated_registry_and_preserve_an_existing_pack() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let pack: serde_json::Value =
            serde_json::from_slice(include_bytes!("icon_registry/bpmn.json")).unwrap();
        assert_eq!(pack["icons"].as_object().unwrap().len(), 29);
        for name in pack["icons"].as_object().unwrap().keys() {
            let svg = bpmn_icon(None, &format!("bpmn:{name}"), policy).unwrap();
            assert!(svg.contains("currentColor"), "{name}: {svg}");
            assert!(svg.contains(r#"viewBox="0 0 24 24""#));
            assert!(!svg.contains("ArialMT"));
        }
        let custom = br#"{"prefix":"custom","icons":{"message":{"body":"<path d=\"M1 2h3\"/>"}}}"#;
        let registry =
            IconRegistry::from_packs([IconPack::new(custom).with_registration_name("bpmn")])
                .unwrap();
        assert!(registry.is_pack_registered("bpmn"));
        let svg = bpmn_icon(Some(&registry), "message", policy).unwrap();
        assert!(svg.contains("M1 2h3"));
        assert!(!svg.contains("bpmn-glyph-fold"));
        let empty =
            IconRegistry::from_packs([IconPack::new(br#"{"prefix":"bpmn","icons":{}}"#)]).unwrap();
        assert!(empty.is_pack_registered("bpmn"));
        let missing = bpmn_icon(Some(&empty), "message", policy).unwrap();
        assert!(missing.contains("ArialMT"));
        assert!(missing.contains(r#"width="18" height="18""#));
        assert!(!missing.contains("bpmn-glyph-fold"));
    }

    #[test]
    fn bpmn_glyphs_cannot_bypass_svg_or_work_limits() {
        for limit in [
            ResourceLimitId::MaxSvgBytes,
            ResourceLimitId::MaxLayoutWorkUnits,
        ] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(limit, 1)
                .unwrap();
            assert!(matches!(
                bpmn_icon(None, "bpmn:message", policy),
                Err(crate::Error::ResourceLimitExceeded(_))
            ));
        }
    }
}
