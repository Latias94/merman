//! Helpers for Mermaid-like icon substitutions inside HTML-ish labels.

use regex::Regex;
use std::sync::OnceLock;

pub fn replace_fontawesome_icons(input: &str) -> String {
    // Fast path: avoid the regex engine for the common case (no icon markers).
    if !input.contains(":fa-") {
        return input.to_string();
    }
    // Mermaid `rendering-util/createText.ts::replaceIconSubstring()` converts icon notations like:
    //   `fa:fa-user` -> `<i class="fa fa-user"></i>`
    //
    // Mermaid@11.12.2 upstream SVG baselines use double quotes for the class attribute.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"(fa[bklrs]?):fa-([A-Za-z0-9_-]+)").expect("valid regex"));

    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("fa");
        let icon = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!(r#"<i class="{prefix} fa-{icon}"></i>"#)
    })
    .to_string()
}
