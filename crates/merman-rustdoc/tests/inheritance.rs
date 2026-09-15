#![cfg(feature = "svg")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod support;
use support::proc_macro_artifact;

#[test]
fn nested_macros_inherit_explicitly_override_and_reset_parent_configuration() {
    let temp = TempDir::new();
    let source = temp.0.join("lib.rs");
    fs::write(&source, r####"
#[merman_rustdoc::merman(scope = "tree", theme = "dark", background = "#123456", source = "details", id_prefix = "parent")]
pub mod parent {
    use merman_rustdoc::merman;
    use merman_rustdoc::merman as diagram;

    #[merman_rustdoc::merman(theme = "forest")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn local() {}

    #[merman(theme = "forest")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn imported() {}

    #[diagram(background = "#aabbcc")]
    #[merman(source = "hide")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn stacked() {}

    #[cfg_attr(any(), merman_rustdoc::merman(inherit = "invalid"))]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn inactive() {}

    #[cfg_attr(all(), merman_rustdoc::merman(background = "#445566"))]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn active() {}

    #[merman_rustdoc::merman(inherit = "off")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub fn reset() {}

    #[diagram(scope = "tree", background = "#abcdef")]
    pub mod nested {
        #[merman_rustdoc::merman(source = "hide")]
        /// ```mermaid
        /// flowchart TD
        /// A-->B
        /// ```
        pub fn grandchild() {}
    }

    #[merman_rustdoc::merman(scope = "item", background = "#987654")]
    /// ```mermaid
    /// flowchart TD
    /// A-->B
    /// ```
    pub mod item_only {
        /// ```mermaid
        /// flowchart TD
        /// A-->B
        /// ```
        pub fn descendant() {}
    }

    #[cfg_attr(all(), cfg_attr(all(), merman_rustdoc::merman(scope = "tree", inherit = "off")))]
    pub mod reset_tree {
        #[merman_rustdoc::merman(theme = "forest")]
        /// ```mermaid
        /// flowchart TD
        /// A-->B
        /// ```
        pub fn descendant() {}
    }
    #[diagram(scope = "item", background = "#fedcba")]
    #[diagram(scope = "tree", source = "hide")]
    pub mod no_documentation {
        /// ```mermaid
        /// flowchart TD
        /// A-->B
        /// ```
        pub fn descendant() {}
    }
}
"####).unwrap();
    let (dependencies, artifact) = proc_macro_artifact();
    let output = Command::new(std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into()))
        .arg("--edition=2024")
        .args(["--crate-name", "inheritance"])
        .arg("--extern")
        .arg(format!("merman_rustdoc={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("-o")
        .arg(temp.0.join("doc"))
        .arg(source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustdoc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (page, background, details, prefix, count) in [
        ("fn.local.html", "#123456", true, true, 1),
        ("fn.imported.html", "#123456", true, true, 1),
        ("fn.stacked.html", "#aabbcc", false, true, 1),
        ("fn.inactive.html", "#123456", true, true, 1),
        ("fn.active.html", "#445566", true, true, 1),
        ("fn.reset.html", "transparent", false, false, 2),
        ("nested/fn.grandchild.html", "#abcdef", false, true, 1),
        ("item_only/index.html", "#987654", true, true, 1),
        ("item_only/fn.descendant.html", "#123456", true, true, 1),
        (
            "reset_tree/fn.descendant.html",
            "transparent",
            false,
            false,
            1,
        ),
        (
            "no_documentation/fn.descendant.html",
            "#123456",
            false,
            true,
            1,
        ),
    ] {
        let html = fs::read_to_string(temp.0.join("doc/inheritance/parent").join(page)).unwrap();
        let html = if page.ends_with("index.html") {
            html.split_once("<h2 id=\"functions\"")
                .expect("module has a function section after its own documentation")
                .0
        } else {
            &html
        };
        assert_eq!(
            html.contains("class=\"merman-rustdoc-source\""),
            details,
            "source display in {page}"
        );
        let mut actual_count = 0;
        for segment in html.split("<svg").skip(1) {
            let end = segment.find("</svg>").unwrap() + "</svg>".len();
            let svg = format!("<svg{}", &segment[..end]);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = document.root_element();
            if root.attribute("aria-roledescription").is_none() {
                continue;
            }
            actual_count += 1;
            let actual_background = root
                .attribute("style")
                .unwrap_or_default()
                .split(';')
                .filter_map(|rule| rule.split_once(':'))
                .find_map(|(name, value)| {
                    (name.trim() == "background-color").then_some(value.trim())
                });
            assert_eq!(actual_background, Some(background), "background in {page}");
            assert_eq!(
                root.attribute("id").unwrap().contains("parent"),
                prefix,
                "ID prefix in {page}"
            );
        }
        assert_eq!(actual_count, count, "theme selection in {page}");
    }
}

#[test]
fn tree_scope_preserves_missing_documentation_diagnostics() {
    let temp = TempDir::new();
    let source = temp.0.join("lib.rs");
    fs::write(
        &source,
        r#"
#![deny(missing_docs)]
//! A documented crate.

#[merman_rustdoc::merman(scope = "tree")]
/// A documented module.
pub mod documented {
    pub fn undocumented() {}
}
"#,
    )
    .unwrap();
    let (dependencies, artifact) = proc_macro_artifact();
    let output = Command::new(std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into()))
        .arg("--edition=2024")
        .args(["--crate-name", "missing_documentation"])
        .arg("--extern")
        .arg(format!("merman_rustdoc={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("-o")
        .arg(temp.0.join("doc"))
        .arg(source)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "missing_docs must reject undocumented children"
    );
    assert!(
        stderr.contains("missing documentation for a function"),
        "{stderr}"
    );
    assert!(stderr.contains("undocumented"), "{stderr}");
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("merman-inheritance-{}-{now}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
