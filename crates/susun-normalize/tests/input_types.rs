//! Construction tests for the expanded merge input types.

use std::{error::Error, sync::Arc};

use indexmap::IndexMap;
use susun_normalize::input::{
    ParsedService, RawMapping, RawPortEntry, RawPortLong, RawPortShort, RawStringOrList,
    RawVolumeMount, RawVolumeLong, RawVolumeShort,
};
use susun_source::{LoadedSource, SourceMap, SourceName, Span, Spanned, TextOffset};

type TestResult = Result<(), Box<dyn Error>>;

fn make_source() -> (SourceMap, susun_source::SourceId) {
    let mut smap = SourceMap::new();
    let id = smap.register(LoadedSource {
        name: SourceName::new("test"),
        path: None,
        contents: Arc::from(""),
    });
    (smap, id)
}

fn sp(id: susun_source::SourceId, s: &str) -> Spanned<String> {
    Spanned::new(s.to_owned(), Span::empty(id, TextOffset::new(0)))
}

// ── RawStringOrList ──────────────────────────────────────────────────────────

#[test]
fn command_default_is_null() -> TestResult {
    let cmd = RawStringOrList::default();
    assert!(matches!(cmd, RawStringOrList::Null));
    Ok(())
}

#[test]
fn command_string_form() -> TestResult {
    let (_, id) = make_source();
    let cmd = RawStringOrList::String(sp(id, "sh -c 'echo hi'"));
    assert!(matches!(cmd, RawStringOrList::String(_)));
    Ok(())
}

#[test]
fn command_list_form_three_items() -> TestResult {
    let (_, id) = make_source();
    let cmd = RawStringOrList::List(vec![sp(id, "sh"), sp(id, "-c"), sp(id, "echo hi")]);
    match &cmd {
        RawStringOrList::List(items) => assert_eq!(items.len(), 3),
        _ => return Err("expected list".into()),
    }
    Ok(())
}

#[test]
fn command_list_form_empty() -> TestResult {
    let cmd = RawStringOrList::List(vec![]);
    match &cmd {
        RawStringOrList::List(items) => assert!(items.is_empty()),
        _ => return Err("expected list".into()),
    }
    Ok(())
}

#[test]
fn entrypoint_uses_same_variants() -> TestResult {
    let (_, id) = make_source();
    let ep = RawStringOrList::String(sp(id, "/entrypoint.sh"));
    assert!(matches!(ep, RawStringOrList::String(_)));
    Ok(())
}

// ── RawMapping ───────────────────────────────────────────────────────────────

#[test]
fn mapping_default_is_empty_map() -> TestResult {
    let env = RawMapping::default();
    match &env {
        RawMapping::Map(m) => assert!(m.is_empty()),
        _ => return Err("expected empty map".into()),
    }
    Ok(())
}

#[test]
fn mapping_map_form_with_value() -> TestResult {
    let (_, id) = make_source();
    let mut m = IndexMap::new();
    m.insert("NODE_ENV".to_owned(), Some(sp(id, "production")));
    let env = RawMapping::Map(m);
    match &env {
        RawMapping::Map(map) => assert_eq!(map.len(), 1),
        _ => return Err("expected map form".into()),
    }
    Ok(())
}

#[test]
fn mapping_map_form_null_value() -> TestResult {
    let mut m: IndexMap<String, Option<Spanned<String>>> = IndexMap::new();
    m.insert("KEY".to_owned(), None);
    let env = RawMapping::Map(m);
    match &env {
        RawMapping::Map(map) => assert!(map["KEY"].is_none()),
        _ => return Err("expected map form".into()),
    }
    Ok(())
}

#[test]
fn mapping_list_form() -> TestResult {
    let (_, id) = make_source();
    let env = RawMapping::List(vec![sp(id, "FOO=bar"), sp(id, "BAZ")]);
    match &env {
        RawMapping::List(items) => assert_eq!(items.len(), 2),
        _ => return Err("expected list form".into()),
    }
    Ok(())
}

// ── RawPortEntry ─────────────────────────────────────────────────────────────

#[test]
fn port_short_form() -> TestResult {
    let (_, id) = make_source();
    let entry = RawPortEntry::Short(RawPortShort(sp(id, "8080:80")));
    assert!(matches!(entry, RawPortEntry::Short(_)));
    Ok(())
}

#[test]
fn port_long_form_with_fields() -> TestResult {
    let (_, id) = make_source();
    let entry = RawPortEntry::Long(RawPortLong {
        target: Some(sp(id, "80")),
        published: Some(sp(id, "8080")),
        protocol: Some(sp(id, "tcp")),
        host_ip: None,
        mode: None,
    });
    match &entry {
        RawPortEntry::Long(l) => {
            assert!(l.target.is_some());
            assert!(l.published.is_some());
            assert!(l.protocol.is_some());
            assert!(l.host_ip.is_none());
        }
        _ => return Err("expected long form".into()),
    }
    Ok(())
}

#[test]
fn port_long_form_default_all_none() -> TestResult {
    let long = RawPortLong::default();
    assert!(long.target.is_none());
    assert!(long.published.is_none());
    assert!(long.host_ip.is_none());
    assert!(long.protocol.is_none());
    assert!(long.mode.is_none());
    Ok(())
}

// ── RawVolumeMount ───────────────────────────────────────────────────────────

#[test]
fn volume_short_form() -> TestResult {
    let (_, id) = make_source();
    let entry = RawVolumeMount::Short(RawVolumeShort(sp(id, "/host/path:/container:ro")));
    assert!(matches!(entry, RawVolumeMount::Short(_)));
    Ok(())
}

#[test]
fn volume_long_form_bind_type() -> TestResult {
    let (_, id) = make_source();
    let entry = RawVolumeMount::Long(RawVolumeLong {
        volume_type: Some(sp(id, "bind")),
        source: Some(sp(id, "/host/path")),
        target: Some(sp(id, "/container")),
        read_only: Some(sp(id, "true")),
    });
    match &entry {
        RawVolumeMount::Long(l) => {
            assert!(l.volume_type.is_some());
            assert!(l.source.is_some());
            assert!(l.target.is_some());
            assert!(l.read_only.is_some());
        }
        _ => return Err("expected long form".into()),
    }
    Ok(())
}

#[test]
fn volume_long_form_default_all_none() -> TestResult {
    let long = RawVolumeLong::default();
    assert!(long.volume_type.is_none());
    assert!(long.source.is_none());
    assert!(long.target.is_none());
    assert!(long.read_only.is_none());
    Ok(())
}

// ── ParsedService ─────────────────────────────────────────────────────────────

#[test]
fn parsed_service_default_all_absent() -> TestResult {
    let svc = ParsedService::default();
    assert!(svc.image.is_none());
    match svc.command {
        RawStringOrList::Null => {}
        _ => return Err("expected command Null".into()),
    }
    match svc.entrypoint {
        RawStringOrList::Null => {}
        _ => return Err("expected entrypoint Null".into()),
    }
    match svc.environment {
        RawMapping::Map(ref m) => assert!(m.is_empty()),
        _ => return Err("expected environment empty map".into()),
    }
    match svc.labels {
        RawMapping::Map(ref m) => assert!(m.is_empty()),
        _ => return Err("expected labels empty map".into()),
    }
    assert!(svc.ports.is_empty());
    assert!(svc.volumes.is_empty());
    Ok(())
}

#[test]
fn parsed_service_all_fields_constructed() -> TestResult {
    let (_, id) = make_source();
    let mut env_map = IndexMap::new();
    env_map.insert("NODE_ENV".to_owned(), Some(sp(id, "production")));
    let svc = ParsedService {
        image: Some(sp(id, "nginx:latest")),
        command: RawStringOrList::String(sp(id, "nginx -g 'daemon off;'")),
        entrypoint: RawStringOrList::List(vec![sp(id, "/entrypoint.sh")]),
        environment: RawMapping::Map(env_map),
        labels: RawMapping::List(vec![sp(id, "app=web")]),
        ports: vec![RawPortEntry::Short(RawPortShort(sp(id, "80:80")))],
        volumes: vec![RawVolumeMount::Short(RawVolumeShort(sp(id, "/data:/data")))],
    };
    assert!(svc.image.is_some());
    assert!(matches!(svc.command, RawStringOrList::String(_)));
    assert!(matches!(svc.entrypoint, RawStringOrList::List(_)));
    assert_eq!(svc.ports.len(), 1);
    assert_eq!(svc.volumes.len(), 1);
    Ok(())
}

#[test]
fn no_parser_vendor_types_in_signatures() -> TestResult {
    // This test verifies (at compile time) that all public types in the input
    // module are constructable without any saphyr / parser-vendor imports.
    // If this file compiles, the invariant holds.
    let _ = ParsedService::default();
    Ok(())
}
