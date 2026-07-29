use super::{cli_command, completion_script};
use clap_complete::aot::Shell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const UPDATE_ENV: &str = "MERMAN_UPDATE_CLI_ASSETS";
const ASSET_ROOTS: &[&str] = &["assets/completions", "assets/man"];
const CLI_MANPAGE_DATE: &str = "2026-07-29";
const CLI_MANPAGE_MANUAL: &str = "Merman CLI Manual";
const CLI_MANPAGE_SOURCE: &str = concat!("Merman ", env!("CARGO_PKG_VERSION"));

struct GeneratedAsset {
    relative_path: String,
    bytes: Vec<u8>,
}

fn generated_assets() -> Vec<GeneratedAsset> {
    let mut assets = vec![
        completion_asset(Shell::Bash, "assets/completions/merman-cli.bash"),
        completion_asset(Shell::Elvish, "assets/completions/merman-cli.elv"),
        completion_asset(Shell::Fish, "assets/completions/merman-cli.fish"),
        completion_asset(Shell::PowerShell, "assets/completions/merman-cli.ps1"),
        completion_asset(Shell::Zsh, "assets/completions/_merman-cli"),
    ];
    assets.extend(manpage_assets());
    assets.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut paths = BTreeSet::new();
    for asset in &assets {
        assert!(
            paths.insert(asset.relative_path.as_str()),
            "generated CLI assets collide at {}",
            asset.relative_path
        );
    }
    assets
}

fn completion_asset(shell: Shell, relative_path: &'static str) -> GeneratedAsset {
    GeneratedAsset {
        relative_path: relative_path.to_owned(),
        bytes: normalized_generated_text(completion_script(shell)),
    }
}

fn manpage_assets() -> Vec<GeneratedAsset> {
    let mut command = normalize_manpage_command(cli_command());
    command.build();
    let mut assets = Vec::new();
    collect_manpages(command, &mut assets);
    assets
}

fn normalize_manpage_command(command: clap::Command) -> clap::Command {
    command
        .mut_args(|argument| argument.hide_short_help(false))
        .mut_subcommands(normalize_manpage_command)
}

fn collect_manpages(command: clap::Command, assets: &mut Vec<GeneratedAsset>) {
    let subcommands = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .cloned()
        .collect::<Vec<_>>();
    for subcommand in subcommands {
        collect_manpages(subcommand, assets);
    }

    let manpage = clap_mangen::Man::new(command);
    let filename = manpage.get_filename();
    let title = filename
        .strip_suffix(".1")
        .expect("section-one manpage filename")
        .to_ascii_uppercase();
    let manpage = manpage
        .title(title)
        .section("1")
        .date(CLI_MANPAGE_DATE)
        .source(CLI_MANPAGE_SOURCE)
        .manual(CLI_MANPAGE_MANUAL);
    let mut bytes = Vec::new();
    manpage
        .render(&mut bytes)
        .expect("render the merman-cli manpage");
    assets.push(GeneratedAsset {
        relative_path: format!("assets/man/{filename}"),
        bytes: normalized_manpage_text(bytes),
    });
}

fn normalized_manpage_text(bytes: Vec<u8>) -> Vec<u8> {
    let text =
        String::from_utf8(normalized_generated_text(bytes)).expect("clap_mangen emits UTF-8");
    text.replace(".br\n\n.br\n", ".sp\n").into_bytes()
}

fn normalized_generated_text(bytes: Vec<u8>) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"\r\n") {
            trim_trailing_horizontal_whitespace(&mut normalized);
            normalized.push(b'\n');
            index += 2;
        } else if bytes[index] == b'\n' {
            trim_trailing_horizontal_whitespace(&mut normalized);
            normalized.push(b'\n');
            index += 1;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    trim_trailing_horizontal_whitespace(&mut normalized);
    normalized
}

fn trim_trailing_horizontal_whitespace(bytes: &mut Vec<u8>) {
    while bytes
        .last()
        .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
    {
        bytes.pop();
    }
}

#[test]
fn generated_text_uses_lf_without_trailing_horizontal_whitespace() {
    assert_eq!(
        normalized_generated_text(b"alpha \r\nbeta\t\ngamma ".to_vec()),
        b"alpha\nbeta\ngamma"
    );
}

#[test]
fn manpage_normalization_removes_only_the_mangen_empty_break_pair() {
    assert_eq!(
        normalized_manpage_text(b"description\r\n.br\r\n\r\n.br\r\nvalues\r\n".to_vec()),
        b"description\n.sp\nvalues\n"
    );
}

#[test]
fn manpage_date_is_a_real_iso_calendar_date() {
    let parsed = chrono::NaiveDate::parse_from_str(CLI_MANPAGE_DATE, "%Y-%m-%d")
        .expect("CLI_MANPAGE_DATE must be a valid YYYY-MM-DD date");
    assert_eq!(parsed.format("%Y-%m-%d").to_string(), CLI_MANPAGE_DATE);
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn asset_path(relative_path: &str) -> PathBuf {
    manifest_dir().join(relative_path)
}

#[test]
fn tracked_distribution_assets_are_current() {
    let assets = generated_assets();
    let expected = assets
        .iter()
        .map(|asset| asset.relative_path.clone())
        .collect::<BTreeSet<_>>();
    let actual = tracked_asset_paths().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        actual, expected,
        "generated CLI asset set is stale; run `python scripts/generate_cli_assets.py --write`"
    );

    for asset in assets {
        let path = asset_path(&asset.relative_path);
        let actual = std::fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "read generated CLI asset {}: {error}; run `python scripts/generate_cli_assets.py --write`",
                path.display()
            )
        });
        assert_eq!(
            actual,
            asset.bytes,
            "generated CLI asset {} is stale; run `python scripts/generate_cli_assets.py --write`",
            path.display()
        );
    }
}

#[test]
#[ignore = "maintainer-only snapshot update"]
fn write_distribution_assets() {
    assert_eq!(
        std::env::var_os(UPDATE_ENV).as_deref(),
        Some(std::ffi::OsStr::new("1")),
        "set {UPDATE_ENV}=1 through the repository generator"
    );
    let assets = generated_assets();
    let expected = assets
        .iter()
        .map(|asset| asset.relative_path.clone())
        .collect::<BTreeSet<_>>();
    for root in ASSET_ROOTS {
        std::fs::create_dir_all(asset_path(root))
            .unwrap_or_else(|error| panic!("create generated asset root {root}: {error}"));
    }
    let actual = tracked_asset_paths().unwrap_or_else(|error| panic!("{error}"));
    let stale = actual.difference(&expected).cloned().collect::<Vec<_>>();
    assert!(
        stale.is_empty(),
        "unexpected files exist under generated CLI asset roots: {stale:?}; review and remove obsolete generated assets explicitly"
    );

    for asset in assets {
        let path = asset_path(&asset.relative_path);
        let parent = path.parent().expect("generated asset has a parent");
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
        write_if_changed(&path, &asset.bytes);
    }
}

fn tracked_asset_paths() -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::new();
    for root in ASSET_ROOTS {
        collect_tracked_asset_paths(&asset_path(root), &mut paths)?;
    }
    Ok(paths)
}

fn collect_tracked_asset_paths(
    directory: &Path,
    paths: &mut BTreeSet<String>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(directory).map_err(|error| {
        format!(
            "read generated asset directory {}: {error}",
            directory.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("read entry under {}: {error}", directory.display()))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect generated asset {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "generated asset path {} must not be a symlink",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_tracked_asset_paths(&path, paths)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(format!(
                "generated asset path {} must be a regular file",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(manifest_dir())
            .map_err(|_| format!("generated asset {} escaped the crate", path.display()))?
            .to_str()
            .ok_or_else(|| format!("generated asset path {} is not UTF-8", path.display()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        paths.insert(relative);
    }
    Ok(())
}

fn write_if_changed(path: &Path, bytes: &[u8]) {
    if std::fs::read(path).ok().as_deref() == Some(bytes) {
        return;
    }
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}
