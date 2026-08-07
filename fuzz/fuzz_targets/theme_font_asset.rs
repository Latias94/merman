#![no_main]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use libfuzzer_sys::fuzz_target;
use merman_render::diagram_theme::{
    FontAssetSpec, FontCatalogSpec, FontEmbeddingRequirement, FontSource, GenericFontFamily,
};
use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};

const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_FUZZ_ASSETS: usize = 4;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > MAX_INPUT_BYTES {
        return;
    }

    let policy = fuzz_resource_policy();
    let Some(input) = decode_input(data, &policy) else {
        return;
    };
    let Some(spec) = catalog_spec(&input) else {
        return;
    };

    let first = spec.clone().compile(&policy);
    let Ok(catalog) = first else {
        return;
    };
    let repeated = spec
        .compile(&policy)
        .expect("the same bounded catalog input must compile deterministically");
    assert_eq!(catalog.fingerprint(), repeated.fingerprint());
    assert!(catalog.assets().len() <= MAX_FUZZ_ASSETS);

    let Some(family) = catalog.faces().first().map(|face| face.family_name()) else {
        return;
    };
    let enriched = FontCatalogSpec::new(
        input
            .assets
            .iter()
            .enumerate()
            .map(|(index, bytes)| FontAssetSpec::new(format!("font-{index}"), bytes)),
    )
    .with_available_sources([FontSource::Embedded])
    .with_alias("fuzz-alias", family)
    .with_generic_family(GenericFontFamily::SansSerif, "fuzz-alias");
    let enriched = enriched
        .compile(&policy)
        .expect("aliases derived from admitted catalog metadata must remain valid");
    assert_eq!(
        enriched.generic_family(GenericFontFamily::SansSerif),
        Some(family)
    );
});

struct FuzzCatalogInput {
    selector: u8,
    assets: Vec<Vec<u8>>,
}

fn decode_input(data: &[u8], policy: &RenderResourcePolicy) -> Option<FuzzCatalogInput> {
    if is_font_container(data) {
        return Some(FuzzCatalogInput {
            selector: 0,
            assets: vec![data.to_vec()],
        });
    }
    if let Some(payload) = data.strip_prefix(b"base64\n") {
        policy.check_theme_encoded_bytes(data.len()).ok()?;
        policy.check_theme_base64_bytes(payload.len()).ok()?;
        return Some(FuzzCatalogInput {
            selector: 1,
            assets: vec![STANDARD.decode(payload).ok()?],
        });
    }

    let (&selector, payload) = data.split_first()?;
    match selector % 3 {
        0 => Some(FuzzCatalogInput {
            selector,
            assets: vec![payload.to_vec()],
        }),
        1 => {
            policy.check_theme_encoded_bytes(data.len()).ok()?;
            policy.check_theme_base64_bytes(payload.len()).ok()?;
            let decoded = STANDARD.decode(payload).ok()?;
            Some(FuzzCatalogInput {
                selector,
                assets: vec![decoded],
            })
        }
        _ => split_catalog(selector, payload),
    }
}

fn split_catalog(selector: u8, payload: &[u8]) -> Option<FuzzCatalogInput> {
    let (&count_hint, payload) = payload.split_first()?;
    let asset_count = usize::from(count_hint % MAX_FUZZ_ASSETS as u8) + 1;
    if payload.len() < asset_count {
        return None;
    }

    let mut assets = Vec::with_capacity(asset_count);
    let mut remaining = payload;
    for remaining_assets in (1..=asset_count).rev() {
        let chunk_len = remaining.len().div_ceil(remaining_assets);
        let (chunk, rest) = remaining.split_at(chunk_len);
        assets.push(chunk.to_vec());
        remaining = rest;
    }
    Some(FuzzCatalogInput { selector, assets })
}

fn catalog_spec(input: &FuzzCatalogInput) -> Option<FontCatalogSpec> {
    if input.assets.is_empty() || input.assets.iter().any(Vec::is_empty) {
        return None;
    }
    let embedding = match (input.selector / 3) % 3 {
        0 => FontEmbeddingRequirement::NoEmbedding,
        1 => FontEmbeddingRequirement::FullFont,
        _ => FontEmbeddingRequirement::Subset,
    };
    let sources = if input.selector & 0x20 == 0 {
        vec![FontSource::Embedded]
    } else {
        vec![FontSource::Embedded, FontSource::System]
    };

    Some(
        FontCatalogSpec::new(
            input
                .assets
                .iter()
                .enumerate()
                .map(|(index, bytes)| FontAssetSpec::new(format!("font-{index}"), bytes)),
        )
        .with_available_sources(sources)
        .with_embedding_requirement(embedding),
    )
}

fn fuzz_resource_policy() -> RenderResourcePolicy {
    let mut policy = RenderResourcePolicy::constrained();
    for (id, value) in [
        (ResourceLimitId::MaxThemeEncodedBytes, MAX_INPUT_BYTES),
        (ResourceLimitId::MaxThemeBase64Bytes, MAX_INPUT_BYTES),
        (
            ResourceLimitId::MaxFontAssetCompressedBytes,
            MAX_INPUT_BYTES,
        ),
        (ResourceLimitId::MaxFontAssetDecodedBytes, 2 * 1024 * 1024),
        (ResourceLimitId::MaxFontCatalogDecodedBytes, 4 * 1024 * 1024),
        (ResourceLimitId::MaxFontAssets, MAX_FUZZ_ASSETS),
        (ResourceLimitId::MaxFontFaces, 32),
        (ResourceLimitId::MaxFontTables, 512),
        (ResourceLimitId::MaxFontAliases, 8),
        (ResourceLimitId::MaxFontDecodedExpansionRatio, 16),
    ] {
        policy
            .apply_limit(id, value)
            .expect("fuzz policy uses valid host-owned ceilings");
    }
    policy
}

fn is_font_container(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0x00, 0x01, 0x00, 0x00] | b"true" | b"typ1" | b"OTTO" | b"ttcf" | b"wOF2")
    )
}
