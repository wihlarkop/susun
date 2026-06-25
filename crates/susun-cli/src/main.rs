//! Susun CLI binary.

use std::process;

use clap::Parser;
use susun::{Analyzer, LoadContext, render_diagnostics, render_diagnostics_json};

mod args;
use args::{Cli, Command, ContextArgs, OutputFormat};

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check => check(&cli.ctx),
        Command::Config => config(&cli.ctx),
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
