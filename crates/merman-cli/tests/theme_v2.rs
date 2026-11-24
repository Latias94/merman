#![cfg(feature = "svg")]

mod support;

use merman::theme::{BuiltinPreset, ThemeCompiler};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
#[cfg(feature = "diagram-flowchart")]
use support::run_with_stdin_in_dir;
use support::{exit_code, run_with_stdin};

#[cfg(feature = "diagram-flowchart")]
const DOCUMENT: &str = r#"{"schema_version":2,"extends":"studio"}"#;
#[cfg(feature = "diagram-flowchart")]
const SOURCE: &str = "flowchart LR\nA --> B\n";

fn run(directory: &Path, args: &[&str]) -> Output {
    Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(directory)
        .args(args)
        .output()
        .expect("run merman-cli")
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

#[test]
fn list_uses_the_shared_catalog_and_keeps_piped_output_plain() {
    let directory = tempfile::tempdir().unwrap();
    let machine = run(directory.path(), &["theme", "list", "--json"]);
    success(&machine);
    let catalog = json(&machine);
    assert_eq!(
        catalog,
        serde_json::to_value(merman::theme::catalog().unwrap()).unwrap()
    );
    assert_eq!(catalog["schema_version"], 2);
    assert_eq!(catalog["presets"].as_array().unwrap().len(), 13);
    assert_eq!(
        catalog["presets"][0]["swatches"].as_array().unwrap().len(),
        6
    );
    assert_eq!(
        catalog["mermaid_themes"],
        serde_json::json!(merman::supported_themes())
    );

    let human = run(directory.path(), &["theme", "list"]);
    success(&human);
    let text = String::from_utf8(human.stdout).unwrap();
    for preset in BuiltinPreset::ALL {
        assert!(text.lines().any(|line| line.starts_with(preset.as_str())));
    }
    assert!(text.contains("Studio") && text.contains("light/dark"));
    assert!(!text.contains('\x1b'));
}

#[test]
fn full_and_minimal_exports_check_to_the_same_palette() {
    let directory = tempfile::tempdir().unwrap();
    let full = run(directory.path(), &["theme", "export", "studio"]);
    success(&full);
    assert_eq!(json(&full)["id"], "studio");
    assert_eq!(json(&full)["schema_version"], 2);
    assert!(String::from_utf8_lossy(&full.stdout).lines().count() > 20);

    let minimal = run(
        directory.path(),
        &["theme", "export", "studio", "--minimal", "-o", "theme.json"],
    );
    success(&minimal);
    assert!(minimal.stdout.is_empty());
    let bytes = fs::read(directory.path().join("theme.json")).unwrap();
    assert!(bytes.len() < full.stdout.len());
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["extends"],
        "studio"
    );

    let full_check = run_with_stdin(
        &["theme", "check", "-", "--appearance", "dark", "--json"],
        std::str::from_utf8(&full.stdout).unwrap(),
    );
    let minimal_check = run(
        directory.path(),
        &[
            "theme",
            "check",
            "theme.json",
            "--appearance",
            "dark",
            "--json",
        ],
    );
    success(&full_check);
    success(&minimal_check);
    let full = json(&full_check);
    let minimal = json(&minimal_check);
    assert_eq!(minimal["ok"], true);
    assert_eq!(minimal["resolved"]["appearance"], "dark");
    assert_eq!(minimal["resolved"]["roles"], full["resolved"]["roles"]);
    assert_eq!(minimal["contrast"], full["contrast"]);
    assert_eq!(minimal["contrast"].as_array().unwrap().len(), 12);

    let text = run(directory.path(), &["theme", "check", "theme.json"]);
    success(&text);
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("WCAG ratio") && text.contains("text.primary"));
}

#[test]
fn check_reports_syntax_location_and_strict_contrast() {
    let bad_color = "{\n  \"schema_version\": 2,\n  \"extends\": \"studio\",\n  \"seed\": {\"accent\": 17}\n}\n";
    let output = run_with_stdin(&["theme", "check", "-", "--json"], bad_color);
    assert_eq!(exit_code(output.status), 2);
    let error = json(&output);
    assert_eq!(error["ok"], false);
    assert_eq!(error["line"], 4);
    assert!(error["column"].as_u64().unwrap() > 0);
    assert_eq!(error["path"], "seed.accent");

    let duplicate = run_with_stdin(
        &["theme", "check", "-", "--json"],
        r#"{"schema_version":2,"extends":"studio","extends":"paper"}"#,
    );
    assert_eq!(exit_code(duplicate.status), 2);
    assert!(
        json(&duplicate)["message"]
            .as_str()
            .unwrap()
            .contains("duplicate key")
    );

    let low_contrast = r##"{
        "schema_version": 2,
        "extends": "studio",
        "roles": {"text.primary": "#FFFFFF"}
    }"##;
    let advisory = run_with_stdin(&["theme", "check", "-", "--json"], low_contrast);
    success(&advisory);
    assert_eq!(json(&advisory)["ok"], true);
    let strict = run_with_stdin(
        &["theme", "check", "-", "--strict-contrast", "--json"],
        low_contrast,
    );
    assert_eq!(exit_code(strict.status), 1);
    let result = json(&strict);
    assert_eq!(result["ok"], false);
    assert_eq!(result["contrast"].as_array().unwrap().len(), 12);
    assert!(result["notices"].as_array().unwrap().iter().any(|notice| {
        notice["kind"] == "low_contrast" && notice["foreground"] == "text.primary"
    }));
}

#[test]
fn check_reports_the_same_gradient_contrast_in_rows_and_notices() {
    let document = r##"{
        "schema_version": 2,
        "extends": "studio",
        "roles": {
            "canvas": {"kind": "linear_gradient", "angle_deg": 90, "stops": [
                {"offset": 0, "color": "#FF0000"}, {"offset": 1, "color": "#008A00"}
            ]},
            "stroke.connector": {"kind": "linear_gradient", "stops": [
                {"offset": 0, "color": "#000000"}, {"offset": 1, "color": "#FFFFFF"}
            ]},
            "text.primary": "#000000",
            "text.muted": "#000000",
            "text.title": "#000000"
        },
        "appearances": {"light": {"data": {
            "ink": ["#000000", "#000000", "#000000", "#000000"],
            "fill": ["#FFFFFF", "#FFFFFF", "#FFFFFF", "#FFFFFF"],
            "fill_ink": "#000000"
        }}}
    }"##;
    let strict = run_with_stdin(
        &["theme", "check", "-", "--strict-contrast", "--json"],
        document,
    );
    assert_eq!(exit_code(strict.status), 1);
    let result = json(&strict);
    assert_eq!(result["ok"], false);
    let rows = result["contrast"].as_array().unwrap();
    assert!(
        rows.iter()
            .all(|row| row["ratio"].as_f64().is_some_and(f64::is_finite))
    );
    let canvas_ratio = |foreground: &str| {
        rows.iter()
            .find(|row| row["foreground"] == foreground && row["background"] == "canvas")
            .and_then(|row| row["ratio"].as_f64())
            .unwrap()
    };
    let ratio = canvas_ratio("text.primary");
    assert!((ratio - 2.75317).abs() < 0.001, "{ratio}");
    assert_eq!(canvas_ratio("stroke.connector"), 1.0);
    let notice = result["notices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|notice| {
            notice["kind"] == "low_contrast"
                && notice["foreground"] == "text.primary"
                && notice["background"] == "canvas"
        })
        .unwrap();
    assert_eq!(notice["ratio"].as_f64().unwrap(), ratio);
    assert_eq!(notice["minimum"], 4.5);
}

#[test]
fn document_limits_apply_before_decoding() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("theme.json");
    fs::File::create(&path)
        .unwrap()
        .set_len(ThemeCompiler::new().max_bytes() as u64 + 1)
        .unwrap();
    let output = run(directory.path(), &["theme", "check", "theme.json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("max_theme_encoded_bytes"));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn custom_theme_file_and_stdin_select_the_same_appearance() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("diagram.mmd"), SOURCE).unwrap();
    fs::write(directory.path().join("theme.JSON"), DOCUMENT).unwrap();
    let render = |theme, appearance| {
        run_with_stdin_in_dir(
            &[
                "render",
                "diagram.mmd",
                "-o",
                "-",
                "--theme",
                theme,
                "--appearance",
                appearance,
            ],
            DOCUMENT,
            Some(directory.path()),
        )
    };
    let file = render("theme.JSON", "dark");
    let stdin = render("-", "dark");
    let light = render("studio", "light");
    for output in [&file, &stdin, &light] {
        success(output);
    }
    assert_eq!(file.stdout, stdin.stdout);
    assert_ne!(canvas_fill(&light.stdout), canvas_fill(&file.stdout));
    assert!(canvas_fill(&file.stdout).is_some());

    let conflict = run_with_stdin(&["render", "-", "--theme", "-"], DOCUMENT);
    assert_eq!(exit_code(conflict.status), 2);
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("cannot both read from stdin"));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn native_theme_selection_and_background_follow_precedence() {
    let source = format!("---\nconfig:\n  theme: forest\n---\n{SOURCE}");
    let host = run_with_stdin(&["render", "-", "--theme", "studio"], &source);
    let source_first = run_with_stdin(
        &[
            "render",
            "-",
            "--theme",
            "studio",
            "--theme-precedence",
            "source",
        ],
        &source,
    );
    let background = run_with_stdin(
        &[
            "render",
            "-",
            "--theme",
            "studio",
            "--background",
            "#112233",
        ],
        &source,
    );
    let transparent = run_with_stdin(
        &[
            "render",
            "-",
            "--theme",
            "studio",
            "--background",
            "transparent",
        ],
        &source,
    );
    for output in [&host, &source_first, &background, &transparent] {
        success(output);
    }
    assert!(canvas_fill(&host.stdout).is_some());
    assert!(canvas_fill(&source_first.stdout).is_none());
    assert!(
        String::from_utf8_lossy(&source_first.stderr).contains("source selected a Mermaid theme")
    );
    assert_eq!(canvas_fill(&background.stdout).as_deref(), Some("#112233"));
    assert!(canvas_fill(&transparent.stdout).is_none());
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn mermaid_selector_overrides_config_and_honors_source_precedence() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("config.json"),
        r#"{"theme":"dark","flowchart":{"theme":"dark"}}"#,
    )
    .unwrap();
    let render = |args: &[&str], source| {
        let mut command = vec!["render", "-"];
        command.extend_from_slice(args);
        let output = run_with_stdin_in_dir(&command, source, Some(directory.path()));
        success(&output);
        let svg = std::str::from_utf8(&output.stdout).unwrap();
        let document = roxmltree::Document::parse(svg).unwrap();
        document
            .root_element()
            .children()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap()
            .to_owned()
    };
    let authored = format!("---\nconfig:\n  theme: dark\n---\n{SOURCE}");
    let forest = render(&["--theme", "forest"], SOURCE);
    let dark = render(&["--theme", "dark"], SOURCE);
    assert_ne!(forest, dark);
    assert_eq!(
        forest,
        render(
            &["--theme", "forest", "--config-file", "config.json"],
            SOURCE
        )
    );
    assert_eq!(forest, render(&["--theme", "forest"], &authored));
    assert_eq!(
        dark,
        render(
            &["--theme", "forest", "--theme-precedence", "source"],
            &authored
        )
    );
}

#[cfg(feature = "diagram-flowchart")]
fn canvas_fill(svg: &[u8]) -> Option<String> {
    let document = roxmltree::Document::parse(std::str::from_utf8(svg).unwrap()).unwrap();
    document
        .root_element()
        .children()
        .find(|node| node.attribute("class") == Some("merman-theme-canvas"))?
        .children()
        .find(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("fill")
                    .is_some_and(|fill| fill.starts_with('#'))
        })
        .and_then(|node| node.attribute("fill"))
        .map(str::to_owned)
}

#[cfg(all(feature = "ascii", feature = "diagram-flowchart"))]
#[test]
fn terminal_color_projects_the_native_appearance() {
    for format in ["ascii", "unicode"] {
        let render = |appearance, color| {
            run_with_stdin(
                &[
                    "render",
                    "-",
                    "-f",
                    format,
                    "--theme",
                    "studio",
                    "--appearance",
                    appearance,
                    "--ascii-color",
                    color,
                ],
                SOURCE,
            )
        };
        let light = render("light", "truecolor");
        let dark = render("dark", "truecolor");
        let plain_light = render("light", "plain");
        let plain_dark = render("dark", "plain");
        for output in [&light, &dark, &plain_light, &plain_dark] {
            success(output);
        }
        assert!(light.stdout.contains(&0x1b) && dark.stdout.contains(&0x1b));
        assert_ne!(light.stdout, dark.stdout);
        assert_eq!(plain_light.stdout, plain_dark.stdout);
    }
}
