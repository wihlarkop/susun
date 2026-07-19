#![allow(missing_docs)]

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use susun::{
    BuildDefinition, BuildEngine, BuildInputPaths, BuildResolveError, BuildxProcessBuildEngine,
    BuildxProcessOptions, DockerfileSource, DockerfileValidationError, resolve_build_inputs,
    validate_dockerfile_source,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn assert_build_engine<T: BuildEngine>() {}

fn unique_temp_dir(label: &str) -> TestResult<PathBuf> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!("{label}-{}-{nanos}", std::process::id())))
}

#[test]
fn buildx_process_engine_is_publicly_constructible_and_implements_build_engine() -> TestResult {
    assert_build_engine::<BuildxProcessBuildEngine>();

    let default_options = BuildxProcessOptions::default();
    assert_eq!(default_options.docker_cli, PathBuf::from("docker"));
    assert!(default_options.load);

    let custom_options = BuildxProcessOptions {
        docker_cli: PathBuf::from("/usr/local/bin/docker"),
        load: false,
    };
    let _engine = BuildxProcessBuildEngine::new(custom_options);
    Ok(())
}

#[test]
fn resolve_build_inputs_is_reachable_with_a_facade_only_build_definition() -> TestResult {
    let project_dir = unique_temp_dir("susun-facade-resolve")?;
    fs::create_dir_all(&project_dir)?;
    fs::write(project_dir.join("Dockerfile"), "FROM scratch\n")?;

    let build = BuildDefinition::default();
    let resolved: Result<BuildInputPaths, BuildResolveError> =
        resolve_build_inputs(&project_dir, &build);
    let paths = resolved.map_err(|error| error.to_string())?;
    assert_eq!(paths.context_dir, paths.project_dir);
    assert!(paths.dockerfile.ends_with("Dockerfile"));

    fs::remove_dir_all(&project_dir).ok();
    Ok(())
}

#[test]
fn validate_dockerfile_source_is_reachable_through_the_facade() -> TestResult {
    let project_dir = unique_temp_dir("susun-facade-dockerfile")?;
    fs::create_dir_all(&project_dir)?;
    let dockerfile_path = project_dir.join("Dockerfile");
    fs::write(&dockerfile_path, "FROM scratch\n")?;

    let source: DockerfileSource = validate_dockerfile_source(&dockerfile_path, Some("build"))
        .map_err(|error| error.to_string())?;
    assert_eq!(source.path, dockerfile_path);
    assert_eq!(source.target.as_deref(), Some("build"));

    let error: DockerfileValidationError =
        match validate_dockerfile_source(&project_dir.join("missing"), None) {
            Ok(_) => return Err("missing dockerfile should fail validation".into()),
            Err(error) => error,
        };
    assert!(matches!(error, DockerfileValidationError::Metadata { .. }));

    fs::remove_dir_all(&project_dir).ok();
    Ok(())
}
