//! Resolve build inputs and execute a neutral fake build.

use std::{path::PathBuf, process::ExitCode};

use susun_build::{
    BuildCancellationToken, BuildEngine, BuildEventSink, BuildInputManifest, BuildRequest,
    BuildSecret, BuildSshForward, CacheEntry, InsecureEntitlements, resolve_build_inputs,
};
use susun_model::BuildDefinition;
use susun_testkit::FakeBuildEngine;

fn main() -> ExitCode {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."));
    let project_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("fixtures/compatibility/build-context"));

    let definition = BuildDefinition {
        context: Some(".".to_owned()),
        dockerfile: Some("Dockerfile".to_owned()),
        ..BuildDefinition::default()
    };

    let paths = match resolve_build_inputs(&project_dir, &definition) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    let dockerignore = susun_build::Dockerignore::default();
    let manifest = match BuildInputManifest::from_context(&paths.context_dir, &dockerignore) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    let request = BuildRequest {
        definition,
        context_dir: paths.context_dir,
        dockerfile: paths.dockerfile,
        manifest,
        image_tag: Some("susun/example:latest".to_owned()),
        secrets: Vec::<BuildSecret>::new(),
        ssh: Vec::<BuildSshForward>::new(),
        cache_from: Vec::<CacheEntry>::new(),
        cache_to: Vec::<CacheEntry>::new(),
        insecure_entitlements: InsecureEntitlements::default(),
        labels: Default::default(),
    };

    let engine = FakeBuildEngine::new(susun_build::BuildImageIdentity {
        reference: "susun/example:latest".to_owned(),
        digest: Some("sha256:example".to_owned()),
    });

    let runtime = match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(engine.build(
        request,
        BuildEventSink::discard(),
        BuildCancellationToken::new(),
    ));

    match result {
        Ok(result) => {
            println!("built {}", result.image.reference);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
