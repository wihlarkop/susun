//! Lightweight Phase 5 compatibility benchmark harness.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use susun_compat::{
    CompatibilityHarness, ComposeReference, CorpusManifest, OracleCommand, matrix_for_current_phase,
};

const CORPUS_JSON: &str = include_str!("../../../fixtures/compatibility/corpus.json");

fn main() {
    let samples = [
        bench("capability_matrix_generation", 1_000, || {
            let matrix = matrix_for_current_phase("0.1.0", "Docker Compose documented");
            black_box(matrix.features.len());
        }),
        bench("corpus_manifest_parse", 200, || {
            let manifest = CorpusManifest::from_json_str(CORPUS_JSON).unwrap_or_else(|error| {
                eprintln!("benchmark setup failed: {error}");
                std::process::exit(2);
            });
            black_box(manifest.fixtures.len());
        }),
        bench("oracle_plan_generation", 1_000, || {
            let manifest = CorpusManifest::from_json_str(CORPUS_JSON).unwrap_or_else(|error| {
                eprintln!("benchmark setup failed: {error}");
                std::process::exit(2);
            });
            let config = manifest.to_oracle_config(
                ComposeReference {
                    name: "docker compose".to_owned(),
                    version: "documented".to_owned(),
                    engine_api_version: None,
                },
                OracleCommand::docker_compose(),
            );
            let harness = CompatibilityHarness::new(config).unwrap_or_else(|error| {
                eprintln!("benchmark setup failed: {error}");
                std::process::exit(2);
            });
            black_box(harness.run_plan().len());
        }),
    ];

    println!("name,iterations,total_ms,avg_us");
    for sample in samples {
        println!(
            "{},{},{},{}",
            sample.name,
            sample.iterations,
            sample.elapsed.as_millis(),
            sample.elapsed.as_micros() / u128::from(sample.iterations)
        );
    }
}

struct Sample {
    name: &'static str,
    iterations: u32,
    elapsed: Duration,
}

fn bench(name: &'static str, iterations: u32, mut operation: impl FnMut()) -> Sample {
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    Sample {
        name,
        iterations,
        elapsed: started.elapsed(),
    }
}
