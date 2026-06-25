//! Minimal `susun config` style example.

use std::process::ExitCode;

use susun::Analyzer;

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    match Analyzer::new(path).analyze() {
        Ok(result) if result.report.has_errors() => ExitCode::from(1),
        Ok(result) => match result.project {
            Some(project) => match serde_json::to_string_pretty(&project) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("susun: {error}");
                    ExitCode::from(2)
                }
            },
            None => ExitCode::from(1),
        },
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
