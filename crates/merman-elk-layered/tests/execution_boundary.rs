use merman_elk_layered::{
    LGraph, LayeredOptions, PipelineError, RandomSeedError, execute_ported_processors,
};

#[test]
fn external_raw_graph_execution_rejects_elk_zero_seed() {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            random_seed: 0,
            ..Default::default()
        },
    );

    assert!(matches!(
        execute_ported_processors(&mut graph),
        Err(PipelineError::RandomSeed(RandomSeedError::Unresolved { graph_path }))
            if graph_path == "root"
    ));
}
