#[cfg(test)]
mod color_tests {
    use merman_render_core::rough_svg::parse_hex_color_to_srgba;

    #[test]
    fn malformed_hex_color_never_slices_inside_utf8() {
        for value in ["#😀ab", "#éa", "#猫", "#ééé", "#abcdefg", "#ggg"] {
            assert!(parse_hex_color_to_srgba(value).is_none(), "{value}");
        }
        for value in ["#abc", "#ABCDEF", " #ff0000 ", "red", "hsl(180 50% 50%)"] {
            assert!(parse_hex_color_to_srgba(value).is_some(), "{value}");
        }
    }
}
