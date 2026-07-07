#![allow(missing_docs)]

use susun_engine::{
    EngineConnectionDisplayName, EngineConnectionProfile, EngineConnectionProfileError,
    EngineConnectionProfileId, EngineConnectionProfileSet, EngineEndpoint,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn profile(
    id: &str,
    display_name: &str,
    endpoint: EngineEndpoint,
) -> Result<EngineConnectionProfile, EngineConnectionProfileError> {
    Ok(EngineConnectionProfile::new(
        EngineConnectionProfileId::new(id)?,
        EngineConnectionDisplayName::new(display_name)?,
        endpoint,
    ))
}

#[test]
fn local_default_profile_is_selected_and_redacted() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![EngineConnectionProfile::local_default()])?;
    let default = set.default_profile().ok_or("expected default profile")?;

    assert_eq!(default.id.as_str(), "local");
    assert!(default.is_default());
    assert_eq!(default.redacted_endpoint(), "local");
    Ok(())
}

#[test]
fn duplicate_profile_ids_are_rejected() -> TestResult {
    let result = EngineConnectionProfileSet::new(vec![
        profile("local", "Local A", EngineEndpoint::Local)?,
        profile("local", "Local B", EngineEndpoint::Local)?,
    ]);

    assert!(matches!(
        result,
        Err(EngineConnectionProfileError::DuplicateId(id)) if id.as_str() == "local"
    ));
    Ok(())
}

#[test]
fn multiple_defaults_are_rejected() -> TestResult {
    let result = EngineConnectionProfileSet::new(vec![
        profile("one", "One", EngineEndpoint::Local)?.with_default(true),
        profile("two", "Two", EngineEndpoint::Local)?.with_default(true),
    ]);

    assert!(matches!(
        result,
        Err(EngineConnectionProfileError::MultipleDefaults)
    ));
    Ok(())
}

#[test]
fn first_profile_is_default_when_none_marked() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![
        profile("one", "One", EngineEndpoint::Local)?,
        profile("two", "Two", EngineEndpoint::Local)?,
    ])?;

    let default = set.default_profile().ok_or("expected default profile")?;
    assert_eq!(default.id.as_str(), "one");
    Ok(())
}

#[test]
fn profile_set_lookup_uses_profile_id() -> TestResult {
    let set = EngineConnectionProfileSet::new(vec![
        profile("one", "One", EngineEndpoint::Local)?,
        profile("two", "Two", EngineEndpoint::Local)?.with_default(true),
    ])?;
    let id = EngineConnectionProfileId::new("two")?;

    let selected = set.get(&id).ok_or("expected profile")?;
    assert_eq!(selected.display_name.as_str(), "Two");
    assert_eq!(
        set.default_profile().ok_or("expected default")?.id.as_str(),
        "two"
    );
    Ok(())
}

#[test]
fn profile_debug_redacts_endpoint() -> TestResult {
    let profile = profile(
        "private",
        "Private socket",
        EngineEndpoint::UnixSocket("/very/private/docker.sock".into()),
    )?;
    let debug = format!("{profile:?}");

    assert!(debug.contains("unix://<local-socket>"));
    assert!(!debug.contains("very/private"));
    assert!(!debug.contains("docker.sock"));
    Ok(())
}
