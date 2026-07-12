use manatee::algo::fcose::IndexedFcoseOptions;
use manatee::{FcoseOptions, FcoseRandomPolicy, FcoseRandomSource};

#[test]
fn fcose_option_struct_literals_remain_source_compatible() {
    let _graph_options = FcoseOptions {
        random_seed: 1,
        random_seed_offset: None,
        rerun: false,
        randomize: true,
        node_separation: None,
        num_iter: None,
        default_edge_length: None,
        alignment_constraint: None,
        relative_placement_constraint: Vec::new(),
        compound_padding: None,
        relocate_center: None,
    };
    let _indexed_options = IndexedFcoseOptions {
        random_seed: 1,
        random_seed_offset: None,
        rerun: false,
        randomize: true,
        node_separation: None,
        num_iter: None,
        default_edge_length: None,
        alignment_constraint: None,
        relative_placement_constraint: Vec::new(),
        compound_padding: None,
        relocate_center: None,
    };

    let policy = FcoseRandomPolicy::seeded(FcoseRandomSource::Mulberry32, 1)
        .with_seed_offset(0)
        .with_reset_seed_each_run(true);
    assert_eq!(policy.seed(), Some(1));
}
