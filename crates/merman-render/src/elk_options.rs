use crate::config::ConfigLookup;
use crate::layout_backend::ElkRootAlgorithm;

pub(crate) fn layout_options(
    config: &(impl ConfigLookup + ?Sized),
    algorithm: ElkRootAlgorithm,
) -> merman_layout_elk::LayoutOptions {
    merman_render_core::elk_adapter::options::layout_options(config, algorithm.algorithm())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_layout_elk::Algorithm;
    use serde_json::json;

    #[test]
    fn registered_root_names_select_the_corresponding_provider() {
        for (name, algorithm) in [
            ("elk", Algorithm::Layered),
            ("elk.stress", Algorithm::Stress),
            ("elk.force", Algorithm::Force),
            ("elk.mrtree", Algorithm::MrTree),
            ("elk.sporeOverlap", Algorithm::SporeOverlap),
            ("elk.box", Algorithm::Box),
            ("elk.rectpacking", Algorithm::Rectpacking),
        ] {
            let config = json!({"layout": name});
            assert_eq!(
                layout_options(&config, ElkRootAlgorithm::for_diagnostic(&config)).algorithm,
                algorithm
            );
        }
    }
}
