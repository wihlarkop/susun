//! Parser spike: marked-yaml 0.8.0 capability tests for susun-loader.
//! Marker API: .character() = byte offset, .line() = 1-based, .column() = 1-based.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use marked_yaml::{parse_yaml, types::MarkedMappingNode};

fn root_mapping(src: &str) -> MarkedMappingNode {
    parse_yaml(0, src)
        .expect("parse ok")
        .as_mapping()
        .cloned()
        .expect("root mapping")
}

fn byte(span: &marked_yaml::Span) -> usize {
    span.start().expect("start marker").character()
}

#[test]
fn key_and_value_have_distinct_spans() {
    let src = "name: nginx\n";
    let map = root_mapping(src);
    let (k, v) = map.iter().next().expect("entry");
    let kb = byte(k.span());
    let vb = byte(v.as_scalar().expect("scalar").span());
    eprintln!("key_byte={kb} val_byte={vb}");
    assert!(kb < vb, "key before value: {kb} < {vb}");
}

#[test]
fn nested_mapping_spans_are_present() {
    let src = "services:\n  web:\n    image: nginx\n";
    let map = root_mapping(src);
    let (_, sv) = map.iter().next().expect("services");
    let (_, wv) = sv
        .as_mapping()
        .expect("services map")
        .iter()
        .next()
        .expect("web");
    let (_, iv) = wv
        .as_mapping()
        .expect("web map")
        .iter()
        .next()
        .expect("image");
    let b = byte(iv.as_scalar().expect("scalar").span());
    assert!(b > 0, "nested value byte={b}");
}

#[test]
fn sequence_items_have_increasing_spans() {
    let src = "ports:\n  - \"8080:80\"\n  - \"9090:90\"\n";
    let map = root_mapping(src);
    let (_, pv) = map.iter().next().expect("ports");
    let items: Vec<_> = pv.as_sequence().expect("seq").iter().collect();
    assert_eq!(items.len(), 2);
    let s0 = byte(items[0].as_scalar().expect("s0").span());
    let s1 = byte(items[1].as_scalar().expect("s1").span());
    assert!(s0 < s1, "items ordered: {s0} < {s1}");
}

#[test]
fn double_quoted_scalar_parsed() {
    let src = "label: \"hello world\"\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("entry");
    assert_eq!(v.as_scalar().expect("scalar").as_str(), "hello world");
}

#[test]
fn single_quoted_scalar_parsed() {
    let src = "label: 'hello world'\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("entry");
    assert_eq!(v.as_scalar().expect("scalar").as_str(), "hello world");
}

#[test]
fn null_scalar_parses() {
    let src = "command: null\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("entry");
    let s = v.as_scalar().expect("scalar").as_str();
    eprintln!("null as_str={s:?}");
    assert_eq!(s, "null");
}

#[test]
fn numeric_scalar_is_string() {
    let src = "port: 8080\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("entry");
    assert_eq!(v.as_scalar().expect("scalar").as_str(), "8080");
}

#[test]
fn boolean_scalar_is_string() {
    let src = "enabled: true\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("entry");
    assert_eq!(v.as_scalar().expect("scalar").as_str(), "true");
}

#[test]
fn flow_mapping_parsed() {
    let src = "labels: {app: web, tier: frontend}\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("labels");
    assert_eq!(v.as_mapping().expect("map").iter().count(), 2);
}

#[test]
fn flow_sequence_parsed() {
    let src = "dns: [8.8.8.8, 1.1.1.1]\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("dns");
    assert_eq!(v.as_sequence().expect("seq").iter().count(), 2);
}

#[test]
fn literal_block_scalar_preserves_newlines() {
    let src = "script: |\n  line1\n  line2\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("script");
    let s = v.as_scalar().expect("scalar").as_str();
    eprintln!("literal block: {s:?}");
    assert!(s.contains('\n'), "literal block has newlines");
}

#[test]
fn folded_block_scalar_parsed() {
    let src = "desc: >\n  line1\n  line2\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("desc");
    let s = v.as_scalar().expect("scalar").as_str();
    eprintln!("folded: {s:?}");
    assert!(!s.is_empty());
}

#[test]
fn anchor_and_alias_not_supported() {
    // FINDING: marked-yaml 0.8.0 returns UnexpectedAnchor — anchors/aliases require
    // explicit pre-processing before passing to parse_yaml. Document in ADR.
    let src = "base: &base\n  image: nginx\nweb:\n  image: *base\n";
    let result = parse_yaml(0, src);
    eprintln!("anchor/alias result: {:?}", result.as_ref().err());
    // Document actual behavior — susun-loader must expand anchors before this call.
    assert!(
        result.is_err(),
        "marked-yaml does not support anchors natively — confirmed CONDITIONAL PASS"
    );
}

#[test]
fn merge_key_not_supported_natively() {
    // FINDING: same UnexpectedAnchor error for << merge keys.
    // Workaround: use yaml-rust2 to pre-expand anchors/merge-keys, then re-parse with marked-yaml,
    // OR build a custom expander on the marked-yaml AST.
    let src = "defaults: &def\n  restart: always\nweb:\n  <<: *def\n  image: nginx\n";
    let result = parse_yaml(0, src);
    eprintln!("merge key result: {:?}", result.as_ref().err());
    assert!(
        result.is_err(),
        "marked-yaml rejects anchor — workaround required"
    );
}

#[test]
fn duplicate_key_behavior() {
    let src = "name: first\nname: second\n";
    match parse_yaml(0, src) {
        Ok(node) => {
            let map = node.as_mapping().expect("map");
            let vals: Vec<_> = map
                .iter()
                .filter(|(k, _)| k.as_str() == "name")
                .map(|(_, v)| v.as_scalar().expect("scalar").as_str())
                .collect();
            eprintln!("duplicate key vals: {vals:?}");
            assert!(!vals.is_empty());
        }
        Err(e) => eprintln!("duplicate key → error: {e}"),
    }
}

#[test]
fn malformed_yaml_returns_error() {
    let src = "key: : bad\n";
    let result = parse_yaml(0, src);
    assert!(result.is_err(), "malformed returns Err");
    eprintln!("malformed err: {}", result.unwrap_err());
}

#[test]
fn unicode_byte_offset_accurate() {
    // "café" = 5 UTF-8 bytes; "name: " = 6 bytes → value at byte 6
    let src = "name: café\n";
    let map = root_mapping(src);
    let (_, v) = map.iter().next().expect("entry");
    let scalar = v.as_scalar().expect("scalar");
    assert_eq!(scalar.as_str(), "café");
    let b = byte(scalar.span());
    eprintln!("unicode value at byte {b}");
    assert_eq!(b, 6, "unicode value at byte 6");
}

#[test]
fn crlf_line_endings_parse() {
    let src = "name: web\r\nimage: nginx\r\n";
    let result = parse_yaml(0, src);
    assert!(result.is_ok(), "CRLF: {:?}", result.err());
    assert_eq!(result.unwrap().as_mapping().expect("map").iter().count(), 2);
}

#[test]
fn multiple_documents_behavior() {
    let src = "name: first\n---\nname: second\n";
    match parse_yaml(0, src) {
        Ok(_) => eprintln!("multiple docs: silently returns first doc"),
        Err(e) => eprintln!("multiple docs: error — {e}"),
    }
}

#[test]
fn span_line_and_column() {
    let src = "name: web\nimage: nginx\n";
    let map = root_mapping(src);
    let (_, v) = map
        .iter()
        .find(|(k, _)| k.as_str() == "image")
        .expect("image");
    let m = v
        .as_scalar()
        .expect("scalar")
        .span()
        .start()
        .expect("marker");
    eprintln!(
        "image: line={} col={} byte={}",
        m.line(),
        m.column(),
        m.character()
    );
    assert_eq!(m.line(), 2, "image on line 2");
}

#[test]
fn compose_long_port_form_parsed() {
    let src = "ports:\n  - target: 80\n    published: 8080\n    protocol: tcp\n    mode: host\n";
    let map = root_mapping(src);
    let (_, pv) = map.iter().next().expect("ports");
    let item = pv.as_sequence().expect("seq").iter().next().expect("item");
    assert_eq!(item.as_mapping().expect("map").iter().count(), 4);
}

#[test]
fn compose_long_depends_on_parsed() {
    let src = "depends_on:\n  db:\n    condition: service_healthy\n    restart: true\n    required: true\n";
    let map = root_mapping(src);
    let (_, dv) = map.iter().next().expect("depends_on");
    let (_, db) = dv.as_mapping().expect("map").iter().next().expect("db");
    assert_eq!(db.as_mapping().expect("db map").iter().count(), 3);
}
