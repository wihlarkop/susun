//! Run `up` then `down` against the local Docker Engine.

use std::{process::ExitCode, sync::Arc};

use susun::{DownPlanOptions, SusunWorkspace, UpPlanOptions, render_diagnostics};
use susun_engine_bollard::BollardEngine;

#[tokio::main]
async fn main() -> ExitCode {
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

    let engine = match BollardEngine::connect_local() {
        Ok(engine) => Arc::new(engine),
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    match project
        .up_with_engine(engine.clone(), UpPlanOptions::default())
        .await
    {
        Ok(result) => println!("up applied {} actions", result.report.summary.total_actions),
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    }

    match project
        .down_with_engine(engine, DownPlanOptions::default())
        .await
    {
        Ok(result) => {
            println!(
                "down applied {} actions",
                result.report.summary.total_actions
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
