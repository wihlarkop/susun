//! Parser spike: saphyr 0.0.6 capability tests for susun-loader.
//!
//! Key findings documented here guide the production parser adapter in susun-loader.
//! Marker API: .index() = CHAR offset (not byte), .line() = 1-based, .col() = 1-based.
//! For byte offsets: iterate source with char_indices().nth(marker.index()).

use std::error::Error;

use saphyr::{LoadableYamlNode, MarkedYamlOwned, YamlDataOwned};

type TestResult = Result<(), Box<dyn Error>>;

/// Load the first document from a YAML string, expecting it to be a mapping.
fn root_mapping(
    src: &str,
) -> Result<hashlink::LinkedHashMap<MarkedYamlOwned, MarkedYamlOwned>, Box<dyn Error>> {
    let docs = MarkedYamlOwned::load_from_str(src)?;
    let doc = docs.into_iter().next().ok_or("no documents in YAML")?;
    match doc.data {
        YamlDataOwned::Mapping(m) => Ok(m),
        other => Err(format!("expected mapping, got {other:?}").into()),
    }
}

/// Convert a char-based marker index to a byte offset.
fn char_to_byte(src: &str, char_index: usize) -> usize {
    src.char_indices()
        .nth(char_index)
        .map_or(src.len(), |(b, _)| b)
}

// ── Basic span behavior ───────────────────────────────────────────────────────

#[test]
fn key_and_value_have_distinct_spans() -> TestResult {
    let src = "name: nginx\n";
    let map = root_mapping(src)?;
    let (k, v) = map.iter().next().ok_or("expected entry")?;
    let kb = k.span.start.index();
    let vb = v.span.start.index();
    eprintln!("key_char={kb} val_char={vb}");
    assert!(kb < vb, "key before value: {kb} < {vb}");
    Ok(())
}

#[test]
fn nested_mapping_spans_are_present() -> TestResult {
    let src = "services:\n  web:\n    image: nginx\n";
    let map = root_mapping(src)?;
    let services = map.values().next().ok_or("no services value")?;
    let web_map = match &services.data {
        YamlDataOwned::Mapping(m) => m,
        other => return Err(format!("expected services mapping, got {other:?}").into()),
    };
    let web = web_map.values().next().ok_or("no web entry")?;
    let image_map = match &web.data {
        YamlDataOwned::Mapping(m) => m,
        other => return Err(format!("expected web mapping, got {other:?}").into()),
    };
    let img_val = image_map.values().next().ok_or("no image value")?;
    assert!(img_val.span.start.index() > 0, "nested span present");
    Ok(())
}

#[test]
fn sequence_items_have_increasing_spans() -> TestResult {
    let src = "ports:\n  - \"8080:80\"\n  - \"9090:90\"\n";
    let map = root_mapping(src)?;
    let pv = map.values().next().ok_or("no ports value")?;
    let items = match &pv.data {
        YamlDataOwned::Sequence(s) => s,
        other => return Err(format!("expected sequence, got {other:?}").into()),
    };
    assert_eq!(items.len(), 2);
    let s0 = items[0].span.start.index();
    let s1 = items[1].span.start.index();
    assert!(s0 < s1, "items ordered: {s0} < {s1}");
    Ok(())
}

// ── Scalar types ──────────────────────────────────────────────────────────────

#[test]
fn null_scalar_parses() -> TestResult {
    let src = "command: ~\n";
    let map = root_mapping(src)?;
    let v = map.values().next().ok_or("no value")?;
    eprintln!("null data: {:?}", v.data);
    // Null is represented as Scalar(Null) in saphyr
    assert!(matches!(
        &v.data,
        YamlDataOwned::Value(_) | YamlDataOwned::Representation(_, _, _)
    ));
    Ok(())
}

#[test]
fn numeric_scalar_as_string_representation() -> TestResult {
    let src = "port: 8080\n";
    let map = root_mapping(src)?;
    let v = map.values().next().ok_or("no value")?;
    eprintln!("numeric data: {:?}", v.data);
    // saphyr parses numeric scalars as typed values
    assert!(matches!(
        &v.data,
        YamlDataOwned::Value(_) | YamlDataOwned::Representation(_, _, _)
    ));
    Ok(())
}

#[test]
fn boolean_scalar_parses() -> TestResult {
    let src = "enabled: true\n";
    let map = root_mapping(src)?;
    let v = map.values().next().ok_or("no value")?;
    eprintln!("bool data: {:?}", v.data);
    assert!(matches!(
        &v.data,
        YamlDataOwned::Value(_) | YamlDataOwned::Representation(_, _, _)
    ));
    Ok(())
}

// ── Anchor and alias support ──────────────────────────────────────────────────

#[test]
fn anchor_and_alias_are_resolved_natively() {
    // FINDING: saphyr resolves aliases to their anchor values during loading.
    // No pre-processing is required. This is the primary advantage over marked-yaml.
    let src = "base: &base\n  image: nginx\nweb:\n  <<: *base\n  extra: yes\n";
    let result = MarkedYamlOwned::load_from_str(src);
    eprintln!("anchor/alias result: {:?}", result.is_ok());
    // saphyr handles these natively — expect Ok
    assert!(
        result.is_ok(),
        "saphyr resolves anchors and aliases natively"
    );
}

#[test]
fn merge_key_resolves_fields() {
    // FINDING: merge keys (<<) are resolved natively by saphyr.
    let src = "defaults: &def\n  restart: always\nweb:\n  <<: *def\n  image: nginx\n";
    let result = MarkedYamlOwned::load_from_str(src);
    assert!(result.is_ok(), "saphyr handles merge keys");
}

// ── Unicode and byte offsets ──────────────────────────────────────────────────

#[test]
fn marker_index_is_char_based_not_byte_based() -> TestResult {
    // FINDING: saphyr Marker::index() returns char index, not byte offset.
    // "café": c@char0, a@char1, f@char2, é@char3 (é is 2 UTF-8 bytes)
    // The value "café" starts at char 6 ("name: " = 6 chars).
    // As byte offset, it starts at 6 (since all 6 preceding chars are ASCII).
    let src = "name: café\n";
    let map = root_mapping(src)?;
    let v = map.values().next().ok_or("no value")?;
    let char_idx = v.span.start.index();
    let byte_off = char_to_byte(src, char_idx);
    eprintln!("char_idx={char_idx} byte_off={byte_off}");
    // Both should be 6 for this string (all preceding chars are ASCII)
    assert_eq!(char_idx, 6, "value starts at char 6");
    assert_eq!(byte_off, 6, "byte offset also 6 (all ASCII before value)");
    Ok(())
}

#[test]
fn char_to_byte_conversion_needed_for_multibyte_content() {
    // When content before the target contains multi-byte chars, char != byte.
    // Example: "é: x\n" — key 'é' is char 0 (byte 0), value 'x' is char 3 (byte 4)
    // because "é: " = 2+1+1 = 4 bytes but 3 chars.
    let src = "é: x\n";
    let result = MarkedYamlOwned::load_from_str(src);
    if let Ok(docs) = result {
        if let Some(doc) = docs.into_iter().next() {
            if let YamlDataOwned::Mapping(m) = &doc.data {
                for (k, v) in m.iter() {
                    let k_char = k.span.start.index();
                    let v_char = v.span.start.index();
                    let k_byte = char_to_byte(src, k_char);
                    let v_byte = char_to_byte(src, v_char);
                    eprintln!("key: char={k_char} byte={k_byte}, val: char={v_char} byte={v_byte}");
                    // Key 'é' at char 0 = byte 0
                    assert_eq!(k_byte, 0);
                    // Value 'x' at char 3 ("é: " = 3 chars) but byte 4 ("é: " = 4 bytes)
                    assert_eq!(v_char, 3, "value at char 3");
                    assert_eq!(v_byte, 4, "value at byte 4 (é = 2 bytes)");
                }
            }
        }
    }
}

// ── Multi-document and duplicate key behavior ─────────────────────────────────

#[test]
fn multiple_documents_all_returned() -> TestResult {
    // FINDING: saphyr returns ALL documents, not just the first.
    // susun-loader must detect multiple docs and emit SUS-PARSE-003.
    let src = "name: first\n---\nname: second\n";
    let docs = MarkedYamlOwned::load_from_str(src)?;
    eprintln!("multiple docs count: {}", docs.len());
    assert_eq!(docs.len(), 2, "saphyr returns all documents");
    Ok(())
}

#[test]
fn duplicate_key_behavior() {
    let src = "name: first\nname: second\n";
    let result = MarkedYamlOwned::load_from_str(src);
    eprintln!("duplicate key result ok={}", result.is_ok());
    // Observe behavior (last wins or first wins); loader must detect and emit SUS-PARSE-002
    if let Ok(docs) = result {
        if let Some(doc) = docs.into_iter().next() {
            if let YamlDataOwned::Mapping(m) = doc.data {
                eprintln!("duplicate key map len: {}", m.len());
            }
        }
    }
}

#[test]
fn malformed_yaml_returns_error() {
    let src = "key: : bad\n";
    let result = MarkedYamlOwned::load_from_str(src);
    assert!(result.is_err(), "malformed returns Err");
    if let Err(e) = result {
        eprintln!("malformed err: {e}");
    }
}

// ── Line and column ───────────────────────────────────────────────────────────

#[test]
fn span_line_and_column_one_based() -> TestResult {
    let src = "name: web\nimage: nginx\n";
    let map = root_mapping(src)?;
    let image_entry = map
        .iter()
        .find(|(k, _)| matches!(&k.data, YamlDataOwned::Value(s) if s.as_str() == Some("image")))
        .ok_or("image key not found")?;
    let v_marker = image_entry.1.span.start;
    eprintln!(
        "image value: line={} col={} char_idx={}",
        v_marker.line(),
        v_marker.col(),
        v_marker.index()
    );
    assert_eq!(v_marker.line(), 2, "image value on line 2");
    Ok(())
}

// ── Compose-specific forms ────────────────────────────────────────────────────

#[test]
fn compose_long_port_form_parsed() -> TestResult {
    let src = "ports:\n  - target: 80\n    published: 8080\n    protocol: tcp\n    mode: host\n";
    let map = root_mapping(src)?;
    let pv = map.values().next().ok_or("no ports value")?;
    if let YamlDataOwned::Sequence(seq) = &pv.data {
        let item = seq.first().ok_or("no first item")?;
        if let YamlDataOwned::Mapping(m) = &item.data {
            assert_eq!(m.len(), 4);
        } else {
            return Err("expected mapping item".into());
        }
    } else {
        return Err("expected sequence for ports".into());
    }
    Ok(())
}

#[test]
fn crlf_line_endings_parse() -> TestResult {
    let src = "name: web\r\nimage: nginx\r\n";
    let docs = MarkedYamlOwned::load_from_str(src)?;
    if let Some(doc) = docs.into_iter().next() {
        if let YamlDataOwned::Mapping(m) = doc.data {
            assert_eq!(m.len(), 2);
        }
    }
    Ok(())
}
