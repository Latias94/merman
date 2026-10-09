// Style parsing helpers (split from parity.rs).

pub(super) fn is_text_style_key(key: &str) -> bool {
    crate::mermaid_style::is_label_style_key(key)
}
