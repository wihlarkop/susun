//! Canonical redacted fingerprint input construction.

use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use susun_model::{
    CanonicalPort, CanonicalVolume, Command, ConfigName, Healthcheck, ImageRef, NetworkAttachment,
    NetworkName, ResourceMount, SecretName, Service, VolumeKind, VolumeName,
};

/// Inputs needed to build a canonical service fingerprint.
#[derive(Debug, Clone, Copy)]
pub struct FingerprintInput<'a> {
    /// Canonical service configuration.
    pub service: &'a Service,
    /// Image identity resolved by policy/runtime.
    pub resolved_image: &'a ResolvedImageIdentity,
    /// Runtime-visible names for referenced project resources.
    pub project_resource_names: &'a ResolvedResourceNames,
    /// Runtime defaults that affect container configuration.
    pub runtime_defaults: &'a RuntimeDefaults,
}

/// Resolved image identity used by fingerprint policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedImageIdentity {
    /// Original image reference selected for the service.
    pub reference: Option<ImageRef>,
    /// Content digest when known, for example `sha256:...`.
    pub digest: Option<String>,
    /// Engine image ID when known.
    pub image_id: Option<String>,
}

impl ResolvedImageIdentity {
    /// Uses the service image reference without a resolved digest.
    pub fn from_service(service: &Service) -> Self {
        Self {
            reference: service.image.clone(),
            digest: None,
            image_id: None,
        }
    }
}

/// Runtime-visible names for project resources referenced by a service.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedResourceNames {
    /// Network runtime names by model name.
    pub networks: IndexMap<NetworkName, String>,
    /// Volume runtime names by model name.
    pub volumes: IndexMap<VolumeName, String>,
    /// Config runtime names by model name.
    pub configs: IndexMap<ConfigName, String>,
    /// Secret runtime names by model name.
    pub secrets: IndexMap<SecretName, String>,
}

/// Runtime defaults that affect container configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDefaults {
    /// Default restart policy if the service does not specify one.
    pub restart_policy: Option<String>,
    /// Default network driver for implicit runtime networks.
    pub network_driver: Option<String>,
    /// Default pull behavior selected by the runtime.
    pub pull_policy: Option<String>,
}

/// Owned canonical fingerprint input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFingerprintInput {
    /// Schema version included in the canonical input bytes.
    pub schema_version: u16,
    /// Resolved image identity.
    pub image: CanonicalImage,
    /// Service command.
    pub command: Option<CanonicalCommand>,
    /// Service entrypoint.
    pub entrypoint: Option<CanonicalCommand>,
    /// Redacted environment entries sorted by key.
    pub environment: Vec<CanonicalEnvironment>,
    /// Container labels sorted by key.
    pub labels: Vec<CanonicalPair>,
    /// Port mappings sorted by semantic fields.
    pub ports: Vec<CanonicalPort>,
    /// Volume mounts sorted by semantic fields.
    pub volumes: Vec<CanonicalVolumeMount>,
    /// Network attachments sorted by network name.
    pub networks: Vec<CanonicalNetworkAttachment>,
    /// Config mounts sorted by source and target.
    pub configs: Vec<CanonicalResourceMount>,
    /// Secret mounts sorted by source and target.
    pub secrets: Vec<CanonicalResourceMount>,
    /// Healthcheck configuration.
    pub healthcheck: Option<Healthcheck>,
    /// Effective restart policy.
    pub restart_policy: Option<String>,
    /// Runtime defaults that affect container configuration.
    pub runtime_defaults: RuntimeDefaults,
}

impl CanonicalFingerprintInput {
    /// Builds canonical redacted input from service and runtime context.
    pub fn from_input(input: FingerprintInput<'_>) -> Self {
        let service = input.service;
        let mut environment = service
            .environment
            .iter()
            .map(|(key, value)| CanonicalEnvironment {
                key: key.clone(),
                value_digest: value.as_deref().map(redacted_value_digest),
                inherited: value.is_none(),
            })
            .collect::<Vec<_>>();
        environment.sort();

        let mut labels = service
            .labels
            .iter()
            .map(|(key, value)| CanonicalPair {
                key: key.clone(),
                value: value.clone(),
            })
            .collect::<Vec<_>>();
        labels.sort();

        let mut ports = service.ports.clone();
        ports.sort_by_key(|value| format!("{value:?}"));

        let mut volumes = service
            .volumes
            .iter()
            .map(|volume| CanonicalVolumeMount::new(volume, input.project_resource_names))
            .collect::<Vec<_>>();
        volumes.sort_by_key(|volume| format!("{volume:?}"));

        let mut networks = service
            .networks
            .iter()
            .map(|(name, attachment)| {
                CanonicalNetworkAttachment::new(
                    name,
                    attachment,
                    input.project_resource_names.networks.get(name),
                )
            })
            .collect::<Vec<_>>();
        networks.sort();

        let mut configs = service
            .configs
            .iter()
            .map(|mount| {
                CanonicalResourceMount::config(
                    mount,
                    input.project_resource_names.configs.get(&mount.source),
                )
            })
            .collect::<Vec<_>>();
        configs.sort();

        let mut secrets = service
            .secrets
            .iter()
            .map(|mount| {
                CanonicalResourceMount::secret(
                    mount,
                    input.project_resource_names.secrets.get(&mount.source),
                )
            })
            .collect::<Vec<_>>();
        secrets.sort();

        Self {
            schema_version: super::schema::CURRENT_FINGERPRINT_VERSION,
            image: CanonicalImage::from_resolved(input.resolved_image),
            command: service.command.as_ref().map(CanonicalCommand::from),
            entrypoint: service.entrypoint.as_ref().map(CanonicalCommand::from),
            environment,
            labels,
            ports,
            volumes,
            networks,
            configs,
            secrets,
            healthcheck: service.healthcheck.clone(),
            restart_policy: service
                .restart
                .clone()
                .or_else(|| input.runtime_defaults.restart_policy.clone()),
            runtime_defaults: input.runtime_defaults.clone(),
        }
    }
}

/// Canonical image identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalImage {
    /// Image reference.
    pub reference: Option<String>,
    /// Resolved image digest.
    pub digest: Option<String>,
    /// Resolved engine image ID.
    pub image_id: Option<String>,
}

impl CanonicalImage {
    fn from_resolved(image: &ResolvedImageIdentity) -> Self {
        Self {
            reference: image
                .reference
                .as_ref()
                .map(|value| value.as_str().to_string()),
            digest: image.digest.clone(),
            image_id: image.image_id.clone(),
        }
    }
}

/// Canonical command representation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalCommand {
    /// Shell command.
    Shell(String),
    /// Exec command.
    Exec(Vec<String>),
}

impl From<&Command> for CanonicalCommand {
    fn from(value: &Command) -> Self {
        match value {
            Command::Shell(command) => Self::Shell(command.clone()),
            Command::Exec(command) => Self::Exec(command.clone()),
        }
    }
}

/// Redacted environment entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalEnvironment {
    /// Environment key.
    pub key: String,
    /// Digest of the value, absent for inherited values.
    pub value_digest: Option<String>,
    /// Whether the value is inherited from the runtime environment.
    pub inherited: bool,
}

/// String key/value pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalPair {
    /// Pair key.
    pub key: String,
    /// Pair value.
    pub value: String,
}

/// Canonical network attachment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalNetworkAttachment {
    /// Model network name.
    pub name: String,
    /// Runtime network name, if resolved.
    pub runtime_name: Option<String>,
    /// Sorted aliases.
    pub aliases: Vec<String>,
}

impl CanonicalNetworkAttachment {
    fn new(
        name: &NetworkName,
        attachment: &NetworkAttachment,
        runtime_name: Option<&String>,
    ) -> Self {
        let mut aliases = attachment.aliases.clone();
        aliases.sort();
        Self {
            name: name.as_str().to_string(),
            runtime_name: runtime_name.cloned(),
            aliases,
        }
    }
}

/// Canonical volume mount with resolved named-volume runtime names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalVolumeMount {
    /// Mount kind.
    pub kind: VolumeKind,
    /// Model source or host path.
    pub source: Option<String>,
    /// Runtime volume name for named volumes.
    pub runtime_name: Option<String>,
    /// Container target path.
    pub target: String,
    /// Whether the mount is read-only.
    pub read_only: bool,
}

impl CanonicalVolumeMount {
    fn new(volume: &CanonicalVolume, names: &ResolvedResourceNames) -> Self {
        let runtime_name = match volume.kind {
            VolumeKind::Volume => volume
                .source
                .as_ref()
                .and_then(|source| names.volumes.get(&VolumeName::new(source.clone())))
                .cloned(),
            VolumeKind::Bind | VolumeKind::Anonymous => None,
        };

        Self {
            kind: volume.kind,
            source: volume.source.clone(),
            runtime_name,
            target: volume.target.clone(),
            read_only: volume.read_only,
        }
    }
}

/// Canonical config or secret mount without secret contents.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalResourceMount {
    /// Model resource name.
    pub source: String,
    /// Runtime resource name, if resolved.
    pub runtime_name: Option<String>,
    /// Container target.
    pub target: Option<String>,
    /// Requested uid.
    pub uid: Option<String>,
    /// Requested gid.
    pub gid: Option<String>,
    /// Requested file mode.
    pub mode: Option<String>,
}

impl CanonicalResourceMount {
    fn config(mount: &ResourceMount<ConfigName>, runtime_name: Option<&String>) -> Self {
        Self {
            source: mount.source.as_str().to_string(),
            runtime_name: runtime_name.cloned(),
            target: mount.target.clone(),
            uid: mount.uid.clone(),
            gid: mount.gid.clone(),
            mode: mount.mode.clone(),
        }
    }

    fn secret(mount: &ResourceMount<SecretName>, runtime_name: Option<&String>) -> Self {
        Self {
            source: mount.source.as_str().to_string(),
            runtime_name: runtime_name.cloned(),
            target: mount.target.clone(),
            uid: mount.uid.clone(),
            gid: mount.gid.clone(),
            mode: mount.mode.clone(),
        }
    }
}

fn redacted_value_digest(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    super::digest::encode_hex(&digest)
}
