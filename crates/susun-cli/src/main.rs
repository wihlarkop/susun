//! Susun CLI binary.

use std::{path::Path, process, time::SystemTime};

use clap::Parser;
use susun::{Analyzer, LoadContext, Planner, render_diagnostics, render_diagnostics_json};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity, ProjectInstanceId};
use susun_planner::{
    DownPlanOptions, ExecutionPlan, UpPlanOptions, render_plan_human, render_plan_json,
};

mod args;
use args::{Cli, Command, ContextArgs, OutputFormat, PlanCommand};

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check => check(&cli.ctx),
        Command::Config => config(&cli.ctx),
        Command::Plan { command } => plan(&cli.ctx, command),
        Command::InspectPlan { path } => inspect_plan(&cli.ctx, &path),
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
                PlanCommand::Up => planner.plan_up(&result, UpPlanOptions::default()),
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
