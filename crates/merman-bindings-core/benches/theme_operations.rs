use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use merman::svg::{DiagramThemeCompiler, ThemePreset, ThemeResourcePolicy};
use merman_bindings_core::{BindingEngine, compile_theme_definition_json_with};
use merman_theme_authoring_fixtures::THEME_AUTHORING_GOLDEN_VECTORS_V1;
use merman_theme_contract::{MaterializedThemeWireV1, ThemeRecipeV1};
use serde_json::Value;
use std::hint::black_box;

const OPTIONS: &[u8] =
    br#"{"version":3,"runtime_policy":"deterministic","resources":{"profile":"interactive"}}"#;
const DEFAULT_DEFINITION: &[u8] =
    br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#;
const SUPPORT_VECTORS: &[u8] =
    include_bytes!("../../merman-theme-authoring-fixtures/fixtures/authoring-v1/support.json");
const PRESET_CATALOG: &[u8] = include_bytes!(
    "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/preset-catalog.json"
);

fn bench_theme_operations(criterion: &mut Criterion) {
    let engine = BindingEngine::from_options(OPTIONS).expect("binding engine");
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::interactive());

    // Validate shared contracts before measuring any operation. JSON decoding and assertions
    // below are setup work; the measured public operations still include their own admission.
    let mut definitions = vec![("default", DEFAULT_DEFINITION)];
    for vector in THEME_AUTHORING_GOLDEN_VECTORS_V1 {
        let materialized: MaterializedThemeWireV1 = serde_json::from_slice(
            &engine
                .materialize_theme(vector.readable_definition_json())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            materialized.spec().canonical_json_bytes().unwrap(),
            vector.canonical_spec_json(),
        );
        let compiled =
            compile_theme_definition_json_with(&compiler, vector.readable_definition_json())
                .unwrap();
        let complete = compiler
            .compile_spec_wire(materialized.into_spec())
            .unwrap();
        assert_eq!(compiled.recipe_fingerprint(), complete.recipe_fingerprint());
        definitions.push((vector.name(), vector.readable_definition_json()));
    }
    compile_theme_definition_json_with(&compiler, DEFAULT_DEFINITION).unwrap();

    let support_vectors: Vec<Value> = serde_json::from_slice(SUPPORT_VECTORS).unwrap();
    let queries: Vec<Vec<u8>> = support_vectors
        .iter()
        .map(|vector| {
            let query = serde_json::to_vec(&vector["query"]).unwrap();
            let actual: Value =
                serde_json::from_slice(&engine.describe_theme_support(&query).unwrap()).unwrap();
            assert_eq!(actual, vector["expected"]);
            query
        })
        .collect();
    assert!(!queries.is_empty());

    let catalog = engine.metadata_json("theme-catalog").unwrap();
    let catalog_value: Value = serde_json::from_slice(&catalog).unwrap();
    let expected_presets: Value = serde_json::from_slice(PRESET_CATALOG).unwrap();
    assert_eq!(catalog_value["presets"], expected_presets);
    assert_eq!(
        BindingEngine::from_options(OPTIONS)
            .unwrap()
            .metadata_json("theme-catalog")
            .unwrap(),
        catalog,
    );
    let presets = [ThemePreset::EditorLight, ThemePreset::Cyberpunk];
    for preset in presets {
        let recipe: ThemeRecipeV1 =
            serde_json::from_slice(&engine.export_theme_preset(preset.id().as_bytes()).unwrap())
                .unwrap();
        assert_eq!(
            compiler
                .compile_recipe(recipe)
                .unwrap()
                .recipe_fingerprint(),
            compiler
                .compile_preset(preset)
                .unwrap()
                .recipe_fingerprint(),
        );
    }

    let mut group = criterion.benchmark_group("theme_compile_definition");
    for (name, definition) in &definitions {
        group.bench_with_input(BenchmarkId::from_parameter(name), definition, |b, input| {
            b.iter(|| {
                black_box(compile_theme_definition_json_with(&compiler, black_box(input)).unwrap())
            });
        });
    }
    group.finish();

    let mut group = criterion.benchmark_group("theme_materialize");
    for (name, definition) in &definitions {
        group.bench_with_input(BenchmarkId::from_parameter(name), definition, |b, input| {
            b.iter(|| black_box(engine.materialize_theme(black_box(input)).unwrap()));
        });
    }
    group.finish();

    for preset in presets {
        criterion.bench_function(&format!("theme_compile_preset/{}", preset.id()), |b| {
            b.iter(|| black_box(compiler.compile_preset(black_box(preset)).unwrap()));
        });
        criterion.bench_function(&format!("theme_export_preset/{}", preset.id()), |b| {
            b.iter(|| {
                black_box(
                    engine
                        .export_theme_preset(black_box(preset.id().as_bytes()))
                        .unwrap(),
                )
            });
        });
    }

    let mut group = criterion.benchmark_group("theme_support");
    group.throughput(Throughput::Elements(queries.len() as u64));
    group.bench_function("shared_vectors", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(engine.describe_theme_support(black_box(query)).unwrap());
            }
        });
    });
    group.finish();

    criterion.bench_function("theme_catalog/reused_engine", |b| {
        b.iter(|| black_box(engine.metadata_json(black_box("theme-catalog")).unwrap()));
    });
    // Fresh-engine work stays in-process: this is not process cold-start evidence.
    criterion.bench_function("theme_catalog/fresh_engine", |b| {
        b.iter(|| {
            let engine = BindingEngine::from_options(black_box(OPTIONS)).unwrap();
            black_box(engine.metadata_json(black_box("theme-catalog")).unwrap());
        });
    });
}

criterion_group!(benches, bench_theme_operations);
criterion_main!(benches);
