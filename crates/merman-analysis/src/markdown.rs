use std::path::Path;

pub fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(is_markdown_extension)
}

pub fn is_markdown_extension(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("md")
        || ext.eq_ignore_ascii_case("markdown")
        || ext.eq_ignore_ascii_case("mdx")
}

pub fn is_mdx_extension(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("mdx")
}
