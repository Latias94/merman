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
    let arm_end = rest.find("\n                    ;;").unwrap_or(rest.len());
    let arm = &rest[..arm_end];
    let Some(values) = arm
        .find("compgen -W \"")
        .map(|index| &arm[index + "compgen -W \"".len()..])
        .and_then(|arm| arm.split_once('"').map(|(values, _)| values))
    else {
        return BTreeSet::new();
    };
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
        "removed native -e must not appear in render completion"
    );
    let batch = bash_options(&script, "batch");
    assert!(batch.contains("-f") && !batch.contains("-e"));
    for options in [&render, &batch] {
        for name in [
            "--theme",
            "--appearance",
            "--theme-precedence",
            "--background",
        ] {
            assert!(options.contains(name), "missing {name}");
        }
        for retired in [
            "--theme-preset",
            "--theme-file",
            "--theme-design-file",
            "--theme-appearance",
            "--theme-report",
            "--theme-portability",
        ] {
            assert!(!options.contains(retired), "unexpected {retired}");
        }
    }
    for command in ["render", "batch"] {
        assert_eq!(
            bash_option_values(&script, command, "--appearance"),
            ["light", "dark"].into_iter().map(str::to_owned).collect()
        );
        assert_eq!(
            bash_option_values(&script, command, "--theme-precedence"),
            ["host", "source"].into_iter().map(str::to_owned).collect()
        );
    }

    let mmdc = bash_options(&script, "mmdc");
    assert!(mmdc.contains("-e"));
    let layout = bash_options(&script, "layout");
    for options in [&mmdc, &layout] {
        assert!(!options.contains("--appearance") && !options.contains("--theme-precedence"));
    }

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
fn generated_assets_cover_the_theme_command_hierarchy() {
    let bash = read_asset("completions/merman-cli.bash");
    for command in ["list", "export", "check"] {
        assert!(bash.contains(&format!("merman__cli__subcmd__theme,{command})")));
        let manpage = read_asset(&format!("man/merman-cli-theme-{command}.1"));
        assert!(manpage.contains(&format!(
            ".TH MERMAN-CLI-THEME-{} ",
            command.to_ascii_uppercase()
        )));
    }
    assert!(!bash.contains("merman__cli__subcmd__theme,inspect)"));
    assert!(bash_options(&bash, "theme__subcmd__export").contains("--minimal"));
    assert!(bash_options(&bash, "theme__subcmd__check").contains("--strict-contrast"));
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
fn complete_profile_assets_expose_native_encoded_defaults_and_font_controls() {
    let script = read_asset("completions/merman-cli.bash");
    for command in ["render", "batch", "mmdc"] {
        let options = bash_options(&script, command);
        for option in [
            "--font-file",
            "--font-dir",
            "--font-measurement",
            "--font-system",
            "--font-max-bytes",
            "--font-max-measurements",
        ] {
            assert!(options.contains(option), "{command} omits {option}");
        }
        assert_eq!(
            bash_option_values(&script, command, "--font-measurement"),
            ["deterministic".to_owned(), "native".to_owned()]
                .into_iter()
                .collect()
        );
        assert_eq!(
            bash_option_values(&script, command, "--font-system"),
            ["off", "on", "bounded", "platform"]
                .map(str::to_owned)
                .into_iter()
                .collect()
        );
        let man = read_asset(&format!("man/merman-cli-{command}.1"));
        assert!(man.contains("Text measurement: SVG defaults to deterministic"));
        assert!(man.contains("PNG/JPEG/PDF use native fonts for shared layout and painting"));
        assert!(man.contains("Explicit deterministic keeps approximate layout"));
        assert!(man.contains("Control system font discovery"));
        assert!(man.contains("Native encoded output enables it by default"));
        assert!(man.contains("enable system fallback for explicit fonts"));
        assert!(man.contains(r"\fB\-\-font\-max\-bytes\fR \fI<SIZE>\fR"));
        assert!(man.contains("Byte budget for the entire native font context"));
        assert!(man.contains("default 256 MiB"));
        assert!(man.contains("256MiB"));
        assert!(man.contains("Maximum uncached native font shaping probes per diagram"));
        assert!(man.contains("default: 16384"));
        assert!(!man.contains("title horizontal bounds only"));
        assert!(!man.contains("deterministic without font sources"));
        assert!(!man.contains("System discovery is deferred"));
    }
    for asset in [
        "completions/merman-cli.elv",
        "completions/merman-cli.fish",
        "completions/merman-cli.ps1",
        "completions/_merman-cli",
    ] {
        let completion = read_asset(asset);
        assert!(
            completion.contains("SVG defaults to deterministic"),
            "{asset} omits the default SVG measurement policy"
        );
        assert!(
            completion.contains("PNG/JPEG/PDF use native fonts for shared layout and painting"),
            "{asset} omits the native encoded-output default"
        );
        assert!(
            completion.contains("Control system font discovery"),
            "{asset} omits system font discovery controls"
        );
        assert!(
            completion.contains("font-max-bytes"),
            "{asset} omits the native font byte budget"
        );
        assert!(
            completion.contains("font-max-measurements"),
            "{asset} omits the native font measurement budget"
        );
        assert!(
            !completion.contains("System discovery is deferred"),
            "{asset} retains the obsolete discovery policy"
        );
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
