//! Plan an `up` operation without mutating a container engine.

use std::{process::ExitCode, time::SystemTime};

use susun::{Analyzer, Planner, render_diagnostics};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity, ProjectInstanceId};
use susun_model::ProjectName;
use susun_planner::UpPlanOptions;

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    let analysis = match Analyzer::new(&path).analyze() {
        Ok(analysis) => analysis,
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    if analysis.report.has_errors() {
        eprint!(
            "{}",
            render_diagnostics(&analysis.report, &analysis.source_map)
        );
        return ExitCode::from(1);
    }

    let identity = ProjectIdentity::new(
        ProjectName::new("example"),
        ProjectInstanceId::derive(&ProjectName::new("example"), "."),
    );
    let planner = Planner::new(
        identity,
        EngineCapabilities::permissive_local(),
        EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
    );

    match planner.plan_up(&analysis, UpPlanOptions::default()) {
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
