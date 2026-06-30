//! Plan convergence from desired Compose state and an empty observed snapshot.

use std::{process::ExitCode, time::SystemTime};

use susun::{Analyzer, render_diagnostics};
use susun_convergence::{
    ConvergenceInput, ConvergencePolicy, DesiredDeployment, DesiredInstanceFingerprints,
    ObservedDeployment, plan_convergence, render_convergence_human,
};
use susun_engine::{EngineCapabilities, EngineSnapshot, ProjectIdentity, ProjectInstanceId};
use susun_model::ProjectName;

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

    let Some(project) = analysis.project else {
        return ExitCode::from(1);
    };
    let Some(selection) = analysis.selection else {
        return ExitCode::from(1);
    };
    let Some(graph) = analysis.graph else {
        return ExitCode::from(1);
    };

    let project_name = ProjectName::new("convergence-example");
    let identity = ProjectIdentity::new(
        project_name.clone(),
        ProjectInstanceId::derive(&project_name, std::env::current_dir().unwrap_or_default()),
    );
    let desired_services = selection.active_services.clone();
    let desired = DesiredDeployment::new(project, selection, graph, identity, Default::default());
    let observed = ObservedDeployment::new(
        EngineSnapshot::empty(SystemTime::UNIX_EPOCH),
        &desired_services,
    );
    let desired_fingerprints = DesiredInstanceFingerprints::default();
    let policy = ConvergencePolicy::default();
    let capabilities = EngineCapabilities::permissive_local();

    match plan_convergence(ConvergenceInput {
        desired: &desired,
        observed: &observed,
        capabilities: &capabilities,
        policy: &policy,
        desired_fingerprints: &desired_fingerprints,
    }) {
        Ok(outcome) => {
            print!("{}", render_convergence_human(&outcome));
            if let Some(plan) = outcome.plan {
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
