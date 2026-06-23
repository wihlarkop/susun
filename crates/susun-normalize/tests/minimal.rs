//! Tests for normalization of the minimal Compose project.

use std::{error::Error, sync::Arc};

use susun_diagnostics::DiagnosticReport;
use susun_loader::parse_compose_str;
use susun_model::{ProjectName, ServiceName};
use susun_normalize::{
    input::MergeProject,
    normalize::{normalize, FinalProjectMetadata},
};
use susun_source::{LoadedSource, SourceMap, SourceName};

type TestResult = Result<(), Box<dyn Error>>;

fn register(map: &mut SourceMap, name: &str, contents: &str) -> susun_source::SourceId {
    map.register(LoadedSource {
        name: SourceName::new(name),
        path: None,
        contents: Arc::from(contents),
    })
}

#[test]
fn one_raw_service_becomes_canonical() -> TestResult {
    let src = "name: myapp\nservices:\n  web:\n    image: nginx:latest\n";
    let mut smap = SourceMap::new();
    let id = register(&mut smap, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let parsed = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;
    let merge = MergeProject::from(parsed);
    let metadata = FinalProjectMetadata { project_name: ProjectName::new("myapp") };
    let outcome = normalize(merge, metadata)?;

    assert!(!outcome.report.has_errors());
    assert_eq!(outcome.project.name.as_ref(), "myapp");

    let web = outcome
        .project
        .services
        .get(&ServiceName::new("web"))
        .ok_or("expected web service")?;
    assert_eq!(web.image.as_ref().map(|i| i.as_ref()), Some("nginx:latest"));

    Ok(())
}

#[test]
fn service_and_image_provenance_span_source_text() -> TestResult {
    let src = "name: myapp\nservices:\n  web:\n    image: nginx:latest\n";
    let mut smap = SourceMap::new();
    let id = register(&mut smap, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let parsed = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;
    let merge = MergeProject::from(parsed);
    let metadata = FinalProjectMetadata { project_name: ProjectName::new("myapp") };
    let outcome = normalize(merge, metadata)?;

    let web_prov = outcome
        .provenance
        .services
        .get("web")
        .ok_or("expected web provenance")?;
    let image_span = web_prov.image_span.ok_or("expected image span")?;
    let start = image_span.start.value() as usize;
    let end = image_span.end.value() as usize;
    assert_eq!(&src[start..end], "nginx:latest", "image span covers value text");

    Ok(())
}

#[test]
fn name_provenance_span_covers_name_value() -> TestResult {
    let src = "name: myapp\nservices:\n  web:\n    image: nginx:latest\n";
    let mut smap = SourceMap::new();
    let id = register(&mut smap, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let parsed = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;
    let merge = MergeProject::from(parsed);
    let metadata = FinalProjectMetadata { project_name: ProjectName::new("myapp") };
    let outcome = normalize(merge, metadata)?;

    let name_span = outcome.provenance.name_span.ok_or("expected name span")?;
    let start = name_span.start.value() as usize;
    let end = name_span.end.value() as usize;
    assert_eq!(&src[start..end], "myapp", "name span covers value text");

    Ok(())
}

#[test]
fn service_without_image_yields_none_image() -> TestResult {
    let src = "name: minimal\nservices:\n  worker:\n    image: ~\n";
    let mut smap = SourceMap::new();
    let id = register(&mut smap, "compose.yaml", src);
    let mut report = DiagnosticReport::new();

    let parsed = parse_compose_str(id, src, &mut report).ok_or("expected parsed project")?;
    let merge = MergeProject::from(parsed);
    let metadata = FinalProjectMetadata { project_name: ProjectName::new("minimal") };
    let outcome = normalize(merge, metadata)?;

    let worker = outcome
        .project
        .services
        .get(&ServiceName::new("worker"))
        .ok_or("expected worker service")?;
    assert!(worker.image.is_none());

    Ok(())
}
