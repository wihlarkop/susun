//! Print the SDK project summary JSON used by desktop integrations.

use std::process::ExitCode;

use susun::{SusunWorkspace, render_diagnostics};

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    let project = match SusunWorkspace::from_file(path).analyze() {
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

    match serde_json::to_string_pretty(&project.summary()) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("susun: failed to serialize project summary: {error}");
            ExitCode::from(2)
        }
    }
}
