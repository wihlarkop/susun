//! Converts saphyr YAML nodes to source-span-aware parsed types.
//!
//! All saphyr types are confined to this module. Nothing from `saphyr` or
//! `saphyr_parser` appears in public signatures outside of `susun-loader`.

use indexmap::IndexMap;
use saphyr::{LoadableYamlNode, MarkedYamlOwned, YamlDataOwned};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Label, Severity};
use susun_normalize::input::{
    ParsedProject, ParsedService, RawBuildDefinition, RawDependency, RawExtends, RawHealthcheck,
    RawMapping, RawNetworkAttachment, RawPortEntry, RawPortLong, RawPortShort,
    RawResourceDefinition, RawResourceMount, RawResources, RawServiceNetworks, RawStringOrList,
    RawVolumeLong, RawVolumeMount, RawVolumeShort, ServiceMergeTag,
};
use susun_source::{SourceId, Span, Spanned, TextOffset};

use crate::{environment::resolve::EnvResolver, interpolation::eval::interpolate};

const PARSE_ERROR: &str = "SUS-PARSE-001";
const MULTI_DOC: &str = "SUS-PARSE-003";
const ROOT_NOT_MAPPING: &str = "SUS-PARSE-010";
const UNKNOWN_FIELD: &str = "SUS-PARSE-011";
const UNSUPPORTED_CAPABILITY: &str = "SUS-CAP-001";

/// Parse `contents` (a single Compose YAML file) into a raw [`ParsedProject`].
///
/// Scalar values are interpolated using `resolver` before typed extraction.
/// Mapping keys are never interpolated. User errors are appended to `report`
/// as diagnostics. Returns `None` only when YAML is so malformed that no
/// structure can be recovered; recoverable problems still yield `Some`.
pub(crate) fn parse(
    source_id: SourceId,
    contents: &str,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<ParsedProject> {
    let docs = match MarkedYamlOwned::load_from_str(contents) {
        Ok(docs) => docs,
        Err(e) => {
            report.push(Diagnostic::new(PARSE_ERROR, Severity::Error, e.to_string()));
            return None;
        }
    };

    if docs.is_empty() {
        return Some(empty_project());
    }

    if docs.len() > 1 {
        let second = &docs[1];
        let span = node_span(contents, source_id, second);
        report.push(
            Diagnostic::new(
                MULTI_DOC,
                Severity::Error,
                "Compose files must contain exactly one YAML document",
            )
            .with_label(Label::primary(span, "second document starts here")),
        );
        return None;
    }

    let Some(doc) = docs.into_iter().next() else {
        return Some(empty_project());
    };

    let mapping = match untagged(&doc).data.clone() {
        YamlDataOwned::Mapping(m) => m,
        _ => {
            let span = node_span(contents, source_id, &doc);
            report.push(
                Diagnostic::new(
                    ROOT_NOT_MAPPING,
                    Severity::Error,
                    "Compose file root must be a YAML mapping",
                )
                .with_label(Label::primary(span, "expected mapping here")),
            );
            return None;
        }
    };

    let mut project_name: Option<Spanned<String>> = None;
    let mut includes = Vec::new();
    let mut services: IndexMap<String, Spanned<ParsedService>> = IndexMap::new();
    let mut networks = IndexMap::new();
    let mut volumes = IndexMap::new();
    let mut configs = IndexMap::new();
    let mut secrets = IndexMap::new();

    for (k_node, v_node) in &mapping {
        // Keys are never interpolated.
        let Some(key) = node_as_str(k_node) else {
            continue;
        };

        match key {
            "name" => {
                if let Some(interpolated) =
                    interpolated_scalar(contents, source_id, v_node, resolver, report)
                {
                    let span = node_span(contents, source_id, v_node);
                    project_name = Some(Spanned::new(interpolated, span));
                }
            }
            "services" => {
                services = parse_services(source_id, contents, v_node, resolver, report);
            }
            "include" => {
                includes = parse_includes(contents, source_id, v_node, resolver, report);
            }
            "networks" => {
                networks = parse_resources(source_id, contents, v_node);
            }
            "volumes" => {
                volumes = parse_resources(source_id, contents, v_node);
            }
            "configs" => {
                configs = parse_resources(source_id, contents, v_node);
            }
            "secrets" => {
                secrets = parse_resources(source_id, contents, v_node);
            }
            "profiles" => {}
            // Extension fields accepted silently
            k if k.starts_with("x-") => {}
            // Deferred fields: known but not yet supported
            "version" | "deploy" | "extends" | "develop" | "watch" => {
                let span = node_span(contents, source_id, k_node);
                report.push(
                    Diagnostic::new(
                        UNKNOWN_FIELD,
                        Severity::Note,
                        format!("field `{key}` is not yet supported and will be ignored"),
                    )
                    .with_label(Label::primary(span, "deferred field")),
                );
            }
            // Truly unknown fields
            _ => {
                let span = node_span(contents, source_id, k_node);
                report.push(
                    Diagnostic::new(
                        UNKNOWN_FIELD,
                        Severity::Warning,
                        format!("unknown top-level field `{key}`"),
                    )
                    .with_label(Label::primary(span, "unknown field")),
                );
            }
        }
    }

    Some(ParsedProject {
        name: project_name,
        includes,
        services,
        networks,
        volumes,
        configs,
        secrets,
    })
}

fn parse_services(
    source_id: SourceId,
    contents: &str,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> IndexMap<String, Spanned<ParsedService>> {
    let mapping = match &untagged(node).data {
        YamlDataOwned::Mapping(m) => m,
        _ => return IndexMap::new(),
    };

    let mut result: IndexMap<String, Spanned<ParsedService>> = IndexMap::new();
    for (k_node, v_node) in mapping {
        // Service names (keys) are not interpolated.
        let Some(name) = node_as_str(k_node) else {
            continue;
        };
        let service = parse_service(source_id, contents, v_node, resolver, report);
        let span = node_span(contents, source_id, v_node);
        result.insert(name.to_owned(), Spanned::new(service, span));
    }
    result
}

fn parse_service(
    source_id: SourceId,
    contents: &str,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> ParsedService {
    let mut merge_tag = ServiceMergeTag::Merge;
    let node = match &node.data {
        YamlDataOwned::Tagged(tag, inner) => {
            merge_tag = merge_tag_from_yaml(tag);
            inner.as_ref()
        }
        _ => node,
    };

    let mapping = match &untagged(node).data {
        YamlDataOwned::Mapping(m) => m,
        _ => return ParsedService::default(),
    };

    let mut image: Option<Spanned<String>> = None;
    let mut build = None;
    let mut extends = None;
    let mut command = RawStringOrList::Null;
    let mut entrypoint = RawStringOrList::Null;
    let mut environment = RawMapping::default();
    let mut labels = RawMapping::default();
    let mut ports = Vec::new();
    let mut volumes = Vec::new();
    let mut depends_on = IndexMap::new();
    let mut networks = IndexMap::new();
    let mut configs = Vec::new();
    let mut secrets = Vec::new();
    let mut healthcheck = None;
    let mut restart = None;
    let mut profiles = Vec::new();

    for (k_node, v_node) in mapping {
        // Field names (keys) are not interpolated.
        let Some(key) = node_as_str(k_node) else {
            continue;
        };
        match key {
            "image" => {
                image = interpolated_spanned(contents, source_id, v_node, resolver, report);
            }
            "build" => {
                build = parse_build(contents, source_id, v_node, resolver, report);
            }
            "extends" => {
                extends = parse_extends(contents, source_id, v_node, resolver, report);
            }
            "command" => {
                command = parse_string_or_list(contents, source_id, v_node, resolver, report)
            }
            "entrypoint" => {
                entrypoint = parse_string_or_list(contents, source_id, v_node, resolver, report);
            }
            "environment" => {
                environment = parse_mapping(contents, source_id, v_node, resolver, report)
            }
            "labels" => labels = parse_mapping(contents, source_id, v_node, resolver, report),
            "ports" => ports = parse_ports(contents, source_id, v_node, resolver, report),
            "volumes" => volumes = parse_volumes(contents, source_id, v_node, resolver, report),
            "depends_on" => {
                depends_on = parse_depends_on(contents, source_id, v_node, resolver, report)
            }
            "networks" => {
                networks = parse_service_networks(contents, source_id, v_node, resolver, report)
            }
            "configs" => {
                configs = parse_resource_mounts(contents, source_id, v_node, resolver, report)
            }
            "secrets" => {
                secrets = parse_resource_mounts(contents, source_id, v_node, resolver, report)
            }
            "healthcheck" => {
                healthcheck = parse_healthcheck(contents, source_id, v_node, resolver, report)
            }
            "restart" => {
                restart = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            "profiles" => {
                profiles = parse_string_sequence(contents, source_id, v_node, resolver, report)
            }
            "deploy" | "develop" | "watch" => {
                let span = node_span(contents, source_id, k_node);
                report.push(
                    Diagnostic::new(
                        UNSUPPORTED_CAPABILITY,
                        Severity::Error,
                        format!("service field `{key}` is not supported in Phase 1"),
                    )
                    .with_label(Label::primary(span, "unsupported Compose capability")),
                );
            }
            _ => {}
        }
    }

    ParsedService {
        merge_tag,
        image,
        build,
        extends,
        command,
        entrypoint,
        environment,
        labels,
        ports,
        volumes,
        depends_on,
        networks,
        configs,
        secrets,
        healthcheck,
        restart,
        profiles,
    }
}

fn merge_tag_from_yaml(tag: &saphyr_parser::Tag) -> ServiceMergeTag {
    match tag.suffix.as_str() {
        "reset" => ServiceMergeTag::Reset,
        "override" => ServiceMergeTag::Override,
        _ => ServiceMergeTag::Merge,
    }
}

fn parse_includes(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Vec<Spanned<String>> {
    match &untagged(node).data {
        YamlDataOwned::Sequence(items) => items
            .iter()
            .filter_map(|item| parse_include_entry(contents, source_id, item, resolver, report))
            .collect(),
        _ => parse_include_entry(contents, source_id, node, resolver, report)
            .into_iter()
            .collect(),
    }
}

fn parse_include_entry(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<Spanned<String>> {
    match &untagged(node).data {
        YamlDataOwned::Mapping(fields) => fields.iter().find_map(|(k_node, v_node)| {
            if node_as_str(k_node) == Some("path") {
                interpolated_spanned(contents, source_id, v_node, resolver, report)
            } else {
                None
            }
        }),
        _ => interpolated_spanned(contents, source_id, node, resolver, report),
    }
}

fn parse_extends(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<RawExtends> {
    match &untagged(node).data {
        YamlDataOwned::Mapping(fields) => {
            let mut service = None;
            let mut file = None;
            for (k_node, v_node) in fields {
                match node_as_str(k_node) {
                    Some("service") => {
                        service =
                            interpolated_spanned(contents, source_id, v_node, resolver, report)
                    }
                    Some("file") => {
                        file = interpolated_spanned(contents, source_id, v_node, resolver, report)
                    }
                    _ => {}
                }
            }
            service.map(|service| RawExtends { service, file })
        }
        _ => interpolated_spanned(contents, source_id, node, resolver, report).map(|service| {
            RawExtends {
                service,
                file: None,
            }
        }),
    }
}

fn parse_build(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<RawBuildDefinition> {
    match &untagged(node).data {
        YamlDataOwned::Mapping(fields) => {
            let mut build = RawBuildDefinition::default();
            for (k_node, v_node) in fields {
                match node_as_str(k_node) {
                    Some("context") => {
                        build.context =
                            interpolated_spanned(contents, source_id, v_node, resolver, report)
                    }
                    Some("dockerfile") => {
                        build.dockerfile =
                            interpolated_spanned(contents, source_id, v_node, resolver, report)
                    }
                    Some("target") => {
                        build.target =
                            interpolated_spanned(contents, source_id, v_node, resolver, report)
                    }
                    Some("args") => {
                        build.args =
                            match parse_mapping(contents, source_id, v_node, resolver, report) {
                                RawMapping::Map(map) => map,
                                RawMapping::List(entries) => args_list_to_map(entries),
                            }
                    }
                    Some("platforms") => {
                        build.platforms =
                            parse_string_sequence(contents, source_id, v_node, resolver, report)
                    }
                    Some("secrets") => {
                        build.secrets =
                            parse_build_identities(contents, source_id, v_node, resolver, report)
                    }
                    Some("ssh") => {
                        build.ssh =
                            parse_build_identities(contents, source_id, v_node, resolver, report)
                    }
                    Some("cache_from") => {
                        build.cache_from =
                            parse_string_sequence(contents, source_id, v_node, resolver, report)
                    }
                    Some("cache_to") => {
                        build.cache_to =
                            parse_string_sequence(contents, source_id, v_node, resolver, report)
                    }
                    _ => {}
                }
            }
            Some(build)
        }
        _ => interpolated_spanned(contents, source_id, node, resolver, report).map(|context| {
            RawBuildDefinition {
                context: Some(context),
                ..RawBuildDefinition::default()
            }
        }),
    }
}

fn args_list_to_map(entries: Vec<Spanned<String>>) -> IndexMap<String, Option<Spanned<String>>> {
    entries
        .into_iter()
        .map(|entry| match entry.value.find('=') {
            Some(eq) => (
                entry.value[..eq].to_owned(),
                Some(Spanned::new(entry.value[eq + 1..].to_owned(), entry.span)),
            ),
            None => (entry.value, None),
        })
        .collect()
}

fn parse_build_identities(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Vec<Spanned<String>> {
    let YamlDataOwned::Sequence(items) = &untagged(node).data else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match &untagged(item).data {
            YamlDataOwned::Mapping(fields) => {
                parse_build_identity_mapping(contents, source_id, fields.iter(), resolver, report)
            }
            _ => interpolated_spanned(contents, source_id, item, resolver, report),
        })
        .collect()
}

fn parse_build_identity_mapping<'a>(
    contents: &str,
    source_id: SourceId,
    fields: impl IntoIterator<Item = (&'a MarkedYamlOwned, &'a MarkedYamlOwned)>,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<Spanned<String>> {
    let mut id = None;
    let mut source = None;
    for (k_node, v_node) in fields {
        match node_as_str(k_node) {
            Some("id") => id = interpolated_spanned(contents, source_id, v_node, resolver, report),
            Some("source") => {
                source = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            _ => {}
        }
    }
    id.or(source)
}

fn empty_project() -> ParsedProject {
    ParsedProject {
        name: None,
        includes: Vec::new(),
        services: IndexMap::new(),
        networks: IndexMap::new(),
        volumes: IndexMap::new(),
        configs: IndexMap::new(),
        secrets: IndexMap::new(),
    }
}

fn parse_resources(source_id: SourceId, contents: &str, node: &MarkedYamlOwned) -> RawResources {
    let YamlDataOwned::Mapping(mapping) = &untagged(node).data else {
        return IndexMap::new();
    };

    let mut resources = IndexMap::new();
    for (k_node, v_node) in mapping {
        let Some(name) = node_as_str(k_node) else {
            continue;
        };
        let mut definition = RawResourceDefinition::default();
        if let YamlDataOwned::Mapping(fields) = &untagged(v_node).data {
            for (field_node, value_node) in fields {
                if node_as_str(field_node) == Some("external") {
                    definition.external = scalar_spanned(contents, source_id, value_node);
                }
            }
        }
        let span = node_span(contents, source_id, v_node);
        resources.insert(name.to_owned(), Spanned::new(definition, span));
    }
    resources
}

fn parse_string_or_list(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawStringOrList {
    match &untagged(node).data {
        YamlDataOwned::Sequence(items) => RawStringOrList::List(
            items
                .iter()
                .filter_map(|item| {
                    interpolated_spanned(contents, source_id, item, resolver, report)
                })
                .collect(),
        ),
        _ => interpolated_spanned(contents, source_id, node, resolver, report)
            .map(RawStringOrList::String)
            .unwrap_or(RawStringOrList::Null),
    }
}

fn parse_string_sequence(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Vec<Spanned<String>> {
    let YamlDataOwned::Sequence(items) = &untagged(node).data else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| interpolated_spanned(contents, source_id, item, resolver, report))
        .collect()
}

fn parse_mapping(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawMapping {
    match &untagged(node).data {
        YamlDataOwned::Mapping(mapping) => {
            let mut result = IndexMap::new();
            for (k_node, v_node) in mapping {
                let Some(key) = node_as_str(k_node) else {
                    continue;
                };
                result.insert(
                    key.to_owned(),
                    interpolated_spanned(contents, source_id, v_node, resolver, report),
                );
            }
            RawMapping::Map(result)
        }
        YamlDataOwned::Sequence(items) => RawMapping::List(
            items
                .iter()
                .filter_map(|item| {
                    interpolated_spanned(contents, source_id, item, resolver, report)
                })
                .collect(),
        ),
        _ => RawMapping::default(),
    }
}

fn parse_ports(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Vec<RawPortEntry> {
    let YamlDataOwned::Sequence(items) = &untagged(node).data else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match &untagged(item).data {
            YamlDataOwned::Mapping(fields) => Some(RawPortEntry::Long(parse_port_long(
                contents,
                source_id,
                fields.iter(),
                resolver,
                report,
            ))),
            _ => interpolated_spanned(contents, source_id, item, resolver, report)
                .map(|value| RawPortEntry::Short(RawPortShort(value))),
        })
        .collect()
}

fn parse_port_long<'a>(
    contents: &str,
    source_id: SourceId,
    fields: impl IntoIterator<Item = (&'a MarkedYamlOwned, &'a MarkedYamlOwned)>,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawPortLong {
    let mut port = RawPortLong::default();
    for (k_node, v_node) in fields {
        match node_as_str(k_node) {
            Some("target") => {
                port.target = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("published") => {
                port.published = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("host_ip") => {
                port.host_ip = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("protocol") => {
                port.protocol = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("mode") => {
                port.mode = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            _ => {}
        }
    }
    port
}

fn parse_volumes(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Vec<RawVolumeMount> {
    let YamlDataOwned::Sequence(items) = &untagged(node).data else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match &untagged(item).data {
            YamlDataOwned::Mapping(fields) => Some(RawVolumeMount::Long(parse_volume_long(
                contents,
                source_id,
                fields.iter(),
                resolver,
                report,
            ))),
            _ => interpolated_spanned(contents, source_id, item, resolver, report)
                .map(|value| RawVolumeMount::Short(RawVolumeShort(value))),
        })
        .collect()
}

fn parse_volume_long<'a>(
    contents: &str,
    source_id: SourceId,
    fields: impl IntoIterator<Item = (&'a MarkedYamlOwned, &'a MarkedYamlOwned)>,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawVolumeLong {
    let mut volume = RawVolumeLong::default();
    for (k_node, v_node) in fields {
        match node_as_str(k_node) {
            Some("type") => {
                volume.volume_type =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("source") => {
                volume.source = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("target") => {
                volume.target = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("read_only") => {
                volume.read_only =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            _ => {}
        }
    }
    volume
}

fn parse_depends_on(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> IndexMap<String, RawDependency> {
    match &untagged(node).data {
        YamlDataOwned::Sequence(items) => items
            .iter()
            .filter_map(|item| interpolated_spanned(contents, source_id, item, resolver, report))
            .map(|name| (name.value, RawDependency::default()))
            .collect(),
        YamlDataOwned::Mapping(mapping) => {
            let mut result = IndexMap::new();
            for (k_node, v_node) in mapping {
                let Some(name) = node_as_str(k_node) else {
                    continue;
                };
                result.insert(
                    name.to_owned(),
                    parse_dependency(contents, source_id, v_node, resolver, report),
                );
            }
            result
        }
        _ => IndexMap::new(),
    }
}

fn parse_dependency(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawDependency {
    let YamlDataOwned::Mapping(fields) = &untagged(node).data else {
        return RawDependency::default();
    };
    let mut dep = RawDependency::default();
    for (k_node, v_node) in fields {
        match node_as_str(k_node) {
            Some("condition") => {
                dep.condition = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("restart") => {
                dep.restart = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("required") => {
                dep.required = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            _ => {}
        }
    }
    dep
}

fn parse_service_networks(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawServiceNetworks {
    match &untagged(node).data {
        YamlDataOwned::Sequence(items) => items
            .iter()
            .filter_map(|item| interpolated_spanned(contents, source_id, item, resolver, report))
            .map(|name| (name.value, RawNetworkAttachment::default()))
            .collect(),
        YamlDataOwned::Mapping(mapping) => {
            let mut result = IndexMap::new();
            for (k_node, v_node) in mapping {
                let Some(name) = node_as_str(k_node) else {
                    continue;
                };
                result.insert(
                    name.to_owned(),
                    parse_network_attachment(contents, source_id, v_node, resolver, report),
                );
            }
            result
        }
        _ => IndexMap::new(),
    }
}

fn parse_network_attachment(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> RawNetworkAttachment {
    let YamlDataOwned::Mapping(fields) = &untagged(node).data else {
        return RawNetworkAttachment::default();
    };
    let mut attachment = RawNetworkAttachment::default();
    for (k_node, v_node) in fields {
        if node_as_str(k_node) == Some("aliases") {
            attachment.aliases =
                parse_string_sequence(contents, source_id, v_node, resolver, report);
        }
    }
    attachment
}

fn parse_resource_mounts(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Vec<RawResourceMount> {
    let YamlDataOwned::Sequence(items) = &untagged(node).data else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match &untagged(item).data {
            YamlDataOwned::Mapping(fields) => {
                parse_resource_mount(contents, source_id, fields.iter(), resolver, report)
            }
            _ => interpolated_spanned(contents, source_id, item, resolver, report).map(|source| {
                RawResourceMount {
                    source,
                    target: None,
                }
            }),
        })
        .collect()
}

fn parse_resource_mount<'a>(
    contents: &str,
    source_id: SourceId,
    fields: impl IntoIterator<Item = (&'a MarkedYamlOwned, &'a MarkedYamlOwned)>,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<RawResourceMount> {
    let mut source = None;
    let mut target = None;
    for (k_node, v_node) in fields {
        match node_as_str(k_node) {
            Some("source") => {
                source = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("target") => {
                target = interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            _ => {}
        }
    }
    source.map(|source| RawResourceMount { source, target })
}

fn parse_healthcheck(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<RawHealthcheck> {
    let YamlDataOwned::Mapping(fields) = &untagged(node).data else {
        return None;
    };
    let mut healthcheck = RawHealthcheck::default();
    for (k_node, v_node) in fields {
        match node_as_str(k_node) {
            Some("test") => {
                healthcheck.test =
                    parse_string_or_list(contents, source_id, v_node, resolver, report)
            }
            Some("interval") => {
                healthcheck.interval =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("timeout") => {
                healthcheck.timeout =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("start_period") => {
                healthcheck.start_period =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("retries") => {
                healthcheck.retries =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            Some("disable") => {
                healthcheck.disable =
                    interpolated_spanned(contents, source_id, v_node, resolver, report)
            }
            _ => {}
        }
    }
    Some(healthcheck)
}

/// Extracts a scalar string from `node` and applies environment interpolation.
///
/// Returns `None` if the node is not a scalar. Any `SUS-ENV-001` diagnostics
/// from required-variable failures are appended to `report`.
fn interpolated_scalar(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<String> {
    let raw = node_as_str(node)?;
    let span = node_span(contents, source_id, node);
    Some(interpolate(raw, resolver, span, report))
}

fn interpolated_spanned(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
    resolver: &EnvResolver,
    report: &mut DiagnosticReport,
) -> Option<Spanned<String>> {
    let value = interpolated_scalar(contents, source_id, node, resolver, report)?;
    let span = node_span(contents, source_id, node);
    Some(Spanned::new(value, span))
}

fn scalar_spanned(
    contents: &str,
    source_id: SourceId,
    node: &MarkedYamlOwned,
) -> Option<Spanned<String>> {
    let value = node_as_str(node)?.to_owned();
    let span = node_span(contents, source_id, node);
    Some(Spanned::new(value, span))
}

/// Extract a string value from a YAML node, handling both `Value` and tagged `Representation`.
fn node_as_str(node: &MarkedYamlOwned) -> Option<&str> {
    match &untagged(node).data {
        YamlDataOwned::Value(v) => v.as_str(),
        YamlDataOwned::Representation(v, _, _) => Some(v.as_str()),
        _ => None,
    }
}

fn untagged(node: &MarkedYamlOwned) -> &MarkedYamlOwned {
    match &node.data {
        YamlDataOwned::Tagged(_, inner) => untagged(inner.as_ref()),
        _ => node,
    }
}

/// Convert a saphyr node's char-based span into a byte-offset [`Span`].
fn node_span(contents: &str, source_id: SourceId, node: &MarkedYamlOwned) -> Span {
    let start_byte = char_to_byte(contents, node.span.start.index()) as u32;
    let end_byte = char_to_byte(contents, node.span.end.index()) as u32;
    let start = TextOffset::new(start_byte);
    // Guard against end < start (defensive; saphyr should never produce this)
    let end = TextOffset::new(end_byte.max(start_byte));
    Span::new(source_id, start, end).unwrap_or_else(|_| Span::empty(source_id, start))
}

/// Convert a saphyr char-index to a UTF-8 byte offset in `src`.
fn char_to_byte(src: &str, char_index: usize) -> usize {
    src.char_indices()
        .nth(char_index)
        .map_or(src.len(), |(b, _)| b)
}
