//! SDK-first workflow facade for applications embedding Susun.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use susun_engine::{
    ContainerEngine, EngineCapabilities, EngineError, EngineSnapshot, ProjectIdentity,
    ProjectInstanceId, RuntimeDoctorReport, RuntimeDoctorStatus,
};
use susun_model::{Project, ProjectName, port::PublishedPort, volume::VolumeKind};
use susun_planner::{
    BuildPolicy, DownPlanOptions, ExecutionPlan, PlanError, PlanOutcome, UpPlanOptions,
};

use crate::{
    AnalysisResult, Analyzer, Error, LoadContext, Planner, RuntimeOperationError,
    RuntimeOperationResult, RuntimeOverview, RuntimeStatusSummary,
    down_with_engine as execute_down_with_engine,
    down_with_engine_events as execute_down_with_engine_events, runtime_overview,
    runtime_status_from_snapshot as summarize_runtime_status,
    up_with_engine as execute_up_with_engine,
    up_with_engine_events as execute_up_with_engine_events,
};
use susun_runtime::{CancellationToken, EventSink};

/// High-level SDK workspace builder.
///
/// Use this type when embedding Susun in another application. It owns the same
/// context flags a CLI would normally parse, but returns structured Rust data.
#[derive(Debug, Clone)]
pub struct SusunWorkspace {
    files: Vec<PathBuf>,
    env_file: Option<PathBuf>,
    project_name: Option<String>,
    profiles: Vec<String>,
}

impl SusunWorkspace {
    /// Creates a workspace using the default `compose.yaml` path.
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            env_file: None,
            project_name: None,
            profiles: Vec::new(),
        }
    }

    /// Creates a workspace from one primary Compose file.
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        Self::new().with_file(path)
    }

    /// Adds a Compose file. Later files overlay earlier files.
    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push(path.into());
        self
    }

    /// Replaces the full Compose file list.
    pub fn with_files(mut self, files: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        self.files = files.into_iter().map(Into::into).collect();
        self
    }

    /// Sets an explicit `.env`-format file.
    pub fn with_env_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.env_file = Some(path.into());
        self
    }

    /// Overrides the Compose project name.
    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = Some(name.into());
        self
    }

    /// Activates one Compose profile.
    pub fn with_profile(mut self, profile: impl Into<String>) -> Self {
        self.profiles.push(profile.into());
        self
    }

    /// Activates multiple Compose profiles.
    pub fn with_profiles(mut self, profiles: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.profiles = profiles.into_iter().map(Into::into).collect();
        self
    }

    /// Returns the primary Compose file, defaulting to `compose.yaml`.
    pub fn primary_file(&self) -> PathBuf {
        self.files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("compose.yaml"))
    }

    /// Returns the project directory used for stable identity derivation.
    pub fn project_directory(&self) -> PathBuf {
        self.primary_file()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Builds an [`Analyzer`] from this workspace.
    pub fn analyzer(&self) -> Analyzer {
        let primary = self.primary_file();
        let additional = if self.files.len() > 1 {
            self.files[1..].to_vec()
        } else {
            Vec::new()
        };

        let mut context = LoadContext::new(primary);
        if !additional.is_empty() {
            context = context.with_additional_files(additional);
        }
        if let Some(name) = &self.project_name {
            context = context.with_project_name(name);
        }
        if !self.profiles.is_empty() {
            context = context.with_profiles(self.profiles.clone());
        }

        let mut analyzer = Analyzer::with_context(context);
        if let Some(env_file) = &self.env_file {
            analyzer = analyzer.with_env_file(env_file);
        }
        analyzer
    }

    /// Runs analysis and packages the result with SDK-facing metadata.
    pub fn analyze(&self) -> Result<SdkProject, Error> {
        let analysis = self.analyzer().analyze()?;
        let identity = analysis.project.as_ref().map(|project| {
            ProjectIdentity::new(
                project.name.clone(),
                ProjectInstanceId::derive(&project.name, self.project_directory()),
            )
        });
        Ok(SdkProject {
            workspace: self.clone(),
            analysis,
            identity,
        })
    }
}

impl Default for SusunWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

/// An analyzed project plus SDK convenience operations.
#[derive(Debug)]
pub struct SdkProject {
    workspace: SusunWorkspace,
    analysis: AnalysisResult,
    identity: Option<ProjectIdentity>,
}

impl SdkProject {
    /// Returns the original workspace options.
    pub fn workspace(&self) -> &SusunWorkspace {
        &self.workspace
    }

    /// Returns the full analysis result.
    pub fn analysis(&self) -> &AnalysisResult {
        &self.analysis
    }

    /// Returns the canonical project when analysis produced one.
    pub fn project(&self) -> Option<&Project> {
        self.analysis.project.as_ref()
    }

    /// Returns the stable project identity when analysis produced a project.
    pub fn identity(&self) -> Option<&ProjectIdentity> {
        self.identity.as_ref()
    }

    /// Returns a serializable summary suitable for apps, CLIs, and UIs.
    pub fn summary(&self) -> ProjectSummary {
        ProjectSummary::from_sdk_project(self)
    }

    /// Builds runtime status from an already-acquired engine snapshot.
    pub fn runtime_status_from_snapshot(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Option<RuntimeStatusSummary> {
        self.identity
            .as_ref()
            .map(|identity| summarize_runtime_status(identity, snapshot))
    }

    /// Acquires a project-scoped snapshot and returns SDK-friendly runtime status.
    pub async fn runtime_status_with_engine<E>(
        &self,
        engine: &E,
    ) -> Result<Option<RuntimeStatusSummary>, EngineError>
    where
        E: ContainerEngine + ?Sized,
    {
        let Some(identity) = &self.identity else {
            return Ok(None);
        };
        let snapshot = engine.snapshot(identity).await?;
        Ok(Some(summarize_runtime_status(identity, &snapshot)))
    }

    /// Combines a runtime doctor report with project status from a supplied engine.
    pub async fn runtime_overview_with_engine<E>(
        &self,
        doctor: RuntimeDoctorReport,
        engine: &E,
    ) -> Result<RuntimeOverview, EngineError>
    where
        E: ContainerEngine + ?Sized,
    {
        let status = if doctor.status == RuntimeDoctorStatus::Available {
            self.runtime_status_with_engine(engine).await?
        } else {
            None
        };
        Ok(runtime_overview(doctor, status))
    }

    /// Plans `up` against explicit capabilities and snapshot.
    pub fn plan_up(
        &self,
        capabilities: EngineCapabilities,
        snapshot: EngineSnapshot,
        options: UpPlanOptions,
    ) -> Result<PlanOutcome, PlanError> {
        let Some(identity) = self.identity.clone() else {
            return Ok(Planner::blocked_by_analysis());
        };
        Planner::new(identity, capabilities, snapshot).plan_up(&self.analysis, options)
    }

    /// Plans `down` against explicit capabilities and snapshot.
    pub fn plan_down(
        &self,
        capabilities: EngineCapabilities,
        snapshot: EngineSnapshot,
        options: DownPlanOptions,
    ) -> Result<PlanOutcome, PlanError> {
        let Some(identity) = self.identity.clone() else {
            return Ok(Planner::blocked_by_analysis());
        };
        Planner::new(identity, capabilities, snapshot).plan_down(&self.analysis, options)
    }

    /// Plans `up` using capabilities and a project snapshot from a supplied engine.
    pub async fn plan_up_with_engine<E>(
        &self,
        engine: &E,
        options: UpPlanOptions,
    ) -> Result<PlanOutcome, RuntimeOperationError>
    where
        E: ContainerEngine + ?Sized,
    {
        let identity = self
            .identity
            .clone()
            .ok_or(RuntimeOperationError::MissingProject)?;
        let capabilities = engine.capabilities().await?;
        let snapshot = engine.snapshot(&identity).await?;
        Ok(Planner::new(identity, capabilities, snapshot).plan_up(&self.analysis, options)?)
    }

    /// Plans `down` using capabilities and a project snapshot from a supplied engine.
    pub async fn plan_down_with_engine<E>(
        &self,
        engine: &E,
        options: DownPlanOptions,
    ) -> Result<PlanOutcome, RuntimeOperationError>
    where
        E: ContainerEngine + ?Sized,
    {
        let identity = self
            .identity
            .clone()
            .ok_or(RuntimeOperationError::MissingProject)?;
        let capabilities = engine.capabilities().await?;
        let snapshot = engine.snapshot(&identity).await?;
        Ok(Planner::new(identity, capabilities, snapshot).plan_down(&self.analysis, options)?)
    }

    /// Creates a daemon-free local `up` plan for inspection and approvals.
    pub fn dry_run_up(&self, build: bool) -> Result<PlanOutcome, PlanError> {
        let options = UpPlanOptions {
            build_policy: if build {
                BuildPolicy::BuildDeclared
            } else {
                BuildPolicy::NeverBuild
            },
            ..UpPlanOptions::default()
        };
        self.plan_up(
            EngineCapabilities::permissive_local(),
            EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
            options,
        )
    }

    /// Returns the successful plan from [`Self::dry_run_up`] when available.
    pub fn dry_run_up_plan(&self, build: bool) -> Result<Option<ExecutionPlan>, PlanError> {
        self.dry_run_up(build).map(|outcome| outcome.plan)
    }

    /// Creates a daemon-free local `down` plan for inspection and approvals.
    pub fn dry_run_down(&self) -> Result<PlanOutcome, PlanError> {
        self.plan_down(
            EngineCapabilities::permissive_local(),
            EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
            DownPlanOptions::default(),
        )
    }

    /// Returns the successful plan from [`Self::dry_run_down`] when available.
    pub fn dry_run_down_plan(&self) -> Result<Option<ExecutionPlan>, PlanError> {
        self.dry_run_down().map(|outcome| outcome.plan)
    }

    /// Plans and executes `up` with a supplied engine.
    pub async fn up_with_engine<E>(
        &self,
        engine: Arc<E>,
        options: UpPlanOptions,
    ) -> Result<RuntimeOperationResult, RuntimeOperationError>
    where
        E: ContainerEngine + 'static,
    {
        let identity = self
            .identity
            .clone()
            .ok_or(RuntimeOperationError::MissingProject)?;
        execute_up_with_engine(&self.analysis, identity, engine, options).await
    }

    /// Plans and executes `down` with a supplied engine.
    pub async fn down_with_engine<E>(
        &self,
        engine: Arc<E>,
        options: DownPlanOptions,
    ) -> Result<RuntimeOperationResult, RuntimeOperationError>
    where
        E: ContainerEngine + 'static,
    {
        let identity = self
            .identity
            .clone()
            .ok_or(RuntimeOperationError::MissingProject)?;
        execute_down_with_engine(&self.analysis, identity, engine, options).await
    }

    /// Plans and executes `up` with event streaming and cooperative cancellation.
    pub async fn up_with_engine_events<E>(
        &self,
        engine: Arc<E>,
        options: UpPlanOptions,
        events: EventSink,
        cancellation: CancellationToken,
    ) -> Result<RuntimeOperationResult, RuntimeOperationError>
    where
        E: ContainerEngine + 'static,
    {
        let identity = self
            .identity
            .clone()
            .ok_or(RuntimeOperationError::MissingProject)?;
        execute_up_with_engine_events(
            &self.analysis,
            identity,
            engine,
            options,
            events,
            cancellation,
        )
        .await
    }

    /// Plans and executes `down` with event streaming and cooperative cancellation.
    pub async fn down_with_engine_events<E>(
        &self,
        engine: Arc<E>,
        options: DownPlanOptions,
        events: EventSink,
        cancellation: CancellationToken,
    ) -> Result<RuntimeOperationResult, RuntimeOperationError>
    where
        E: ContainerEngine + 'static,
    {
        let identity = self
            .identity
            .clone()
            .ok_or(RuntimeOperationError::MissingProject)?;
        execute_down_with_engine_events(
            &self.analysis,
            identity,
            engine,
            options,
            events,
            cancellation,
        )
        .await
    }
}

/// Serializable project summary for SDK consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSummary {
    /// Serialized project summary schema version.
    pub schema_version: ProjectSummarySchemaVersion,
    /// Project name, when analysis produced a canonical project.
    pub project_name: Option<String>,
    /// Opaque project instance ID, when analysis produced a canonical project.
    pub project_instance: Option<String>,
    /// Number of declared services.
    pub service_count: usize,
    /// Number of active services after profile selection.
    pub active_service_count: usize,
    /// Number of declared networks.
    pub network_count: usize,
    /// Number of declared volumes.
    pub volume_count: usize,
    /// Number of declared configs.
    pub config_count: usize,
    /// Number of declared secrets.
    pub secret_count: usize,
    /// Declared networks in canonical project order.
    pub networks: Vec<ProjectResourceSummary>,
    /// Declared volumes in canonical project order.
    pub volumes: Vec<ProjectResourceSummary>,
    /// Declared configs in canonical project order.
    pub configs: Vec<ProjectResourceSummary>,
    /// Declared secrets in canonical project order.
    pub secrets: Vec<ProjectResourceSummary>,
    /// Whether analysis emitted error diagnostics.
    pub has_errors: bool,
    /// Number of diagnostics at all severities.
    pub diagnostic_count: usize,
    /// Per-service summaries in canonical project order.
    pub services: Vec<ServiceSummary>,
}

impl ProjectSummary {
    fn from_sdk_project(project: &SdkProject) -> Self {
        let Some(canonical) = project.analysis.project.as_ref() else {
            return Self {
                schema_version: ProjectSummarySchemaVersion::CURRENT,
                project_name: None,
                project_instance: None,
                service_count: 0,
                active_service_count: 0,
                network_count: 0,
                volume_count: 0,
                config_count: 0,
                secret_count: 0,
                networks: Vec::new(),
                volumes: Vec::new(),
                configs: Vec::new(),
                secrets: Vec::new(),
                has_errors: project.analysis.report.has_errors(),
                diagnostic_count: project.analysis.report.sorted().len(),
                services: Vec::new(),
            };
        };

        let active = project
            .analysis
            .selection
            .as_ref()
            .map(|selection| &selection.active_services);

        let services = canonical
            .services
            .iter()
            .map(|(name, service)| ServiceSummary {
                name: name.as_str().to_owned(),
                active: active.is_some_and(|services| services.contains(name)),
                image: service
                    .image
                    .as_ref()
                    .map(|image| image.as_str().to_owned()),
                has_build: service.build.is_some(),
                profile_count: service.profiles.len(),
                profiles: service.profiles.clone(),
                port_count: service.ports.len(),
                ports: service
                    .ports
                    .iter()
                    .map(ServicePortSummary::from_canonical)
                    .collect(),
                volume_count: service.volumes.len(),
                volumes: service
                    .volumes
                    .iter()
                    .map(ServiceVolumeSummary::from_canonical)
                    .collect(),
                network_count: service.networks.len(),
                networks: service
                    .networks
                    .keys()
                    .map(|network| network.as_str().to_owned())
                    .collect(),
                config_count: service.configs.len(),
                configs: service
                    .configs
                    .iter()
                    .map(|mount| mount.source.as_str().to_owned())
                    .collect(),
                secret_count: service.secrets.len(),
                secrets: service
                    .secrets
                    .iter()
                    .map(|mount| mount.source.as_str().to_owned())
                    .collect(),
                dependency_count: service.depends_on.len(),
                dependencies: service
                    .depends_on
                    .keys()
                    .map(|dependency| dependency.as_str().to_owned())
                    .collect(),
            })
            .collect();

        Self {
            schema_version: ProjectSummarySchemaVersion::CURRENT,
            project_name: Some(canonical.name.as_str().to_owned()),
            project_instance: project
                .identity
                .as_ref()
                .map(|id| id.working_set.to_string()),
            service_count: canonical.services.len(),
            active_service_count: active.map_or(0, |services| services.len()),
            network_count: canonical.networks.len(),
            volume_count: canonical.volumes.len(),
            config_count: canonical.configs.len(),
            secret_count: canonical.secrets.len(),
            networks: canonical
                .networks
                .iter()
                .map(|(name, definition)| ProjectResourceSummary {
                    name: name.as_str().to_owned(),
                    external: definition.external,
                })
                .collect(),
            volumes: canonical
                .volumes
                .iter()
                .map(|(name, definition)| ProjectResourceSummary {
                    name: name.as_str().to_owned(),
                    external: definition.external,
                })
                .collect(),
            configs: canonical
                .configs
                .iter()
                .map(|(name, definition)| ProjectResourceSummary {
                    name: name.as_str().to_owned(),
                    external: definition.external,
                })
                .collect(),
            secrets: canonical
                .secrets
                .iter()
                .map(|(name, definition)| ProjectResourceSummary {
                    name: name.as_str().to_owned(),
                    external: definition.external,
                })
                .collect(),
            has_errors: project.analysis.report.has_errors(),
            diagnostic_count: project.analysis.report.sorted().len(),
            services,
        }
    }
}

/// Renders a project summary as pretty JSON using the public SDK schema.
pub fn render_project_summary_json(summary: &ProjectSummary) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(summary)
}

/// Parses a project summary from JSON using the public SDK schema.
pub fn parse_project_summary_json(input: &str) -> Result<ProjectSummary, serde_json::Error> {
    serde_json::from_str(input)
}

/// Serializable top-level project resource summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectResourceSummary {
    /// Resource name.
    pub name: String,
    /// Whether the resource is external.
    pub external: bool,
}

/// Serialized project summary schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSummarySchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl ProjectSummarySchemaVersion {
    /// Current project summary schema version.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };
}

/// Serializable service summary for SDK consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSummary {
    /// Service name.
    pub name: String,
    /// Whether this service is active after profile selection.
    pub active: bool,
    /// Image reference, if declared.
    pub image: Option<String>,
    /// Whether the service declares a build definition.
    pub has_build: bool,
    /// Number of profiles on this service.
    pub profile_count: usize,
    /// Profile names on this service.
    pub profiles: Vec<String>,
    /// Number of published/container ports in the canonical model.
    pub port_count: usize,
    /// Port mappings in canonical service order.
    pub ports: Vec<ServicePortSummary>,
    /// Number of service volume mounts.
    pub volume_count: usize,
    /// Volume mounts in canonical service order.
    pub volumes: Vec<ServiceVolumeSummary>,
    /// Number of attached networks.
    pub network_count: usize,
    /// Attached network names.
    pub networks: Vec<String>,
    /// Number of mounted configs.
    pub config_count: usize,
    /// Referenced config names.
    pub configs: Vec<String>,
    /// Number of mounted secrets.
    pub secret_count: usize,
    /// Referenced secret names. Secret values and file contents are never included.
    pub secrets: Vec<String>,
    /// Number of service dependencies.
    pub dependency_count: usize,
    /// Dependency service names.
    pub dependencies: Vec<String>,
}

/// Serializable service port summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServicePortSummary {
    /// Host IP address to bind, if specified.
    pub host_ip: Option<String>,
    /// Host-side published port or range, if specified.
    pub published: Option<String>,
    /// Container-side target port.
    pub target: u16,
    /// Transport protocol.
    pub protocol: String,
}

impl ServicePortSummary {
    fn from_canonical(port: &susun_model::port::CanonicalPort) -> Self {
        Self {
            host_ip: port.host_ip.clone(),
            published: port.published.map(published_port_summary),
            target: port.target,
            protocol: match port.protocol {
                susun_model::port::Protocol::Tcp => "tcp",
                susun_model::port::Protocol::Udp => "udp",
                susun_model::port::Protocol::Sctp => "sctp",
            }
            .to_owned(),
        }
    }
}

/// Serializable service volume summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceVolumeSummary {
    /// Mount kind: `volume`, `bind`, or `anonymous`.
    pub kind: String,
    /// Source path or volume name, when declared.
    pub source: Option<String>,
    /// Container target path.
    pub target: String,
    /// Whether the mount is read-only.
    pub read_only: bool,
}

impl ServiceVolumeSummary {
    fn from_canonical(volume: &susun_model::volume::CanonicalVolume) -> Self {
        Self {
            kind: match volume.kind {
                VolumeKind::Volume => "volume",
                VolumeKind::Bind => "bind",
                VolumeKind::Anonymous => "anonymous",
            }
            .to_owned(),
            source: volume.source.clone(),
            target: volume.target.clone(),
            read_only: volume.read_only,
        }
    }
}

fn published_port_summary(port: PublishedPort) -> String {
    match port {
        PublishedPort::Single(port) => port.to_string(),
        PublishedPort::Range { start, end } => format!("{start}-{end}"),
    }
}

/// Derives the standard project identity from a project and directory.
pub fn project_identity(project: &Project, project_directory: impl AsRef<Path>) -> ProjectIdentity {
    ProjectIdentity::new(
        project.name.clone(),
        ProjectInstanceId::derive(&project.name, project_directory),
    )
}

/// Derives a project identity from a name and directory.
pub fn project_identity_from_name(
    name: ProjectName,
    project_directory: impl AsRef<Path>,
) -> ProjectIdentity {
    ProjectIdentity::new(
        name.clone(),
        ProjectInstanceId::derive(&name, project_directory),
    )
}
