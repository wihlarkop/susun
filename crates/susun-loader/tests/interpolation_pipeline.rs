#![allow(missing_docs)]

use std::error::Error;

use susun_loader::{LoadContext, MapEnvironment, ProjectLoader};
use susun_source::MemorySourceProvider;

type TestResult = Result<(), Box<dyn Error>>;

fn image_of<'a>(
    parsed: &'a susun_normalize::input::ParsedProject,
    service: &str,
) -> Option<&'a str> {
    parsed
        .services
        .get(service)
        .and_then(|s| s.value.image.as_ref())
        .map(|img| img.value.as_str())
}

// ── scalar substitution in values ──────────────────────────────────────────────

#[test]
fn image_var_substituted_from_env_provider() -> TestResult {
    let yaml =
        "name: test\nservices:\n  web:\n    image: ${REGISTRY:-docker.io}/${IMAGE:-nginx}:${TAG:-latest}\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::from([
            ("REGISTRY", "myregistry"),
            ("IMAGE", "myapp"),
            ("TAG", "v1.0"),
        ]));
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert_eq!(image_of(&parsed, "web"), Some("myregistry/myapp:v1.0"));
    assert!(!result.report.has_errors());
    Ok(())
}

#[test]
fn image_var_falls_back_to_default_when_unset() -> TestResult {
    let yaml = "name: test\nservices:\n  web:\n    image: ${REGISTRY:-docker.io}/${IMAGE:-nginx}\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::default());
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert_eq!(image_of(&parsed, "web"), Some("docker.io/nginx"));
    assert!(!result.report.has_errors());
    Ok(())
}

#[test]
fn project_name_field_is_interpolated() -> TestResult {
    let yaml = "name: ${APP_NAME:-fallback}\nservices:\n  web:\n    image: nginx\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::from([("APP_NAME", "myapp")]));
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert_eq!(parsed.name.as_ref().map(|s| s.value.as_str()), Some("myapp"));
    Ok(())
}

#[test]
fn project_name_falls_back_to_default_when_var_unset() -> TestResult {
    let yaml = "name: ${APP_NAME:-fallback}\nservices:\n  web:\n    image: nginx\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::default());
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert_eq!(parsed.name.as_ref().map(|s| s.value.as_str()), Some("fallback"));
    Ok(())
}

// ── required variable diagnostics ─────────────────────────────────────────────

#[test]
fn required_var_missing_emits_sus_env_001() -> TestResult {
    let yaml =
        "name: test\nservices:\n  web:\n    image: ${REQUIRED_IMAGE:?REQUIRED_IMAGE must be set}\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::default());
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    assert!(result.report.has_errors());
    let diag = result
        .report
        .iter()
        .find(|d| d.code.as_str() == "SUS-ENV-001")
        .ok_or("expected SUS-ENV-001")?;
    assert!(diag.message.contains("REQUIRED_IMAGE"));
    Ok(())
}

#[test]
fn required_var_set_produces_no_error() -> TestResult {
    let yaml =
        "name: test\nservices:\n  web:\n    image: ${REQUIRED_IMAGE:?REQUIRED_IMAGE must be set}\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::from([("REQUIRED_IMAGE", "nginx:latest")]));
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert!(!result.report.has_errors());
    assert_eq!(image_of(&parsed, "web"), Some("nginx:latest"));
    Ok(())
}

// ── mapping keys are not interpolated ─────────────────────────────────────────

#[test]
fn service_name_key_is_not_interpolated() -> TestResult {
    // The service named literally "${svc}" should not be resolved from env.
    let yaml = "name: test\nservices:\n  ${svc}:\n    image: nginx\n";
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::from([("svc", "web")]));
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml)]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    // The literal key "${svc}" should appear, not "web".
    assert!(
        parsed.services.contains_key("${svc}"),
        "expected literal key '${{svc}}', got: {:?}",
        parsed.services.keys().collect::<Vec<_>>(),
    );
    Ok(())
}

// ── fixture-based tests ────────────────────────────────────────────────────────

fn workspace_root() -> Result<std::path::PathBuf, Box<dyn Error>> {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "workspace root not found".into())
}

#[test]
fn fixture_substitution_with_all_vars_set() -> TestResult {
    let fixture = workspace_root()?.join("fixtures/interpolation/compose/substitution.yaml");
    let yaml = std::fs::read_to_string(fixture)?;
    let context = LoadContext::new("compose.yaml").with_env_provider(MapEnvironment::from([
        ("PROJECT_NAME", "my-project"),
        ("REGISTRY", "registry.example.com"),
        ("IMAGE", "backend"),
        ("TAG", "stable"),
    ]));
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml.as_str())]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert!(!result.report.has_errors());
    assert_eq!(parsed.name.as_ref().map(|s| s.value.as_str()), Some("my-project"));
    assert_eq!(
        image_of(&parsed, "web"),
        Some("registry.example.com/backend:stable"),
    );
    Ok(())
}

#[test]
fn fixture_substitution_with_defaults_used() -> TestResult {
    let fixture = workspace_root()?.join("fixtures/interpolation/compose/substitution.yaml");
    let yaml = std::fs::read_to_string(fixture)?;
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::default());
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml.as_str())]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    let parsed = result.parsed.ok_or("should parse")?;
    assert!(!result.report.has_errors());
    assert_eq!(parsed.name.as_ref().map(|s| s.value.as_str()), Some("default-project"));
    assert_eq!(image_of(&parsed, "web"), Some("docker.io/nginx:latest"));
    Ok(())
}

#[test]
fn fixture_required_missing_emits_error() -> TestResult {
    let fixture = workspace_root()?.join("fixtures/interpolation/compose/required-missing.yaml");
    let yaml = std::fs::read_to_string(fixture)?;
    let context = LoadContext::new("compose.yaml")
        .with_env_provider(MapEnvironment::default());
    let provider = MemorySourceProvider::with_files([("compose.yaml", yaml.as_str())]);
    let result = ProjectLoader::with_context_and_provider(context, provider).load()?;
    assert!(result.report.has_errors());
    assert!(result
        .report
        .iter()
        .any(|d| d.code.as_str() == "SUS-ENV-001"));
    Ok(())
}
