//! Fingerprint digest construction and parsing.

use sha2::{Digest, Sha256};
use susun_engine::ConfigurationFingerprint;
use susun_model::{CanonicalPort, Healthcheck, PublishedPort, VolumeKind};

use crate::{
    ConvergenceError,
    fingerprint::{
        input::{
            CanonicalCommand, CanonicalEnvironment, CanonicalFingerprintInput, CanonicalImage,
            CanonicalNetworkAttachment, CanonicalPair, CanonicalResourceMount,
            CanonicalVolumeMount, FingerprintInput,
        },
        schema::{
            FINGERPRINT_ALGORITHM, FINGERPRINT_LABEL_PREFIX, FingerprintDigest, FingerprintVersion,
            VersionedFingerprint,
        },
    },
};

/// Computes the engine label value for a service configuration fingerprint.
pub fn compute_configuration_fingerprint(
    input: FingerprintInput<'_>,
) -> Result<ConfigurationFingerprint, ConvergenceError> {
    let canonical = CanonicalFingerprintInput::from_input(input);
    let digest = digest_canonical_input(&canonical)?;
    let fingerprint = VersionedFingerprint::current(digest);
    ConfigurationFingerprint::new(fingerprint.label_value()).map_err(|_| {
        ConvergenceError::FingerprintInvariant {
            detail: "computed fingerprint label was empty".to_string(),
        }
    })
}

/// Parses an observed engine fingerprint label.
pub fn parse_configuration_fingerprint(
    value: &ConfigurationFingerprint,
) -> Result<VersionedFingerprint, ConvergenceError> {
    let raw = value.as_str();
    let mut parts = raw.split(':');
    let version_part = parts.next().ok_or_else(invalid_fingerprint)?;
    let algorithm = parts.next().ok_or_else(invalid_fingerprint)?;
    let digest = parts.next().ok_or_else(invalid_fingerprint)?;
    if parts.next().is_some() {
        return Err(invalid_fingerprint());
    }

    let version = version_part
        .strip_prefix(FINGERPRINT_LABEL_PREFIX)
        .and_then(|value| value.strip_prefix("-v"))
        .and_then(|value| value.parse::<u16>().ok())
        .map(FingerprintVersion::new)
        .ok_or_else(invalid_fingerprint)?;

    if !version.is_supported() {
        return Err(ConvergenceError::FingerprintInvariant {
            detail: format!(
                "unsupported observed fingerprint version {}; supported version is {}",
                version.as_u16(),
                FingerprintVersion::CURRENT.as_u16()
            ),
        });
    }

    if algorithm != FINGERPRINT_ALGORITHM {
        return Err(ConvergenceError::FingerprintInvariant {
            detail: "unsupported observed fingerprint algorithm".to_string(),
        });
    }

    Ok(VersionedFingerprint {
        version,
        algorithm: FINGERPRINT_ALGORITHM,
        digest: FingerprintDigest::new(digest)?,
    })
}

fn digest_canonical_input(
    input: &CanonicalFingerprintInput,
) -> Result<FingerprintDigest, ConvergenceError> {
    let mut encoder = CanonicalEncoder::default();
    encoder.atom("schema.version", input.schema_version.to_string());
    encode_image(&mut encoder, &input.image);
    encode_command(&mut encoder, "command", input.command.as_ref());
    encode_command(&mut encoder, "entrypoint", input.entrypoint.as_ref());
    encode_environment(&mut encoder, &input.environment);
    encode_pairs(&mut encoder, "labels", &input.labels);
    encode_ports(&mut encoder, &input.ports);
    encode_volumes(&mut encoder, &input.volumes);
    encode_networks(&mut encoder, &input.networks);
    encode_mounts(&mut encoder, "configs", &input.configs);
    encode_mounts(&mut encoder, "secrets", &input.secrets);
    encode_healthcheck(&mut encoder, input.healthcheck.as_ref());
    encoder.atom_opt("restart.policy", input.restart_policy.as_deref());
    encoder.atom_opt(
        "runtime.default.restart_policy",
        input.runtime_defaults.restart_policy.as_deref(),
    );
    encoder.atom_opt(
        "runtime.default.network_driver",
        input.runtime_defaults.network_driver.as_deref(),
    );
    encoder.atom_opt(
        "runtime.default.pull_policy",
        input.runtime_defaults.pull_policy.as_deref(),
    );

    let digest = Sha256::digest(encoder.finish());
    FingerprintDigest::new(encode_hex(&digest))
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn invalid_fingerprint() -> ConvergenceError {
    ConvergenceError::FingerprintInvariant {
        detail: "observed fingerprint label is not a supported Susun fingerprint".to_string(),
    }
}

#[derive(Default)]
struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    fn atom(&mut self, key: &str, value: impl AsRef<str>) {
        let value = value.as_ref();
        self.bytes
            .extend_from_slice(key.len().to_string().as_bytes());
        self.bytes.push(b':');
        self.bytes.extend_from_slice(key.as_bytes());
        self.bytes.push(b'=');
        self.bytes
            .extend_from_slice(value.len().to_string().as_bytes());
        self.bytes.push(b':');
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(b'\n');
    }

    fn atom_opt(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(value) => self.atom(key, value),
            None => self.atom(key, "<absent>"),
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn encode_image(encoder: &mut CanonicalEncoder, image: &CanonicalImage) {
    encoder.atom_opt("image.reference", image.reference.as_deref());
    encoder.atom_opt("image.digest", image.digest.as_deref());
    encoder.atom_opt("image.id", image.image_id.as_deref());
}

fn encode_command(
    encoder: &mut CanonicalEncoder,
    prefix: &str,
    command: Option<&CanonicalCommand>,
) {
    match command {
        Some(CanonicalCommand::Shell(value)) => {
            encoder.atom(&format!("{prefix}.kind"), "shell");
            encoder.atom(&format!("{prefix}.value"), value);
        }
        Some(CanonicalCommand::Exec(values)) => {
            encoder.atom(&format!("{prefix}.kind"), "exec");
            encoder.atom(&format!("{prefix}.argc"), values.len().to_string());
            for (index, value) in values.iter().enumerate() {
                encoder.atom(&format!("{prefix}.arg.{index}"), value);
            }
        }
        None => encoder.atom(&format!("{prefix}.kind"), "<absent>"),
    }
}

fn encode_environment(encoder: &mut CanonicalEncoder, values: &[CanonicalEnvironment]) {
    encoder.atom("environment.count", values.len().to_string());
    for value in values {
        let prefix = format!("environment.{}", value.key);
        encoder.atom(&format!("{prefix}.inherited"), value.inherited.to_string());
        encoder.atom_opt(
            &format!("{prefix}.value_digest"),
            value.value_digest.as_deref(),
        );
    }
}

fn encode_pairs(encoder: &mut CanonicalEncoder, prefix: &str, values: &[CanonicalPair]) {
    encoder.atom(&format!("{prefix}.count"), values.len().to_string());
    for value in values {
        encoder.atom(&format!("{prefix}.{}.value", value.key), &value.value);
    }
}

fn encode_ports(encoder: &mut CanonicalEncoder, ports: &[CanonicalPort]) {
    encoder.atom("ports.count", ports.len().to_string());
    for (index, port) in ports.iter().enumerate() {
        let prefix = format!("ports.{index}");
        encoder.atom_opt(&format!("{prefix}.host_ip"), port.host_ip.as_deref());
        match port.published {
            Some(PublishedPort::Single(port)) => {
                encoder.atom(&format!("{prefix}.published.kind"), "single");
                encoder.atom(&format!("{prefix}.published.value"), port.to_string());
            }
            Some(PublishedPort::Range { start, end }) => {
                encoder.atom(&format!("{prefix}.published.kind"), "range");
                encoder.atom(&format!("{prefix}.published.start"), start.to_string());
                encoder.atom(&format!("{prefix}.published.end"), end.to_string());
            }
            None => encoder.atom(&format!("{prefix}.published.kind"), "<absent>"),
        }
        encoder.atom(&format!("{prefix}.target"), port.target.to_string());
        encoder.atom(
            &format!("{prefix}.protocol"),
            format!("{:?}", port.protocol),
        );
    }
}

fn encode_volumes(encoder: &mut CanonicalEncoder, volumes: &[CanonicalVolumeMount]) {
    encoder.atom("volumes.count", volumes.len().to_string());
    for (index, volume) in volumes.iter().enumerate() {
        let prefix = format!("volumes.{index}");
        let kind = match volume.kind {
            VolumeKind::Volume => "volume",
            VolumeKind::Bind => "bind",
            VolumeKind::Anonymous => "anonymous",
        };
        encoder.atom(&format!("{prefix}.kind"), kind);
        encoder.atom_opt(&format!("{prefix}.source"), volume.source.as_deref());
        encoder.atom_opt(
            &format!("{prefix}.runtime_name"),
            volume.runtime_name.as_deref(),
        );
        encoder.atom(&format!("{prefix}.target"), &volume.target);
        encoder.atom(&format!("{prefix}.read_only"), volume.read_only.to_string());
    }
}

fn encode_networks(encoder: &mut CanonicalEncoder, networks: &[CanonicalNetworkAttachment]) {
    encoder.atom("networks.count", networks.len().to_string());
    for network in networks {
        let prefix = format!("networks.{}", network.name);
        encoder.atom_opt(
            &format!("{prefix}.runtime_name"),
            network.runtime_name.as_deref(),
        );
        encoder.atom(
            &format!("{prefix}.aliases.count"),
            network.aliases.len().to_string(),
        );
        for (index, alias) in network.aliases.iter().enumerate() {
            encoder.atom(&format!("{prefix}.aliases.{index}"), alias);
        }
    }
}

fn encode_mounts(encoder: &mut CanonicalEncoder, prefix: &str, mounts: &[CanonicalResourceMount]) {
    encoder.atom(&format!("{prefix}.count"), mounts.len().to_string());
    for mount in mounts {
        let key = format!("{prefix}.{}", mount.source);
        encoder.atom_opt(
            &format!("{key}.runtime_name"),
            mount.runtime_name.as_deref(),
        );
        encoder.atom_opt(&format!("{key}.target"), mount.target.as_deref());
    }
}

fn encode_healthcheck(encoder: &mut CanonicalEncoder, healthcheck: Option<&Healthcheck>) {
    match healthcheck {
        Some(healthcheck) => {
            encoder.atom("healthcheck.present", "true");
            let command = healthcheck.test.as_ref().map(CanonicalCommand::from);
            encode_command(encoder, "healthcheck.test", command.as_ref());
            encoder.atom_opt("healthcheck.interval", healthcheck.interval.as_deref());
            encoder.atom_opt("healthcheck.timeout", healthcheck.timeout.as_deref());
            encoder.atom_opt(
                "healthcheck.start_period",
                healthcheck.start_period.as_deref(),
            );
            let retries = healthcheck.retries.map(|value| value.to_string());
            encoder.atom_opt("healthcheck.retries", retries.as_deref());
            encoder.atom("healthcheck.disable", healthcheck.disable.to_string());
        }
        None => encoder.atom("healthcheck.present", "false"),
    }
}
