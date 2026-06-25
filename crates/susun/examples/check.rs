//! Minimal `susun check` style example.

use std::process::ExitCode;

use susun::{Analyzer, render_diagnostics};

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compose.yaml".to_owned());

    match Analyzer::new(path).analyze() {
        Ok(result) => {
            if result.report.has_errors() {
                eprint!("{}", render_diagnostics(&result.report, &result.source_map));
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("susun: {error}");
            ExitCode::from(2)
        }
    }
}
