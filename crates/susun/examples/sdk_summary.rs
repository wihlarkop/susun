//! Print the SDK project summary JSON used by desktop integrations.

use std::process::ExitCode;

use susun::{SusunWorkspace, render_project_summary_json};

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

    if project.has_errors() {
        eprint!("{}", project.render_diagnostics());
        return ExitCode::from(1);
    }

    match render_project_summary_json(&project.summary()) {
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
