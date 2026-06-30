//! Run `up` then `down` against the local Docker Engine.

use std::{process::ExitCode, sync::Arc};

use susun::{Analyzer, down_with_engine, up_with_engine};
use susun_engine::{ProjectIdentity, ProjectInstanceId};
use susun_engine_bollard::BollardEngine;
use susun_model::ProjectName;
use susun_planner::{DownPlanOptions, UpPlanOptions};

#[tokio::main]
async fn main() -> ExitCode {
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

    let engine = match BollardEngine::connect_local() {
        Ok(engine) => Arc::new(engine),
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    let project = ProjectName::new("local-docker-example");
    let identity = ProjectIdentity::new(
        project.clone(),
        ProjectInstanceId::derive(&project, std::env::current_dir().unwrap_or_default()),
    );

    match up_with_engine(
        &analysis,
        identity.clone(),
        engine.clone(),
        UpPlanOptions::default(),
    )
    .await
    {
        Ok(result) => println!("up applied {} actions", result.report.summary.total_actions),
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    }

    match down_with_engine(&analysis, identity, engine, DownPlanOptions::default()).await {
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
