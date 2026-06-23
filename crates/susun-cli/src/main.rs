//! Susun CLI binary.

use std::{path::Path, process};

use clap::Parser;
use susun::{render_diagnostics, Analyzer};

mod args;
use args::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check { file } => check(&file),
        Command::Config { file } => config(&file),
    };
    process::exit(code);
}

fn check(file: &Path) -> i32 {
    match Analyzer::new(file).analyze() {
        Err(e) => {
            eprintln!("susun: {e}");
            2
        }
        Ok(result) => {
            if result.report.has_errors() {
                eprint!("{}", render_diagnostics(&result.report, &result.source_map));
                1
            } else {
                0
            }
        }
    }
}

fn config(file: &Path) -> i32 {
    match Analyzer::new(file).analyze() {
        Err(e) => {
            eprintln!("susun: {e}");
            2
        }
        Ok(result) => {
            if result.report.has_errors() {
                eprint!("{}", render_diagnostics(&result.report, &result.source_map));
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
