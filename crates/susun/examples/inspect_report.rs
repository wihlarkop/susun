//! Create an execution report from a daemon-free plan.

use std::{process::ExitCode, time::SystemTime};

use susun::{Analyzer, Planner};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity, ProjectInstanceId};
use susun_model::ProjectName;
use susun_planner::UpPlanOptions;
use susun_runtime::ExecutionReport;

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    let analysis = match Analyzer::new(&path).analyze() {
        Ok(analysis) if !analysis.report.has_errors() => analysis,
        Ok(_) => return ExitCode::from(1),
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    let project = ProjectName::new("report-example");
    let identity = ProjectIdentity::new(
        project.clone(),
        ProjectInstanceId::derive(&project, std::env::current_dir().unwrap_or_default()),
    );
    let planner = Planner::new(
        identity,
        EngineCapabilities::permissive_local(),
        EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
    );

    match planner.plan_up(&analysis, UpPlanOptions::default()) {
        Ok(outcome) => match outcome.plan {
            Some(plan) => {
                let report = ExecutionReport::pending(&plan);
                println!("plan {}", report.plan_id);
                println!("total actions {}", report.summary.total_actions);
                println!("pending actions {}", report.actions.len());
                ExitCode::SUCCESS
            }
            None => ExitCode::from(1),
        },
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
