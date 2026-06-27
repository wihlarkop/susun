//! Runtime plan application.

use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, SystemTime},
};

use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use indexmap::{IndexMap, IndexSet};
use susun_engine::{
    ContainerEngine, ContainerRef, ContainerState, CreateContainerRequest, CreateNetworkRequest,
    CreateVolumeRequest, EngineError, EngineSnapshot, HealthState, LabelKey, LabelValue,
    NetworkRef, ProgressSink, PullImageRequest, PullPolicy, RemoveContainerOptions,
    StopContainerRequest, VolumeRef,
};
use susun_planner::{
    ActionId, ExecutionPlan, PlanAction, PlanActionNode, topological_action_order,
};

use crate::{
    ActionExecutionResult, ActionOutput, ActionStatus, CancellationToken, EventSink,
    ExecutionReport, RetryPolicy, RuntimeError, RuntimeEvent, validate_plan_for_execution,
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
    /// Conservative retry policy.
    pub retry_policy: RetryPolicy,
    /// Maximum time to wait for dependency conditions.
    pub dependency_wait_timeout: Duration,
    /// Interval between dependency condition checks.
    pub dependency_poll_interval: Duration,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            max_concurrency: NonZeroUsize::MIN,
            cancellation_grace_period: Duration::from_secs(10),
            default_stop_timeout: Duration::from_secs(10),
            retry_policy: RetryPolicy::default(),
            dependency_wait_timeout: Duration::from_secs(60),
            dependency_poll_interval: Duration::from_millis(500),
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
        self.apply_cancellable(plan, CancellationToken::new()).await
    }

    /// Applies a plan with cooperative cancellation.
    pub async fn apply_cancellable(
        &self,
        plan: &ExecutionPlan,
        cancellation: CancellationToken,
    ) -> Result<ExecutionReport, RuntimeError> {
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
        let mut graph = RuntimeGraph::new(plan);
        let mut ready = VecDeque::from(order);
        ready.retain(|id| graph.remaining_dependencies.get(id).copied().unwrap_or(0) == 0);
        let mut running = FuturesUnordered::new();
        let mut completed = IndexSet::new();

        self.events
            .emit(RuntimeEvent::PlanStarted {
                plan_id: plan.plan_id.clone(),
            })
            .await;

        loop {
            while !cancellation.is_cancelled() && running.len() < self.options.max_concurrency.get()
            {
                let Some(action_id) = ready.pop_front() else {
                    break;
                };
                if completed.contains(&action_id) {
                    continue;
                }
                let Some(node) = plan.actions.get(&action_id) else {
                    return Err(RuntimeError::InternalInvariant {
                        detail: format!("missing action {action_id} after DAG validation"),
                    });
                };

                if has_failed_dependency(node, &failed) {
                    mark_skipped(&mut report, &action_id);
                    failed.insert(action_id.clone());
                    completed.insert(action_id.clone());
                    self.events
                        .emit(RuntimeEvent::ActionFinished {
                            action_id: action_id.clone(),
                            status: ActionStatus::SkippedDependencyFailed,
                        })
                        .await;
                    release_dependents(&mut graph, &action_id, &completed, &mut ready);
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

                running.push(self.spawn_action(plan, action_id, node, outputs.clone()));
            }

            if running.is_empty() {
                break;
            }

            let Some((action_id, action, result)) = running.next().await else {
                break;
            };
            update_outputs(&action, &result, &mut outputs);
            set_result(&mut report, action_id.clone(), result.clone());
            self.events
                .emit(RuntimeEvent::ActionFinished {
                    action_id: action_id.clone(),
                    status: result.status,
                })
                .await;

            if result.status != ActionStatus::Succeeded {
                failed.insert(action_id.clone());
            }
            completed.insert(action_id.clone());
            release_dependents(&mut graph, &action_id, &completed, &mut ready);
        }

        mark_remaining_cancelled(&mut report, &completed);

        report.refresh_summary();
        self.events
            .emit(RuntimeEvent::PlanFinished {
                plan_id: plan.plan_id.clone(),
                summary: report.summary.clone(),
            })
            .await;
        Ok(report)
    }

    fn spawn_action<'a>(
        &'a self,
        plan: &'a ExecutionPlan,
        action_id: ActionId,
        node: &PlanActionNode,
        outputs: RuntimeOutputs,
    ) -> BoxFuture<'a, (ActionId, PlanAction, ActionExecutionResult)> {
        let action = node.action.clone();
        async move {
            let result = self
                .execute_action(plan, &action_id, &action, &outputs)
                .await;
            (action_id, action, result)
        }
        .boxed()
    }

    async fn execute_action(
        &self,
        plan: &ExecutionPlan,
        action_id: &ActionId,
        action: &PlanAction,
        outputs: &RuntimeOutputs,
    ) -> ActionExecutionResult {
        let started_at = SystemTime::now();
        let mut attempts = 0;
        let execution = loop {
            attempts += 1;
            let result = self
                .execute_action_inner(plan, action_id, action, outputs)
                .await;
            match result {
                Ok(output) => break Ok(output),
                Err(error)
                    if self
                        .options
                        .retry_policy
                        .should_retry(action, &error, attempts) =>
                {
                    continue;
                }
                Err(error) => break Err(error),
            }
        };
        let finished_at = SystemTime::now();

        match execution {
            Ok(output) => ActionExecutionResult {
                action_id: action_id.clone(),
                status: ActionStatus::Succeeded,
                started_at: Some(started_at),
                finished_at: Some(finished_at),
                attempts,
                output: Some(output),
                error: None,
            },
            Err(error) => ActionExecutionResult {
                action_id: action_id.clone(),
                status: ActionStatus::Failed,
                started_at: Some(started_at),
                finished_at: Some(finished_at),
                attempts,
                output: None,
                error: Some(error.to_string()),
            },
        }
    }

    async fn execute_action_inner(
        &self,
        plan: &ExecutionPlan,
        action_id: &ActionId,
        action: &PlanAction,
        outputs: &RuntimeOutputs,
    ) -> Result<ActionOutput, EngineError> {
        match action {
            PlanAction::VerifyBuildInputs(_) | PlanAction::BuildImage(_) => Ok(ActionOutput::None),
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
                        command: action.command.clone(),
                        entrypoint: action.entrypoint.clone(),
                        environment: action.environment.clone(),
                        container_labels: action.labels.clone(),
                        ports: action.ports.clone(),
                        volumes: action.volumes.clone(),
                        configs: action.configs.clone(),
                        secrets: action.secrets.clone(),
                        networks: action.networks.clone(),
                        healthcheck: action.healthcheck.clone(),
                        restart: action.restart.clone(),
                        labels: ownership_labels(
                            plan,
                            ResourceLabel::Service(action.identity.service.as_str()),
                            Some(action.identity.replica.ordinal()),
                        ),
                    })
                    .await?;
                Ok(ActionOutput::Container(container))
            }
            PlanAction::StartContainer(action) => {
                let container = outputs.container_for(action.identity.service.as_ref())?;
                self.engine.start_container(&container).await?;
                Ok(ActionOutput::None)
            }
            PlanAction::WaitForDependency(action) => {
                self.wait_for_dependency(plan, action).await?;
                Ok(ActionOutput::None)
            }
            PlanAction::NoOp(_) => Ok(ActionOutput::None),
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
            PlanAction::RenameContainer(_) => Err(EngineError::Unsupported {
                capability: "container rename during convergence replacement",
            }),
            PlanAction::RecreateContainer(_)
            | PlanAction::PreserveVolume(_)
            | PlanAction::VerifyReplacement(_)
            | PlanAction::RemoveOrphan(_)
            | PlanAction::ScaleUpReplica(_)
            | PlanAction::ScaleDownReplica(_) => Ok(ActionOutput::None),
        }
    }

    async fn wait_for_dependency(
        &self,
        plan: &ExecutionPlan,
        action: &susun_planner::WaitForDependencyAction,
    ) -> Result<(), EngineError> {
        let started = std::time::Instant::now();
        loop {
            let snapshot = self.engine.snapshot(&plan.project).await?;
            if dependency_condition_met(&snapshot, action) {
                return Ok(());
            }
            if started.elapsed() >= self.options.dependency_wait_timeout {
                return Err(EngineError::Unsupported {
                    capability: "dependency condition was not satisfied before timeout",
                });
            }
            tokio::time::sleep(self.options.dependency_poll_interval).await;
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

fn mark_remaining_cancelled(report: &mut ExecutionReport, completed: &IndexSet<ActionId>) {
    for (action_id, result) in &mut report.actions {
        if completed.contains(action_id) {
            continue;
        }
        if matches!(result.status, ActionStatus::Pending | ActionStatus::Ready) {
            result.status = ActionStatus::Cancelled;
            result.finished_at = Some(SystemTime::now());
        }
    }
}

fn release_dependents(
    graph: &mut RuntimeGraph,
    action_id: &ActionId,
    completed: &IndexSet<ActionId>,
    ready: &mut VecDeque<ActionId>,
) {
    let Some(dependents) = graph.dependents.get(action_id) else {
        return;
    };

    for dependent in dependents {
        let Some(remaining) = graph.remaining_dependencies.get_mut(dependent) else {
            continue;
        };
        *remaining = remaining.saturating_sub(1);
        if *remaining == 0 && !completed.contains(dependent) {
            ready.push_back(dependent.clone());
        }
    }
}

fn set_result(report: &mut ExecutionReport, action_id: ActionId, result: ActionExecutionResult) {
    report.actions.insert(action_id, result);
}

fn update_outputs(
    action: &PlanAction,
    result: &ActionExecutionResult,
    outputs: &mut RuntimeOutputs,
) {
    if result.status != ActionStatus::Succeeded {
        return;
    }

    match (action, result.output.as_ref()) {
        (PlanAction::CreateContainer(action), Some(ActionOutput::Container(container))) => {
            outputs.containers.insert(
                action.identity.service.as_str().to_owned(),
                container.clone(),
            );
        }
        (PlanAction::CreateNetwork(action), Some(ActionOutput::Network(network))) => {
            outputs
                .networks
                .insert(action.identity.network.as_str().to_owned(), network.clone());
        }
        (PlanAction::CreateVolume(action), Some(ActionOutput::Volume(volume))) => {
            outputs
                .volumes
                .insert(action.identity.volume.as_str().to_owned(), volume.clone());
        }
        _ => {}
    }
}

fn dependency_condition_met(
    snapshot: &EngineSnapshot,
    action: &susun_planner::WaitForDependencyAction,
) -> bool {
    snapshot.containers.values().any(|container| {
        if container.service_identity.as_ref() != Some(&action.dependency) {
            return false;
        }
        match action.condition.as_str() {
            "ServiceStarted" | "service_started" => container.state == ContainerState::Running,
            "ServiceHealthy" | "service_healthy" => container.health == Some(HealthState::Healthy),
            "ServiceCompletedSuccessfully" | "service_completed_successfully" => {
                container.state == ContainerState::Exited
            }
            _ => false,
        }
    })
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

#[derive(Debug, Clone, Default)]
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
                capability: "container reference is unavailable for this action",
            })
    }

    fn network_for(&self, network: &str) -> Result<NetworkRef, EngineError> {
        self.networks
            .get(network)
            .cloned()
            .ok_or_else(|| EngineError::Unsupported {
                capability: "network reference is unavailable for this action",
            })
    }

    fn volume_for(&self, volume: &str) -> Result<VolumeRef, EngineError> {
        self.volumes
            .get(volume)
            .cloned()
            .ok_or_else(|| EngineError::Unsupported {
                capability: "volume reference is unavailable for this action",
            })
    }
}

#[derive(Debug)]
struct RuntimeGraph {
    dependents: IndexMap<ActionId, IndexSet<ActionId>>,
    remaining_dependencies: IndexMap<ActionId, usize>,
}

impl RuntimeGraph {
    fn new(plan: &ExecutionPlan) -> Self {
        let mut dependents = IndexMap::new();
        let mut remaining_dependencies = IndexMap::new();

        for (id, node) in &plan.actions {
            dependents.entry(id.clone()).or_insert_with(IndexSet::new);
            remaining_dependencies.insert(id.clone(), node.dependencies.len());
            for dependency in &node.dependencies {
                dependents
                    .entry(dependency.clone())
                    .or_insert_with(IndexSet::new)
                    .insert(id.clone());
            }
        }

        Self {
            dependents,
            remaining_dependencies,
        }
    }
}
