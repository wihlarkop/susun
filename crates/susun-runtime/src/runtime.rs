//! Runtime plan application.

use std::{
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, SystemTime},
};

use indexmap::{IndexMap, IndexSet};
use susun_engine::{
    ContainerEngine, ContainerRef, CreateContainerRequest, CreateNetworkRequest,
    CreateVolumeRequest, EngineError, EngineSnapshot, LabelKey, LabelValue, NetworkRef,
    ProgressSink, PullImageRequest, PullPolicy, RemoveContainerOptions, StopContainerRequest,
    VolumeRef,
};
use susun_planner::{
    ActionId, ExecutionPlan, PlanAction, PlanActionNode, topological_action_order,
};

use crate::{
    ActionExecutionResult, ActionOutput, ActionStatus, EventSink, ExecutionReport, RuntimeError,
    RuntimeEvent, validate_plan_for_execution,
};

/// Runtime options.
#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    /// Maximum actions to execute at once.
    pub max_concurrency: NonZeroUsize,
    /// Cancellation grace period.
    pub cancellation_grace_period: Duration,
    /// Default stop timeout.
    pub default_stop_timeout: Duration,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            max_concurrency: NonZeroUsize::MIN,
            cancellation_grace_period: Duration::from_secs(10),
            default_stop_timeout: Duration::from_secs(10),
        }
    }
}

/// Immutable plan runtime.
#[derive(Debug)]
pub struct Runtime<E> {
    engine: Arc<E>,
    options: RuntimeOptions,
    events: EventSink,
}

impl<E> Runtime<E>
where
    E: ContainerEngine + 'static,
{
    /// Creates a runtime.
    pub fn new(engine: Arc<E>) -> Self {
        Self {
            engine,
            options: RuntimeOptions::default(),
            events: EventSink::discard(),
        }
    }

    /// Replaces runtime options.
    pub fn with_options(mut self, options: RuntimeOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets an event sink.
    pub fn with_events(mut self, events: EventSink) -> Self {
        self.events = events;
        self
    }

    /// Applies an immutable execution plan and returns a complete report.
    pub async fn apply(&self, plan: &ExecutionPlan) -> Result<ExecutionReport, RuntimeError> {
        validate_plan_for_execution(plan)?;
        self.engine
            .capabilities()
            .await
            .map_err(RuntimeError::Capabilities)?;
        let snapshot = self
            .engine
            .snapshot(&plan.project)
            .await
            .map_err(RuntimeError::Capabilities)?;

        let order = topological_action_order(&plan.actions).map_err(|error| {
            RuntimeError::InvalidPlan(crate::PlanValidationError::InvalidDag(error))
        })?;
        let mut report = ExecutionReport::pending(plan);
        let mut failed = IndexSet::new();
        let mut outputs = RuntimeOutputs::from_snapshot(&snapshot);

        self.events
            .emit(RuntimeEvent::PlanStarted {
                plan_id: plan.plan_id.clone(),
            })
            .await;

        for action_id in order {
            let Some(node) = plan.actions.get(&action_id) else {
                return Err(RuntimeError::InternalInvariant {
                    detail: format!("missing action {action_id} after DAG validation"),
                });
            };

            if has_failed_dependency(node, &failed) {
                mark_skipped(&mut report, &action_id);
                failed.insert(action_id.clone());
                self.events
                    .emit(RuntimeEvent::ActionFinished {
                        action_id,
                        status: ActionStatus::SkippedDependencyFailed,
                    })
                    .await;
                continue;
            }

            self.events
                .emit(RuntimeEvent::ActionQueued {
                    action_id: action_id.clone(),
                })
                .await;
            self.events
                .emit(RuntimeEvent::ActionStarted {
                    action_id: action_id.clone(),
                })
                .await;

            let result = self
                .execute_action(plan, &action_id, node, &mut outputs)
                .await;
            set_result(&mut report, action_id.clone(), result.clone());
            self.events
                .emit(RuntimeEvent::ActionFinished {
                    action_id: action_id.clone(),
                    status: result.status,
                })
                .await;

            if result.status != ActionStatus::Succeeded {
                failed.insert(action_id);
            }
        }

        report.refresh_summary();
        self.events
            .emit(RuntimeEvent::PlanFinished {
                plan_id: plan.plan_id.clone(),
                summary: report.summary.clone(),
            })
            .await;
        Ok(report)
    }

    async fn execute_action(
        &self,
        plan: &ExecutionPlan,
        action_id: &ActionId,
        node: &PlanActionNode,
        outputs: &mut RuntimeOutputs,
    ) -> ActionExecutionResult {
        let started_at = SystemTime::now();
        let execution = self
            .execute_action_inner(plan, action_id, node, outputs)
            .await;
        let finished_at = SystemTime::now();

        match execution {
            Ok(output) => ActionExecutionResult {
                action_id: action_id.clone(),
                status: ActionStatus::Succeeded,
                started_at: Some(started_at),
                finished_at: Some(finished_at),
                attempts: 1,
                output: Some(output),
                error: None,
            },
            Err(error) => ActionExecutionResult {
                action_id: action_id.clone(),
                status: ActionStatus::Failed,
                started_at: Some(started_at),
                finished_at: Some(finished_at),
                attempts: 1,
                output: None,
                error: Some(error.to_string()),
            },
        }
    }

    async fn execute_action_inner(
        &self,
        plan: &ExecutionPlan,
        action_id: &ActionId,
        node: &PlanActionNode,
        outputs: &mut RuntimeOutputs,
    ) -> Result<ActionOutput, EngineError> {
        match &node.action {
            PlanAction::PullImage(action) => {
                let progress_action = action_id.clone();
                let events = self.events.clone();
                let progress = ProgressSink::new(move |progress| {
                    let events = events.clone();
                    let progress_action = progress_action.clone();
                    Box::pin(async move {
                        events
                            .emit(RuntimeEvent::ActionProgress {
                                action_id: progress_action,
                                progress,
                            })
                            .await;
                    })
                });
                let image = self
                    .engine
                    .pull_image(
                        PullImageRequest {
                            image: action.image.clone(),
                            policy: PullPolicy::Missing,
                        },
                        progress,
                    )
                    .await?;
                Ok(ActionOutput::Image(image))
            }
            PlanAction::CreateNetwork(action) => {
                let network = self
                    .engine
                    .create_network(CreateNetworkRequest {
                        project: plan.project.clone(),
                        name: action.name.clone(),
                        labels: ownership_labels(
                            plan,
                            ResourceLabel::Network(action.identity.network.as_str()),
                            None,
                        ),
                    })
                    .await?;
                outputs
                    .networks
                    .insert(action.identity.network.as_str().to_owned(), network.clone());
                Ok(ActionOutput::Network(network))
            }
            PlanAction::CreateVolume(action) => {
                let volume = self
                    .engine
                    .create_volume(CreateVolumeRequest {
                        project: plan.project.clone(),
                        name: action.name.clone(),
                        labels: ownership_labels(
                            plan,
                            ResourceLabel::Volume(action.identity.volume.as_str()),
                            None,
                        ),
                    })
                    .await?;
                outputs
                    .volumes
                    .insert(action.identity.volume.as_str().to_owned(), volume.clone());
                Ok(ActionOutput::Volume(volume))
            }
            PlanAction::CreateContainer(action) => {
                let container = self
                    .engine
                    .create_container(CreateContainerRequest {
                        project: plan.project.clone(),
                        service: action.identity.clone(),
                        name: action.name.clone(),
                        image: action.image.clone(),
                        labels: ownership_labels(
                            plan,
                            ResourceLabel::Service(action.identity.service.as_str()),
                            Some(action.identity.replica.ordinal()),
                        ),
                    })
                    .await?;
                outputs.containers.insert(
                    action.identity.service.as_str().to_owned(),
                    container.clone(),
                );
                Ok(ActionOutput::Container(container))
            }
            PlanAction::StartContainer(action) => {
                let container = outputs.container_for(action.identity.service.as_ref())?;
                self.engine.start_container(&container).await?;
                Ok(ActionOutput::None)
            }
            PlanAction::WaitForDependency(_) | PlanAction::NoOp(_) => Ok(ActionOutput::None),
            PlanAction::StopContainer(action) => {
                let container = outputs.container_for(action.identity.service.as_ref())?;
                self.engine
                    .stop_container(StopContainerRequest {
                        container,
                        timeout: self.options.default_stop_timeout,
                    })
                    .await?;
                Ok(ActionOutput::None)
            }
            PlanAction::RemoveContainer(action) => {
                let container = outputs.container_for(action.identity.service.as_ref())?;
                self.engine
                    .remove_container(
                        &container,
                        RemoveContainerOptions {
                            remove_anonymous_volumes: false,
                            force: false,
                        },
                    )
                    .await?;
                Ok(ActionOutput::None)
            }
            PlanAction::RemoveNetwork(action) => {
                let network = outputs.network_for(action.identity.network.as_str())?;
                self.engine.remove_network(network).await?;
                Ok(ActionOutput::None)
            }
            PlanAction::RemoveVolume(action) => {
                let volume = outputs.volume_for(action.identity.volume.as_str())?;
                self.engine.remove_volume(volume).await?;
                Ok(ActionOutput::None)
            }
        }
    }
}

fn has_failed_dependency(node: &PlanActionNode, failed: &IndexSet<ActionId>) -> bool {
    node.dependencies
        .iter()
        .any(|dependency| failed.contains(dependency))
}

fn mark_skipped(report: &mut ExecutionReport, action_id: &ActionId) {
    if let Some(result) = report.actions.get_mut(action_id) {
        result.status = ActionStatus::SkippedDependencyFailed;
        result.finished_at = Some(SystemTime::now());
    }
}

fn set_result(report: &mut ExecutionReport, action_id: ActionId, result: ActionExecutionResult) {
    report.actions.insert(action_id, result);
}

fn ownership_labels(
    plan: &ExecutionPlan,
    resource: ResourceLabel<'_>,
    replica: Option<u32>,
) -> IndexMap<LabelKey, LabelValue> {
    let pairs = [
        ("io.susun.project", plan.project.name.as_str().to_owned()),
        (
            "io.susun.project-instance",
            plan.project.working_set.as_str().to_owned(),
        ),
        ("io.susun.managed", "true".to_owned()),
        ("io.susun.model-version", "1".to_owned()),
    ];
    let mut labels = IndexMap::new();
    for (key, value) in pairs {
        insert_label(&mut labels, key, value);
    }
    match resource {
        ResourceLabel::Service(service) => {
            insert_label(&mut labels, "io.susun.service", service.to_owned());
        }
        ResourceLabel::Network(network) => {
            insert_label(&mut labels, "io.susun.network", network.to_owned());
        }
        ResourceLabel::Volume(volume) => {
            insert_label(&mut labels, "io.susun.volume", volume.to_owned());
        }
    }
    if let Some(replica) = replica {
        insert_label(&mut labels, "io.susun.replica", replica.to_string());
    }
    labels
}

enum ResourceLabel<'a> {
    Service(&'a str),
    Network(&'a str),
    Volume(&'a str),
}

fn insert_label(labels: &mut IndexMap<LabelKey, LabelValue>, key: &str, value: String) {
    let Ok(key) = LabelKey::new(key) else {
        return;
    };
    let Ok(value) = LabelValue::new(value) else {
        return;
    };
    labels.insert(key, value);
}

#[derive(Debug, Default)]
struct RuntimeOutputs {
    containers: IndexMap<String, ContainerRef>,
    networks: IndexMap<String, NetworkRef>,
    volumes: IndexMap<String, VolumeRef>,
}

impl RuntimeOutputs {
    fn from_snapshot(snapshot: &EngineSnapshot) -> Self {
        let mut outputs = Self::default();
        for container in snapshot.containers.values() {
            if let Some(identity) = &container.service_identity {
                outputs.containers.insert(
                    identity.service.as_str().to_owned(),
                    ContainerRef {
                        id: container.id.clone(),
                    },
                );
            }
        }
        for network in snapshot.networks.values() {
            if let Some(identity) = &network.network_identity {
                outputs.networks.insert(
                    identity.network.as_str().to_owned(),
                    NetworkRef {
                        id: network.id.clone(),
                    },
                );
            }
        }
        for volume in snapshot.volumes.values() {
            if let Some(identity) = &volume.volume_identity {
                outputs.volumes.insert(
                    identity.volume.as_str().to_owned(),
                    VolumeRef {
                        id: volume.id.clone(),
                    },
                );
            }
        }
        outputs
    }

    fn container_for(&self, service: &str) -> Result<ContainerRef, EngineError> {
        self.containers
            .get(service)
            .cloned()
            .ok_or_else(|| EngineError::Unsupported {
                capability: "container lookup from existing snapshot is not wired yet",
            })
    }

    fn network_for(&self, network: &str) -> Result<NetworkRef, EngineError> {
        self.networks
            .get(network)
            .cloned()
            .ok_or_else(|| EngineError::Unsupported {
                capability: "network lookup from existing snapshot is not wired yet",
            })
    }

    fn volume_for(&self, volume: &str) -> Result<VolumeRef, EngineError> {
        self.volumes
            .get(volume)
            .cloned()
            .ok_or_else(|| EngineError::Unsupported {
                capability: "volume lookup from existing snapshot is not wired yet",
            })
    }
}
