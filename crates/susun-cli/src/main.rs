//! Susun CLI binary.

use std::{
    collections::BTreeSet,
    fs,
    io::{self, Cursor},
    path::{Path, PathBuf},
    process,
    sync::Arc,
    time::{Duration, SystemTime},
};

use clap::Parser;
use futures_util::StreamExt;
use indexmap::IndexMap;
use susun::{
    Analyzer, EngineConnectionDisplayName, EngineConnectionProfile, EngineConnectionProfileId,
    Planner, RuntimeDoctorReport, RuntimeDoctorStatus, SusunWorkspace, down_with_engine,
    render_diagnostics, render_diagnostics_json, render_project_summary_json, up_with_engine,
};
use susun_build::{
    BuildCancellationToken, BuildEngine, BuildEventSink, BuildInputManifest, BuildRequest,
    BuildSecret, BuildSshForward, BuildxProcessBuildEngine, CacheEntry, Dockerignore,
    InsecureEntitlements, resolve_build_inputs, validate_dockerfile_source,
};
use susun_compat::{
    CompatibilityHarness, ComposeReference, CorpusManifest, OracleCommand, matrix_for_current_phase,
};
use susun_engine::{
    ContainerEngine, ContainerRef, CopyFromContainerRequest, CopyToContainerRequest,
    CreateContainerRequest, EngineCapabilities, EngineEndpoint, EngineEvent, EngineSnapshot,
    EventsRequest, LabelKey, LabelValue, LogsRequest, PortRequest, ProjectIdentity,
    ProjectInstanceId, RemoveContainerOptions, ReplicaIndex, ResourceName, ServiceInstanceId,
    StopContainerRequest, WaitContainerRequest,
};
use susun_engine_bollard::BollardEngine;
use susun_model::Command as EngineCommand;
use susun_planner::{
    BuildPolicy, DownPlanOptions, ExecutionPlan, UpPlanOptions, render_plan_human, render_plan_json,
};
use susun_runtime::ExecutionReport;
use susun_watch::{WatchEvent, WatchEventKind, WatchOptions, WatchSession};

mod args;
use args::{Cli, Command, ContextArgs, OutputFormat, PlanCommand, WatchAction};

#[tokio::main]
async fn main() {
    init_tracing();
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check => check(&cli.ctx),
        Command::Config => config(&cli.ctx),
        Command::Summary => summary(&cli.ctx),
        Command::Doctor => runtime_doctor(&cli.ctx).await,
        Command::Plan { command } => plan(&cli.ctx, command),
        Command::InspectPlan { path } => inspect_plan(&cli.ctx, &path),
        Command::Up {
            build,
            detach: _,
            scale,
            remove_orphans,
            force_recreate,
            no_recreate,
            renew_anon_volumes,
        } => {
            runtime_up(
                &cli.ctx,
                build,
                scale,
                remove_orphans,
                force_recreate,
                no_recreate,
                renew_anon_volumes,
            )
            .await
        }
        Command::Build => build_images(&cli.ctx).await,
        Command::Compatibility {
            corpus,
            security_audit,
        } => compatibility(corpus.as_deref(), security_audit.as_deref()),
        Command::Run {
            no_rm,
            service,
            command,
        } => runtime_run(&cli.ctx, !no_rm, service, command).await,
        Command::Exec {
            tty,
            stdin,
            service,
            command,
        } => runtime_exec(&cli.ctx, tty, stdin, service, command).await,
        Command::Events { service } => runtime_events(&cli.ctx, service).await,
        Command::Wait { service } => runtime_wait(&cli.ctx, service).await,
        Command::Cp { source, target } => runtime_cp(&cli.ctx, source, target).await,
        Command::Port {
            service,
            private_port,
        } => runtime_port(&cli.ctx, service, private_port).await,
        Command::Watch {
            action,
            service,
            sync,
            watch,
            debounce_ms,
        } => runtime_watch(&cli.ctx, action, service, sync, watch, debounce_ms).await,
        Command::Down {
            remove_volumes,
            remove_orphans: _,
        } => runtime_down(&cli.ctx, remove_volumes).await,
        Command::Ps => runtime_ps(&cli.ctx).await,
        Command::Logs {
            follow,
            timestamps,
            tail,
            service,
        } => runtime_logs(&cli.ctx, follow, timestamps, tail, service).await,
        Command::Start { service } => runtime_start(&cli.ctx, service).await,
        Command::Stop { service } => runtime_stop(&cli.ctx, service).await,
        Command::Restart { service } => runtime_restart(&cli.ctx, service).await,
    };
    process::exit(code);
}

fn init_tracing() {
    let filter = match std::env::var("SUSUN_LOG").or_else(|_| std::env::var("RUST_LOG")) {
        Ok(filter) => filter,
        Err(_) => return,
    };
    let env_filter = match tracing_subscriber::EnvFilter::try_new(filter) {
        Ok(filter) => filter,
        Err(error) => {
            eprintln!("susun: invalid tracing filter: {error}");
            return;
        }
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn compatibility(corpus: Option<&Path>, security_audit: Option<&Path>) -> i32 {
    if let Some(path) = corpus {
        return compatibility_corpus(path);
    }
    if let Some(path) = security_audit {
        return compatibility_security_audit(path);
    }

    let matrix = matrix_for_current_phase(env!("CARGO_PKG_VERSION"), "Docker Compose documented");
    match serde_json::to_string_pretty(&matrix) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(error) => {
            eprintln!("susun: failed to serialize capability matrix: {error}");
            2
        }
    }
}

fn compatibility_security_audit(path: &Path) -> i32 {
    let Some(manifest) = read_corpus_manifest(path) else {
        return 2;
    };
    let report = susun_compat::audit_corpus_security(&manifest);
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            println!("{json}");
            if report.has_errors() { 1 } else { 0 }
        }
        Err(error) => {
            eprintln!("susun: failed to serialize compatibility security audit: {error}");
            2
        }
    }
}

fn compatibility_corpus(path: &Path) -> i32 {
    let Some(manifest) = read_corpus_manifest(path) else {
        return 2;
    };
    let config = manifest.to_oracle_config(
        ComposeReference {
            name: "docker compose".to_owned(),
            version: "documented".to_owned(),
            engine_api_version: None,
        },
        OracleCommand::docker_compose(),
    );
    let harness = match CompatibilityHarness::new(config) {
        Ok(harness) => harness,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    match serde_json::to_string_pretty(&harness.run_plan()) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(error) => {
            eprintln!("susun: failed to serialize compatibility run plan: {error}");
            2
        }
    }
}

fn read_corpus_manifest(path: &Path) -> Option<CorpusManifest> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("susun: failed to read compatibility corpus: {error}");
            return None;
        }
    };
    match CorpusManifest::from_json_str(&content) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            eprintln!("susun: {error}");
            None
        }
    }
}

fn build_analyzer(ctx: &ContextArgs) -> Analyzer {
    workspace_from_context(ctx).analyzer()
}

fn workspace_from_context(ctx: &ContextArgs) -> SusunWorkspace {
    let files = if ctx.file.is_empty() {
        vec![std::path::PathBuf::from("compose.yaml")]
    } else {
        ctx.file.clone()
    };
    let mut workspace = SusunWorkspace::new().with_files(files);
    if let Some(env_file) = &ctx.env_file {
        workspace = workspace.with_env_file(env_file);
    }
    if let Some(name) = &ctx.project_name {
        workspace = workspace.with_project_name(name);
    }
    if !ctx.profile.is_empty() {
        workspace = workspace.with_profiles(ctx.profile.clone());
    }
    workspace
}

fn primary_file(ctx: &ContextArgs) -> std::path::PathBuf {
    ctx.file
        .first()
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from("compose.yaml"))
}

fn check(ctx: &ContextArgs) -> i32 {
    match build_analyzer(ctx).analyze() {
        Err(e) => {
            eprintln!("susun: {e}");
            2
        }
        Ok(result) => {
            if !ctx.quiet && (result.report.has_errors() || !result.report.is_empty()) {
                match ctx.format {
                    OutputFormat::Human => {
                        eprint!("{}", render_diagnostics(&result.report, &result.source_map));
                    }
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            render_diagnostics_json(&result.report, &result.source_map)
                        );
                    }
                }
            }
            if result.report.has_errors() { 1 } else { 0 }
        }
    }
}

fn config(ctx: &ContextArgs) -> i32 {
    match build_analyzer(ctx).analyze() {
        Err(e) => {
            eprintln!("susun: {e}");
            2
        }
        Ok(result) => {
            if result.report.has_errors() {
                if !ctx.quiet {
                    match ctx.format {
                        OutputFormat::Human => {
                            eprint!("{}", render_diagnostics(&result.report, &result.source_map));
                        }
                        OutputFormat::Json => {
                            eprintln!(
                                "{}",
                                render_diagnostics_json(&result.report, &result.source_map)
                            );
                        }
                    }
                }
                1
            } else {
                match result.project {
                    None => {
                        eprintln!("susun: no project to output");
                        1
                    }
                    Some(project) => match serde_json::to_string_pretty(&project) {
                        Err(e) => {
                            eprintln!("susun: failed to serialize project: {e}");
                            2
                        }
                        Ok(json) => {
                            println!("{json}");
                            0
                        }
                    },
                }
            }
        }
    }
}

fn summary(ctx: &ContextArgs) -> i32 {
    match workspace_from_context(ctx).analyze() {
        Err(e) => {
            eprintln!("susun: {e}");
            2
        }
        Ok(project) => {
            let summary = project.summary();
            if summary.has_errors {
                if !ctx.quiet {
                    render_analysis_diagnostics(ctx, project.analysis());
                }
                return 1;
            }

            match ctx.format {
                OutputFormat::Json => match render_project_summary_json(&summary) {
                    Ok(json) => {
                        println!("{json}");
                        0
                    }
                    Err(e) => {
                        eprintln!("susun: failed to serialize project summary: {e}");
                        2
                    }
                },
                OutputFormat::Human => {
                    println!(
                        "{}: {} service(s), {} active",
                        summary.project_name.as_deref().unwrap_or("<unknown>"),
                        summary.service_count,
                        summary.active_service_count
                    );
                    for service in summary.services {
                        let marker = if service.active { "*" } else { "-" };
                        let image = service.image.as_deref().unwrap_or("<no image>");
                        println!("{marker} {} ({image})", service.name);
                    }
                    0
                }
            }
        }
    }
}

fn plan(ctx: &ContextArgs, command: PlanCommand) -> i32 {
    match build_analyzer(ctx).analyze() {
        Err(e) => {
            eprintln!("susun: {e}");
            2
        }
        Ok(result) => {
            if result.report.has_errors() {
                if !ctx.quiet {
                    render_analysis_diagnostics(ctx, &result);
                }
                return 1;
            }

            let Some(project) = result.project.as_ref() else {
                eprintln!("susun: no project to plan");
                return 1;
            };
            let identity = ProjectIdentity::new(
                project.name.clone(),
                ProjectInstanceId::derive(&project.name, project_directory(ctx)),
            );
            let planner = Planner::new(
                identity,
                EngineCapabilities::permissive_local(),
                EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
            );

            let outcome = match command {
                PlanCommand::Up { build } => {
                    let options = UpPlanOptions {
                        build_policy: if build {
                            BuildPolicy::BuildDeclared
                        } else {
                            BuildPolicy::NeverBuild
                        },
                        ..UpPlanOptions::default()
                    };
                    planner.plan_up(&result, options)
                }
                PlanCommand::Down { remove_volumes } => {
                    let options = DownPlanOptions {
                        remove_volumes,
                        ..DownPlanOptions::default()
                    };
                    planner.plan_down(&result, options)
                }
            };

            match outcome {
                Err(e) => {
                    eprintln!("susun: {e}");
                    2
                }
                Ok(outcome) => {
                    if outcome.diagnostics.has_errors() {
                        if !ctx.quiet {
                            render_plan_diagnostics(ctx, &outcome.diagnostics, &result.source_map);
                        }
                        return 1;
                    }

                    let Some(plan) = outcome.plan else {
                        eprintln!("susun: planner did not produce a plan");
                        return 1;
                    };

                    emit_plan(ctx, &plan)
                }
            }
        }
    }
}

fn inspect_plan(ctx: &ContextArgs, path: &Path) -> i32 {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("susun: failed to read plan: {e}");
            return 2;
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("susun: failed to parse plan JSON: {e}");
            return 2;
        }
    };
    let major = value
        .get("schema_version")
        .and_then(|schema| schema.get("major"))
        .and_then(serde_json::Value::as_u64);
    if major != Some(1) {
        eprintln!("susun: unsupported plan schema major {:?}", major);
        return 1;
    }

    let plan: ExecutionPlan = match serde_json::from_value(value) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("susun: invalid plan JSON: {e}");
            return 2;
        }
    };

    emit_plan(ctx, &plan)
}

async fn runtime_up(
    ctx: &ContextArgs,
    build: bool,
    scale: Vec<String>,
    remove_orphans: bool,
    force_recreate: bool,
    no_recreate: bool,
    renew_anon_volumes: bool,
) -> i32 {
    if force_recreate && no_recreate {
        eprintln!("susun: --force-recreate conflicts with --no-recreate");
        return 2;
    }
    let _convergence_options = (scale, remove_orphans, renew_anon_volumes);
    let Some((analysis, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    if build {
        let build_code = build_images_from_analysis(ctx, &analysis).await;
        if build_code != 0 {
            return build_code;
        }
    }
    let engine = match connect_engine() {
        Ok(engine) => Arc::new(engine),
        Err(code) => return code,
    };
    match up_with_engine(&analysis, identity, engine, UpPlanOptions::default()).await {
        Ok(result) => emit_execution_report(ctx, &result.report),
        Err(error) => {
            eprintln!("susun: {error}");
            2
        }
    }
}

async fn build_images(ctx: &ContextArgs) -> i32 {
    let Some((analysis, _)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    build_images_from_analysis(ctx, &analysis).await
}

async fn build_images_from_analysis(ctx: &ContextArgs, analysis: &susun::AnalysisResult) -> i32 {
    let Some(project) = analysis.project.as_ref() else {
        eprintln!("susun: no project to build");
        return 1;
    };
    let Some(selection) = analysis.selection.as_ref() else {
        eprintln!("susun: no selected services to build");
        return 1;
    };

    let build_engine = BuildxProcessBuildEngine::default();
    let project_dir = project_directory(ctx);
    let mut built = 0_usize;

    for service_name in &selection.active_services {
        let Some(service) = project.services.get(service_name) else {
            continue;
        };
        let Some(build) = &service.build else {
            continue;
        };

        let paths = match resolve_build_inputs(&project_dir, build) {
            Ok(paths) => paths,
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        };
        if let Err(error) = validate_dockerfile_source(&paths.dockerfile, build.target.as_deref()) {
            eprintln!("susun: {error}");
            return 2;
        }
        let dockerignore = read_dockerignore(&paths.context_dir);
        let manifest = match BuildInputManifest::from_context(&paths.context_dir, &dockerignore) {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        };

        let request = BuildRequest {
            definition: build.clone(),
            context_dir: paths.context_dir,
            dockerfile: paths.dockerfile,
            manifest,
            image_tag: service
                .image
                .as_ref()
                .map(|image| image.as_str().to_owned()),
            secrets: build
                .secrets
                .iter()
                .map(|id| BuildSecret {
                    id: id.clone(),
                    source: None,
                })
                .collect(),
            ssh: build
                .ssh
                .iter()
                .map(|id| BuildSshForward { id: id.clone() })
                .collect(),
            cache_from: build
                .cache_from
                .iter()
                .map(|spec| CacheEntry { spec: spec.clone() })
                .collect(),
            cache_to: build
                .cache_to
                .iter()
                .map(|spec| CacheEntry { spec: spec.clone() })
                .collect(),
            insecure_entitlements: InsecureEntitlements::default(),
            labels: Default::default(),
        };

        match build_engine
            .build(
                request,
                BuildEventSink::discard(),
                BuildCancellationToken::new(),
            )
            .await
        {
            Ok(result) => {
                built += 1;
                if !ctx.quiet && ctx.format == OutputFormat::Human {
                    println!(
                        "built {} as {}",
                        service_name.as_str(),
                        result.image.reference
                    );
                }
            }
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        }
    }

    if built == 0 {
        if !ctx.quiet {
            println!("no build definitions found");
        }
    } else if ctx.format == OutputFormat::Json {
        println!("{}", serde_json::json!({ "built": built }));
    }
    0
}

fn read_dockerignore(context_dir: &Path) -> Dockerignore {
    let path = context_dir.join(".dockerignore");
    match std::fs::read_to_string(path) {
        Ok(contents) => Dockerignore::parse(&contents),
        Err(_) => Dockerignore::default(),
    }
}

async fn runtime_down(ctx: &ContextArgs, remove_volumes: bool) -> i32 {
    let Some((analysis, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => Arc::new(engine),
        Err(code) => return code,
    };
    let options = DownPlanOptions {
        remove_volumes,
        ..DownPlanOptions::default()
    };
    match down_with_engine(&analysis, identity, engine, options).await {
        Ok(result) => emit_execution_report(ctx, &result.report),
        Err(error) => {
            eprintln!("susun: {error}");
            2
        }
    }
}

async fn runtime_run(ctx: &ContextArgs, rm: bool, service: String, command: Vec<String>) -> i32 {
    let Some((analysis, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let Some(project) = analysis.project.as_ref() else {
        eprintln!("susun: no project to run");
        return 1;
    };
    let service_name = susun_model::ServiceName::new(service.clone());
    let Some(service_model) = project.services.get(&service_name) else {
        eprintln!("susun: service '{service}' was not found");
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };

    let instance = ServiceInstanceId::new(
        identity.working_set.clone(),
        service_name.clone(),
        ReplicaIndex::new(one_off_replica_index()),
    );
    let name = match ResourceName::new(format!(
        "susun-{}-{}-run-{}",
        identity.working_set.as_str(),
        service_name.as_str(),
        one_off_suffix()
    )) {
        Ok(name) => name,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let container = match engine
        .create_container(CreateContainerRequest {
            project: identity.clone(),
            service: instance,
            name,
            image: service_model.image.clone(),
            command: (!command.is_empty()).then_some(EngineCommand::Exec(command)),
            entrypoint: service_model.entrypoint.clone(),
            environment: service_model.environment.clone(),
            container_labels: service_model.labels.clone(),
            ports: Vec::new(),
            volumes: service_model.volumes.clone(),
            configs: Vec::new(),
            secrets: Vec::new(),
            networks: IndexMap::new(),
            healthcheck: None,
            restart: Some("no".to_owned()),
            labels: one_off_labels(&identity, service_name.as_str()),
        })
        .await
    {
        Ok(container) => container,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };

    if let Err(error) = engine.start_container(&container).await {
        eprintln!("susun: {error}");
        let _ = cleanup_one_off(&engine, &container, rm).await;
        return 2;
    }

    let logs_result = stream_container_logs(&engine, &container, true, false, None, None).await;
    let wait = engine
        .wait_container(WaitContainerRequest {
            container: container.clone(),
        })
        .await;
    let cleanup = cleanup_one_off(&engine, &container, rm).await;

    if let Err(error) = logs_result {
        eprintln!("susun: {error}");
        return 2;
    }
    let exit_code = match wait {
        Ok(result) => result.exit_code,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    if let Err(error) = cleanup {
        eprintln!("susun: {error}");
        return 2;
    }
    i32::try_from(exit_code).unwrap_or(1)
}

async fn runtime_exec(
    ctx: &ContextArgs,
    tty: bool,
    stdin: bool,
    service: String,
    command: Vec<String>,
) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let Some(container) = selected_service_container(&snapshot, &service) else {
        eprintln!("susun: running service container '{service}' was not found");
        return 1;
    };
    let mut output = match engine
        .exec(susun_engine::ExecRequest {
            container,
            command,
            tty,
            stdin,
            user: None,
            working_dir: None,
        })
        .await
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    while let Some(event) = output.next().await {
        match event {
            Ok(event) => print!("{}", event.line),
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        }
    }
    0
}

async fn runtime_events(ctx: &ContextArgs, service: Vec<String>) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let mut events = match engine
        .events(EventsRequest {
            project: identity.clone(),
        })
        .await
    {
        Ok(events) => events,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let selected = service.into_iter().collect::<BTreeSet<_>>();
    while let Some(event) = events.next().await {
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        };
        if !selected.is_empty()
            && !event_service(&event).is_some_and(|name| selected.contains(name))
        {
            continue;
        }
        emit_event(ctx, &event);
    }
    0
}

async fn runtime_wait(ctx: &ContextArgs, service: Vec<String>) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let selected = service.into_iter().collect::<BTreeSet<_>>();
    let containers = matching_service_containers(&snapshot, &selected);
    if containers.is_empty() {
        eprintln!("susun: no matching project service containers were found");
        return 1;
    }

    let mut exit_code = 0;
    for (service, container) in containers {
        match engine
            .wait_container(WaitContainerRequest {
                container: container.clone(),
            })
            .await
        {
            Ok(result) => {
                if !ctx.quiet && ctx.format == OutputFormat::Human {
                    println!("{service} exited with {}", result.exit_code);
                }
                if result.exit_code != 0 {
                    exit_code = 1;
                }
            }
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        }
    }
    exit_code
}

async fn runtime_cp(ctx: &ContextArgs, source: String, target: String) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let source = match parse_cp_location(&source) {
        Ok(location) => location,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let target = match parse_cp_location(&target) {
        Ok(location) => location,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };

    match (source, target) {
        (CopyLocation::Container { service, path }, CopyLocation::Host(target)) => {
            let Some(container) = selected_service_container(&snapshot, &service) else {
                eprintln!("susun: running service container '{service}' was not found");
                return 1;
            };
            let mut stream = match engine
                .copy_from_container(CopyFromContainerRequest { container, path })
                .await
            {
                Ok(stream) => stream,
                Err(error) => {
                    eprintln!("susun: {error}");
                    return 2;
                }
            };
            let mut archive = Vec::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => archive.extend(chunk),
                    Err(error) => {
                        eprintln!("susun: {error}");
                        return 2;
                    }
                }
            }
            match extract_container_archive(&archive, &target) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("susun: failed to extract archive: {error}");
                    2
                }
            }
        }
        (CopyLocation::Host(source), CopyLocation::Container { service, path }) => {
            let Some(container) = selected_service_container(&snapshot, &service) else {
                eprintln!("susun: running service container '{service}' was not found");
                return 1;
            };
            let archive = match build_host_archive(&source) {
                Ok(archive) => archive,
                Err(error) => {
                    eprintln!("susun: failed to build archive: {error}");
                    return 2;
                }
            };
            match engine
                .copy_to_container(CopyToContainerRequest {
                    container,
                    path,
                    archive,
                })
                .await
            {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("susun: {error}");
                    2
                }
            }
        }
        (CopyLocation::Host(_), CopyLocation::Host(_)) => {
            eprintln!("susun: cp requires exactly one SERVICE:PATH endpoint");
            2
        }
        (CopyLocation::Container { .. }, CopyLocation::Container { .. }) => {
            eprintln!("susun: container-to-container cp is not supported");
            2
        }
    }
}

async fn runtime_port(ctx: &ContextArgs, service: String, private_port: Option<String>) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let (private_port, protocol) = match private_port.as_deref().map(parse_private_port).transpose()
    {
        Ok(value) => value.unwrap_or((None, None)),
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let Some(container) = selected_service_container(&snapshot, &service) else {
        eprintln!("susun: running service container '{service}' was not found");
        return 1;
    };
    let bindings = match engine
        .port(PortRequest {
            container,
            private_port,
            protocol,
        })
        .await
    {
        Ok(bindings) => bindings,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    if bindings.is_empty() {
        eprintln!("susun: no published ports found for service '{service}'");
        return 1;
    }
    match ctx.format {
        OutputFormat::Json => match serde_json::to_string_pretty(&bindings) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("susun: failed to serialize ports: {error}");
                return 2;
            }
        },
        OutputFormat::Human => {
            for binding in bindings {
                let host_ip = binding.host_ip.unwrap_or_else(|| "0.0.0.0".to_owned());
                println!(
                    "{}/{} -> {}:{}",
                    binding.private_port, binding.protocol, host_ip, binding.host_port
                );
            }
        }
    }
    0
}

async fn runtime_watch(
    ctx: &ContextArgs,
    action: WatchAction,
    service: Vec<String>,
    sync: Vec<String>,
    watch: Vec<PathBuf>,
    debounce_ms: u64,
) -> i32 {
    let Some((analysis, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let project_dir = project_directory(ctx);
    let sync_specs = match parse_sync_specs(sync) {
        Ok(specs) => specs,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    if matches!(action, WatchAction::Sync | WatchAction::SyncRestart) && sync_specs.is_empty() {
        eprintln!(
            "susun: watch action requires at least one --sync SERVICE:HOST_PATH:CONTAINER_DIR mapping"
        );
        return 2;
    }

    let dockerignore = read_dockerignore(&project_dir);
    let options = WatchOptions::new(project_dir.clone())
        .with_paths(watch)
        .with_debounce(Duration::from_millis(debounce_ms))
        .with_ignore(dockerignore);
    let session = match WatchSession::start(options) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    if !ctx.quiet {
        eprintln!("susun: watching for file changes");
    }

    loop {
        let event = match session.recv() {
            Ok(event) => event,
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        };
        if !ctx.quiet && ctx.format == OutputFormat::Human {
            eprintln!("susun: {:?} {}", event.kind, event.relative_path.display());
        }
        if let Err(code) = apply_watch_action(
            ctx,
            &analysis,
            &identity,
            action,
            &service,
            &sync_specs,
            &event,
        )
        .await
        {
            return code;
        }
    }
}

async fn apply_watch_action(
    ctx: &ContextArgs,
    analysis: &susun::AnalysisResult,
    identity: &ProjectIdentity,
    action: WatchAction,
    services: &[String],
    sync_specs: &[SyncSpec],
    event: &WatchEvent,
) -> Result<(), i32> {
    match action {
        WatchAction::Rebuild => {
            let code = build_images_from_analysis(ctx, analysis).await;
            if code == 0 { Ok(()) } else { Err(code) }
        }
        WatchAction::Restart => restart_services(ctx, services.to_vec()).await,
        WatchAction::Sync => sync_watch_event(identity, sync_specs, event).await,
        WatchAction::SyncRestart => {
            sync_watch_event(identity, sync_specs, event).await?;
            restart_services(ctx, services.to_vec()).await
        }
    }
}

async fn restart_services(ctx: &ContextArgs, services: Vec<String>) -> Result<(), i32> {
    let code = runtime_lifecycle(ctx, services, LifecycleCommand::Restart).await;
    if code == 0 { Ok(()) } else { Err(code) }
}

async fn sync_watch_event(
    identity: &ProjectIdentity,
    sync_specs: &[SyncSpec],
    event: &WatchEvent,
) -> Result<(), i32> {
    if event.kind == WatchEventKind::Removed {
        eprintln!(
            "susun: remove event for {} not synced; destructive sync requires explicit support",
            event.relative_path.display()
        );
        return Ok(());
    }
    let matching_specs = sync_specs
        .iter()
        .filter_map(|spec| spec.target_for_event(event))
        .collect::<Vec<_>>();
    if matching_specs.is_empty() {
        return Ok(());
    }

    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return Err(code),
    };
    let snapshot = match engine.snapshot(identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return Err(2);
        }
    };
    for target in matching_specs {
        let Some(container) = selected_service_container(&snapshot, &target.service) else {
            eprintln!(
                "susun: running service container '{}' was not found",
                target.service
            );
            return Err(1);
        };
        let archive = match build_host_archive(&event.absolute_path) {
            Ok(archive) => archive,
            Err(error) => {
                eprintln!("susun: failed to build sync archive: {error}");
                return Err(2);
            }
        };
        if let Err(error) = engine
            .copy_to_container(CopyToContainerRequest {
                container,
                path: target.container_dir,
                archive,
            })
            .await
        {
            eprintln!("susun: {error}");
            return Err(2);
        }
    }
    Ok(())
}

fn event_service(event: &EngineEvent) -> Option<&str> {
    event.attributes.get("io.susun.service").map(String::as_str)
}

fn emit_event(ctx: &ContextArgs, event: &EngineEvent) {
    match ctx.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::json!({
                "kind": event.kind,
                "action": event.action,
                "resource_id": event.resource_id,
                "attributes": event.attributes,
                "time": event.time,
                "time_nano": event.time_nano,
            })
        ),
        OutputFormat::Human => {
            let service = event_service(event).unwrap_or("-");
            let resource = event.resource_id.as_deref().unwrap_or("-");
            println!(
                "{}\t{}\t{}\t{}",
                event.kind, event.action, service, resource
            );
        }
    }
}

fn matching_service_containers(
    snapshot: &EngineSnapshot,
    selected: &BTreeSet<String>,
) -> Vec<(String, ContainerRef)> {
    snapshot
        .containers
        .values()
        .filter_map(|container| {
            let service = container
                .service_identity
                .as_ref()
                .map(|identity| identity.service.as_str().to_owned())?;
            if !selected.is_empty() && !selected.contains(&service) {
                return None;
            }
            Some((
                service,
                ContainerRef {
                    id: container.id.clone(),
                },
            ))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SyncSpec {
    service: String,
    source_root: PathBuf,
    container_root: String,
}

#[derive(Debug, Clone)]
struct SyncTarget {
    service: String,
    container_dir: String,
}

impl SyncSpec {
    fn target_for_event(&self, event: &WatchEvent) -> Option<SyncTarget> {
        if !event.absolute_path.starts_with(&self.source_root) {
            return None;
        }
        let relative = event.absolute_path.strip_prefix(&self.source_root).ok()?;
        let upload_dir = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| container_join(&self.container_root, parent))
            .unwrap_or_else(|| self.container_root.clone());
        Some(SyncTarget {
            service: self.service.clone(),
            container_dir: upload_dir,
        })
    }
}

fn parse_sync_specs(values: Vec<String>) -> Result<Vec<SyncSpec>, String> {
    values
        .into_iter()
        .map(|value| parse_sync_spec(&value))
        .collect()
}

fn parse_sync_spec(value: &str) -> Result<SyncSpec, String> {
    let Some(service_separator) = value.find(':') else {
        return Err(format!(
            "invalid sync mapping '{value}'; expected SERVICE:HOST_PATH:CONTAINER_DIR"
        ));
    };
    let service = &value[..service_separator];
    if service.is_empty() {
        return Err(format!("invalid sync mapping '{value}'; service is empty"));
    }
    let remainder = &value[service_separator + 1..];
    let Some(target_separator) = container_target_separator(remainder) else {
        return Err(format!(
            "invalid sync mapping '{value}'; container directory must be an absolute path"
        ));
    };
    let host_path = &remainder[..target_separator];
    let container_root = &remainder[target_separator + 1..];
    validate_container_path(container_root)?;
    if !container_root.starts_with('/') {
        return Err(format!(
            "invalid sync mapping '{value}'; container directory must start with /"
        ));
    }
    let source_root = fs::canonicalize(host_path)
        .map_err(|error| format!("failed to resolve sync host path '{host_path}': {error}"))?;
    Ok(SyncSpec {
        service: service.to_owned(),
        source_root,
        container_root: container_root.to_owned(),
    })
}

fn container_target_separator(value: &str) -> Option<usize> {
    value
        .char_indices()
        .filter_map(|(index, ch)| {
            (ch == ':'
                && value
                    .get(index + 1..)
                    .is_some_and(|tail| tail.starts_with('/') || tail.starts_with('\\')))
            .then_some(index)
        })
        .next_back()
}

fn container_join(root: &str, relative_parent: &Path) -> String {
    let suffix = relative_parent
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if suffix.is_empty() {
        root.to_owned()
    } else {
        format!("{}/{}", root.trim_end_matches('/'), suffix)
    }
}

#[derive(Debug)]
enum CopyLocation {
    Host(PathBuf),
    Container { service: String, path: String },
}

fn parse_cp_location(value: &str) -> Result<CopyLocation, String> {
    let Some(index) = container_location_separator(value) else {
        return Ok(CopyLocation::Host(PathBuf::from(value)));
    };
    let (service, path) = value.split_at(index);
    let path = &path[1..];
    if service.is_empty() {
        return Err("container copy source is missing a service name".to_owned());
    }
    validate_container_path(path)?;
    Ok(CopyLocation::Container {
        service: service.to_owned(),
        path: path.to_owned(),
    })
}

fn container_location_separator(value: &str) -> Option<usize> {
    let index = value.find(':')?;
    if index == 1
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
    {
        return None;
    }
    let first_separator = value.find(['/', '\\']).unwrap_or(usize::MAX);
    (index < first_separator).then_some(index)
}

fn validate_container_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("container copy path cannot be empty".to_owned());
    }
    if path.contains('\0') {
        return Err("container copy path cannot contain NUL bytes".to_owned());
    }
    Ok(())
}

fn parse_private_port(value: &str) -> Result<(Option<u16>, Option<String>), String> {
    let (port, protocol) = value
        .split_once('/')
        .map(|(port, protocol)| (port, Some(protocol)))
        .unwrap_or((value, None));
    let port = port
        .parse::<u16>()
        .map_err(|_| format!("invalid private port '{value}'"))?;
    let protocol = protocol.map(|protocol| protocol.to_ascii_lowercase());
    if protocol
        .as_deref()
        .is_some_and(|protocol| !matches!(protocol, "tcp" | "udp" | "sctp"))
    {
        return Err(format!("invalid private port protocol in '{value}'"));
    }
    Ok((Some(port), protocol))
}

fn build_host_archive(source: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::metadata(source)?;
    let mut archive = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut archive);
        let name = source.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "source has no file name")
        })?;
        if metadata.is_dir() {
            builder.append_dir_all(name, source)?;
        } else {
            builder.append_path_with_name(source, name)?;
        }
        builder.finish()?;
    }
    Ok(archive)
}

fn extract_container_archive(archive: &[u8], target: &Path) -> io::Result<()> {
    if target.exists() && target.is_file()
        || !target.exists() && archive_contains_single_file(archive)?
    {
        extract_single_file(tar::Archive::new(Cursor::new(archive)), target)
    } else {
        fs::create_dir_all(target)?;
        let mut archive = tar::Archive::new(Cursor::new(archive));
        archive.unpack(target)
    }
}

fn archive_contains_single_file(archive: &[u8]) -> io::Result<bool> {
    let mut archive = tar::Archive::new(Cursor::new(archive));
    let mut entries = archive.entries()?;
    let Some(entry) = entries.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "container archive is empty",
        ));
    };
    let entry = entry?;
    Ok(entry.header().entry_type().is_file() && entries.next().is_none())
}

fn extract_single_file<R: io::Read>(mut archive: tar::Archive<R>, target: &Path) -> io::Result<()> {
    let mut entries = archive.entries()?;
    let Some(entry) = entries.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "container archive is empty",
        ));
    };
    if entries.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot copy multiple archive entries to a file target",
        ));
    }
    let mut entry = entry?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    entry.unpack(target).map(|_| ())
}

fn selected_service_container(snapshot: &EngineSnapshot, service: &str) -> Option<ContainerRef> {
    snapshot
        .containers
        .values()
        .find(|container| {
            container
                .service_identity
                .as_ref()
                .is_some_and(|identity| identity.service.as_str() == service)
                && container.state == susun_engine::ContainerState::Running
        })
        .map(|container| ContainerRef {
            id: container.id.clone(),
        })
}

async fn stream_container_logs(
    engine: &impl ContainerEngine,
    container: &ContainerRef,
    follow: bool,
    timestamps: bool,
    tail: Option<usize>,
    prefix: Option<&str>,
) -> Result<(), susun_engine::EngineError> {
    let mut logs = engine
        .logs(LogsRequest {
            container: container.clone(),
            follow,
            timestamps,
            tail,
        })
        .await?;
    while let Some(event) = logs.next().await {
        let event = event?;
        if let Some(prefix) = prefix {
            print!("{prefix} | ");
        }
        print!("{}", event.line);
    }
    Ok(())
}

async fn cleanup_one_off(
    engine: &impl ContainerEngine,
    container: &ContainerRef,
    rm: bool,
) -> Result<(), susun_engine::EngineError> {
    if !rm {
        return Ok(());
    }
    engine
        .remove_container(
            container,
            RemoveContainerOptions {
                remove_anonymous_volumes: true,
                force: false,
            },
        )
        .await
}

fn one_off_labels(identity: &ProjectIdentity, service: &str) -> IndexMap<LabelKey, LabelValue> {
    let mut labels = IndexMap::new();
    insert_label(&mut labels, "io.susun.project", identity.name.as_str());
    insert_label(
        &mut labels,
        "io.susun.project-instance",
        identity.working_set.as_str(),
    );
    insert_label(&mut labels, "io.susun.service", service);
    insert_label(&mut labels, "io.susun.oneoff", "true");
    insert_label(&mut labels, "io.susun.model-version", "1");
    labels
}

fn insert_label(labels: &mut IndexMap<LabelKey, LabelValue>, key: &str, value: &str) {
    if let (Ok(key), Ok(value)) = (LabelKey::new(key), LabelValue::new(value)) {
        labels.insert(key, value);
    }
}

fn one_off_suffix() -> String {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{millis:x}")
}

fn one_off_replica_index() -> u32 {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    u32::try_from(millis % u128::from(u32::MAX)).unwrap_or_default()
}

async fn runtime_ps(ctx: &ContextArgs) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    match ctx.format {
        OutputFormat::Json => match serde_json::to_string_pretty(&snapshot.stable_projection()) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("susun: failed to serialize status: {error}");
                return 2;
            }
        },
        OutputFormat::Human => {
            println!("NAME\tSERVICE\tSTATE\tIMAGE");
            for container in snapshot.containers.values() {
                let service = container
                    .service_identity
                    .as_ref()
                    .map(|identity| identity.service.as_str())
                    .unwrap_or("-");
                println!(
                    "{}\t{}\t{:?}\t{:?}",
                    container.name.as_str(),
                    service,
                    container.state,
                    container.image
                );
            }
        }
    }
    0
}

async fn runtime_logs(
    ctx: &ContextArgs,
    follow: bool,
    timestamps: bool,
    tail: Option<usize>,
    service: Vec<String>,
) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let selected = service.into_iter().collect::<BTreeSet<_>>();
    for container in snapshot.containers.values() {
        let service = container
            .service_identity
            .as_ref()
            .map(|identity| identity.service.as_str().to_owned());
        if !selected.is_empty() && !service.as_ref().is_some_and(|name| selected.contains(name)) {
            continue;
        }
        let mut logs = match engine
            .logs(LogsRequest {
                container: ContainerRef {
                    id: container.id.clone(),
                },
                follow,
                timestamps,
                tail,
            })
            .await
        {
            Ok(logs) => logs,
            Err(error) => {
                eprintln!("susun: {error}");
                return 2;
            }
        };
        while let Some(event) = logs.next().await {
            match event {
                Ok(event) => {
                    if let Some(service) = &service {
                        print!("{service} | ");
                    }
                    print!("{}", event.line);
                }
                Err(error) => {
                    eprintln!("susun: {error}");
                    return 2;
                }
            }
        }
    }
    0
}

async fn runtime_start(ctx: &ContextArgs, service: Vec<String>) -> i32 {
    runtime_lifecycle(ctx, service, LifecycleCommand::Start).await
}

async fn runtime_stop(ctx: &ContextArgs, service: Vec<String>) -> i32 {
    runtime_lifecycle(ctx, service, LifecycleCommand::Stop).await
}

async fn runtime_restart(ctx: &ContextArgs, service: Vec<String>) -> i32 {
    runtime_lifecycle(ctx, service, LifecycleCommand::Restart).await
}

async fn runtime_lifecycle(
    ctx: &ContextArgs,
    service: Vec<String>,
    command: LifecycleCommand,
) -> i32 {
    let Some((_, identity)) = analyze_for_runtime(ctx) else {
        return 1;
    };
    let engine = match connect_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let snapshot = match engine.snapshot(&identity).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let selected = service.into_iter().collect::<BTreeSet<_>>();
    for container in snapshot.containers.values() {
        let service_name = container
            .service_identity
            .as_ref()
            .map(|identity| identity.service.as_str().to_owned());
        if !selected.is_empty()
            && !service_name
                .as_ref()
                .is_some_and(|name| selected.contains(name))
        {
            continue;
        }
        let container_ref = ContainerRef {
            id: container.id.clone(),
        };
        let result = match command {
            LifecycleCommand::Start => engine.start_container(&container_ref).await,
            LifecycleCommand::Stop => {
                engine
                    .stop_container(StopContainerRequest {
                        container: container_ref,
                        timeout: std::time::Duration::from_secs(10),
                    })
                    .await
            }
            LifecycleCommand::Restart => {
                if let Err(error) = engine
                    .stop_container(StopContainerRequest {
                        container: container_ref.clone(),
                        timeout: std::time::Duration::from_secs(10),
                    })
                    .await
                {
                    return lifecycle_error(error);
                }
                engine.start_container(&container_ref).await
            }
        };
        if let Err(error) = result {
            return lifecycle_error(error);
        }
    }
    0
}

fn lifecycle_error(error: susun_engine::EngineError) -> i32 {
    eprintln!("susun: {error}");
    2
}

#[derive(Debug, Clone, Copy)]
enum LifecycleCommand {
    Start,
    Stop,
    Restart,
}

fn analyze_for_runtime(ctx: &ContextArgs) -> Option<(susun::AnalysisResult, ProjectIdentity)> {
    let result = match build_analyzer(ctx).analyze() {
        Ok(result) => result,
        Err(error) => {
            eprintln!("susun: {error}");
            return None;
        }
    };
    if result.report.has_errors() {
        if !ctx.quiet {
            render_analysis_diagnostics(ctx, &result);
        }
        return None;
    }
    let Some(project) = result.project.as_ref() else {
        eprintln!("susun: no project to run");
        return None;
    };
    let identity = ProjectIdentity::new(
        project.name.clone(),
        ProjectInstanceId::derive(&project.name, project_directory(ctx)),
    );
    Some((result, identity))
}

fn connect_engine() -> Result<BollardEngine, i32> {
    BollardEngine::connect_local().map_err(|error| {
        eprintln!("susun: {error}");
        2
    })
}

async fn runtime_doctor(ctx: &ContextArgs) -> i32 {
    let profile = match local_runtime_profile() {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!("susun: {error}");
            return 2;
        }
    };
    let report = BollardEngine::doctor_profile(&profile).await;
    emit_runtime_doctor_report(ctx, &profile, &report)
}

fn local_runtime_profile() -> Result<EngineConnectionProfile, susun::EngineConnectionProfileError> {
    Ok(EngineConnectionProfile::new(
        EngineConnectionProfileId::new("local")?,
        EngineConnectionDisplayName::new("Local Docker-compatible runtime")?,
        EngineEndpoint::Local,
    ))
}

fn emit_runtime_doctor_report(
    ctx: &ContextArgs,
    profile: &EngineConnectionProfile,
    report: &RuntimeDoctorReport,
) -> i32 {
    match ctx.format {
        OutputFormat::Json => match serde_json::to_string_pretty(report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("susun: failed to serialize runtime doctor report: {error}");
                return 2;
            }
        },
        OutputFormat::Human => {
            println!(
                "{} [{}]: {}",
                profile.display_name.as_str(),
                report.endpoint,
                runtime_doctor_status_name(report.status)
            );
            println!("{}", report.message);
            if let Some(probe) = &report.probe {
                if let Some(version) = &probe.engine_version {
                    println!("engine version: {}", version.as_str());
                }
                if let Some(api_version) = &probe.api_version {
                    println!("api version: {}", api_version.as_str());
                }
            }
        }
    }
    if report.status == RuntimeDoctorStatus::Available {
        0
    } else {
        2
    }
}

fn runtime_doctor_status_name(status: RuntimeDoctorStatus) -> &'static str {
    match status {
        RuntimeDoctorStatus::Available => "available",
        RuntimeDoctorStatus::Unavailable => "unavailable",
        RuntimeDoctorStatus::AuthenticationFailed => "authentication_failed",
        RuntimeDoctorStatus::Unsupported => "unsupported",
        RuntimeDoctorStatus::Misconfigured => "misconfigured",
    }
}

fn emit_execution_report(ctx: &ContextArgs, report: &ExecutionReport) -> i32 {
    match ctx.format {
        OutputFormat::Json => match serde_json::to_string_pretty(report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("susun: failed to serialize execution report: {error}");
                return 2;
            }
        },
        OutputFormat::Human => {
            println!(
                "executed {} action(s): {} succeeded, {} failed, {} skipped, {} cancelled",
                report.summary.total_actions,
                report.summary.succeeded,
                report.summary.failed,
                report.summary.skipped,
                report.summary.cancelled
            );
        }
    }
    if report.summary.failed == 0 && report.summary.cancelled == 0 {
        0
    } else {
        1
    }
}

fn emit_plan(ctx: &ContextArgs, plan: &ExecutionPlan) -> i32 {
    match ctx.format {
        OutputFormat::Human => {
            print!("{}", render_plan_human(plan));
            0
        }
        OutputFormat::Json => match render_plan_json(plan) {
            Ok(json) => {
                println!("{json}");
                0
            }
            Err(e) => {
                eprintln!("susun: failed to serialize plan: {e}");
                2
            }
        },
    }
}

fn project_directory(ctx: &ContextArgs) -> std::path::PathBuf {
    primary_file(ctx)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn render_analysis_diagnostics(ctx: &ContextArgs, result: &susun::AnalysisResult) {
    match ctx.format {
        OutputFormat::Human => {
            eprint!("{}", render_diagnostics(&result.report, &result.source_map));
        }
        OutputFormat::Json => {
            eprintln!(
                "{}",
                render_diagnostics_json(&result.report, &result.source_map)
            );
        }
    }
}

fn render_plan_diagnostics(
    ctx: &ContextArgs,
    diagnostics: &susun_diagnostics::DiagnosticReport,
    source_map: &susun_source::SourceMap,
) {
    match ctx.format {
        OutputFormat::Human => {
            eprint!("{}", render_diagnostics(diagnostics, source_map));
        }
        OutputFormat::Json => {
            eprintln!("{}", render_diagnostics_json(diagnostics, source_map));
        }
    }
}
