#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const SOURCE: &str = "flowchart LR\nA[Read] --> B[Export]\n";

fn paired_recipe() -> Value {
    json!({
        "schema_version": 1,
        "kind": "design",
        "design": {
            "mermaid": {"theme": "default", "dark_mode": false},
            "rules": [{"target": "node", "style": {"stroke": {"paint": "#52748c"}}}],
            "appearances": {
                "light": {
                    "canvas": {"base": "#faf7f0"},
                    "rules": [{"target": "node", "style": {"fill": "#fef8ea"}}]
                },
                "dark": {
                    "canvas": {"base": "#08101f"},
                    "rules": [{"target": "node", "style": {"fill": "#16324a"}}]
                }
            },
            "default_appearance": "dark"
        }
    })
}

fn setup(directory: &Path) {
    fs::write(directory.join("input.mmd"), SOURCE).unwrap();
    write_theme(directory, &paired_recipe());
}

fn write_theme(directory: &Path, theme: &Value) {
    fs::write(
        directory.join("theme.json"),
        serde_json::to_vec(theme).unwrap(),
    )
    .unwrap();
}

fn run(directory: &Path, args: &[&str]) -> Output {
    Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "exit={:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn report(directory: &Path) -> Value {
    serde_json::from_slice(&fs::read(directory.join("report.json")).unwrap()).unwrap()
}

fn render(directory: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "render",
        "input.mmd",
        "--theme-file",
        "theme.json",
        "--svg-id",
        "appearance-test",
        "--theme-report",
        "report.json",
        "--quiet",
        "-o",
        "-",
    ];
    args.extend_from_slice(extra);
    run(directory, &args)
}

fn assert_canvas(svg: &[u8], appearance: &str) {
    let svg = std::str::from_utf8(svg).unwrap();
    let document = roxmltree::Document::parse(svg).unwrap();
    let canvas = document
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-theme-canvas-base"))
        .expect("authored canvas");
    let (background, fill) = match appearance {
        "light" => ("#faf7f0", "#fef8ea"),
        "dark" => ("#08101f", "#16324a"),
        _ => panic!("unknown test appearance"),
    };
    assert_eq!(canvas.attribute("fill"), Some(background));
    assert!(svg.contains(fill), "{appearance} node fill: {svg}");
    assert!(svg.contains("#52748c"), "shared node outline: {svg}");
}

#[test]
fn one_recipe_selects_both_appearances_and_uses_its_declared_default() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());

    let default = render(directory.path(), &[]);
    success(&default);
    assert_canvas(&default.stdout, "dark");
    assert_eq!(
        report(directory.path())["results"][0]["selected_appearance"],
        "dark"
    );

    for appearance in ["light", "dark"] {
        let output = render(directory.path(), &["--theme-appearance", appearance]);
        success(&output);
        assert_canvas(&output.stdout, appearance);
        let evidence = report(directory.path());
        assert_eq!(evidence["results"][0]["selected_appearance"], appearance);
        assert_eq!(
            evidence["results"][0]["evidence"]["materialized_theme"], "default",
            "a Merman appearance must not select a Mermaid theme or darkMode"
        );
        if appearance == "dark" {
            assert_eq!(output.stdout, default.stdout);
        }

        write_theme(
            directory.path(),
            &json!({"recipe": paired_recipe(), "appearance": appearance}),
        );
        for extra in [vec![], vec!["--theme-appearance", appearance]] {
            let wrapped = render(directory.path(), &extra);
            success(&wrapped);
            assert_eq!(wrapped.stdout, output.stdout);
            assert_eq!(
                report(directory.path())["results"][0]["selected_appearance"],
                appearance
            );
        }
        write_theme(directory.path(), &paired_recipe());
    }
}

#[test]
fn grouped_preset_and_file_selection_agree_with_the_existing_concrete_route() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for (appearance, legacy) in [("light", "editor-light"), ("dark", "editor-dark")] {
        let args = [
            "render",
            "input.mmd",
            "--theme-preset",
            "editor",
            "--theme-appearance",
            appearance,
            "--svg-id",
            "appearance-test",
            "--quiet",
            "-o",
            "-",
        ];
        let grouped = run(directory.path(), &args);
        success(&grouped);
        let pinned = run(
            directory.path(),
            &[
                "render",
                "input.mmd",
                "--theme-preset",
                legacy,
                "--svg-id",
                "appearance-test",
                "--quiet",
                "-o",
                "-",
            ],
        );
        success(&pinned);
        assert_eq!(grouped.stdout, pinned.stdout);

        for selection in [
            json!({"preset": "editor"}),
            json!({"preset": "editor", "appearance": appearance}),
            json!({"preset": legacy, "appearance": appearance}),
        ] {
            write_theme(directory.path(), &selection);
            let output = render(directory.path(), &["--theme-appearance", appearance]);
            success(&output);
            assert_eq!(output.stdout, grouped.stdout);
            assert_eq!(
                report(directory.path())["results"][0]["selected_appearance"],
                appearance
            );
        }
    }
}

#[test]
fn report_keeps_the_mermaid_palette_separate_from_the_merman_appearance() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    let output = render(
        directory.path(),
        &["--theme", "dark", "--theme-appearance", "light"],
    );
    success(&output);
    assert_canvas(&output.stdout, "light");
    let report = report(directory.path());
    assert_eq!(report["results"][0]["selected_appearance"], "light");
    assert_eq!(
        report["results"][0]["evidence"]["materialized_theme"],
        "dark"
    );
}

#[test]
fn invalid_appearance_selections_preserve_the_previous_image() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    let mut light_only = paired_recipe();
    light_only["design"]["appearances"]
        .as_object_mut()
        .unwrap()
        .remove("dark");
    light_only["design"]["default_appearance"] = json!("light");
    let mut invalid_inactive = paired_recipe();
    invalid_inactive["design"]["appearances"]["light"]["future"] = json!(true);
    for selection in [
        json!({"recipe": paired_recipe(), "appearance": "light"}),
        json!({"preset": "editor", "appearance": "light"}),
        json!({"preset": "editor-light"}),
        json!({"recipe": light_only}),
        json!({"recipe": invalid_inactive}),
        json!({"schema_version": 1, "kind": "design", "design": {}}),
        json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}}),
        json!({"spec": {}}),
        json!({"appearance": "dark"}),
        json!({"preset": "editor", "appearance": null}),
        json!({"recipe": paired_recipe(), "appearance": "auto"}),
    ] {
        write_theme(directory.path(), &selection);
        fs::write(directory.path().join("image.svg"), b"previous image").unwrap();
        let output = run(
            directory.path(),
            &[
                "render",
                "input.mmd",
                "--theme-file",
                "theme.json",
                "--theme-appearance",
                "dark",
                "--output",
                "image.svg",
                "--quiet",
            ],
        );
        assert_eq!(output.status.code(), Some(2), "{selection}: {output:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(directory.path().join("image.svg")).unwrap(),
            b"previous image",
            "{selection}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("invalid theme selection file"), "{stderr}");
    }
}

#[test]
fn theme_files_keep_duplicate_fields_visible_to_shared_admission() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for bytes in [
        r#"{"preset":"editor","appearance":"light","appearance":"dark"}"#,
        r#"{"preset":"editor","preset":"editor-dark"}"#,
        r#"{"recipe":{"schema_version":1,"kind":"design","design":{"rules":[],"rules":[]}}}"#,
        r#"{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"light":{}},"default_appearance":"light"}}"#,
    ] {
        fs::write(directory.path().join("theme.json"), bytes).unwrap();
        for extra in [vec![], vec!["--theme-appearance", "dark"]] {
            let output = render(directory.path(), &extra);
            assert_eq!(output.status.code(), Some(2), "{bytes}: {output:?}");
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("duplicate"), "{bytes}: {stderr}");
        }
    }
}

#[test]
fn unclassified_recipes_and_specs_render_without_guessing_an_appearance() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for selection in [
        json!({"schema_version": 1, "kind": "design", "design": {}}),
        json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}}),
        json!({"recipe": {"schema_version": 1, "kind": "design", "design": {}}}),
        json!({"spec": {}}),
    ] {
        write_theme(directory.path(), &selection);
        let output = render(directory.path(), &[]);
        success(&output);
        assert!(
            report(directory.path())["results"][0]
                .get("selected_appearance")
                .is_none()
        );
        let rejected = render(directory.path(), &["--theme-appearance", "light"]);
        assert_eq!(rejected.status.code(), Some(2), "{selection}: {rejected:?}");
        assert!(rejected.stdout.is_empty());
    }
}

#[test]
fn appearance_requires_a_merman_selection_and_never_accepts_auto() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        vec!["render", "missing.mmd", "--theme-appearance", "dark"],
        vec![
            "render",
            "missing.mmd",
            "--theme",
            "dark",
            "--theme-appearance",
            "light",
        ],
        vec![
            "render",
            "missing.mmd",
            "--theme-file",
            "missing.json",
            "--theme-appearance",
            "auto",
        ],
        vec![
            "render",
            "missing.mmd",
            "--theme-preset",
            "editor",
            "--theme-appearance",
            "system",
        ],
        vec![
            "render",
            "missing.mmd",
            "--theme-design-file",
            "missing.json",
            "--theme-appearance",
            "dark",
        ],
        #[cfg(feature = "markdown")]
        vec!["batch", "missing.md", "--theme-appearance", "dark"],
    ] {
        let output = run(directory.path(), &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--theme-appearance"), "{stderr}");
        assert!(
            !stderr.contains("failed to"),
            "argument validation must precede file IO: {stderr}"
        );
    }
}

#[test]
fn every_graphical_format_uses_the_same_selected_appearance_and_report() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    for (format, signature) in [
        ("svg", b"<svg".as_slice()),
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        #[cfg(feature = "jpeg")]
        ("jpg", b"\xff\xd8".as_slice()),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF-".as_slice()),
    ] {
        let output = render(
            directory.path(),
            &["--format", format, "--theme-appearance", "light"],
        );
        success(&output);
        assert!(output.stdout.starts_with(signature), "{format}");
        assert!(output.stderr.is_empty(), "{format}");
        let report = report(directory.path());
        assert_eq!(report["results"][0]["selected_appearance"], "light");
        assert_eq!(
            report["results"][0]["evidence"]["materialized_theme"],
            "default"
        );
        assert_eq!(
            report["results"][0]["target"]["kind"],
            if format == "jpg" { "jpeg" } else { format }
        );
    }
}

#[cfg(feature = "markdown")]
#[test]
fn batch_replaces_both_images_without_changing_manifest_ownership() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    fs::write(
        directory.path().join("input.md"),
        format!("```mermaid\n{SOURCE}```\n\n```mermaid\n{SOURCE}```\n"),
    )
    .unwrap();
    let mut previous_inventory = None;
    for appearance in ["light", "dark"] {
        let output = run(
            directory.path(),
            &[
                "batch",
                "input.md",
                "--output-dir",
                "generated",
                "--theme-file",
                "theme.json",
                "--theme-appearance",
                appearance,
                "--theme-report",
                "report.json",
                "--quiet",
            ],
        );
        success(&output);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        let report = report(directory.path());
        let results = report["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        for (index, result) in results.iter().enumerate() {
            let filename = format!("input-{}.svg", index + 1);
            assert_eq!(result["chart"], index + 1);
            assert_eq!(result["selected_appearance"], appearance);
            assert!(result["output"].as_str().unwrap().ends_with(&filename));
            assert_canvas(
                &fs::read(directory.path().join("generated").join(filename)).unwrap(),
                appearance,
            );
        }
        let manifest: Value = serde_json::from_slice(
            &fs::read(directory.path().join("generated/.merman-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["owner"]["dialect"], "native_batch_v1");
        assert_eq!(manifest["artifacts"].as_array().unwrap().len(), 2);
        if let Some(previous) = previous_inventory {
            assert_eq!(manifest["artifacts"], previous);
        }
        previous_inventory = Some(manifest["artifacts"].clone());
    }
}

#[cfg(feature = "markdown")]
#[test]
fn empty_batches_still_reject_invalid_or_conflicting_appearance_selections() {
    let directory = tempfile::tempdir().unwrap();
    setup(directory.path());
    fs::write(directory.path().join("empty.md"), "# No diagrams\n").unwrap();
    write_theme(
        directory.path(),
        &json!({"recipe": paired_recipe(), "appearance": "light"}),
    );
    let output = run(
        directory.path(),
        &[
            "batch",
            "empty.md",
            "--output-dir",
            "generated",
            "--theme-file",
            "theme.json",
            "--theme-appearance",
            "dark",
            "--quiet",
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicts"));
    assert!(
        !directory
            .path()
            .join("generated/.merman-manifest.json")
            .exists()
    );
}
