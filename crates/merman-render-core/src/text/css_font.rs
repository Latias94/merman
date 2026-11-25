pub fn is_css_wide_keyword(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "inherit" | "initial" | "revert" | "revert-layer" | "unset"
    )
}

pub fn resolve_css_font_weight(value: &str, inherited: u16) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "bolder" => Some(relative_font_weight(inherited, true)),
        "lighter" => Some(relative_font_weight(inherited, false)),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight)),
    }
}

fn relative_font_weight(inherited: u16, bolder: bool) -> u16 {
    match (inherited, bolder) {
        (1..=99, true) => 400,
        (1..=99, false) => inherited,
        (100..=349, true) => 400,
        (100..=349, false) => 100,
        (350..=549, true) => 700,
        (350..=549, false) => 100,
        (550..=749, true) => 900,
        (550..=749, false) => 400,
        (750..=899, true) => 900,
        (750..=899, false) => 700,
        (900..=1000, true) => inherited,
        (900..=1000, false) => 700,
        _ => inherited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_legacy_font_weights_resolve_against_the_theme_weight() {
        for (inherited, bolder, lighter) in [
            (1, 400, 1),
            (99, 400, 99),
            (100, 400, 100),
            (349, 400, 100),
            (350, 700, 100),
            (549, 700, 100),
            (550, 900, 400),
            (749, 900, 400),
            (750, 900, 700),
            (899, 900, 700),
            (900, 900, 700),
            (1000, 1000, 700),
        ] {
            assert_eq!(resolve_css_font_weight("bolder", inherited), Some(bolder));
            assert_eq!(resolve_css_font_weight("lighter", inherited), Some(lighter));
        }
        assert_eq!(resolve_css_font_weight("calc(400)", 400), None);
    }
}
