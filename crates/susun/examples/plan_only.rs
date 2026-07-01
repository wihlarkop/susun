//! Plan an `up` operation without mutating a container engine.

use std::process::ExitCode;

use susun::{SusunWorkspace, render_diagnostics};

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    let project = match SusunWorkspace::from_file(&path).analyze() {
        Ok(project) => project,
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };
    let analysis = project.analysis();

    if analysis.report.has_errors() {
        eprint!(
            "{}",
            render_diagnostics(&analysis.report, &analysis.source_map)
        );
        return ExitCode::from(1);
    }

    match project.dry_run_up(false) {
        Ok(outcome) => {
            for diagnostic in outcome.diagnostics.sorted() {
                eprintln!(
                    "{}[{}]: {}",
                    diagnostic.severity, diagnostic.code, diagnostic.message
                );
            }
            if let Some(plan) = outcome.plan {
                println!("plan {}", plan.plan_id);
                println!("operation {:?}", plan.operation);
                println!("actions {}", plan.actions.len());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
