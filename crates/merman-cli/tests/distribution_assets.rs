use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANPAGE_DATE: &str = "2026-08-15";

fn asset_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn read_asset(relative: &str) -> String {
    let path = asset_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read generated asset {}: {error}", path.display()))
}

fn bash_command_block<'a>(script: &'a str, command: &str) -> &'a str {
    let marker = format!("\n        merman__cli__subcmd__{command})");
    let start = script
        .find(&marker)
        .unwrap_or_else(|| panic!("Bash completion omits {command:?} command state"));
    let rest = &script[start + marker.len()..];
    let end = rest
        .find("\n        merman__cli__subcmd__")
        .or_else(|| rest.find("\n    esac"))
        .unwrap_or(rest.len());
    &rest[..end]
}

fn bash_options(script: &str, command: &str) -> BTreeSet<String> {
    let block = bash_command_block(script, command);
    let options = block
        .lines()
        .find_map(|line| line.trim().strip_prefix("opts=\""))
        .and_then(|line| line.strip_suffix('"'))
        .unwrap_or_else(|| panic!("Bash completion omits the {command:?} option set"));
    options
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect()
}

fn bash_option_values(script: &str, command: &str, option: &str) -> BTreeSet<String> {
    let block = bash_command_block(script, command);
    let marker = format!("                {option})");
    let start = block
        .find(&marker)
        .unwrap_or_else(|| panic!("Bash completion omits {command} {option} values"));
    let rest = &block[start + marker.len()..];
    let values = rest
        .find("compgen -W \"")
        .map(|index| &rest[index + "compgen -W \"".len()..])
        .and_then(|rest| rest.split_once('"').map(|(values, _)| values))
        .unwrap_or_else(|| panic!("Bash completion omits {command} {option} value candidates"));
    values.split_ascii_whitespace().map(str::to_owned).collect()
}

#[test]
fn complete_profile_bash_completion_preserves_native_and_mmdc_contracts() {
    let script = read_asset("completions/merman-cli.bash");
    let render = bash_options(&script, "render");
    assert!(
        render.contains("-f"),
        "native render completion must expose -f"
    );
    assert!(
        !render.contains("-e"),
        "deprecated native -e must stay hidden from completion"
    );

    let batch = bash_options(&script, "batch");
    assert!(
        batch.contains("-f"),
        "native batch completion must expose -f"
    );
    assert!(
        !batch.contains("-e"),
        "deprecated batch -e must stay hidden from completion"
    );

    let mmdc = bash_options(&script, "mmdc");
    assert!(
        mmdc.contains("-e"),
        "the permanent mmdc compatibility surface must retain -e"
    );

    let native_themes = bash_option_values(&script, "render", "--theme");
    let expected_native_themes = merman::supported_themes()
        .iter()
        .map(|theme| (*theme).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(native_themes, expected_native_themes);

    let native_presentation_profiles =
        bash_option_values(&script, "render", "--presentation-profile");
    let expected_presentation_profiles = merman::svg::presentation_profile_descriptors()
        .iter()
        .map(|descriptor| descriptor.id().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(native_presentation_profiles, expected_presentation_profiles);

    let mmdc_presentation_profiles = bash_option_values(&script, "mmdc", "--presentation-profile");
    assert_eq!(mmdc_presentation_profiles, expected_presentation_profiles);

    assert!(!script.contains("--text-measurer"));

    let mmdc_themes = bash_option_values(&script, "mmdc", "--theme");
    assert_eq!(
        mmdc_themes,
        ["default", "forest", "dark", "neutral"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

#[test]
fn generated_manpages_have_deterministic_metadata_and_descriptions() {
    let man_root = asset_root().join("man");
    let mut manpages = fs::read_dir(&man_root)
        .unwrap_or_else(|error| panic!("read {}: {error}", man_root.display()))
        .map(|entry| entry.expect("read manpage entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "1"))
        .collect::<Vec<_>>();
    manpages.sort();
    assert!(!manpages.is_empty(), "generated manpage set is empty");

    for path in manpages {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read generated manpage {}: {error}", path.display()));
        let title = text
            .lines()
            .find(|line| line.starts_with(".TH "))
            .unwrap_or_else(|| panic!("{} omits a .TH title", path.display()));
        assert!(
            title.contains(&format!(" 1 {MANPAGE_DATE} "))
                && title.contains("\"Merman ")
                && title.ends_with("\"Merman CLI Manual\""),
            "{} has non-deterministic or incomplete title metadata: {title}",
            path.display()
        );

        for (index, block) in text.split("\n.TP\n").skip(1).enumerate() {
            let mut lines = block.lines();
            let signature = lines.next().unwrap_or_default();
            let description = lines.next().unwrap_or_default();
            assert!(
                !signature.is_empty()
                    && !description.is_empty()
                    && !description.starts_with('.')
                    && !description.contains("Possible values:"),
                "{} contains an option or subcommand without a description in .TP block {}",
                path.display(),
                index + 1
            );
        }
    }
}

#[test]
fn complete_profile_assets_expose_the_full_rustdoc_command_hierarchy() {
    for (asset, required) in [
        (
            "completions/merman-cli.bash",
            [
                "merman__cli,rustdoc)",
                "merman__cli__subcmd__rustdoc,build)",
                "merman__cli__subcmd__rustdoc,check)",
            ],
        ),
        (
            "completions/_merman-cli",
            [
                "_merman-cli__subcmd__rustdoc_commands",
                "_merman-cli__subcmd__rustdoc__subcmd__build_commands",
                "_merman-cli__subcmd__rustdoc__subcmd__check_commands",
            ],
        ),
        (
            "completions/merman-cli.fish",
            [
                "-a \"rustdoc\"",
                "__fish_seen_subcommand_from build",
                "__fish_seen_subcommand_from check",
            ],
        ),
        (
            "completions/merman-cli.elv",
            [
                "&'merman-cli;rustdoc'=",
                "&'merman-cli;rustdoc;build'=",
                "&'merman-cli;rustdoc;check'=",
            ],
        ),
        (
            "completions/merman-cli.ps1",
            [
                "'merman-cli;rustdoc'",
                "'merman-cli;rustdoc;build'",
                "'merman-cli;rustdoc;check'",
            ],
        ),
    ] {
        let text = read_asset(asset);
        for token in required {
            assert!(text.contains(token), "{asset} omits {token:?}");
        }
    }

    for (asset, required) in [
        ("man/merman-cli.1", "merman\\-cli\\-rustdoc(1)"),
        (
            "man/merman-cli-rustdoc.1",
            "merman\\-cli\\-rustdoc\\-build(1)",
        ),
        (
            "man/merman-cli-rustdoc.1",
            "merman\\-cli\\-rustdoc\\-check(1)",
        ),
        (
            "man/merman-cli-rustdoc-build.1",
            "merman\\-cli rustdoc build",
        ),
        (
            "man/merman-cli-rustdoc-check.1",
            "merman\\-cli rustdoc check",
        ),
    ] {
        assert!(
            read_asset(asset).contains(required),
            "{asset} omits {required:?}"
        );
    }
}

#[test]
fn readme_labels_tracked_assets_as_complete_profile_snapshots() {
    let readme = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
        .expect("read merman-cli README");
    assert!(
        readme.contains("canonical `cli-release` complete profile"),
        "tracked assets must not be presented as feature-neutral runtime output"
    );
}
