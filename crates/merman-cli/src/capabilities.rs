use crate::error::CliError;
use crate::io::write_stdout;
use crate::runtime::SharedWriter;
use serde::Serialize;

const CLI_CAPABILITIES_SCHEMA_VERSION: u32 = 2;
const CLI_CONTRACT_VERSION: u32 = 5;
#[cfg(feature = "ascii")]
const ASCII_CAPABILITIES_SCHEMA_VERSION: u16 = 1;

#[allow(dead_code)]
mod descriptor {
    include!("generated/capability_surface.rs");
}

#[cfg(feature = "rustdoc")]
pub(crate) const fn capability_descriptor_digest() -> &'static str {
    descriptor::CAPABILITY_DESCRIPTOR_DIGEST
}

#[derive(Serialize)]
struct CapabilityDocument<'a> {
    schema_version: u32,
    cli_contract_version: u32,
    package: PackageView<'a>,
    compatibility: CompatibilityView<'a>,
    descriptor: DescriptorView<'a>,
    commands: Vec<String>,
    capabilities: Vec<CapabilityView<'a>>,
    outputs: Vec<OutputView<'a>>,
    #[cfg(feature = "ascii")]
    ascii: AsciiCapabilityDocument,
}

#[derive(Serialize)]
struct PackageView<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
struct CompatibilityView<'a> {
    mermaid: &'a str,
    mmdc: &'a str,
}

#[derive(Serialize)]
struct DescriptorView<'a> {
    schema_version: u32,
    digest: &'a str,
}

#[derive(Serialize)]
struct CapabilityView<'a> {
    id: &'a str,
    kind: &'a str,
    description: &'a str,
    implications: Vec<&'a str>,
}

#[derive(Serialize)]
struct OutputView<'a> {
    id: &'a str,
    description: &'a str,
    media_type: &'a str,
    system_fonts: Option<SystemFontView<'a>>,
    embedded_images: Option<EmbeddedImageView<'a>>,
}

#[derive(Serialize)]
struct SystemFontView<'a> {
    source_id: &'a str,
    discovery: &'a str,
    cache_scope: &'a str,
    host_dependent: bool,
    caller_configurable: bool,
    resource_bounded: bool,
}

#[derive(Serialize)]
struct EmbeddedImageView<'a> {
    source_ids: &'a [&'a str],
    filesystem_access: bool,
    network_access: bool,
    caller_configurable: bool,
    limits: EmbeddedImageLimitsView,
}

#[derive(Serialize)]
struct EmbeddedImageLimitsView {
    max_bytes_per_image: Option<u64>,
    max_total_bytes: Option<u64>,
    max_pixels_per_image: Option<u64>,
    max_total_pixels: Option<u64>,
}

#[cfg(feature = "ascii")]
#[derive(Serialize)]
struct AsciiCapabilityDocument {
    schema_version: u16,
    output_schema_version: u16,
    report: AsciiReportContractView,
    families: Vec<AsciiFamilyCapabilityView>,
    detected_type_mappings: Vec<AsciiDetectedTypeMappingView>,
}

#[cfg(feature = "ascii")]
#[derive(Serialize)]
struct AsciiReportContractView {
    success_schema_version: u16,
    error_schema_version: u16,
    encoding: &'static str,
    styled_output: bool,
    success_stream: &'static str,
    error_stream: &'static str,
}

#[cfg(feature = "ascii")]
#[derive(Serialize)]
struct AsciiFamilyCapabilityView {
    family: &'static str,
    display_name: &'static str,
    semantic_coverage: Option<&'static str>,
    primary_projection: &'static str,
    structured_text_fallback: bool,
    support_level: &'static str,
    layout_profiles: Vec<&'static str>,
    width_profiles: Vec<&'static str>,
    encodings: Vec<&'static str>,
    fallback_encodings: Vec<&'static str>,
}

#[cfg(feature = "ascii")]
#[derive(Serialize)]
struct AsciiDetectedTypeMappingView {
    detected_type: &'static str,
    family: &'static str,
}

pub(crate) fn write_compiled_capabilities(
    json: bool,
    stdout: &SharedWriter,
) -> Result<(), CliError> {
    let capability_ids = compiled_capability_ids();
    let document = CapabilityDocument {
        schema_version: CLI_CAPABILITIES_SCHEMA_VERSION,
        cli_contract_version: CLI_CONTRACT_VERSION,
        package: PackageView {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
        },
        compatibility: CompatibilityView {
            mermaid: merman::baseline::PINNED_MERMAID_BASELINE_VERSION,
            mmdc: merman::baseline::PINNED_MERMAID_CLI_VERSION,
        },
        descriptor: DescriptorView {
            schema_version: descriptor::CAPABILITY_DESCRIPTOR_SCHEMA_VERSION,
            digest: descriptor::CAPABILITY_DESCRIPTOR_DIGEST,
        },
        commands: compiled_command_ids(),
        capabilities: descriptor::CAPABILITIES
            .iter()
            .filter(|capability| capability_ids.contains(&capability.id))
            .map(|capability| CapabilityView {
                id: capability.id,
                kind: capability.kind,
                description: capability.description,
                implications: capability.implications.iter().map(|key| key.id()).collect(),
            })
            .collect(),
        outputs: descriptor::OUTPUTS
            .iter()
            .filter(|output| capability_ids.contains(&output.capability.id()))
            .map(output_view)
            .collect(),
        #[cfg(feature = "ascii")]
        ascii: ascii_capability_document(),
    };

    if json {
        return crate::diagnostics::write_json_stdout(&document, true, stdout);
    }

    let mut output = String::from("ID\tKind\tDescription\n");
    for capability in &document.capabilities {
        output.push_str(capability.id);
        output.push('\t');
        output.push_str(capability.kind);
        output.push('\t');
        output.push_str(capability.description);
        output.push('\n');
    }
    write_stdout(output.as_bytes(), stdout)
}

#[cfg(feature = "ascii")]
fn ascii_capability_document() -> AsciiCapabilityDocument {
    let mut families = merman::ascii::ascii_capabilities()
        .iter()
        .map(|capability| AsciiFamilyCapabilityView {
            family: capability.diagram_type,
            display_name: capability.display_name,
            semantic_coverage: capability
                .semantic_coverage
                .map(merman::ascii::AsciiSemanticCoverage::as_str),
            primary_projection: capability.primary_projection.as_str(),
            structured_text_fallback: capability.structured_text_fallback,
            support_level: capability.support_level.as_str(),
            layout_profiles: capability
                .layout_profiles
                .iter()
                .copied()
                .map(merman::ascii::AsciiLayoutProfile::as_str)
                .collect(),
            width_profiles: capability
                .width_profiles
                .iter()
                .copied()
                .map(merman::ascii::TerminalWidthProfile::as_str)
                .collect(),
            encodings: capability
                .encodings
                .iter()
                .copied()
                .map(merman::ascii::AsciiOutputEncoding::as_str)
                .collect(),
            fallback_encodings: capability
                .fallback_encodings
                .iter()
                .copied()
                .map(merman::ascii::AsciiOutputEncoding::as_str)
                .collect(),
        })
        .collect::<Vec<_>>();
    families.sort_by_key(|capability| capability.family);

    let known_families = families
        .iter()
        .map(|capability| capability.family)
        .collect::<std::collections::BTreeSet<_>>();
    let render_model_families = merman::built_in_typed_render_families()
        .iter()
        .map(|family| (family.render_model_kind, family.diagram_type))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut detected_type_mappings = merman::diagram_family_capabilities()
        .iter()
        .filter(|capability| capability.has_detector)
        .filter_map(|capability| {
            let family = render_model_families.get(capability.render_model_kind?)?;
            known_families
                .contains(family)
                .then_some(AsciiDetectedTypeMappingView {
                    detected_type: capability.diagram_type,
                    family,
                })
        })
        .collect::<Vec<_>>();
    detected_type_mappings.sort_by_key(|mapping| mapping.detected_type);

    AsciiCapabilityDocument {
        schema_version: ASCII_CAPABILITIES_SCHEMA_VERSION,
        output_schema_version: merman::ascii::ASCII_OUTPUT_SCHEMA_VERSION,
        report: AsciiReportContractView {
            success_schema_version: merman::ascii::ASCII_OUTPUT_SCHEMA_VERSION,
            error_schema_version: crate::error::ASCII_ERROR_REPORT_SCHEMA_VERSION,
            encoding: "plain",
            styled_output: false,
            success_stream: "output",
            error_stream: "stderr",
        },
        families,
        detected_type_mappings,
    }
}

fn output_view(output: &descriptor::OutputDescriptor) -> OutputView<'_> {
    match output.id {
        "ascii" | "svg" => OutputView {
            id: output.id,
            description: output.description,
            media_type: output.media_type,
            system_fonts: None,
            embedded_images: None,
        },
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        "jpeg" | "pdf" | "png" => {
            let environment = merman::svg::export::output_environment_contract(output.id)
                .expect("a compiled CLI export must have an environment contract");
            let system_fonts = environment
                .system_fonts
                .expect("native CLI exports must disclose host system fonts");
            let limits = environment.embedded_images.default_limits;
            OutputView {
                id: output.id,
                description: output.description,
                media_type: output.media_type,
                system_fonts: Some(SystemFontView {
                    source_id: system_fonts.source_id,
                    discovery: system_fonts.discovery,
                    cache_scope: system_fonts.cache_scope,
                    host_dependent: system_fonts.host_dependent,
                    caller_configurable: false,
                    resource_bounded: system_fonts.resource_bounded,
                }),
                embedded_images: Some(EmbeddedImageView {
                    source_ids: environment.embedded_images.source_ids,
                    filesystem_access: environment.embedded_images.filesystem_access,
                    network_access: environment.embedded_images.network_access,
                    caller_configurable: true,
                    limits: EmbeddedImageLimitsView {
                        max_bytes_per_image: limits.max_bytes_per_image,
                        max_total_bytes: limits.max_total_bytes,
                        max_pixels_per_image: limits.max_pixels_per_image,
                        max_total_pixels: limits.max_total_pixels,
                    },
                }),
            }
        }
        _ => panic!(
            "descriptor output `{}` has no CLI output-contract owner",
            output.id
        ),
    }
}

fn compiled_command_ids() -> Vec<String> {
    let mut command = crate::app::cli_command();
    command.build();
    let mut commands = command
        .get_subcommands()
        .map(|command| command.get_name().to_owned())
        .collect::<Vec<_>>();
    commands.sort();
    commands
}

fn compiled_capability_ids() -> Vec<&'static str> {
    let mut capabilities = Vec::new();

    macro_rules! include_capability {
        ($feature:literal, $id:literal) => {
            if cfg!(feature = $feature) {
                capabilities.push($id);
            }
        };
    }

    include_capability!("analysis", "analysis");
    include_capability!("ascii", "ascii");
    include_capability!("icons", "icons");
    include_capability!("jpeg", "jpeg");
    include_capability!("layout-cytoscape", "layout-cytoscape");
    include_capability!("layout-elk", "layout-elk");
    include_capability!("markdown", "markdown");
    include_capability!("math", "math");
    include_capability!("network-icons", "network-icons");
    include_capability!("parallel-markdown", "parallel-markdown");
    include_capability!("pdf", "pdf");
    include_capability!("png", "png");
    include_capability!("rustdoc", "rustdoc");
    include_capability!("shell-completions", "shell-completions");
    include_capability!("svg", "svg");
    include_capability!("system-clock", "system-clock");
    include_capability!("system-random", "system-random");
    include_capability!("system-timezone", "system-timezone");
    include_capability!("system-timing", "system-timing");

    capabilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_ids_never_escape_the_canonical_descriptor() {
        let declared = descriptor::CAPABILITY_IDS;
        assert!(
            compiled_capability_ids()
                .iter()
                .all(|id| declared.contains(id)),
            "CLI reported a capability absent from the canonical descriptor"
        );
    }

    #[test]
    fn compiled_command_ids_are_sorted_and_unique() {
        let commands = compiled_command_ids();
        assert!(commands.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            commands.iter().any(|id| id == "rustdoc"),
            cfg!(feature = "rustdoc")
        );
    }

    #[test]
    fn compiled_tool_ids_follow_their_feature_boundaries() {
        let ids = compiled_capability_ids();

        assert_eq!(ids.contains(&"icons"), cfg!(feature = "icons"));
        assert_eq!(ids.contains(&"markdown"), cfg!(feature = "markdown"));
        assert_eq!(ids.contains(&"rustdoc"), cfg!(feature = "rustdoc"));
        assert_eq!(
            ids.contains(&"network-icons"),
            cfg!(feature = "network-icons")
        );
        assert_eq!(
            ids.contains(&"parallel-markdown"),
            cfg!(feature = "parallel-markdown")
        );

        if cfg!(feature = "network-icons") {
            assert!(ids.contains(&"icons"));
        }
        if cfg!(feature = "parallel-markdown") {
            assert!(ids.contains(&"markdown"));
        }
    }
}
