use crate::error::CliError;
use crate::io::write_stdout;
use crate::runtime::SharedWriter;
use serde::Serialize;

const CLI_CAPABILITIES_SCHEMA_VERSION: u32 = 2;
const CLI_CONTRACT_VERSION: u32 = 2;

#[allow(dead_code)]
mod descriptor {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../capabilities/generated/capability_surface.rs"
    ));
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
    implications: &'a [&'a str],
}

#[derive(Serialize)]
struct OutputView<'a> {
    id: &'a str,
    description: &'a str,
    media_type: &'a str,
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
                implications: capability.implications,
            })
            .collect(),
        outputs: descriptor::OUTPUTS
            .iter()
            .filter(|output| capability_ids.contains(&output.capability))
            .map(|output| OutputView {
                id: output.id,
                description: output.description,
                media_type: output.media_type,
            })
            .collect(),
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
    }

    #[test]
    fn compiled_tool_ids_follow_their_feature_boundaries() {
        let ids = compiled_capability_ids();

        assert_eq!(ids.contains(&"icons"), cfg!(feature = "icons"));
        assert_eq!(ids.contains(&"markdown"), cfg!(feature = "markdown"));
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
