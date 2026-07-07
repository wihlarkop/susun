//! Probe a Docker-compatible runtime and print a redacted readiness report.

use std::process::ExitCode;

use susun::{EngineConnectionDisplayName, EngineConnectionProfile, EngineConnectionProfileId};
use susun_engine::EngineEndpoint;
use susun_engine_bollard::BollardEngine;

#[tokio::main]
async fn main() -> ExitCode {
    let profile = match local_profile() {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!("susun: {error}");
            return ExitCode::from(2);
        }
    };

    let report = BollardEngine::doctor_profile(&profile).await;
    println!(
        "{} [{}] {}: {}",
        profile.display_name.as_str(),
        report.endpoint,
        status_name(report.status),
        report.message
    );

    match report.probe {
        Some(probe) => {
            if let Some(version) = probe.engine_version {
                println!("engine version: {}", version.as_str());
            }
            if let Some(api_version) = probe.api_version {
                println!("api version: {}", api_version.as_str());
            }
            ExitCode::SUCCESS
        }
        None => ExitCode::from(2),
    }
}

fn local_profile() -> Result<EngineConnectionProfile, Box<dyn std::error::Error>> {
    Ok(EngineConnectionProfile::new(
        EngineConnectionProfileId::new("local")?,
        EngineConnectionDisplayName::new("Local Docker-compatible runtime")?,
        EngineEndpoint::Local,
    ))
}

fn status_name(status: susun::RuntimeDoctorStatus) -> &'static str {
    match status {
        susun::RuntimeDoctorStatus::Available => "available",
        susun::RuntimeDoctorStatus::Unavailable => "unavailable",
        susun::RuntimeDoctorStatus::AuthenticationFailed => "authentication_failed",
        susun::RuntimeDoctorStatus::Unsupported => "unsupported",
        susun::RuntimeDoctorStatus::Misconfigured => "misconfigured",
    }
}
