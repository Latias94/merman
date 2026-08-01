use manatee::algo::fcose::IndexedFcoseOptions;
use manatee::{Algorithm, FcoseOptions, FcoseRandomPolicy, FcoseRandomSource};

#[test]
fn cose_bilkent_algorithm_has_no_phantom_options() {
    let algorithm = Algorithm::CoseBilkent;

    assert!(matches!(algorithm, Algorithm::CoseBilkent));
}

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
    let seed: u64 = policy.seed();
    assert_eq!(seed, 1);
}
