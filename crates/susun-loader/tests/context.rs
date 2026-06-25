#![allow(missing_docs)]

use std::path::PathBuf;

use susun_loader::{LoadContext, MapEnvironment};

fn path(p: &str) -> PathBuf {
    PathBuf::from(p)
}

/// Precedence 1: explicit override wins over all other sources.
#[test]
fn explicit_override_wins_over_compose_project_name_and_file() {
    let env = MapEnvironment::from([("COMPOSE_PROJECT_NAME", "from-env")]);
    let ctx = LoadContext::new(path("/project/compose.yaml"))
        .with_project_name("explicit")
        .with_env_provider(env);
    assert_eq!(
        ctx.resolve_project_name(Some("from-file")).as_str(),
        "explicit"
    );
}

#[test]
fn explicit_override_wins_over_directory_fallback() {
    let env = MapEnvironment::default();
    let ctx = LoadContext::new(path("/my-dir/compose.yaml"))
        .with_project_name("override")
        .with_env_provider(env);
    assert_eq!(ctx.resolve_project_name(None).as_str(), "override");
}

/// Precedence 2: COMPOSE_PROJECT_NAME wins over file name and directory.
#[test]
fn compose_project_name_env_wins_over_file_name() {
    let env = MapEnvironment::from([("COMPOSE_PROJECT_NAME", "from-env")]);
    let ctx = LoadContext::new(path("/project/compose.yaml")).with_env_provider(env);
    assert_eq!(
        ctx.resolve_project_name(Some("from-file")).as_str(),
        "from-env"
    );
}

#[test]
fn compose_project_name_env_wins_over_directory_fallback() {
    let env = MapEnvironment::from([("COMPOSE_PROJECT_NAME", "env-name")]);
    let ctx = LoadContext::new(path("/my-dir/compose.yaml")).with_env_provider(env);
    assert_eq!(ctx.resolve_project_name(None).as_str(), "env-name");
}

/// An empty COMPOSE_PROJECT_NAME must be treated as unset.
#[test]
fn empty_compose_project_name_is_ignored() {
    let env = MapEnvironment::from([("COMPOSE_PROJECT_NAME", "")]);
    let ctx = LoadContext::new(path("/project/compose.yaml")).with_env_provider(env);
    assert_eq!(
        ctx.resolve_project_name(Some("from-file")).as_str(),
        "from-file"
    );
}

/// Precedence 3: name: field from the file wins over directory fallback.
#[test]
fn file_name_wins_over_directory_fallback() {
    let env = MapEnvironment::default();
    let ctx = LoadContext::new(path("/my-dir/compose.yaml")).with_env_provider(env);
    assert_eq!(
        ctx.resolve_project_name(Some("from-file")).as_str(),
        "from-file"
    );
}

/// Precedence 4: directory name is the last resort.
#[test]
fn directory_name_is_fallback_when_no_other_source() {
    let env = MapEnvironment::default();
    let ctx = LoadContext::new(path("/my-project/compose.yaml")).with_env_provider(env);
    assert_eq!(ctx.resolve_project_name(None).as_str(), "my-project");
}

/// env_get delegates to the configured provider.
#[test]
fn env_get_reads_from_provider() {
    let env = MapEnvironment::from([("MY_VAR", "hello")]);
    let ctx = LoadContext::new(path("/p/compose.yaml")).with_env_provider(env);
    assert_eq!(ctx.env_get("MY_VAR"), Some("hello".to_owned()));
    assert_eq!(ctx.env_get("MISSING"), None);
}
