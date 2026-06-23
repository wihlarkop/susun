#![allow(missing_docs)]

use susun_loader::{EnvironmentProvider, MapEnvironment, ProcessEnvironment};

#[test]
fn map_environment_get_known_key() {
    let env = MapEnvironment::from([("FOO", "bar")]);
    assert_eq!(env.get("FOO"), Some("bar".to_owned()));
}

#[test]
fn map_environment_get_missing_key_returns_none() {
    let env = MapEnvironment::from([("FOO", "bar")]);
    assert_eq!(env.get("MISSING"), None);
}

#[test]
fn map_environment_vars_alphabetically_sorted() {
    let env = MapEnvironment::from([("C", "3"), ("A", "1"), ("B", "2")]);
    assert_eq!(
        env.vars(),
        vec![
            ("A".to_owned(), "1".to_owned()),
            ("B".to_owned(), "2".to_owned()),
            ("C".to_owned(), "3".to_owned()),
        ]
    );
}

#[test]
fn map_environment_default_is_empty() {
    let env = MapEnvironment::default();
    assert!(env.vars().is_empty());
    assert_eq!(env.get("ANYTHING"), None);
}

#[test]
fn map_environment_duplicate_key_last_value_wins() {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    map.insert("KEY".to_owned(), "first".to_owned());
    map.insert("KEY".to_owned(), "second".to_owned());
    let env = MapEnvironment::new(map);
    assert_eq!(env.get("KEY"), Some("second".to_owned()));
}

#[test]
fn process_environment_returns_none_for_unknown_key() {
    let env = ProcessEnvironment;
    assert_eq!(env.get("SUSUN_DEFINITELY_NOT_SET_XYZ_12345"), None);
}

#[test]
fn process_environment_vars_returns_sorted_pairs() {
    let env = ProcessEnvironment;
    let vars = env.vars();
    let is_sorted = vars.windows(2).all(|w| w[0].0 <= w[1].0);
    assert!(is_sorted, "vars() must return alphabetically sorted pairs");
}

#[test]
fn environment_provider_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MapEnvironment>();
    assert_send_sync::<ProcessEnvironment>();
}
