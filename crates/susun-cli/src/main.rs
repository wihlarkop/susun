//! Susun CLI binary.

use std::{collections::BTreeSet, path::Path, process, sync::Arc, time::SystemTime};

use clap::Parser;
use futures_util::StreamExt;
use susun::{
    Analyzer, LoadContext, Planner, down_with_engine, render_diagnostics, render_diagnostics_json,
    up_with_engine,
};
use susun_build::{
    BuildCancellationToken, BuildEngine, BuildEventSink, BuildInputManifest, BuildRequest,
    BuildSecret, BuildSshForward, BuildxProcessBuildEngine, CacheEntry, Dockerignore,
    InsecureEntitlements, resolve_build_inputs, validate_dockerfile_source,
};
use susun_engine::{
    ContainerEngine, ContainerRef, EngineCapabilities, EngineSnapshot, LogsRequest,
    ProjectIdentity, ProjectInstanceId, StopContainerRequest,
};
use susun_engine_bollard::BollardEngine;
use susun_planner::{
    BuildPolicy, DownPlanOptions, ExecutionPlan, UpPlanOptions, render_plan_human, render_plan_json,
};
use susun_runtime::ExecutionReport;

mod args;
use args::{Cli, Command, ContextArgs, OutputFormat, PlanCommand};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check => check(&cli.ctx),
        Command::Config => config(&cli.ctx),
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

fn build_analyzer(ctx: &ContextArgs) -> Analyzer {
    // Primary file: first -f argument, or "compose.yaml" if none given.
    let (primary, rest) = match ctx.file.as_slice() {
        [] => (std::path::PathBuf::from("compose.yaml"), &[][..]),
        [first, tail @ ..] => (first.clone(), tail),
    };

    let mut context = LoadContext::new(primary);
    if !rest.is_empty() {
        context = context.with_additional_files(rest.to_vec());
    }
    if let Some(name) = &ctx.project_name {
        context = context.with_project_name(name);
    }
    if !ctx.profile.is_empty() {
        context = context.with_profiles(ctx.profile.clone());
    }
    let mut analyzer = Analyzer::with_context(context);
    if let Some(env_file) = &ctx.env_file {
        analyzer = analyzer.with_env_file(env_file);
    }
    analyzer
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
