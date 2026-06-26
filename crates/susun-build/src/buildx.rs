//! Buildx process adapter.

use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::{
    BoxBuildFuture, BuildCancellationToken, BuildCapabilities, BuildEngine, BuildError, BuildEvent,
    BuildEventSink, BuildId, BuildImageIdentity, BuildLogStream, BuildRequest, BuildResult,
    BuildVertexId, BuildVertexStatus,
};

/// Buildx process adapter options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildxProcessOptions {
    /// Docker CLI executable.
    pub docker_cli: PathBuf,
    /// Whether to add `--load` when no explicit output is configured.
    pub load: bool,
}

impl Default for BuildxProcessOptions {
    fn default() -> Self {
        Self {
            docker_cli: PathBuf::from("docker"),
            load: true,
        }
    }
}

/// Build engine backed by `docker buildx build`.
#[derive(Debug, Clone, Default)]
pub struct BuildxProcessBuildEngine {
    options: BuildxProcessOptions,
}

impl BuildxProcessBuildEngine {
    /// Creates a process adapter.
    pub fn new(options: BuildxProcessOptions) -> Self {
        Self { options }
    }
}

impl BuildEngine for BuildxProcessBuildEngine {
    fn capabilities(&self) -> BoxBuildFuture<'_, BuildCapabilities> {
        Box::pin(async { Ok(BuildCapabilities::buildx_process()) })
    }

    fn build(
        &self,
        request: BuildRequest,
        events: BuildEventSink,
        cancellation: BuildCancellationToken,
    ) -> BoxBuildFuture<'_, BuildResult> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(BuildError::Cancelled);
            }

            let build_id = request
                .image_tag
                .clone()
                .unwrap_or_else(|| "susun-build".to_owned());
            events
                .emit(BuildEvent::Started {
                    build_id: BuildId(build_id),
                })
                .await;

            let mut command = Command::new(&self.options.docker_cli);
            command
                .arg("buildx")
                .arg("build")
                .arg("--progress=plain")
                .arg("--file")
                .arg(&request.dockerfile);

            if self.options.load {
                command.arg("--load");
            }
            if let Some(target) = &request.definition.target {
                command.arg("--target").arg(target);
            }
            if let Some(tag) = &request.image_tag {
                command.arg("--tag").arg(tag);
            }
            for platform in &request.definition.platforms {
                command.arg("--platform").arg(platform);
            }
            for (key, value) in &request.definition.args {
                match value {
                    Some(value) => {
                        command.arg("--build-arg").arg(format!("{key}={value}"));
                    }
                    None => {
                        command.arg("--build-arg").arg(key);
                    }
                }
            }
            for secret in &request.secrets {
                command.arg("--secret").arg(match &secret.source {
                    Some(source) => format!("id={},src={}", secret.id, source.display()),
                    None => format!("id={}", secret.id),
                });
            }
            for ssh in &request.ssh {
                command.arg("--ssh").arg(&ssh.id);
            }
            for cache in &request.cache_from {
                command.arg("--cache-from").arg(&cache.spec);
            }
            for cache in &request.cache_to {
                command.arg("--cache-to").arg(&cache.spec);
            }
            for (key, value) in &request.labels {
                command.arg("--label").arg(format!("{key}={value}"));
            }
            if request.insecure_entitlements.network_host {
                command.arg("--allow").arg("network.host");
            }
            if request.insecure_entitlements.security_insecure {
                command.arg("--allow").arg("security.insecure");
            }
            command
                .arg(&request.context_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let output = command.output().map_err(|source| BuildError::Launch {
                program: self.options.docker_cli.clone(),
                source,
            })?;

            emit_process_output(&events, &output.stdout, BuildLogStream::Stdout).await;
            emit_process_output(&events, &output.stderr, BuildLogStream::Stderr).await;

            if cancellation.is_cancelled() {
                return Err(BuildError::Cancelled);
            }
            if !output.status.success() {
                return Err(BuildError::ProcessFailed {
                    status: output.status.to_string(),
                });
            }

            events.emit(BuildEvent::Finished).await;
            let reference = request.image_tag.ok_or(BuildError::MissingImageIdentity)?;
            Ok(BuildResult {
                image: BuildImageIdentity {
                    reference,
                    digest: None,
                },
            })
        })
    }
}

async fn emit_process_output(events: &BuildEventSink, bytes: &[u8], stream: BuildLogStream) {
    let vertex = BuildVertexId("buildx".to_owned());
    if bytes.is_empty() {
        return;
    }
    events
        .emit(BuildEvent::VertexStarted {
            vertex: vertex.clone(),
            name: "buildx".to_owned(),
        })
        .await;
    let text = String::from_utf8_lossy(bytes);
    for line in text.lines() {
        events
            .emit(BuildEvent::VertexLog {
                vertex: vertex.clone(),
                stream,
                text: redact_line(line),
            })
            .await;
    }
    events
        .emit(BuildEvent::VertexFinished {
            vertex,
            status: BuildVertexStatus::Succeeded,
        })
        .await;
}

fn redact_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("password")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("authorization")
    {
        "<redacted>".to_owned()
    } else {
        line.to_owned()
    }
}
