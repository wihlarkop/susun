//! Compatibility corpus security audit helpers.

use std::path::{Component, Path};

use crate::{CorpusManifest, SecretHygiene};

/// Severity for a compatibility security audit finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SecurityFindingSeverity {
    /// Informational note.
    Info,
    /// Deferred coverage that must be resolved before a stronger claim.
    Deferred,
    /// Blocking security issue.
    Error,
}

/// One finding from a compatibility security audit.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SecurityFinding {
    /// Finding severity.
    pub severity: SecurityFindingSeverity,
    /// Stable finding code.
    pub code: String,
    /// Optional fixture identifier.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub fixture_id: Option<String>,
    /// Redacted finding message.
    pub message: String,
}

/// Security audit report for a compatibility corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SecurityAuditReport {
    /// Audited corpus name.
    pub corpus: String,
    /// Number of audited fixtures.
    pub fixture_count: usize,
    /// Audit findings.
    pub findings: Vec<SecurityFinding>,
}

impl SecurityAuditReport {
    /// Returns whether the report contains blocking findings.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity == SecurityFindingSeverity::Error)
    }
}

/// Audits corpus metadata for path confinement and secret hygiene.
#[must_use]
pub fn audit_corpus_security(manifest: &CorpusManifest) -> SecurityAuditReport {
    let mut findings = Vec::new();

    for fixture in &manifest.fixtures {
        audit_path(
            &mut findings,
            Some(&fixture.id),
            "project_dir",
            &fixture.project_dir,
        );
        for compose_file in &fixture.compose_files {
            audit_path(
                &mut findings,
                Some(&fixture.id),
                "compose_file",
                compose_file,
            );
        }

        match fixture.secret_hygiene {
            SecretHygiene::NoSecrets => findings.push(info(
                Some(&fixture.id),
                "SUS-COMPAT-SEC-NO-SECRETS",
                "fixture declares no secret material",
            )),
            SecretHygiene::SyntheticOnly => findings.push(info(
                Some(&fixture.id),
                "SUS-COMPAT-SEC-SYNTHETIC",
                "fixture uses synthetic placeholder secret material only",
            )),
        }

        for deferred in &fixture.deferred {
            findings.push(SecurityFinding {
                severity: SecurityFindingSeverity::Deferred,
                code: "SUS-COMPAT-SEC-DEFERRED".to_owned(),
                fixture_id: Some(fixture.id.clone()),
                message: redact_sensitive_text(deferred),
            });
        }
    }

    SecurityAuditReport {
        corpus: manifest.name.clone(),
        fixture_count: manifest.fixtures.len(),
        findings,
    }
}

/// Redacts common credential-bearing fragments from report text.
#[must_use]
pub fn redact_sensitive_text(input: &str) -> String {
    input
        .split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.contains("password")
                || lower.contains("secret")
                || lower.contains("token")
                || lower.contains("credential")
            {
                "<redacted>"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn audit_path(
    findings: &mut Vec<SecurityFinding>,
    fixture_id: Option<&str>,
    kind: &str,
    path: &Path,
) {
    if path.is_absolute() {
        findings.push(error(
            fixture_id,
            "SUS-COMPAT-SEC-ABSOLUTE-PATH",
            format!("{kind} must be repository-relative"),
        ));
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        findings.push(error(
            fixture_id,
            "SUS-COMPAT-SEC-PARENT-PATH",
            format!("{kind} must not contain parent-directory traversal"),
        ));
    }
}

fn info(fixture_id: Option<&str>, code: &str, message: impl Into<String>) -> SecurityFinding {
    SecurityFinding {
        severity: SecurityFindingSeverity::Info,
        code: code.to_owned(),
        fixture_id: fixture_id.map(ToOwned::to_owned),
        message: message.into(),
    }
}

fn error(fixture_id: Option<&str>, code: &str, message: impl Into<String>) -> SecurityFinding {
    SecurityFinding {
        severity: SecurityFindingSeverity::Error,
        code: code.to_owned(),
        fixture_id: fixture_id.map(ToOwned::to_owned),
        message: message.into(),
    }
}
