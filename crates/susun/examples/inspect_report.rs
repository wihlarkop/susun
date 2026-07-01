//! Create an execution report from a daemon-free plan.

use std::process::ExitCode;

use susun::SusunWorkspace;
use susun_runtime::ExecutionReport;

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    let project = match SusunWorkspace::from_file(&path).analyze() {
        Ok(project) if !project.analysis().report.has_errors() => project,
        Ok(_) => return ExitCode::from(1),
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    match project.dry_run_up(false) {
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
